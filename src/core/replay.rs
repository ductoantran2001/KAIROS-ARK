//! Replay Engine for KAIROS-ARK.
//!
//! Provides the ability to reconstruct execution states from saved ledgers
//! without re-invoking external tools.

use std::collections::HashMap;
use std::io;
use std::path::Path;

use crate::core::ledger::{EventType};
use crate::core::persistence::{DurableLedger, PersistentEvent, StateSnapshot};

/// Replay mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayMode {
    /// Fast-forward using recorded outputs (no execution)
    FastForward,
    /// Re-execute and verify against recorded outputs
    Verify,
}

/// State store for replay and recovery.
#[derive(Clone, Debug, Default)]
pub struct StateStore {
    /// Outputs from each node
    pub node_outputs: HashMap<String, String>,
    /// Current RNG state
    pub rng_state: Option<u64>,
    /// Current logical clock value
    pub clock_value: u64,
    /// Last completed node
    pub last_node: Option<String>,
}

impl StateStore {
    /// Create from a snapshot.
    pub fn from_snapshot(snapshot: &StateSnapshot) -> Self {
        Self {
            node_outputs: snapshot.node_outputs.clone(),
            rng_state: Some(snapshot.rng_state),
            clock_value: snapshot.clock_value,
            last_node: snapshot.last_node.clone(),
        }
    }

    /// Convert to snapshot.
    pub fn to_snapshot(&self, run_id: Option<String>, last_timestamp: u64) -> StateSnapshot {
        StateSnapshot::new(
            run_id,
            self.clock_value,
            self.rng_state.unwrap_or(0),
            self.node_outputs.clone(),
            self.last_node.clone(),
            last_timestamp,
        )
    }

    /// Record an output.
    pub fn record_output(&mut self, node_id: &str, output: &str) {
        self.node_outputs.insert(node_id.to_string(), output.to_string());
        self.last_node = Some(node_id.to_string());
    }

    /// Get recorded output for a node.
    pub fn get_output(&self, node_id: &str) -> Option<&String> {
        self.node_outputs.get(node_id)
    }
}

/// Replay scheduler that reconstructs execution from ledger.
pub struct ReplayScheduler {
    /// Events loaded from ledger
    events: Vec<PersistentEvent>,
    /// Current position in event stream
    current_index: usize,
    /// Reconstructed state
    state: StateStore,
    /// Replay mode
    mode: ReplayMode,
    /// Run ID being replayed
    _run_id: Option<String>,
}

impl ReplayScheduler {
    /// Create from a ledger file.
    pub fn from_ledger(path: &Path, mode: ReplayMode) -> io::Result<Self> {
        let events = DurableLedger::read_all(path)?;
        let run_id = events.first().and_then(|e| e.run_id.clone());

        Ok(Self {
            events,
            current_index: 0,
            state: StateStore::default(),
            mode,
            _run_id: run_id,
        })
    }

    /// Create from a ledger file for a specific run.
    pub fn from_ledger_run(path: &Path, run_id: &str, mode: ReplayMode) -> io::Result<Self> {
        let events = DurableLedger::read_run(path, run_id)?;

        Ok(Self {
            events,
            current_index: 0,
            state: StateStore::default(),
            mode,
            _run_id: Some(run_id.to_string()),
        })
    }

    /// Create from a snapshot + remaining events.
    pub fn from_snapshot(
        snapshot: &StateSnapshot,
        remaining_events: Vec<PersistentEvent>,
        mode: ReplayMode,
    ) -> Self {
        Self {
            events: remaining_events,
            current_index: 0,
            state: StateStore::from_snapshot(snapshot),
            mode,
            _run_id: snapshot.run_id.clone(),
        }
    }

    /// Get the replay mode.
    pub fn mode(&self) -> ReplayMode {
        self.mode
    }

    /// Get current event index.
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// Get total event count.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Check if replay is complete.
    pub fn is_complete(&self) -> bool {
        self.current_index >= self.events.len()
    }

    /// Get the current state.
    pub fn state(&self) -> &StateStore {
        &self.state
    }

    /// Get mutable state.
    pub fn state_mut(&mut self) -> &mut StateStore {
        &mut self.state
    }

    /// Advance to next event and update state.
    pub fn step(&mut self) -> Option<&PersistentEvent> {
        if self.current_index >= self.events.len() {
            return None;
        }

        // Clone necessary data to avoid borrow conflict
        let idx = self.current_index;
        let event_data = self.events[idx].clone();
        self.apply_event_data(&event_data);
        self.current_index += 1;

        Some(&self.events[idx])
    }

    /// Apply event data to state.
    fn apply_event_data(&mut self, event: &PersistentEvent) {
        self.state.clock_value = event.event.logical_timestamp;

        if let Some(rng) = event.rng_state {
            self.state.rng_state = Some(rng);
        }

        match &event.event.event_type {
            EventType::End => {
                self.state.last_node = Some(event.event.node_id.clone());
                if let Some(payload) = &event.event.payload {
                    self.state.record_output(&event.event.node_id, payload);
                }
            }
            EventType::ToolOutput { data } => {
                self.state.record_output(&event.event.node_id, data);
            }
            EventType::RngSeedCaptured { seed } => {
                self.state.rng_state = Some(*seed);
            }
            _ => {}
        }
    }

