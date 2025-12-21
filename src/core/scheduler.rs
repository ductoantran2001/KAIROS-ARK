//! Deterministic Scheduler for KAIROS-ARK.
//! 
//! The scheduler decouples execution order from wall-clock time, ensuring
//! bit-for-bit identical replayability through logical clocks and 
//! deterministic join semantics.

use rayon::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet, BinaryHeap};
use std::cmp::Ordering;
use std::sync::Arc;

use crate::core::{
    Graph, NodeType, NodeId,
    AuditLedger, LogicalClock, EventType,
    SchedulerError, SchedulerResult, NodeStatus, NodeResult,
};

/// A task in the priority queue.
#[derive(Debug, Clone, Eq, PartialEq)]
struct PriorityTask {
    node_id: NodeId,
    priority: i32,
    /// Lower sequence number = added earlier (for FIFO within same priority)
    sequence: u64,
}

impl Ord for PriorityTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then earlier sequence
        match self.priority.cmp(&other.priority) {
            Ordering::Equal => other.sequence.cmp(&self.sequence), // Lower sequence = higher priority
            other => other,
        }
    }
}

impl PartialOrd for PriorityTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Priority-based task queue.
#[derive(Debug)]
pub struct TaskQueue {
    heap: Mutex<BinaryHeap<PriorityTask>>,
    sequence_counter: Mutex<u64>,
}

impl TaskQueue {
    pub fn new() -> Self {
        Self {
            heap: Mutex::new(BinaryHeap::new()),
            sequence_counter: Mutex::new(0),
        }
    }

    /// Push a task with the given priority.
    pub fn push(&self, node_id: NodeId, priority: i32) {
        let mut seq = self.sequence_counter.lock();
        let sequence = *seq;
        *seq += 1;
        
        self.heap.lock().push(PriorityTask {
            node_id,
            priority,
            sequence,
        });
    }

    /// Pop the highest priority task.
    pub fn pop(&self) -> Option<NodeId> {
        self.heap.lock().pop().map(|t| t.node_id)
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.heap.lock().is_empty()
    }

    /// Get the number of pending tasks.
    pub fn len(&self) -> usize {
        self.heap.lock().len()
    }

    /// Clear the queue.
    pub fn clear(&self) {
        self.heap.lock().clear();
    }
}

impl Default for TaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Handler function type for executing nodes.
pub type NodeHandler = Box<dyn Fn(&NodeId, &SchedulerContext) -> SchedulerResult<String> + Send + Sync>;

/// Context passed to node handlers during execution.
#[derive(Clone)]
pub struct SchedulerContext {
    /// The logical clock for timestamping
    pub clock: Arc<LogicalClock>,
    /// The audit ledger for logging
    pub ledger: Arc<AuditLedger>,
    /// The RNG for stochastic operations
    rng: Arc<Mutex<ChaCha8Rng>>,
    /// The seed used to initialize the RNG
    pub seed: u64,
}

impl SchedulerContext {
    pub fn new(seed: u64, clock: Arc<LogicalClock>, ledger: Arc<AuditLedger>) -> Self {
        let rng = ChaCha8Rng::seed_from_u64(seed);
        Self {
            clock,
            ledger,
            rng: Arc::new(Mutex::new(rng)),
            seed,
        }
    }

    /// Get a random number using the seeded RNG.
    /// Results are deterministic for the same seed.
    pub fn random<T>(&self) -> T
    where
        rand::distributions::Standard: rand::distributions::Distribution<T>,
    {
        self.rng.lock().gen()
    }

    /// Get a random number in a range.
    pub fn random_range(&self, min: u64, max: u64) -> u64 {
        self.rng.lock().gen_range(min..max)
    }

    /// Get a random float for temperature-like parameters (0.0 to 1.0).
    pub fn random_temperature(&self) -> f64 {
        self.rng.lock().gen_range(0.0..1.0)
    }
}

/// Execution state tracking for join nodes.
#[derive(Debug, Default)]
struct ExecutionState {
    /// Nodes that have completed execution
    completed: HashSet<NodeId>,
    /// Results from completed nodes
    results: HashMap<NodeId, NodeResult>,
    /// Current status of each node
    status: HashMap<NodeId, NodeStatus>,
}

impl ExecutionState {
    fn new() -> Self {
        Self::default()
    }

    fn mark_completed(&mut self, node_id: NodeId, result: NodeResult) {
        self.completed.insert(node_id.clone());
        self.status.insert(node_id.clone(), result.status);
        self.results.insert(node_id, result);
    }

    fn is_completed(&self, node_id: &NodeId) -> bool {
        self.completed.contains(node_id)
    }

