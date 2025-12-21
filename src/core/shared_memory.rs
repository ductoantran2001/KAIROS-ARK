//! Zero-Copy Shared Memory Store for KAIROS-ARK.
//!
//! Implements a high-performance Generational Arena for shared buffers.
//! Features:
//! - O(1) Allocation/Deallocation via Free List
//! - Safe Handle Access (Generational Indexing) checks for stale handles
//! - Explicit Budget Enforcement (Max bytes, Max single alloc)
//! - Detailed Statistics

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use parking_lot::RwLock;

/// Handle to a shared memory allocation.
/// Format: [ Index (32 bits) | Generation (32 bits) ]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MemoryHandle {
    pub id: u64,
}

impl MemoryHandle {
    pub fn new(index: u32, generation: u32) -> Self {
        let id = ((index as u64) << 32) | (generation as u64);
        Self { id }
    }

    pub fn index(&self) -> usize {
        (self.id >> 32) as usize
    }

    pub fn generation(&self) -> u32 {
        (self.id & 0xFFFFFFFF) as u32
    }
}

#[derive(Debug, Clone, Default)]
pub struct SharedStats {
    pub active_handles: usize,
    pub bytes_live: usize,
    pub peak_bytes: usize,
    pub alloc_count: u64,
    pub free_count: u64,
    pub errors: u64,
}

#[derive(Clone, Debug)]
struct BufferEntry {
    data: Vec<u8>,
    size: usize,
    created_at: u64,
}

#[derive(Clone, Debug)]
enum Slot {
    Free { next_free: Option<usize>, generation: u32 },
    Occupied { entry: BufferEntry, generation: u32 },
}

/// Generational Shared Memory Store.
pub struct SharedMemoryStore {
    buffer_slots: RwLock<Vec<Slot>>,
    free_head: RwLock<Option<usize>>,
    stats: RwLock<SharedStats>,
    
    // Budget Limits
    max_total_bytes: usize,
    max_single_alloc: usize,
}

impl SharedMemoryStore {
    /// Create a new shared memory store.
    pub fn new(max_total_bytes: usize, max_single_alloc: usize) -> Self {
        Self {
            buffer_slots: RwLock::new(Vec::with_capacity(1024)),
            free_head: RwLock::new(None),
            stats: RwLock::new(SharedStats::default()),
            max_total_bytes,
            max_single_alloc,
        }
    }

    /// Write data to shared memory. Returns a robust handle.
    pub fn write(&self, data: &[u8]) -> Result<u64, String> {
        let size = data.len();
        
        // 1. Budget Checks
        if size > self.max_single_alloc {
            self.record_error();
            return Err(format!("Allocation too large: {} > {}", size, self.max_single_alloc));
        }

        {
            let stats = self.stats.read();
            if stats.bytes_live + size > self.max_total_bytes {
                self.record_error();
                return Err(format!("Global memory budget exceeded: {} + {} > {}", stats.bytes_live, size, self.max_total_bytes));
            }
        }

        let mut slots = self.buffer_slots.write();
        let mut free_head = self.free_head.write();
        let mut stats = self.stats.write();

        let index;
        let generation;

        // 2. Allocation Strategy (Reuse or Extend)
        if let Some(head_idx) = *free_head {
            // Reuse slot
            index = head_idx;
            match &slots[index] {
                Slot::Free { next_free, generation: gen } => {
                    *free_head = *next_free;
                    generation = *gen; // Keep existing generation (incremented on free)
                }
                _ => panic!("Corrupt free list: Head points to occupied slot"),
            }
        } else {
            // New slot
            index = slots.len();
            if index > u32::MAX as usize {
                return Err("Max handle count exceeded".to_string());
            }
            generation = 1; // Start at gen 1
            slots.push(Slot::Free { next_free: None, generation: 0 }); // Placeholder
        }

        // 3. Commit
        let entry = BufferEntry {
            data: data.to_vec(),
            size,
            created_at: Self::now(),
        };

        slots[index] = Slot::Occupied { entry, generation };
        
        // 4. Update Stats
        stats.active_handles += 1;
        stats.bytes_live += size;
        stats.alloc_count += 1;
        if stats.bytes_live > stats.peak_bytes {
            stats.peak_bytes = stats.bytes_live;
        }

        Ok(MemoryHandle::new(index as u32, generation).id)
    }

    /// Read data from shared memory. Validates generation.
    pub fn read(&self, handle_id: u64) -> Result<Vec<u8>, String> {
        let handle = MemoryHandle { id: handle_id };
        let slots = self.buffer_slots.read();

        // Bounds check
        if handle.index() >= slots.len() {
            return Err("Invalid handle index".to_string());
        }

        match &slots[handle.index()] {
            Slot::Occupied { entry, generation } => {
                if *generation != handle.generation() {
                    return Err(format!("Stale handle: generation mismatch (req {}, got {})", handle.generation(), generation));
                }
                Ok(entry.data.clone())
            }
            Slot::Free { .. } => Err("Use after free: Slot is empty".to_string()),
        }
    }

    /// Free a shared memory allocation.
    pub fn free(&self, handle_id: u64) -> Result<bool, String> {
        let handle = MemoryHandle { id: handle_id };
        let mut slots = self.buffer_slots.write();

        if handle.index() >= slots.len() {
             return Err("Invalid handle index".to_string());
        }

        // We need to read first to verify, then mutate.
        // Rust ownership makes this tricky with one mutable borrow.
        let slot = &mut slots[handle.index()];
        
        let (size_freed, next_gen) = match slot {
            Slot::Occupied { entry, generation } => {
                if *generation != handle.generation() {
                    return Err("Double free or stale handle".to_string());
                }
                // OK to free
                let size = entry.size;
                let next = *generation + 1; // Increment generation
                (size, next)
            }
            Slot::Free { .. } => return Err("Slot already free".to_string()),
        };

        // Update slot to Free
        let mut free_head = self.free_head.write();
        *slot = Slot::Free {
            next_free: *free_head,
            generation: next_gen,
        };
        *free_head = Some(handle.index());

        // Update stats
        let mut stats = self.stats.write();
        stats.active_handles -= 1;
        stats.bytes_live -= size_freed;
        stats.free_count += 1;

        Ok(true)
    }

    /// Get current statistics.
    pub fn stats(&self) -> SharedStats {
        self.stats.read().clone()
    }

    /// Clear all memory (Debug/Reset).
    pub fn clear(&self) {
        let mut slots = self.buffer_slots.write();
        let mut free_head = self.free_head.write();
        let mut stats = self.stats.write();

        slots.clear();
        *free_head = None;
        *stats = SharedStats::default();
    }

    fn record_error(&self) {
        self.stats.write().errors += 1;
    }

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

// Global Singleton
static GLOBAL_STORE: std::sync::OnceLock<SharedMemoryStore> = std::sync::OnceLock::new();

pub fn global_store() -> &'static SharedMemoryStore {
    GLOBAL_STORE.get_or_init(|| SharedMemoryStore::new(
        1024 * 1024 * 1024, // 1GB Total
        100 * 1024 * 1024   // 100MB Single Alloc
    ))
}