    /// Fast-forward to a specific timestamp.
    pub fn fast_forward_to(&mut self, timestamp: u64) {
        while self.current_index < self.events.len() {
            if self.events[self.current_index].event.logical_timestamp >= timestamp {
                break;
            }
            self.step();
        }
    }

    /// Fast-forward to a specific node.
    pub fn fast_forward_to_node(&mut self, node_id: &str) {
        while self.current_index < self.events.len() {
            let is_target = self.events[self.current_index].event.node_id == node_id;
            self.step();
            if is_target {
                break;
            }
        }
    }

    /// Get recorded output for a node (for injection during replay).
    pub fn get_recorded_output(&self, node_id: &str) -> Option<&String> {
        self.state.get_output(node_id)
    }

    /// Fork execution from current state.
    pub fn fork(&self) -> StateStore {
        self.state.clone()
    }

    /// Verify that an actual output matches the recorded one.
    pub fn verify(&self, node_id: &str, actual: &str) -> VerifyResult {
        match self.state.get_output(node_id) {
            Some(recorded) if recorded == actual => VerifyResult::Match,
            Some(recorded) => VerifyResult::Mismatch {
                recorded: recorded.clone(),
                actual: actual.to_string(),
            },
            None => VerifyResult::NotRecorded,
        }
    }

    /// Get all events.
    pub fn events(&self) -> &[PersistentEvent] {
        &self.events
    }

    /// Get remaining events from current position.
    pub fn remaining_events(&self) -> &[PersistentEvent] {
        &self.events[self.current_index..]
    }
}

/// Result of verification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifyResult {
    /// Output matches recorded value
    Match,
    /// Output differs from recorded
    Mismatch {
        recorded: String,
        actual: String,
    },
    /// No recorded value for this node
    NotRecorded,
}

impl VerifyResult {
    pub fn is_match(&self) -> bool {
        matches!(self, Self::Match)
    }
}

/// Replay a ledger and return the final state.
pub fn replay_ledger(path: &Path) -> io::Result<StateStore> {
    let mut scheduler = ReplayScheduler::from_ledger(path, ReplayMode::FastForward)?;

    while scheduler.step().is_some() {}

    Ok(scheduler.fork())
}

/// Replay until a specific node and return state for branching.
pub fn replay_to_node(path: &Path, node_id: &str) -> io::Result<StateStore> {
    let mut scheduler = ReplayScheduler::from_ledger(path, ReplayMode::FastForward)?;
    scheduler.fast_forward_to_node(node_id);
    Ok(scheduler.fork())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ledger::EventType;
    use tempfile::tempdir;

    fn make_events() -> Vec<PersistentEvent> {
        vec![
            PersistentEvent {
                event: Event::new(1, "node1".to_string(), EventType::Start, None),
                run_id: Some("test".to_string()),
                wall_clock_ms: 1000,
                rng_state: Some(42),
            },
            PersistentEvent {
                event: Event::new(2, "node1".to_string(), 
                    EventType::ToolOutput { data: "output1".to_string() }, 
                    Some("output1".to_string())),
                run_id: Some("test".to_string()),
                wall_clock_ms: 1001,
                rng_state: Some(42),
            },
            PersistentEvent {
                event: Event::new(3, "node1".to_string(), 
                    EventType::End, 
                    Some("output1".to_string())),
                run_id: Some("test".to_string()),
                wall_clock_ms: 1002,
                rng_state: Some(42),
            },
        ]
    }

    #[test]
    fn test_replay_scheduler() {
        let events = make_events();
        let mut scheduler = ReplayScheduler {
            events,
            current_index: 0,
            state: StateStore::default(),
            mode: ReplayMode::FastForward,
            run_id: Some("test".to_string()),
        };

        // Step through all events
        while scheduler.step().is_some() {}

        assert!(scheduler.is_complete());
        assert_eq!(scheduler.state.clock_value, 3);
        assert_eq!(scheduler.get_recorded_output("node1"), Some(&"output1".to_string()));
    }

    #[test]
    fn test_verify_output() {
        let events = make_events();
        let mut scheduler = ReplayScheduler {
            events,
            current_index: 0,
            state: StateStore::default(),
            mode: ReplayMode::Verify,
            run_id: Some("test".to_string()),
        };

        while scheduler.step().is_some() {}

        assert!(scheduler.verify("node1", "output1").is_match());
        assert!(!scheduler.verify("node1", "different").is_match());
    }

    #[test]
    fn test_fork_state() {
        let events = make_events();
        let mut scheduler = ReplayScheduler {
            events,
            current_index: 0,
            state: StateStore::default(),
            mode: ReplayMode::FastForward,
            run_id: Some("test".to_string()),
        };

        while scheduler.step().is_some() {}

        let forked = scheduler.fork();
        assert_eq!(forked.clock_value, 3);
        assert_eq!(forked.get_output("node1"), Some(&"output1".to_string()));
    }
}
