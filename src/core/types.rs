//! Common types used across the KAIROS-ARK kernel.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a node in the execution graph.
pub type NodeId = String;

/// Result type for scheduler operations.
pub type SchedulerResult<T> = Result<T, SchedulerError>;

/// Errors that can occur during scheduling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchedulerError {
    /// Node not found in graph
    NodeNotFound(NodeId),
    /// Cycle detected in graph
    CycleDetected(Vec<NodeId>),
    /// Join node missing required parents
    JoinIncomplete { node_id: NodeId, missing: Vec<NodeId> },
    /// Timeout exceeded for node execution
    Timeout { node_id: NodeId, timeout_ms: u64 },
    /// Branch condition evaluation failed
    BranchEvaluationFailed { node_id: NodeId, error: String },
    /// Python callback error
    PythonError(String),
    /// Generic execution error
    ExecutionError(String),
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchedulerError::NodeNotFound(id) => write!(f, "Node not found: {}", id),
            SchedulerError::CycleDetected(nodes) => write!(f, "Cycle detected: {:?}", nodes),
            SchedulerError::JoinIncomplete { node_id, missing } => {
                write!(f, "Join node {} missing parents: {:?}", node_id, missing)
            }
            SchedulerError::Timeout { node_id, timeout_ms } => {
                write!(f, "Node {} timed out after {}ms", node_id, timeout_ms)
            }
            SchedulerError::BranchEvaluationFailed { node_id, error } => {
                write!(f, "Branch {} evaluation failed: {}", node_id, error)
            }
            SchedulerError::PythonError(msg) => write!(f, "Python error: {}", msg),
            SchedulerError::ExecutionError(msg) => write!(f, "Execution error: {}", msg),
        }
    }
}

impl std::error::Error for SchedulerError {}

/// Status of a node during execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

/// Result of executing a single node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResult {
    pub node_id: NodeId,
    pub status: NodeStatus,
    pub output: Option<String>,
    pub error: Option<String>,
    pub logical_timestamp: u64,
}