    fn all_completed(&self, node_ids: &[NodeId]) -> bool {
        node_ids.iter().all(|id| self.completed.contains(id))
    }
}

/// The deterministic scheduler.
/// 
/// Executes graphs with support for:
/// - Conditional branching (Branch nodes)
/// - Parallel execution (Fork/Join)
/// - Priority-based task ordering
/// - Logical clock for deterministic event ordering
/// - Audit logging for replay
pub struct Scheduler {
    /// The execution graph
    graph: Arc<Graph>,
    /// The audit ledger
    ledger: Arc<AuditLedger>,
    /// The logical clock
    clock: Arc<LogicalClock>,
    /// RNG seed for determinism
    seed: u64,
    /// Rayon thread pool for parallel execution
    thread_pool: rayon::ThreadPool,
    /// Task queue with priority support
    _task_queue: Arc<TaskQueue>,
    /// Registered node handlers
    handlers: Arc<Mutex<HashMap<String, NodeHandler>>>,
    /// Branch condition evaluators
    conditions: Arc<Mutex<HashMap<String, Box<dyn Fn() -> bool + Send + Sync>>>>,
}

impl Scheduler {
    /// Create a new scheduler with default configuration.
    pub fn new(graph: Graph) -> Self {
        Self::with_config(graph, None, None)
    }

