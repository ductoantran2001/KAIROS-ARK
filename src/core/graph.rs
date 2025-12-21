//! Extended Graph Data Model for KAIROS-ARK.
//! 
//! Supports non-linear flow with conditional branching (Branch nodes),
//! parallel execution (Fork/Join), and node resource metadata.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::core::types::NodeId;

/// Type of node in the execution graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    /// Executable task node with a handler identifier
    Task {
        /// Identifier for the task handler (Python function name or callable ID)
        handler: String,
    },
    /// Conditional branch node that evaluates an expression
    Branch {
        /// Identifier for the condition evaluator
        condition: String,
        /// Node to execute if condition is true
        true_branch: NodeId,
        /// Node to execute if condition is false
        false_branch: NodeId,
    },
    /// Parallel fork node - spawns multiple child nodes concurrently
    Fork {
        /// Child nodes to execute in parallel
        children: Vec<NodeId>,
    },
    /// Join node - waits for multiple parent nodes to complete
    Join {
        /// Parent nodes that must complete before this node
        required_parents: Vec<NodeId>,
    },
    /// Entry point node (virtual, no execution)
    Entry,
    /// Exit point node (virtual, no execution)
    Exit,
}

/// A node in the execution graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier for this node
    pub id: NodeId,
    /// Type and configuration of this node
    pub node_type: NodeType,
    /// Outgoing edges to successor nodes
    pub edges: Vec<NodeId>,
    /// Optional timeout in milliseconds for task execution
    pub timeout_ms: Option<u64>,
    /// Priority for scheduling (higher = execute first)
    pub priority: i32,
    /// Optional metadata for the node
    pub metadata: HashMap<String, String>,
}

impl Node {
    /// Create a new task node.
    pub fn task(id: impl Into<NodeId>, handler: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            node_type: NodeType::Task { handler: handler.into() },
            edges: Vec::new(),
            timeout_ms: None,
            priority: 0,
            metadata: HashMap::new(),
        }
    }

    /// Create a new branch node.
    pub fn branch(
        id: impl Into<NodeId>,
        condition: impl Into<String>,
        true_branch: impl Into<NodeId>,
        false_branch: impl Into<NodeId>,
    ) -> Self {
        Self {
            id: id.into(),
            node_type: NodeType::Branch {
                condition: condition.into(),
                true_branch: true_branch.into(),
                false_branch: false_branch.into(),
            },
            edges: Vec::new(),
            timeout_ms: None,
            priority: 0,
            metadata: HashMap::new(),
        }
    }

    /// Create a new fork node.
    pub fn fork(id: impl Into<NodeId>, children: Vec<NodeId>) -> Self {
        Self {
            id: id.into(),
            node_type: NodeType::Fork { children: children.clone() },
            edges: children,
            timeout_ms: None,
            priority: 0,
            metadata: HashMap::new(),
        }
    }

    /// Create a new join node.
    pub fn join(id: impl Into<NodeId>, required_parents: Vec<NodeId>) -> Self {
        Self {
            id: id.into(),
            node_type: NodeType::Join { required_parents },
            edges: Vec::new(),
            timeout_ms: None,
            priority: 0,
            metadata: HashMap::new(),
        }
    }

    /// Set the timeout for this node.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Set the priority for this node.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Add an outgoing edge.
    pub fn with_edge(mut self, target: impl Into<NodeId>) -> Self {
        self.edges.push(target.into());
        self
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// The execution graph containing all nodes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Graph {
    /// All nodes in the graph, keyed by their ID
    nodes: HashMap<NodeId, Node>,
    /// Entry point node ID
    entry: Option<NodeId>,
}

impl Graph {
    /// Create a new empty graph.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            entry: None,
        }
    }

    /// Add a node to the graph.
    pub fn add_node(&mut self, node: Node) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// Set the entry point for graph execution.
    pub fn set_entry(&mut self, node_id: impl Into<NodeId>) {
        self.entry = Some(node_id.into());
    }

    /// Get the entry point node ID.
    pub fn entry(&self) -> Option<&NodeId> {
        self.entry.as_ref()
    }

    /// Get a node by ID.
    pub fn get(&self, id: &NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// Get a mutable reference to a node by ID.
    pub fn get_mut(&mut self, id: &NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    /// Check if a node exists.
    pub fn contains(&self, id: &NodeId) -> bool {
        self.nodes.contains_key(id)
    }

    /// Get all node IDs.
    pub fn node_ids(&self) -> impl Iterator<Item = &NodeId> {
        self.nodes.keys()
    }

    /// Get all nodes.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    /// Get the number of nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the graph is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Add an edge between two existing nodes.
    pub fn add_edge(&mut self, from: &NodeId, to: impl Into<NodeId>) -> bool {
        if let Some(node) = self.nodes.get_mut(from) {
            node.edges.push(to.into());
            true
        } else {
            false
        }
    }

    /// Get successor nodes of a given node.
    pub fn successors(&self, id: &NodeId) -> Option<Vec<&Node>> {
        self.nodes.get(id).map(|node| {
            node.edges
                .iter()
                .filter_map(|edge_id| self.nodes.get(edge_id))
                .collect()
        })
    }

    /// Get nodes sorted by priority (highest first).
    pub fn nodes_by_priority(&self) -> Vec<&Node> {
        let mut nodes: Vec<_> = self.nodes.values().collect();
        nodes.sort_by(|a, b| b.priority.cmp(&a.priority));
        nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_task_node() {
        let node = Node::task("task1", "my_handler")
            .with_priority(10)
            .with_timeout(5000);
        
        assert_eq!(node.id, "task1");
        assert_eq!(node.priority, 10);
        assert_eq!(node.timeout_ms, Some(5000));
    }

    #[test]
    fn test_create_branch_node() {
        let node = Node::branch("branch1", "check_condition", "yes", "no");
        
        match &node.node_type {
            NodeType::Branch { condition, true_branch, false_branch } => {
                assert_eq!(condition, "check_condition");
                assert_eq!(true_branch, "yes");
                assert_eq!(false_branch, "no");
            }
            _ => panic!("Expected Branch node type"),
        }
    }

    #[test]
    fn test_create_fork_join() {
        let fork = Node::fork("fork1", vec!["a".into(), "b".into(), "c".into()]);
        let join = Node::join("join1", vec!["a".into(), "b".into(), "c".into()]);

        match &fork.node_type {
            NodeType::Fork { children } => {
                assert_eq!(children.len(), 3);
            }
            _ => panic!("Expected Fork node type"),
        }

        match &join.node_type {
            NodeType::Join { required_parents } => {
                assert_eq!(required_parents.len(), 3);
            }
            _ => panic!("Expected Join node type"),
        }
    }

    #[test]
    fn test_graph_operations() {
        let mut graph = Graph::new();
        graph.add_node(Node::task("a", "handler_a").with_priority(1));
        graph.add_node(Node::task("b", "handler_b").with_priority(2));
        graph.add_node(Node::task("c", "handler_c").with_priority(3));
        graph.set_entry("a");

        assert_eq!(graph.len(), 3);
        assert_eq!(graph.entry(), Some(&"a".to_string()));

        let by_priority = graph.nodes_by_priority();
        assert_eq!(by_priority[0].id, "c");
        assert_eq!(by_priority[1].id, "b");
        assert_eq!(by_priority[2].id, "a");
    }
}
