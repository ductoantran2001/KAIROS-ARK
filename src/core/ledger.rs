//! Audit Ledger for system-level tracing.
//! 
//! The ledger is an append-only, thread-safe record of execution events.
//! Every event is stamped with a logical timestamp for consistent ordering.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::core::types::NodeId;

/// Types of events that can be recorded in the audit ledger.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventType {
    /// Node execution started
    Start,
    /// Node execution completed successfully
    End,
    /// Branch condition was evaluated
    BranchDecision {
        /// The branch path that was chosen
        chosen_path: NodeId,
        /// Result of the condition evaluation
        condition_result: bool,
    },
    /// Fork node spawned parallel children
    ForkSpawn {
        /// Child nodes spawned in parallel
        children: Vec<NodeId>,
    },
    /// Join node completed after all parents finished
    JoinComplete {
        /// Parent nodes that were joined
        parents: Vec<NodeId>,
    },
    /// Tool/handler produced output
    ToolOutput {
        /// Output data from the tool
        data: String,
    },
    /// Error occurred during execution
    Error {
        /// Error message
        message: String,
    },
    /// RNG seed was captured for replay
    RngSeedCaptured {
        /// The seed value
        seed: u64,
    },
    /// Graph execution started
    ExecutionStart {
        /// Entry node ID
        entry_node: NodeId,
    },
    /// Graph execution completed
    ExecutionEnd {
        /// Whether execution succeeded
        success: bool,
    },
    /// Tool execution allowed by policy
    PolicyAllow {
        /// Tool that was allowed
        tool_id: String,
        /// Capabilities that were checked
        capabilities_checked: Vec<String>,
    },
    /// Tool execution denied by policy
    PolicyDeny {
        /// Tool that was denied
        tool_id: String,
        /// Rule that triggered the denial
        rule: String,
        /// Human-readable reason
        reason: String,
    },
    /// Content was redacted by policy
    ContentRedacted {
        /// Original content length
        original_length: usize,
        /// Redacted content length
        redacted_length: usize,
        /// Patterns that matched
        patterns_matched: Vec<String>,
    },
    /// Call limit exceeded for tool
    CallLimitExceeded {
        /// Tool that exceeded limit
        tool_id: String,
        /// The configured limit
        limit: u32,
        /// Number of attempts made
        attempted: u32,
    },
    /// State snapshot was created
    SnapshotCreated {
        /// Path to snapshot file
        path: String,
        /// Event count at snapshot
        event_count: u64,
    },
    /// Execution resumed from checkpoint
    ResumeFromCheckpoint {
        /// Logical timestamp of checkpoint
        checkpoint_ts: u64,
        /// Node to resume from
        resume_node: Option<String>,
    },
    /// External/nondeterministic value was captured
    ExternalCapture {
        /// Source of the value (e.g., "llm", "http", "timestamp")
        source: String,
        /// Captured value (serialized)
        value: String,
    },
}

impl EventType {
    /// Get a string representation of the event type.
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::Start => "START",
            EventType::End => "END",
            EventType::BranchDecision { .. } => "BRANCH_DECISION",
            EventType::ForkSpawn { .. } => "FORK_SPAWN",
            EventType::JoinComplete { .. } => "JOIN_COMPLETE",
            EventType::ToolOutput { .. } => "TOOL_OUTPUT",
            EventType::Error { .. } => "ERROR",
            EventType::RngSeedCaptured { .. } => "RNG_SEED_CAPTURED",
            EventType::ExecutionStart { .. } => "EXECUTION_START",
            EventType::ExecutionEnd { .. } => "EXECUTION_END",
            EventType::PolicyAllow { .. } => "POLICY_ALLOW",
            EventType::PolicyDeny { .. } => "POLICY_DENY",
            EventType::ContentRedacted { .. } => "CONTENT_REDACTED",
            EventType::CallLimitExceeded { .. } => "CALL_LIMIT_EXCEEDED",
            EventType::SnapshotCreated { .. } => "SNAPSHOT_CREATED",
            EventType::ResumeFromCheckpoint { .. } => "RESUME_FROM_CHECKPOINT",
            EventType::ExternalCapture { .. } => "EXTERNAL_CAPTURE",
        }
    }
}