    /// Create a new scheduler with custom configuration.
    pub fn with_config(graph: Graph, seed: Option<u64>, num_threads: Option<usize>) -> Self {
        let seed = seed.unwrap_or_else(|| {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64
        });

        let num_threads = num_threads.unwrap_or_else(|| rayon::current_num_threads());
        
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .expect("Failed to create thread pool");

        let ledger = Arc::new(AuditLedger::new());
        let clock = Arc::new(LogicalClock::new());

        // Log the RNG seed for replay
        ledger.append(crate::core::ledger::Event::new(
            clock.tick(),
            "_scheduler".to_string(),
            EventType::RngSeedCaptured { seed },
            Some(format!("seed={}", seed)),
        ));

        Self {
            graph: Arc::new(graph),
            ledger,
            clock,
            seed,
            thread_pool,
            _task_queue: Arc::new(TaskQueue::new()),
            handlers: Arc::new(Mutex::new(HashMap::new())),
            conditions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a handler for a specific handler ID.
    pub fn register_handler<F>(&self, handler_id: impl Into<String>, handler: F)
    where
        F: Fn(&NodeId, &SchedulerContext) -> SchedulerResult<String> + Send + Sync + 'static,
    {
        self.handlers.lock().insert(handler_id.into(), Box::new(handler));
    }

    /// Register a condition evaluator for branch nodes.
    pub fn register_condition<F>(&self, condition_id: impl Into<String>, condition: F)
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        self.conditions.lock().insert(condition_id.into(), Box::new(condition));
    }

    /// Execute the graph starting from the entry node.
    pub fn execute(&self) -> SchedulerResult<Vec<NodeResult>> {
        let entry = self.graph.entry()
            .ok_or_else(|| SchedulerError::ExecutionError("No entry node defined".into()))?
            .clone();

        self.execute_from(&entry)
    }

    /// Execute the graph starting from a specific node.
    pub fn execute_from(&self, start_node: &NodeId) -> SchedulerResult<Vec<NodeResult>> {
        // Log execution start
        self.ledger.append(crate::core::ledger::Event::new(
            self.clock.tick(),
            start_node.clone(),
            EventType::ExecutionStart { entry_node: start_node.clone() },
            None,
        ));

        let state = Arc::new(Mutex::new(ExecutionState::new()));
        let ctx = SchedulerContext::new(self.seed, Arc::clone(&self.clock), Arc::clone(&self.ledger));
        
        // Execute recursively
        self.execute_node(start_node, &ctx, &state)?;

        // Log execution end
        self.ledger.append(crate::core::ledger::Event::new(
            self.clock.tick(),
            start_node.clone(),
            EventType::ExecutionEnd { success: true },
            None,
        ));

        let results: Vec<NodeResult> = state.lock().results.values().cloned().collect();
        Ok(results)
    }

    /// Execute a single node and its successors.
    fn execute_node(
        &self,
        node_id: &NodeId,
        ctx: &SchedulerContext,
        state: &Arc<Mutex<ExecutionState>>,
    ) -> SchedulerResult<Option<String>> {
        let node = self.graph.get(node_id)
            .ok_or_else(|| SchedulerError::NodeNotFound(node_id.clone()))?
            .clone();

        // Check if already executed (for join convergence)
        if state.lock().is_completed(node_id) {
            return Ok(state.lock().results.get(node_id).and_then(|r| r.output.clone()));
        }

        // Mark as running
        state.lock().status.insert(node_id.clone(), NodeStatus::Running);

        // Log start
        let start_ts = ctx.clock.tick();
        ctx.ledger.log_start(start_ts, node_id);

        // Execute based on node type
        let result = match &node.node_type {
            NodeType::Task { handler } => {
                self.execute_task(node_id, handler, ctx)?
            }
            NodeType::Branch { condition, true_branch, false_branch } => {
                self.execute_branch(node_id, condition, true_branch, false_branch, ctx, state)?
            }
            NodeType::Fork { children } => {
                self.execute_fork(node_id, children, ctx, state)?
            }
            NodeType::Join { required_parents } => {
                self.execute_join(node_id, required_parents, ctx, state)?
            }
            NodeType::Entry | NodeType::Exit => {
                None // Virtual nodes, no execution
            }
        };

        // Log end
        let end_ts = ctx.clock.tick();
        ctx.ledger.log_end(end_ts, node_id, result.clone());

        // Mark as completed
        state.lock().mark_completed(
            node_id.clone(),
            NodeResult {
                node_id: node_id.clone(),
                status: NodeStatus::Completed,
                output: result.clone(),
                error: None,
                logical_timestamp: end_ts,
            },
        );

        // Execute successor nodes (for non-branching nodes)
        if !matches!(node.node_type, NodeType::Branch { .. } | NodeType::Fork { .. }) {
            for successor_id in &node.edges {
                self.execute_node(successor_id, ctx, state)?;
            }
        }

        Ok(result)
    }

    /// Execute a task node.
    fn execute_task(
        &self,
        node_id: &NodeId,
        handler_id: &str,
        ctx: &SchedulerContext,
    ) -> SchedulerResult<Option<String>> {
        let handlers = self.handlers.lock();
        if let Some(handler) = handlers.get(handler_id) {
            let output = handler(node_id, ctx)?;
            
            // Log tool output
            ctx.ledger.append(crate::core::ledger::Event::new(
                ctx.clock.tick(),
                node_id.clone(),
                EventType::ToolOutput { data: output.clone() },
                Some(output.clone()),
            ));
            
            Ok(Some(output))
        } else {
            // No handler registered, return empty result
            Ok(None)
        }
    }

    /// Execute a branch node - evaluates condition and follows one path.
    fn execute_branch(
        &self,
        node_id: &NodeId,
        condition_id: &str,
        true_branch: &NodeId,
        false_branch: &NodeId,
        ctx: &SchedulerContext,
        state: &Arc<Mutex<ExecutionState>>,
    ) -> SchedulerResult<Option<String>> {
        // Evaluate condition
        let condition_result = {
            let conditions = self.conditions.lock();
            if let Some(condition) = conditions.get(condition_id) {
                condition()
            } else {
                return Err(SchedulerError::BranchEvaluationFailed {
                    node_id: node_id.clone(),
                    error: format!("Condition '{}' not registered", condition_id),
                });
            }
        };

        // Choose path
        let chosen_path = if condition_result {
            true_branch.clone()
        } else {
            false_branch.clone()
        };

        // Log branch decision
        ctx.ledger.log_branch_decision(
            ctx.clock.tick(),
            node_id,
            chosen_path.clone(),
            condition_result,
        );

        // Execute chosen path
        self.execute_node(&chosen_path, ctx, state)?;

        Ok(Some(chosen_path))
    }

    /// Execute a fork node - spawns parallel children.
    fn execute_fork(
        &self,
        node_id: &NodeId,
        children: &[NodeId],
        ctx: &SchedulerContext,
        state: &Arc<Mutex<ExecutionState>>,
    ) -> SchedulerResult<Option<String>> {
        // Log fork spawn
        ctx.ledger.log_fork_spawn(ctx.clock.tick(), node_id, children.to_vec());

        // Execute children in parallel using Rayon
        let results: Vec<SchedulerResult<Option<String>>> = self.thread_pool.install(|| {
            children.par_iter()
                .map(|child_id| {
                    self.execute_node(child_id, ctx, state)
                })
                .collect()
        });

        // Check for errors and collect results
        let mut outputs = Vec::new();
        for result in results {
            if let Some(output) = result? {
                outputs.push(output);
            }
        }

        Ok(Some(outputs.join(",")))
    }

    /// Execute a join node - waits for all parents to complete.
    fn execute_join(
        &self,
        node_id: &NodeId,
        required_parents: &[NodeId],
        ctx: &SchedulerContext,
        state: &Arc<Mutex<ExecutionState>>,
    ) -> SchedulerResult<Option<String>> {
        // Verify all required parents have completed
        {
            let state_guard = state.lock();
            if !state_guard.all_completed(required_parents) {
                let missing: Vec<_> = required_parents.iter()
                    .filter(|id| !state_guard.is_completed(id))
                    .cloned()
                    .collect();
                
                return Err(SchedulerError::JoinIncomplete {
                    node_id: node_id.clone(),
                    missing,
                });
            }
        }

        // Collect parent outputs in deterministic order (sorted by node ID)
        let mut sorted_parents = required_parents.to_vec();
        sorted_parents.sort();

        let outputs: Vec<String> = {
            let state_guard = state.lock();
            sorted_parents.iter()
                .filter_map(|id| {
                    state_guard.results.get(id).and_then(|r| r.output.clone())
                })
                .collect()
        };

        // Log join complete
        ctx.ledger.log_join_complete(ctx.clock.tick(), node_id, sorted_parents);

        Ok(Some(outputs.join(",")))
    }

    /// Get the audit ledger.
    pub fn get_ledger(&self) -> Arc<AuditLedger> {
        Arc::clone(&self.ledger)
    }

    /// Get the logical clock.
    pub fn get_clock(&self) -> Arc<LogicalClock> {
        Arc::clone(&self.clock)
    }

    /// Get the current RNG seed.
    pub fn get_seed(&self) -> u64 {
        self.seed
    }

    /// Get the audit log as a vector of events.
    pub fn get_audit_log(&self) -> Vec<crate::core::ledger::Event> {
        self.ledger.get_events_sorted()
    }

    /// Get the graph.
    pub fn get_graph(&self) -> Arc<Graph> {
        Arc::clone(&self.graph)
    }

    /// Update the graph (for dynamic graph modifications).
    pub fn set_graph(&mut self, graph: Graph) {
        self.graph = Arc::new(graph);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_queue() {
        let queue = TaskQueue::new();
        
        queue.push("low".into(), 1);
        queue.push("high".into(), 10);
        queue.push("medium".into(), 5);

        assert_eq!(queue.pop(), Some("high".into()));
        assert_eq!(queue.pop(), Some("medium".into()));
        assert_eq!(queue.pop(), Some("low".into()));
        assert!(queue.is_empty());
    }

    #[test]
    fn test_simple_task_execution() {
        let mut graph = Graph::new();
        graph.add_node(Node::task("task1", "handler1"));
        graph.set_entry("task1");

        let scheduler = Scheduler::new(graph);
        scheduler.register_handler("handler1", |node_id, _ctx| {
            Ok(format!("executed_{}", node_id))
        });

        let results = scheduler.execute().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].output, Some("executed_task1".into()));
    }

