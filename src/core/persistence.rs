//! Durable Audit Ledger Persistence for KAIROS-ARK.
//!
//! Provides disk-based event logging with configurable flush modes
//! for crash recovery and replay capabilities.

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::core::ledger::Event;

/// Flush mode for the durable ledger.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FlushMode {
    /// Sync to disk after every event (safest, slower)
    Sync,
    /// Batch flush at intervals (faster, small data loss risk)
    Batch {
        /// Flush interval in milliseconds
        interval_ms: u64,
    },
}

impl Default for FlushMode {
    fn default() -> Self {
        FlushMode::Batch { interval_ms: 100 }
    }
}

/// Configuration for durable ledger.
#[derive(Clone, Debug)]
pub struct LedgerConfig {
    /// Path to the ledger file
    pub path: PathBuf,
    /// Flush mode
    pub flush_mode: FlushMode,
    /// Maximum file size before rotation (None = no limit)
    pub max_file_size: Option<u64>,
    /// Events between snapshots (None = no auto-snapshots)
    pub snapshot_interval: Option<u64>,
    /// Run ID for this execution
    pub run_id: Option<String>,
}

impl LedgerConfig {
    /// Create a new config with defaults.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            flush_mode: FlushMode::default(),
            max_file_size: None,
            snapshot_interval: None,
            run_id: None,
        }
    }

    /// Set flush mode.
    pub fn with_flush_mode(mut self, mode: FlushMode) -> Self {
        self.flush_mode = mode;
        self
    }

    /// Set sync flush mode.
    pub fn with_sync_flush(mut self) -> Self {
        self.flush_mode = FlushMode::Sync;
        self
    }

    /// Set batch flush mode.
    pub fn with_batch_flush(mut self, interval_ms: u64) -> Self {
        self.flush_mode = FlushMode::Batch { interval_ms };
        self
    }

    /// Set run ID.
    pub fn with_run_id(mut self, run_id: impl Into<String>) -> Self {
        self.run_id = Some(run_id.into());
        self
    }

    /// Set snapshot interval.
    pub fn with_snapshot_interval(mut self, events: u64) -> Self {
        self.snapshot_interval = Some(events);
        self
    }
}

/// Persistent event with additional metadata for replay.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistentEvent {
    /// The core event data
    #[serde(flatten)]
    pub event: Event,
    /// Run ID for grouping events
    pub run_id: Option<String>,
    /// Wall clock timestamp (Unix ms)
    pub wall_clock_ms: u64,
    /// RNG state at this point (for replay)
    pub rng_state: Option<u64>,
}

impl PersistentEvent {
    /// Create from an Event with current wall clock.
    pub fn from_event(event: Event, run_id: Option<String>, rng_state: Option<u64>) -> Self {
        let wall_clock_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Self {
            event,
            run_id,
            wall_clock_ms,
            rng_state,
        }
    }
}

/// Thread-safe durable ledger with JSONL persistence.
pub struct DurableLedger {
    config: LedgerConfig,
    writer: Mutex<Option<BufWriter<File>>>,
    event_count: AtomicU64,
    bytes_written: AtomicU64,
    last_flush: Mutex<std::time::Instant>,
}

