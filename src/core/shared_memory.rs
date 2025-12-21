//! Zero-Copy Shared Memory Store for KAIROS-ARK.
//!
//! Provides efficient data passing between nodes without serialization.
//! Large payloads are stored in a memory pool and passed by reference.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;

/// Handle to a shared memory allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MemoryHandle {
    /// Unique identifier for this allocation
    pub id: u64,
    /// Offset into the memory pool
    offset: usize,
    /// Length of the allocation
    len: usize,
}

impl MemoryHandle {
    /// Get the length of data referenced.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if handle points to empty data.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Allocation metadata.
#[derive(Clone, Debug)]
struct Allocation {
    offset: usize,
    len: usize,
    ref_count: usize,
}

/// Zero-copy shared memory pool.
///
/// Stores large data blobs and returns lightweight handles that can be
/// passed between nodes without copying the underlying data.
pub struct SharedMemoryStore {
    /// Pre-allocated memory pool
    pool: RwLock<Vec<u8>>,
    /// Active allocations
    allocations: RwLock<HashMap<u64, Allocation>>,
    /// Free list (offset, len)
    free_list: RwLock<Vec<(usize, usize)>>,
    /// Next handle ID
    next_id: AtomicU64,
    /// Pool capacity
    capacity: usize,
    /// Current used bytes
    used: AtomicU64,
}

impl SharedMemoryStore {
    /// Create a new shared memory store with given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            pool: RwLock::new(vec![0u8; capacity]),
            allocations: RwLock::new(HashMap::new()),
            free_list: RwLock::new(vec![(0, capacity)]),
            next_id: AtomicU64::new(1),
            capacity,
            used: AtomicU64::new(0),
        }
    }

    /// Get the pool capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get currently used bytes.
    pub fn used(&self) -> u64 {
        self.used.load(Ordering::SeqCst)
    }

    /// Get available bytes.
    pub fn available(&self) -> usize {
        self.capacity.saturating_sub(self.used.load(Ordering::SeqCst) as usize)
    }

    /// Write data to the pool, returning a handle.
    ///
    /// This is an O(n) operation for finding free space.
    /// Consider using a more sophisticated allocator for production.
    pub fn write(&self, data: &[u8]) -> Option<MemoryHandle> {
        if data.is_empty() {
            return Some(MemoryHandle {
                id: 0,
                offset: 0,
                len: 0,
            });
        }

        let len = data.len();

        // Find a free block
        let mut free_list = self.free_list.write();
        let (block_idx, offset) = free_list
            .iter()
            .enumerate()
            .find(|(_, (_, size))| *size >= len)
            .map(|(idx, (off, _))| (idx, *off))?;

        let (block_offset, block_size) = free_list[block_idx];

        // Update or remove the free block
        if block_size > len {
            free_list[block_idx] = (block_offset + len, block_size - len);
        } else {
            free_list.remove(block_idx);
        }
        drop(free_list);

        // Copy data to pool
        {
            let mut pool = self.pool.write();
            pool[offset..offset + len].copy_from_slice(data);
        }

        // Create allocation record
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let alloc = Allocation {
            offset,
            len,
            ref_count: 1,
        };
        self.allocations.write().insert(id, alloc);
        self.used.fetch_add(len as u64, Ordering::SeqCst);

        Some(MemoryHandle { id, offset, len })
    }

    /// Read data by handle.
    ///
    /// Returns a copy of the data. For true zero-copy, use `read_slice`.
    pub fn read(&self, handle: &MemoryHandle) -> Option<Vec<u8>> {
        if handle.id == 0 {
            return Some(Vec::new());
        }

        let allocs = self.allocations.read();
        let alloc = allocs.get(&handle.id)?;

        let pool = self.pool.read();
        Some(pool[alloc.offset..alloc.offset + alloc.len].to_vec())
    }

    /// Read data as a slice (borrowing from pool).
    ///
    /// This is the true zero-copy path when you can work with borrowed data.
    pub fn read_with<F, R>(&self, handle: &MemoryHandle, f: F) -> Option<R>
    where
        F: FnOnce(&[u8]) -> R,
    {
        if handle.id == 0 {
            return Some(f(&[]));
        }

        let allocs = self.allocations.read();
        let alloc = allocs.get(&handle.id)?;

        let pool = self.pool.read();
        Some(f(&pool[alloc.offset..alloc.offset + alloc.len]))
    }

    /// Increment reference count for a handle.
    pub fn add_ref(&self, handle: &MemoryHandle) -> bool {
        if handle.id == 0 {
            return true;
        }

        let mut allocs = self.allocations.write();
        if let Some(alloc) = allocs.get_mut(&handle.id) {
            alloc.ref_count += 1;
            true
        } else {
            false
        }
    }

    /// Free an allocation (decrements ref count).
    ///
    /// Data is only actually freed when ref count reaches zero.
    pub fn free(&self, handle: MemoryHandle) -> bool {
        if handle.id == 0 {
            return true;
        }

        let mut allocs = self.allocations.write();
        if let Some(alloc) = allocs.get_mut(&handle.id) {
            alloc.ref_count -= 1;
            if alloc.ref_count == 0 {
                let offset = alloc.offset;
                let len = alloc.len;
                allocs.remove(&handle.id);
                
                // Return to free list
                self.free_list.write().push((offset, len));
                self.used.fetch_sub(len as u64, Ordering::SeqCst);
            }
            true
        } else {
            false
        }
    }

    /// Get the number of active allocations.
    pub fn allocation_count(&self) -> usize {
        self.allocations.read().len()
    }

    /// Clear all allocations and reset the pool.
    pub fn clear(&self) {
        self.allocations.write().clear();
        *self.free_list.write() = vec![(0, self.capacity)];
        self.used.store(0, Ordering::SeqCst);
    }

    /// Read data by handle ID only.
    ///
    /// This is useful when you only have the ID, not the full handle.
    pub fn read_by_id(&self, id: u64) -> Option<Vec<u8>> {
        if id == 0 {
            return Some(Vec::new());
        }

        let allocs = self.allocations.read();
        let alloc = allocs.get(&id)?;

        let pool = self.pool.read();
        Some(pool[alloc.offset..alloc.offset + alloc.len].to_vec())
    }
}