/// A single event in the audit ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Monotonic logical timestamp for ordering
    pub logical_timestamp: u64,
    /// ID of the node associated with this event
    pub node_id: NodeId,
    /// Type of event
    pub event_type: EventType,
    /// Optional payload (inputs/outputs, additional context)
    pub payload: Option<String>,
    /// Thread ID that generated this event (for debugging)
    pub thread_id: u64,
}

impl Event {
    /// Create a new event.
    pub fn new(
        logical_timestamp: u64,
        node_id: impl Into<NodeId>,
        event_type: EventType,
        payload: Option<String>,
    ) -> Self {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        
        let thread_id = {
            let mut hasher = DefaultHasher::new();
            std::thread::current().id().hash(&mut hasher);
            hasher.finish()
        };
        
        Self {
            logical_timestamp,
            node_id: node_id.into(),
            event_type,
            payload,
            thread_id,
        }
    }

    /// Create a start event for a node.
    pub fn start(logical_timestamp: u64, node_id: impl Into<NodeId>) -> Self {
        Self::new(logical_timestamp, node_id, EventType::Start, None)
    }

    /// Create an end event for a node.
    pub fn end(logical_timestamp: u64, node_id: impl Into<NodeId>, output: Option<String>) -> Self {
        Self::new(logical_timestamp, node_id, EventType::End, output)
    }

    /// Create a branch decision event.
    pub fn branch_decision(
        logical_timestamp: u64,
        node_id: impl Into<NodeId>,
        chosen_path: NodeId,
        condition_result: bool,
    ) -> Self {
        Self::new(
            logical_timestamp,
            node_id,
            EventType::BranchDecision {
                chosen_path,
                condition_result,
            },
            None,
        )
    }

    /// Create a fork spawn event.
    pub fn fork_spawn(
        logical_timestamp: u64,
        node_id: impl Into<NodeId>,
        children: Vec<NodeId>,
    ) -> Self {
        Self::new(
            logical_timestamp,
            node_id,
            EventType::ForkSpawn { children },
            None,
        )
    }

    /// Create a join complete event.
    pub fn join_complete(
        logical_timestamp: u64,
        node_id: impl Into<NodeId>,
        parents: Vec<NodeId>,
    ) -> Self {
        Self::new(
            logical_timestamp,
            node_id,
            EventType::JoinComplete { parents },
            None,
        )
    }

    /// Create an error event.
    pub fn error(logical_timestamp: u64, node_id: impl Into<NodeId>, message: String) -> Self {
        Self::new(
            logical_timestamp,
            node_id,
            EventType::Error { message: message.clone() },
            Some(message),
        )
    }
}

/// Thread-safe append-only audit ledger.
/// 
/// Uses a mutex-guarded vector for thread-safe event logging.
/// For extremely high-frequency scenarios, consider using crossbeam's
/// lock-free structures.
#[derive(Debug)]
pub struct AuditLedger {
    events: Mutex<Vec<Event>>,
}