impl DurableLedger {
    /// Create a new durable ledger, truncating any existing file.
    pub fn new(config: LedgerConfig) -> io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&config.path)?;

        Ok(Self {
            config,
            writer: Mutex::new(Some(BufWriter::new(file))),
            event_count: AtomicU64::new(0),
            bytes_written: AtomicU64::new(0),
            last_flush: Mutex::new(std::time::Instant::now()),
        })
    }

    /// Open an existing ledger for appending.
    pub fn open_append(config: LedgerConfig) -> io::Result<Self> {
        // Count existing events
        let existing_count = if config.path.exists() {
            Self::count_events(&config.path)?
        } else {
            0
        };

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.path)?;

        Ok(Self {
            config,
            writer: Mutex::new(Some(BufWriter::new(file))),
            event_count: AtomicU64::new(existing_count),
            bytes_written: AtomicU64::new(0),
            last_flush: Mutex::new(std::time::Instant::now()),
        })
    }

    /// Count events in a ledger file.
    fn count_events(path: &Path) -> io::Result<u64> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Ok(reader.lines().count() as u64)
    }

    /// Append an event to the ledger.
    pub fn append(&self, event: Event, rng_state: Option<u64>) -> io::Result<()> {
        let persistent = PersistentEvent::from_event(
            event,
            self.config.run_id.clone(),
            rng_state,
        );

        let json = serde_json::to_string(&persistent)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let mut writer = self.writer.lock();
        if let Some(w) = writer.as_mut() {
            writeln!(w, "{}", json)?;
            self.event_count.fetch_add(1, Ordering::SeqCst);
            self.bytes_written.fetch_add(json.len() as u64 + 1, Ordering::SeqCst);

            // Check flush mode
            match &self.config.flush_mode {
                FlushMode::Sync => {
                    w.flush()?;
                }
                FlushMode::Batch { interval_ms } => {
                    let mut last_flush = self.last_flush.lock();
                    if last_flush.elapsed().as_millis() >= *interval_ms as u128 {
                        w.flush()?;
                        *last_flush = std::time::Instant::now();
                    }
                }
            }
        }

        Ok(())
    }

    /// Force flush to disk.
    pub fn flush(&self) -> io::Result<()> {
        let mut writer = self.writer.lock();
        if let Some(w) = writer.as_mut() {
            w.flush()?;
            *self.last_flush.lock() = std::time::Instant::now();
        }
        Ok(())
    }

    /// Close the ledger file.
    pub fn close(&self) -> io::Result<()> {
        let mut writer = self.writer.lock();
        if let Some(w) = writer.take() {
            drop(w);
        }
        Ok(())
    }

    /// Get event count.
    pub fn event_count(&self) -> u64 {
        self.event_count.load(Ordering::SeqCst)
    }

    /// Get bytes written.
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written.load(Ordering::SeqCst)
    }

    /// Get the ledger path.
    pub fn path(&self) -> &Path {
        &self.config.path
    }

    /// Read all events from the ledger file.
    pub fn read_all(path: &Path) -> io::Result<Vec<PersistentEvent>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: PersistentEvent = serde_json::from_str(&line)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            events.push(event);
        }

        Ok(events)
    }

    /// Read events for a specific run ID.
    pub fn read_run(path: &Path, run_id: &str) -> io::Result<Vec<PersistentEvent>> {
        let all = Self::read_all(path)?;
        Ok(all
            .into_iter()
            .filter(|e| e.run_id.as_deref() == Some(run_id))
            .collect())
    }
}

impl std::fmt::Debug for DurableLedger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DurableLedger")
            .field("path", &self.config.path)
            .field("event_count", &self.event_count.load(Ordering::SeqCst))
            .field("bytes_written", &self.bytes_written.load(Ordering::SeqCst))
            .finish()
    }
}

/// State snapshot for fast recovery.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Snapshot version for compatibility
    pub version: u32,
    /// Run ID
    pub run_id: Option<String>,
    /// Logical clock value at snapshot
    pub clock_value: u64,
    /// RNG seed/state
    pub rng_state: u64,
    /// Node outputs captured so far
    pub node_outputs: std::collections::HashMap<String, String>,
    /// Last completed node
    pub last_node: Option<String>,
    /// Timestamp of last event
    pub last_timestamp: u64,
    /// Wall clock when snapshot was created
    pub created_at_ms: u64,
}

impl StateSnapshot {
    /// Create a new snapshot.
    pub fn new(
        run_id: Option<String>,
        clock_value: u64,
        rng_state: u64,
        node_outputs: std::collections::HashMap<String, String>,
        last_node: Option<String>,
        last_timestamp: u64,
    ) -> Self {
        let created_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Self {
            version: 1,
            run_id,
            clock_value,
            rng_state,
            node_outputs,
            last_node,
            last_timestamp,
            created_at_ms,
        }
    }

    /// Save snapshot to file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }

    /// Load snapshot from file.
    pub fn load(path: &Path) -> io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ledger::EventType;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_durable_ledger_write_read() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.jsonl");

        let config = LedgerConfig::new(&path).with_sync_flush();
        let ledger = DurableLedger::new(config).unwrap();

        // Write events
        let event1 = Event::new(1, "node1".to_string(), EventType::Start, None);
        let event2 = Event::new(2, "node1".to_string(), EventType::End, Some("output".to_string()));

        ledger.append(event1, Some(42)).unwrap();
        ledger.append(event2, Some(42)).unwrap();
        ledger.flush().unwrap();

        assert_eq!(ledger.event_count(), 2);

        // Read back
        let events = DurableLedger::read_all(&path).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event.logical_timestamp, 1);
        assert_eq!(events[1].event.logical_timestamp, 2);
    }

    #[test]
    fn test_state_snapshot() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("snapshot.json");

        let mut outputs = std::collections::HashMap::new();
        outputs.insert("node1".to_string(), "output1".to_string());

        let snapshot = StateSnapshot::new(
            Some("run123".to_string()),
            100,
            42,
            outputs,
            Some("node1".to_string()),
            100,
        );

        snapshot.save(&path).unwrap();
        let loaded = StateSnapshot::load(&path).unwrap();

        assert_eq!(loaded.run_id, Some("run123".to_string()));
        assert_eq!(loaded.clock_value, 100);
        assert_eq!(loaded.rng_state, 42);
    }
}
