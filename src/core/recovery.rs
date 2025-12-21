//! Recovery Manager for KAIROS-ARK.
//!
//! Handles failure recovery by resuming interrupted runs from
//! the last known good state using saved ledgers and snapshots.

use std::fs;
use std::io;
use std::path::{PathBuf};

use crate::core::persistence::{DurableLedger, PersistentEvent, StateSnapshot};
use crate::core::replay::{ReplayScheduler, ReplayMode, StateStore};
use crate::core::ledger::EventType;

/// Recovery point information.
#[derive(Clone, Debug)]
pub struct RecoveryPoint {
    /// Run ID
    pub run_id: String,
    /// Last completed node
    pub last_node: Option<String>,
    /// Last event timestamp
    pub last_timestamp: u64,
    /// Path to snapshot (if exists)
    pub snapshot_path: Option<PathBuf>,
    /// Path to ledger file
    pub ledger_path: PathBuf,
    /// Number of events in ledger
    pub event_count: u64,
    /// Whether the run completed successfully
    pub completed: bool,
}

/// State for resuming execution.
#[derive(Debug)]
pub struct ResumeState {
    /// Events replayed
    pub events: Vec<PersistentEvent>,
    /// Reconstructed state
    pub state: StateStore,
    /// Node to resume from (next to execute)
    pub resume_from: Option<String>,
    /// RNG state for continuation
    pub rng_state: Option<u64>,
}

/// Recovery manager for handling interrupted runs.
pub struct RecoveryManager {
    /// Directory containing ledger files
    ledger_dir: PathBuf,
    /// Snapshot directory
    snapshot_dir: PathBuf,
}

impl RecoveryManager {
    /// Create a new recovery manager.
    pub fn new(ledger_dir: impl Into<PathBuf>) -> Self {
        let ledger_dir = ledger_dir.into();
        let snapshot_dir = ledger_dir.join("snapshots");
        Self { ledger_dir, snapshot_dir }
    }

    /// Get the ledger path for a run.
    pub fn ledger_path(&self, run_id: &str) -> PathBuf {
        self.ledger_dir.join(format!("{}.jsonl", run_id))
    }

    /// Get the snapshot path for a run.
    pub fn snapshot_path(&self, run_id: &str) -> PathBuf {
        self.snapshot_dir.join(format!("{}.snapshot.json", run_id))
    }

    /// Check if there's a pending (incomplete) run.
    pub fn has_pending_run(&self, run_id: &str) -> bool {
        let ledger_path = self.ledger_path(run_id);
        if !ledger_path.exists() {
            return false;
        }

        // Check if the run completed
        match self.get_recovery_point(run_id) {
            Ok(Some(point)) => !point.completed,
            _ => false,
        }
    }

    /// List all run IDs with ledgers.
    pub fn list_runs(&self) -> io::Result<Vec<String>> {
        let mut runs = Vec::new();

        if !self.ledger_dir.exists() {
            return Ok(runs);
        }

        for entry in fs::read_dir(&self.ledger_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "jsonl") {
                if let Some(stem) = path.file_stem() {
                    runs.push(stem.to_string_lossy().to_string());
                }
            }
        }