impl AuditLedger {
    /// Create a new empty audit ledger.
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::with_capacity(1024)),
        }
    }

    /// Create a new ledger with a specific initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            events: Mutex::new(Vec::with_capacity(capacity)),
        }
    }

    /// Append an event to the ledger.
    /// 
    /// This operation is thread-safe and will not block other readers.
    #[inline]
    pub fn append(&self, event: Event) {
        self.events.lock().push(event);
    }

    /// Log a start event for a node.
    #[inline]
    pub fn log_start(&self, logical_timestamp: u64, node_id: impl Into<NodeId>) {
        self.append(Event::start(logical_timestamp, node_id));
    }

    /// Log an end event for a node.
    #[inline]
    pub fn log_end(&self, logical_timestamp: u64, node_id: impl Into<NodeId>, output: Option<String>) {
        self.append(Event::end(logical_timestamp, node_id, output));
    }

    /// Log a branch decision.
    #[inline]
    pub fn log_branch_decision(
        &self,
        logical_timestamp: u64,
        node_id: impl Into<NodeId>,
        chosen_path: NodeId,
        condition_result: bool,
    ) {
        self.append(Event::branch_decision(logical_timestamp, node_id, chosen_path, condition_result));
    }

    /// Log a fork spawn.
    #[inline]
    pub fn log_fork_spawn(
        &self,
        logical_timestamp: u64,
        node_id: impl Into<NodeId>,
        children: Vec<NodeId>,
    ) {
        self.append(Event::fork_spawn(logical_timestamp, node_id, children));
    }

    /// Log a join completion.
    #[inline]
    pub fn log_join_complete(
        &self,
        logical_timestamp: u64,
        node_id: impl Into<NodeId>,
        parents: Vec<NodeId>,
    ) {
        self.append(Event::join_complete(logical_timestamp, node_id, parents));
    }

    /// Log an error.
    #[inline]
    pub fn log_error(&self, logical_timestamp: u64, node_id: impl Into<NodeId>, message: String) {
        self.append(Event::error(logical_timestamp, node_id, message));
    }

    /// Get a snapshot of all events.
    /// 
    /// Returns a cloned copy of the event log.
    pub fn get_events(&self) -> Vec<Event> {
        self.events.lock().clone()
    }

    /// Get events sorted by logical timestamp.
    pub fn get_events_sorted(&self) -> Vec<Event> {
        let mut events = self.get_events();
        events.sort_by_key(|e| e.logical_timestamp);
        events
    }

    /// Get the number of events in the ledger.
    pub fn len(&self) -> usize {
        self.events.lock().len()
    }

    /// Check if the ledger is empty.
    pub fn is_empty(&self) -> bool {
        self.events.lock().is_empty()
    }

    /// Clear all events from the ledger.
    pub fn clear(&self) {
        self.events.lock().clear();
    }

    /// Serialize the ledger to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.get_events_sorted())
    }

    /// Create a ledger from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let events: Vec<Event> = serde_json::from_str(json)?;
        let ledger = Self::new();
        *ledger.events.lock() = events;
        Ok(ledger)
    }
}

impl Default for AuditLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AuditLedger {
    fn clone(&self) -> Self {
        let ledger = Self::new();
        *ledger.events.lock() = self.get_events();
        ledger
    }
}

/// Thread-safe reference to an audit ledger.
pub type SharedLedger = Arc<AuditLedger>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_append_and_retrieve() {
        let ledger = AuditLedger::new();
        
        ledger.log_start(1, "node1");
        ledger.log_end(2, "node1", Some("output".to_string()));
        
        let events = ledger.get_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].logical_timestamp, 1);
        assert_eq!(events[1].logical_timestamp, 2);
    }

    #[test]
    fn test_concurrent_append() {
        let ledger = Arc::new(AuditLedger::new());
        let mut handles = vec![];

        for i in 0..10 {
            let ledger_clone = Arc::clone(&ledger);
            handles.push(thread::spawn(move || {
                for j in 0..100 {
                    ledger_clone.log_start((i * 100 + j) as u64, format!("node_{}", i));
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(ledger.len(), 1000);
    }

    #[test]
    fn test_sorted_retrieval() {
        let ledger = AuditLedger::new();
        
        // Insert out of order
        ledger.append(Event::start(3, "c"));
        ledger.append(Event::start(1, "a"));
        ledger.append(Event::start(2, "b"));
        
        let sorted = ledger.get_events_sorted();
        assert_eq!(sorted[0].node_id, "a");
        assert_eq!(sorted[1].node_id, "b");
        assert_eq!(sorted[2].node_id, "c");
    }

    #[test]
    fn test_json_serialization() {
        let ledger = AuditLedger::new();
        ledger.log_start(1, "node1");
        ledger.log_branch_decision(2, "branch1", "yes".to_string(), true);
        
        let json = ledger.to_json().unwrap();
        let restored = AuditLedger::from_json(&json).unwrap();
        
        assert_eq!(restored.len(), 2);
    }
}