    #[test]
    fn test_branch_execution() {
        let mut graph = Graph::new();
        graph.add_node(Node::branch("branch1", "check_true", "yes", "no"));
        graph.add_node(Node::task("yes", "yes_handler"));
        graph.add_node(Node::task("no", "no_handler"));
        graph.set_entry("branch1");

        let scheduler = Scheduler::new(graph);
        scheduler.register_condition("check_true", || true);
        scheduler.register_handler("yes_handler", |_, _| Ok("YES".into()));
        scheduler.register_handler("no_handler", |_, _| Ok("NO".into()));

        let results = scheduler.execute().unwrap();
        
        // Should have executed branch and yes path
        let log = scheduler.get_audit_log();
        let branch_decisions: Vec<_> = log.iter()
            .filter(|e| matches!(e.event_type, EventType::BranchDecision { .. }))
            .collect();
        
        assert_eq!(branch_decisions.len(), 1);
        if let EventType::BranchDecision { chosen_path, condition_result } = &branch_decisions[0].event_type {
            assert_eq!(chosen_path, "yes");
            assert!(*condition_result);
        }
    }

    #[test]
    fn test_scheduler_context_rng() {
        let clock = Arc::new(LogicalClock::new());
        let ledger = Arc::new(AuditLedger::new());
        
        // Same seed should produce same random values
        let ctx1 = SchedulerContext::new(42, Arc::clone(&clock), Arc::clone(&ledger));
        let ctx2 = SchedulerContext::new(42, Arc::clone(&clock), Arc::clone(&ledger));

        let vals1: Vec<u64> = (0..10).map(|_| ctx1.random()).collect();
        let vals2: Vec<u64> = (0..10).map(|_| ctx2.random()).collect();

        assert_eq!(vals1, vals2);
    }
}