        Ok(runs)
    }

    /// Get recovery point for a run.
    pub fn get_recovery_point(&self, run_id: &str) -> io::Result<Option<RecoveryPoint>> {
        let ledger_path = self.ledger_path(run_id);
        
        if !ledger_path.exists() {
            return Ok(None);
        }

        let events = DurableLedger::read_all(&ledger_path)?;
        if events.is_empty() {
            return Ok(None);
        }

        // Find last completed node and check if run finished
        let mut last_node = None;
        let mut last_timestamp = 0u64;
        let mut completed = false;

        for event in &events {
            last_timestamp = event.event.logical_timestamp;

            match &event.event.event_type {
                EventType::End => {
                    last_node = Some(event.event.node_id.clone());
                }
                EventType::ExecutionEnd { success } => {
                    completed = *success;
                }
                _ => {}
            }
        }

        // Check for snapshot
        let snapshot_path = self.snapshot_path(run_id);
        let snapshot_exists = snapshot_path.exists();

        Ok(Some(RecoveryPoint {
            run_id: run_id.to_string(),
            last_node,
            last_timestamp,
            snapshot_path: if snapshot_exists { Some(snapshot_path) } else { None },
            ledger_path,
            event_count: events.len() as u64,
            completed,
        }))
    }

    /// Resume execution from a recovery point.
    pub fn resume(&self, run_id: &str) -> io::Result<ResumeState> {
        let point = self.get_recovery_point(run_id)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No recovery point found"))?;

        // Try to load from snapshot first for faster recovery
        if let Some(snapshot_path) = &point.snapshot_path {
            if let Ok(snapshot) = StateSnapshot::load(snapshot_path) {
                // Load events after snapshot
                let all_events = DurableLedger::read_all(&point.ledger_path)?;
                let remaining: Vec<_> = all_events
                    .into_iter()
                    .filter(|e| e.event.logical_timestamp > snapshot.last_timestamp)
                    .collect();

                let mut scheduler = ReplayScheduler::from_snapshot(
                    &snapshot,
                    remaining,
                    ReplayMode::FastForward,
                );

                // Replay remaining events
                while scheduler.step().is_some() {}

                return Ok(ResumeState {
                    events: scheduler.events().to_vec(),
                    state: scheduler.fork(),
                    resume_from: point.last_node,
                    rng_state: Some(snapshot.rng_state),
                });
            }
        }

        // Fall back to replaying entire ledger
        let mut scheduler = ReplayScheduler::from_ledger(&point.ledger_path, ReplayMode::FastForward)?;

        while scheduler.step().is_some() {}

        Ok(ResumeState {
            events: scheduler.events().to_vec(),
            state: scheduler.fork(),
            resume_from: point.last_node,
            rng_state: scheduler.state().rng_state,
        })
    }

    /// Create a snapshot for a run.
    pub fn create_snapshot(&self, run_id: &str, state: &StateStore, last_timestamp: u64) -> io::Result<PathBuf> {
        // Ensure snapshot directory exists
        fs::create_dir_all(&self.snapshot_dir)?;

        let snapshot = state.to_snapshot(Some(run_id.to_string()), last_timestamp);
        let path = self.snapshot_path(run_id);
        snapshot.save(&path)?;

        Ok(path)
    }

    /// Delete a run's ledger and snapshot.
    pub fn delete_run(&self, run_id: &str) -> io::Result<()> {
        let ledger_path = self.ledger_path(run_id);
        let snapshot_path = self.snapshot_path(run_id);

        if ledger_path.exists() {
            fs::remove_file(ledger_path)?;
        }
        if snapshot_path.exists() {
            fs::remove_file(snapshot_path)?;
        }

        Ok(())
    }

    /// Clean up completed runs.
    pub fn cleanup_completed(&self) -> io::Result<usize> {
        let runs = self.list_runs()?;
        let mut deleted = 0;

        for run_id in runs {
            if let Ok(Some(point)) = self.get_recovery_point(&run_id) {
                if point.completed {
                    self.delete_run(&run_id)?;
                    deleted += 1;
                }
            }
        }

        Ok(deleted)
    }
}

impl std::fmt::Debug for RecoveryManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecoveryManager")
            .field("ledger_dir", &self.ledger_dir)
            .field("snapshot_dir", &self.snapshot_dir)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ledger::Event;
    use crate::core::persistence::LedgerConfig;
    use tempfile::tempdir;

    #[test]
    fn test_recovery_manager() {
        let dir = tempdir().unwrap();
        let manager = RecoveryManager::new(dir.path());

        // Create a test ledger
        let ledger_path = manager.ledger_path("test_run");
        let config = LedgerConfig::new(&ledger_path)
            .with_run_id("test_run")
            .with_sync_flush();
        
        let ledger = DurableLedger::new(config).unwrap();
        
        ledger.append(
            Event::new(1, "node1".to_string(), EventType::Start, None),
            Some(42),
        ).unwrap();
        ledger.append(
            Event::new(2, "node1".to_string(), EventType::End, Some("output".to_string())),
            Some(42),
        ).unwrap();
        ledger.flush().unwrap();

        // Check recovery point
        assert!(manager.has_pending_run("test_run"));
        
        let point = manager.get_recovery_point("test_run").unwrap().unwrap();
        assert_eq!(point.run_id, "test_run");
        assert_eq!(point.last_node, Some("node1".to_string()));
        assert_eq!(point.event_count, 2);
        assert!(!point.completed);
    }

    #[test]
    fn test_resume_from_ledger() {
        let dir = tempdir().unwrap();
        let manager = RecoveryManager::new(dir.path());

        // Create a test ledger
        let ledger_path = manager.ledger_path("resume_test");
        let config = LedgerConfig::new(&ledger_path)
            .with_run_id("resume_test")
            .with_sync_flush();
        
        let ledger = DurableLedger::new(config).unwrap();
        
        ledger.append(
            Event::new(1, "a".to_string(), EventType::Start, None),
            Some(42),
        ).unwrap();
        ledger.append(
            Event::new(2, "a".to_string(), 
                EventType::ToolOutput { data: "A".to_string() }, 
                Some("A".to_string())),
            Some(42),
        ).unwrap();
        ledger.append(
            Event::new(3, "a".to_string(), EventType::End, Some("A".to_string())),
            Some(42),
        ).unwrap();
        ledger.flush().unwrap();

        // Resume
        let resume = manager.resume("resume_test").unwrap();
        assert_eq!(resume.resume_from, Some("a".to_string()));
        assert_eq!(resume.state.get_output("a"), Some(&"A".to_string()));
    }
}