impl Default for SharedMemoryStore {
    fn default() -> Self {
        // Default to 64MB pool
        Self::new(64 * 1024 * 1024)
    }
}

impl std::fmt::Debug for SharedMemoryStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedMemoryStore")
            .field("capacity", &self.capacity)
            .field("used", &self.used.load(Ordering::SeqCst))
            .field("allocations", &self.allocation_count())
            .finish()
    }
}

/// Global shared memory store.
static GLOBAL_STORE: std::sync::OnceLock<SharedMemoryStore> = std::sync::OnceLock::new();

/// Get the global shared memory store.
pub fn global_store() -> &'static SharedMemoryStore {
    GLOBAL_STORE.get_or_init(|| SharedMemoryStore::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_read() {
        let store = SharedMemoryStore::new(1024);
        let data = b"Hello, World!";

        let handle = store.write(data).expect("write should succeed");
        assert_eq!(handle.len(), data.len());

        let read = store.read(&handle).expect("read should succeed");
        assert_eq!(&read, data);
    }

    #[test]
    fn test_zero_copy_read() {
        let store = SharedMemoryStore::new(1024);
        let data = b"Zero copy test";

        let handle = store.write(data).unwrap();
        
        let result = store.read_with(&handle, |slice| {
            assert_eq!(slice, data);
            slice.len()
        });
        
        assert_eq!(result, Some(data.len()));
    }

    #[test]
    fn test_reference_counting() {
        let store = SharedMemoryStore::new(1024);
        let data = b"RefCount test";

        let handle = store.write(data).unwrap();
        assert_eq!(store.allocation_count(), 1);

        // Add reference
        store.add_ref(&handle);

        // First free doesn't deallocate
        store.free(handle);
        assert_eq!(store.allocation_count(), 1);

        // Second free actually frees
        store.free(handle);
        assert_eq!(store.allocation_count(), 0);
    }

    #[test]
    fn test_large_data() {
        let store = SharedMemoryStore::new(20 * 1024 * 1024); // 20MB
        let data = vec![42u8; 10 * 1024 * 1024]; // 10MB

        let handle = store.write(&data).expect("should fit");
        assert_eq!(handle.len(), data.len());

        let read = store.read(&handle).unwrap();
        assert_eq!(read.len(), data.len());
        assert!(read.iter().all(|&b| b == 42));
    }

    #[test]
    fn test_empty_handle() {
        let store = SharedMemoryStore::new(1024);
        
        let handle = store.write(&[]).unwrap();
        assert_eq!(handle.len(), 0);
        assert!(handle.is_empty());
        
        let read = store.read(&handle).unwrap();
        assert!(read.is_empty());
    }
}
