//! Python bindings for the KAIROS-ARK kernel.
//! 
//! Provides the PyKernel class that wraps the Rust scheduler and graph
//! for use from Python code.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use pyo3::exceptions::PyRuntimeError;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

use crate::core::{
    Graph, Node, NodeType,
    Scheduler, AuditLedger, LogicalClock,
    EventType,
};

/// Python-exposed event representation.
#[pyclass]
#[derive(Clone, Debug)]
pub struct PyEvent {
    #[pyo3(get)]
    pub logical_timestamp: u64,
    #[pyo3(get)]
    pub node_id: String,
    #[pyo3(get)]
    pub event_type: String,
    #[pyo3(get)]
    pub payload: Option<String>,
}

#[pymethods]
impl PyEvent {
    fn __repr__(&self) -> String {
        format!(
            "Event(ts={}, node={}, type={}, payload={:?})",
            self.logical_timestamp, self.node_id, self.event_type, self.payload
        )
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("logical_timestamp", self.logical_timestamp)?;
        dict.set_item("node_id", &self.node_id)?;
        dict.set_item("event_type", &self.event_type)?;
        dict.set_item("payload", &self.payload)?;
        Ok(dict.into())
    }
}

/// Python-exposed node representation.
#[pyclass]
#[derive(Clone, Debug)]
pub struct PyNode {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub node_type: String,
    #[pyo3(get)]
    pub priority: i32,
    #[pyo3(get)]
    pub timeout_ms: Option<u64>,
}

#[pymethods]
impl PyNode {
    fn __repr__(&self) -> String {
        format!(
            "Node(id={}, type={}, priority={})",
            self.id, self.node_type, self.priority
        )
    }
}

/// Python-exposed capability flags.
#[pyclass]
pub struct PyCap;

#[pymethods]
#[allow(non_snake_case)]
impl PyCap {
    #[staticmethod]
    fn NET_ACCESS() -> u32 { 0b00000001 }
    
    #[staticmethod]
    fn FILE_SYSTEM_READ() -> u32 { 0b00000010 }
    
    #[staticmethod]
    fn FILE_SYSTEM_WRITE() -> u32 { 0b00000100 }
    
    #[staticmethod]
    fn SUBPROCESS_EXEC() -> u32 { 0b00001000 }
    
    #[staticmethod]
    fn LLM_CALL() -> u32 { 0b00010000 }
    
    #[staticmethod]
    fn MEMORY_ACCESS() -> u32 { 0b00100000 }
    
    #[staticmethod]
    fn SENSITIVE_DATA() -> u32 { 0b01000000 }
    
    #[staticmethod]
    fn EXTERNAL_API() -> u32 { 0b10000000 }
    
    #[staticmethod]
    fn CODE_EXEC() -> u32 { 0b100000000 }
    
    #[staticmethod]
    fn DATABASE_ACCESS() -> u32 { 0b1000000000 }
    
    #[staticmethod]
    fn ALL() -> u32 { u32::MAX }
    
    #[staticmethod]
    fn NONE() -> u32 { 0 }

    /// Combine multiple capability flags.
    #[staticmethod]
    fn combine(caps: Vec<u32>) -> u32 {
        caps.iter().fold(0, |acc, &c| acc | c)
    }
}

/// Python-exposed policy configuration.
#[pyclass]
#[derive(Clone, Debug)]
pub struct PyPolicy {
    pub(crate) allowed_capabilities: u32,
    pub(crate) max_tool_calls: HashMap<String, u32>,
    pub(crate) forbidden_content: Vec<String>,
    pub(crate) _content_action: String,
    pub(crate) name: String,
}

#[pymethods]
impl PyPolicy {
    #[new]
    #[pyo3(signature = (
        allowed_capabilities=None,
        max_tool_calls=None,
        forbidden_content=None,
        content_action="redact",
        name="default"
    ))]
    fn new(
        allowed_capabilities: Option<Vec<u32>>,
        max_tool_calls: Option<HashMap<String, u32>>,
        forbidden_content: Option<Vec<String>>,
        content_action: &str,
        name: &str,
    ) -> Self {
        let caps = allowed_capabilities
            .map(|v| v.iter().fold(0, |acc, &c| acc | c))
            .unwrap_or(u32::MAX); // Default: all capabilities
        
        Self {
            allowed_capabilities: caps,
            max_tool_calls: max_tool_calls.unwrap_or_default(),
            forbidden_content: forbidden_content.unwrap_or_default(),
            _content_action: content_action.to_string(),
            name: name.to_string(),
        }
    }

    /// Create a permissive policy that allows all capabilities.
    #[staticmethod]
    fn permissive() -> Self {
        Self {
            allowed_capabilities: u32::MAX,
            max_tool_calls: HashMap::new(),
            forbidden_content: Vec::new(),
            _content_action: "redact".to_string(),
            name: "permissive".to_string(),
        }
    }

    /// Create a restrictive policy that allows no capabilities.
    #[staticmethod]
    fn restrictive() -> Self {
        Self {
            allowed_capabilities: 0,
            max_tool_calls: HashMap::new(),
            forbidden_content: Vec::new(),
            _content_action: "block".to_string(),
            name: "restrictive".to_string(),
        }
    }

    /// Create a no-network policy.
    #[staticmethod]
    fn no_network() -> Self {
        Self {
            allowed_capabilities: u32::MAX & !(0b00000001 | 0b10000000), // No NET_ACCESS or EXTERNAL_API
            max_tool_calls: HashMap::new(),
            forbidden_content: Vec::new(),
            _content_action: "redact".to_string(),
            name: "no_network".to_string(),
        }
    }

    /// Check if a capability is allowed.
    fn has_capability(&self, cap: u32) -> bool {
        (self.allowed_capabilities & cap) == cap
    }

    fn __repr__(&self) -> String {
        format!(
            "Policy(name={}, caps=0x{:x}, limits={:?})",
            self.name, self.allowed_capabilities, self.max_tool_calls
        )
    }
}

/// Thread-safe storage for Python callbacks.
struct CallbackStore {
    handlers: HashMap<String, PyObject>,
    conditions: HashMap<String, PyObject>,
}

/// Stores tool metadata for policy enforcement.
struct ToolStore {
    metadata: HashMap<String, u32>, // tool_id -> required_capabilities
}

/// The KAIROS-ARK Kernel exposed to Python.
/// 
/// Provides methods for building graphs, registering handlers,
/// and executing workflows with deterministic scheduling.
#[pyclass]
pub struct PyKernel {
    graph: Mutex<Graph>,
    ledger: Arc<AuditLedger>,
    clock: Arc<LogicalClock>,
    seed: Mutex<Option<u64>>,
    callbacks: Mutex<CallbackStore>,
    num_threads: Mutex<Option<usize>>,
    tools: Mutex<ToolStore>,
    policy: Mutex<Option<PyPolicy>>,
}

#[pymethods]
impl PyKernel {
    /// Create a new kernel instance.
    #[new]
    #[pyo3(signature = (seed=None, num_threads=None))]
    fn new(seed: Option<u64>, num_threads: Option<usize>) -> Self {
        Self {
            graph: Mutex::new(Graph::new()),
            ledger: Arc::new(AuditLedger::new()),
            clock: Arc::new(LogicalClock::new()),
            seed: Mutex::new(seed),
            callbacks: Mutex::new(CallbackStore {
                handlers: HashMap::new(),
                conditions: HashMap::new(),
            }),
            num_threads: Mutex::new(num_threads),
            tools: Mutex::new(ToolStore {
                metadata: HashMap::new(),
            }),
            policy: Mutex::new(None),
        }
    }

    /// Add a task node to the graph.
    #[pyo3(signature = (node_id, handler_id, priority=0, timeout_ms=None))]
    fn add_task(
        &self,
        node_id: String,
        handler_id: String,
        priority: i32,
        timeout_ms: Option<u64>,
    ) -> PyResult<()> {
        let mut node = Node::task(&node_id, &handler_id)
            .with_priority(priority);
        
        if let Some(timeout) = timeout_ms {
            node = node.with_timeout(timeout);
        }

        self.graph.lock().add_node(node);
        Ok(())
    }

    /// Add a branch node to the graph.
    fn add_branch(
        &self,
        node_id: String,
        condition_id: String,
        true_node: String,
        false_node: String,
    ) -> PyResult<()> {
        let node = Node::branch(&node_id, &condition_id, &true_node, &false_node);
        self.graph.lock().add_node(node);
        Ok(())
    }

    /// Add a fork node (parallel split) to the graph.
    fn add_fork(&self, node_id: String, children: Vec<String>) -> PyResult<()> {
        let node = Node::fork(&node_id, children);
        self.graph.lock().add_node(node);
        Ok(())
    }

    /// Add a join node (parallel merge) to the graph.
    #[pyo3(signature = (node_id, parents, next_node=None))]
    fn add_join(
        &self,
        node_id: String,
        parents: Vec<String>,
        next_node: Option<String>,
    ) -> PyResult<()> {
        let mut node = Node::join(&node_id, parents);
        
        if let Some(next) = next_node {
            node = node.with_edge(next);
        }

        self.graph.lock().add_node(node);
        Ok(())
    }

    /// Add an edge between two nodes.
    fn add_edge(&self, from_node: String, to_node: String) -> PyResult<bool> {
        Ok(self.graph.lock().add_edge(&from_node, to_node))
    }

    /// Set the entry point for graph execution.
    fn set_entry(&self, node_id: String) -> PyResult<()> {
        self.graph.lock().set_entry(node_id);
        Ok(())
    }

    /// Register a Python handler function for a given handler ID.
    fn register_handler(&self, handler_id: String, handler: PyObject) -> PyResult<()> {
        self.callbacks.lock().handlers.insert(handler_id, handler);
        Ok(())
    }

    /// Register a Python condition function for branch nodes.
    fn register_condition(&self, condition_id: String, condition: PyObject) -> PyResult<()> {
        self.callbacks.lock().conditions.insert(condition_id, condition);
        Ok(())
    }

    /// Set the policy for this kernel.
    fn set_policy(&self, policy: PyPolicy) -> PyResult<()> {
        *self.policy.lock() = Some(policy);
        Ok(())
    }

    /// Get the current policy (if any).
    fn get_policy(&self) -> Option<PyPolicy> {
        self.policy.lock().clone()
    }

    /// Register a tool with its required capabilities.
    fn register_tool(&self, tool_id: String, required_capabilities: u32) -> PyResult<()> {
        self.tools.lock().metadata.insert(tool_id, required_capabilities);
        Ok(())
    }

    /// Check if a tool can be executed under current policy.
    /// Returns (allowed: bool, reason: Option<str>)
    fn check_capability(&self, tool_id: String) -> PyResult<(bool, Option<String>)> {
        let policy = match &*self.policy.lock() {
            Some(p) => p.clone(),
            None => return Ok((true, None)), // No policy = allow all
        };

        let required = match self.tools.lock().metadata.get(&tool_id) {
            Some(&caps) => caps,
            None => return Ok((true, None)), // Unknown tool = allow
        };

        // Check if policy allows all required capabilities
        if (policy.allowed_capabilities & required) == required {
            Ok((true, None))
        } else {
            let missing = required & !policy.allowed_capabilities;
            let reason = format!(
                "Tool '{}' requires capabilities 0x{:x} but policy only allows 0x{:x} (missing: 0x{:x})",
                tool_id, required, policy.allowed_capabilities, missing
            );
            Ok((false, Some(reason)))
        }
    }

    /// Check and increment call counter for a tool.
    fn check_call_limit(&self, tool_id: String) -> PyResult<(bool, Option<String>)> {
        let policy = match &*self.policy.lock() {
            Some(p) => p.clone(),
            None => return Ok((true, None)),
        };

        let _limit = match policy.max_tool_calls.get(&tool_id) {
            Some(&limit) => limit,
            None => return Ok((true, None)), // No limit
        };

        // For now, we track calls in the tools store
        // This is a simplified implementation - in production you'd want separate counters
        Ok((true, None)) // TODO: Implement call counting
    }

    /// Filter content for forbidden patterns.
    fn filter_content(&self, content: String) -> PyResult<(String, Vec<String>)> {
        let policy = match &*self.policy.lock() {
            Some(p) => p.clone(),
            None => return Ok((content, Vec::new())),
        };

        let mut result = content;
        let mut matched = Vec::new();

        for pattern in &policy.forbidden_content {
            if result.contains(pattern) {
                matched.push(format!("substring:{}", pattern));
                result = result.replace(pattern, "[REDACTED]");
            }
        }

        Ok((result, matched))
    }

    /// Execute the graph and return results.
    #[pyo3(signature = (entry_node=None))]
    fn execute<'py>(&self, py: Python<'py>, entry_node: Option<String>) -> PyResult<PyObject> {
        // Clone data we need, releasing locks before execution
        let mut graph = self.graph.lock().clone();
        let seed = *self.seed.lock();
        let num_threads = *self.num_threads.lock();
        
        // Set entry if provided
        if let Some(ref entry) = entry_node {
            graph.set_entry(entry);
        }
        
        let (handlers, conditions) = {
            let callbacks = self.callbacks.lock();
            let handlers: HashMap<String, PyObject> = callbacks.handlers.iter()
                .map(|(k, v)| (k.clone(), v.clone_ref(py)))
                .collect();
            let conditions: HashMap<String, PyObject> = callbacks.conditions.iter()
                .map(|(k, v)| (k.clone(), v.clone_ref(py)))
                .collect();
            (handlers, conditions)
        };
        
        let scheduler = Scheduler::with_config(graph, seed, num_threads);

        // Register handlers (cloned, so no lock held)
        for (handler_id, py_handler) in handlers {
            let handler_clone = py_handler;
            scheduler.register_handler(handler_id, move |node_id, _ctx| {
                Python::with_gil(|py| {
                    let result = handler_clone
                        .call1(py, (node_id.clone(),))
                        .map_err(|e| crate::core::SchedulerError::PythonError(e.to_string()))?;
                    
                    let output: String = result
                        .extract(py)
                        .unwrap_or_else(|_| format!("{:?}", result));
                    
                    Ok(output)
                })
            });
        }

        // Register conditions (cloned, so no lock held)
        for (condition_id, py_condition) in conditions {
            let condition_clone = py_condition;
            scheduler.register_condition(condition_id, move || {
                Python::with_gil(|py| {
                    condition_clone
                        .call0(py)
                        .and_then(|r| r.extract::<bool>(py))
                        .unwrap_or(false)
                })
            });
        }

        // Execute (release GIL to allow parallel threads to call back into Python)
        let (results, audit_log, new_seed) = py.allow_threads(|| {
            let results = scheduler.execute();
            let audit_log = scheduler.get_audit_log();
            let new_seed = scheduler.get_seed();
            (results, audit_log, new_seed)
        });
        
        // Copy events to our ledger
        for event in audit_log {
            self.ledger.append(event);
        }
        
        // Update seed if it was auto-generated
        if self.seed.lock().is_none() {
            *self.seed.lock() = Some(new_seed);
        }

        // Convert results to Python
        match results {
            Ok(node_results) => {
                let py_results = PyList::empty(py);
                for result in node_results {
                    let dict = PyDict::new(py);
                    dict.set_item("node_id", &result.node_id)?;
                    dict.set_item("status", format!("{:?}", result.status))?;
                    dict.set_item("output", &result.output)?;
                    dict.set_item("error", &result.error)?;
                    dict.set_item("logical_timestamp", result.logical_timestamp)?;
                    py_results.append(dict)?;
                }
                Ok(py_results.into())
            }
            Err(e) => Err(PyRuntimeError::new_err(format!("Execution error: {}", e))),
        }
    }

    /// Dispatch a single node for execution (for throughput testing).
    fn dispatch_node(&self, py: Python<'_>, node_id: String) -> PyResult<Option<String>> {
        let callbacks = self.callbacks.lock();
        
        // Find the handler for this node
        let graph = self.graph.lock();
        if let Some(node) = graph.get(&node_id) {
            if let NodeType::Task { handler } = &node.node_type {
                if let Some(py_handler) = callbacks.handlers.get(handler) {
                    let result = py_handler
                        .call1(py, (node_id.clone(),))
                        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
                    
                    let output: String = result
                        .extract(py)
                        .unwrap_or_else(|_| format!("{:?}", result));
                    
                    // Log to ledger
                    let ts = self.clock.tick();
                    self.ledger.log_start(ts, &node_id);
                    let ts = self.clock.tick();
                    self.ledger.log_end(ts, &node_id, Some(output.clone()));
                    
                    return Ok(Some(output));
                }
            }
        }
        
        Ok(None)
    }

    /// Get the audit log as a list of events.
    fn get_audit_log<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let events = self.ledger.get_events_sorted();
        let py_list = PyList::empty(py);
        
        for event in events {
            let event_type_str = match &event.event_type {
                EventType::Start => "Start".to_string(),
                EventType::End => "End".to_string(),
                EventType::BranchDecision { chosen_path, .. } => {
                    format!("BranchDecision({})", chosen_path)
                }
                EventType::ForkSpawn { children } => {
                    format!("ForkSpawn({:?})", children)
                }
                EventType::JoinComplete { parents } => {
                    format!("JoinComplete({:?})", parents)
                }
                EventType::ToolOutput { data } => {
                    format!("ToolOutput({})", data)
                }
                EventType::Error { message } => {
                    format!("Error({})", message)
                }
                EventType::RngSeedCaptured { seed } => {
                    format!("RngSeedCaptured({})", seed)
                }
                EventType::ExecutionStart { entry_node } => {
                    format!("ExecutionStart({})", entry_node)
                }
                EventType::ExecutionEnd { success } => {
                    format!("ExecutionEnd({})", success)
                }
                EventType::PolicyAllow { tool_id, capabilities_checked } => {
                    format!("PolicyAllow({}, {:?})", tool_id, capabilities_checked)
                }
                EventType::PolicyDeny { tool_id, rule, reason } => {
                    format!("PolicyDeny({}, {}, {})", tool_id, rule, reason)
                }
                EventType::ContentRedacted { original_length, redacted_length, patterns_matched } => {
                    format!("ContentRedacted({}->{}, {:?})", original_length, redacted_length, patterns_matched)
                }
                EventType::CallLimitExceeded { tool_id, limit, attempted } => {
                    format!("CallLimitExceeded({}, limit={}, attempted={})", tool_id, limit, attempted)
                }
                EventType::SnapshotCreated { path, event_count } => {
                    format!("SnapshotCreated({}, events={})", path, event_count)
                }
                EventType::ResumeFromCheckpoint { checkpoint_ts, resume_node } => {
                    format!("ResumeFromCheckpoint(ts={}, node={:?})", checkpoint_ts, resume_node)
                }
                EventType::ExternalCapture { source, value } => {
                    format!("ExternalCapture({}, len={})", source, value.len())
                }
            };

            let dict = PyDict::new(py);
            dict.set_item("logical_timestamp", event.logical_timestamp)?;
            dict.set_item("node_id", &event.node_id)?;
            dict.set_item("event_type", event_type_str)?;
            dict.set_item("payload", &event.payload)?;
            py_list.append(dict)?;
        }
        
        Ok(py_list.into())
    }

    /// Get the audit log as JSON.
    fn get_audit_log_json(&self) -> PyResult<String> {
        self.ledger.to_json()
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
    }

    /// Get the current logical clock value.
    fn get_clock_value(&self) -> u64 {
        self.clock.current()
    }

    /// Get the RNG seed.
    fn get_seed(&self) -> Option<u64> {
        *self.seed.lock()
    }

    /// Clear the graph.
    fn clear_graph(&self) -> PyResult<()> {
        *self.graph.lock() = Graph::new();
        Ok(())
    }

    /// Clear the audit log.
    fn clear_audit_log(&self) -> PyResult<()> {
        self.ledger.clear();
        self.clock.reset();
        Ok(())
    }

    /// Get the number of nodes in the graph.
    fn node_count(&self) -> usize {
        self.graph.lock().len()
    }

    /// Get the number of events in the audit log.
    fn event_count(&self) -> usize {
        self.ledger.len()
    }

    /// List all node IDs in the graph.
    fn list_nodes<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let graph = self.graph.lock();
        let nodes: Vec<_> = graph.node_ids().cloned().collect();
        Ok(PyList::new(py, nodes)?.into())
    }

    /// Get information about a specific node.
    fn get_node(&self, node_id: String) -> PyResult<Option<PyNode>> {
        let graph = self.graph.lock();
        Ok(graph.get(&node_id).map(|node| {
            let node_type = match &node.node_type {
                NodeType::Task { .. } => "Task",
                NodeType::Branch { .. } => "Branch",
                NodeType::Fork { .. } => "Fork",
                NodeType::Join { .. } => "Join",
                NodeType::Entry => "Entry",
                NodeType::Exit => "Exit",
            };
            PyNode {
                id: node.id.clone(),
                node_type: node_type.to_string(),
                priority: node.priority,
                timeout_ms: node.timeout_ms,
            }
        }))
    }

    /// Save the current audit log to a JSONL file.
    fn save_ledger(&self, path: String) -> PyResult<()> {
        use crate::core::persistence::{DurableLedger, LedgerConfig};


        let config = LedgerConfig::new(&path).with_sync_flush();
        let durable = DurableLedger::new(config)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create ledger file: {}", e)))?;

        let seed = *self.seed.lock();
        for event in self.ledger.get_events_sorted() {
            durable.append(event, seed)
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to write event: {}", e)))?;
        }
        durable.flush()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to flush: {}", e)))?;

        Ok(())
    }

    /// Load audit log from a JSONL file and return events.
    fn load_ledger<'py>(&self, py: Python<'py>, path: String) -> PyResult<PyObject> {
        use crate::core::persistence::DurableLedger;
        use std::path::Path;

        let events = DurableLedger::read_all(Path::new(&path))
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to read ledger: {}", e)))?;

        let py_list = PyList::empty(py);
        for event in events {
            let dict = PyDict::new(py);
            dict.set_item("logical_timestamp", event.event.logical_timestamp)?;
            dict.set_item("node_id", &event.event.node_id)?;
            dict.set_item("event_type", event.event.event_type.as_str())?;
            dict.set_item("payload", &event.event.payload)?;
            dict.set_item("run_id", &event.run_id)?;
            dict.set_item("wall_clock_ms", event.wall_clock_ms)?;
            dict.set_item("rng_state", event.rng_state)?;
            py_list.append(dict)?;
        }

        Ok(py_list.into())
    }

    /// Replay a ledger and return the final state.
    fn replay_ledger<'py>(&self, py: Python<'py>, path: String) -> PyResult<PyObject> {
        use crate::core::replay::{ReplayScheduler, ReplayMode};
        use std::path::Path;

        let mut scheduler = ReplayScheduler::from_ledger(Path::new(&path), ReplayMode::FastForward)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to load ledger: {}", e)))?;

        // Replay all events
        while scheduler.step().is_some() {}

        let state = scheduler.fork();
        let dict = PyDict::new(py);
        dict.set_item("clock_value", state.clock_value)?;
        dict.set_item("rng_state", state.rng_state)?;
        dict.set_item("last_node", state.last_node)?;

        let outputs = PyDict::new(py);
        for (k, v) in &state.node_outputs {
            outputs.set_item(k, v)?;
        }
        dict.set_item("node_outputs", outputs)?;

        Ok(dict.into())
    }

    /// Create a state snapshot.
    #[pyo3(signature = (path, run_id=None))]
    fn create_snapshot(&self, path: String, run_id: Option<String>) -> PyResult<()> {
        use crate::core::persistence::StateSnapshot;
        use std::path::Path;

        let seed = self.seed.lock().unwrap_or(0);
        let clock = self.clock.current();

        // Collect node outputs from ledger
        let mut outputs = HashMap::new();
        let mut last_node = None;
        for event in self.ledger.get_events_sorted() {
            if let EventType::End = &event.event_type {
                last_node = Some(event.node_id.clone());
                if let Some(payload) = &event.payload {
                    outputs.insert(event.node_id.clone(), payload.clone());
                }
            }
        }

        let snapshot = StateSnapshot::new(
            run_id,
            clock,
            seed,
            outputs,
            last_node,
            clock,
        );

        snapshot.save(Path::new(&path))
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to save snapshot: {}", e)))?;

        Ok(())
    }

    /// Load a snapshot and return state info.
    fn load_snapshot<'py>(&self, py: Python<'py>, path: String) -> PyResult<PyObject> {
        use crate::core::persistence::StateSnapshot;
        use std::path::Path;

        let snapshot = StateSnapshot::load(Path::new(&path))
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to load snapshot: {}", e)))?;

        let dict = PyDict::new(py);
        dict.set_item("version", snapshot.version)?;
        dict.set_item("run_id", snapshot.run_id)?;
        dict.set_item("clock_value", snapshot.clock_value)?;
        dict.set_item("rng_state", snapshot.rng_state)?;
        dict.set_item("last_node", snapshot.last_node)?;
        dict.set_item("last_timestamp", snapshot.last_timestamp)?;
        dict.set_item("created_at_ms", snapshot.created_at_ms)?;

        let outputs = PyDict::new(py);
        for (k, v) in &snapshot.node_outputs {
            outputs.set_item(k, v)?;
        }
        dict.set_item("node_outputs", outputs)?;

        Ok(dict.into())
    }

    /// Check if a run has a recovery point.
    fn has_recovery(&self, ledger_dir: String, run_id: String) -> PyResult<bool> {
        use crate::core::recovery::RecoveryManager;

        let manager = RecoveryManager::new(&ledger_dir);
        Ok(manager.has_pending_run(&run_id))
    }

    /// Get recovery point info.
    fn get_recovery_point<'py>(&self, py: Python<'py>, ledger_dir: String, run_id: String) -> PyResult<Option<PyObject>> {
        use crate::core::recovery::RecoveryManager;

        let manager = RecoveryManager::new(&ledger_dir);
        match manager.get_recovery_point(&run_id) {
            Ok(Some(point)) => {
                let dict = PyDict::new(py);
                dict.set_item("run_id", &point.run_id)?;
                dict.set_item("last_node", &point.last_node)?;
                dict.set_item("last_timestamp", point.last_timestamp)?;
                dict.set_item("event_count", point.event_count)?;
                dict.set_item("completed", point.completed)?;
                dict.set_item("snapshot_path", point.snapshot_path.map(|p| p.to_string_lossy().to_string()))?;
                dict.set_item("ledger_path", point.ledger_path.to_string_lossy().to_string())?;
                Ok(Some(dict.into()))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(PyRuntimeError::new_err(format!("Failed to get recovery point: {}", e))),
        }
    }

    // ===== Phase 4: Shared Memory =====

    /// Write data to shared memory pool, return handle ID.
    fn write_shared(&self, data: Vec<u8>) -> PyResult<u64> {
        use crate::core::shared_memory::global_store;
        
        match global_store().write(&data) {
            Some(handle) => Ok(handle.id),
            None => Err(PyRuntimeError::new_err("Failed to allocate shared memory")),
        }
    }

    /// Read data from shared memory by handle ID.
    fn read_shared(&self, handle_id: u64) -> PyResult<Vec<u8>> {
        use crate::core::shared_memory::global_store;
        
        global_store()
            .read_by_id(handle_id)
            .ok_or_else(|| PyRuntimeError::new_err("Handle not found or invalid"))
    }

    /// Get shared memory pool stats.
    fn shared_memory_stats<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        use crate::core::shared_memory::global_store;
        
        let store = global_store();
        let dict = PyDict::new(py);
        dict.set_item("capacity", store.capacity())?;
        dict.set_item("used", store.used())?;
        dict.set_item("available", store.available())?;
        dict.set_item("allocations", store.allocation_count())?;
        Ok(dict.into())
    }

    // ===== Phase 4: Plugins =====

    /// Register a Rust-native plugin (for testing).
    fn register_plugin(&self, name: String, version: String) -> PyResult<String> {
        use crate::core::plugin::global_loader;
        
        let loader = global_loader();
        let name_clone = name.clone();
        
        // Register a simple echo plugin for testing
        loader.register(&name, &version, move |input| {
            Ok(format!("Plugin[{}]: {}", name_clone, input))
        });
        
        Ok(name)
    }

    /// Invoke a registered plugin.
    fn invoke_plugin(&self, name: String, input: String) -> PyResult<String> {
        use crate::core::plugin::global_loader;
        
        global_loader()
            .invoke(&name, &input)
            .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))
    }

    /// List all loaded plugins.
    fn list_plugins<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        use crate::core::plugin::global_loader;
        
        let plugins = global_loader().list();
        let py_list = PyList::empty(py);
        
        for info in plugins {
            let dict = PyDict::new(py);
            dict.set_item("name", &info.name)?;
            dict.set_item("version", &info.version)?;
            dict.set_item("capabilities", info.capabilities)?;
            dict.set_item("path", &info.path)?;
            py_list.append(dict)?;
        }
        
        Ok(py_list.into())
    }

    // ===== Phase 5: State Store (LangGraph Integration) =====

    /// Set a value in the global state store.
    fn state_set(&self, key: String, value: String) -> PyResult<()> {
        use crate::adapters::global_state_store;
        global_state_store().set_string(key, value);
        Ok(())
    }

    /// Get a value from the global state store.
    fn state_get(&self, key: String) -> PyResult<Option<String>> {
        use crate::adapters::global_state_store;
        Ok(global_state_store().get_string(&key))
    }

    /// Create a state checkpoint.
    fn state_checkpoint(&self, id: String) -> PyResult<()> {
        use crate::adapters::global_state_store;
        global_state_store().checkpoint(id);
        Ok(())
    }

    /// Restore from a state checkpoint.
    fn state_restore(&self, id: String) -> PyResult<bool> {
        use crate::adapters::global_state_store;
        Ok(global_state_store().restore(&id))
    }

    /// Get all state keys.
    fn state_keys<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        use crate::adapters::global_state_store;
        let keys = global_state_store().keys();
        Ok(PyList::new(py, keys)?.into())
    }

    /// Get state version.
    fn state_version(&self) -> u64 {
        use crate::adapters::global_state_store;
        global_state_store().version()
    }

    // ===== Phase 5: MCP Support =====

    /// Register an MCP tool.
    fn mcp_register_tool(&self, name: String, description: String) -> PyResult<()> {
        use crate::adapters::mcp::{global_mcp_server, McpToolInfo, McpResult};
        
        let server = global_mcp_server();
        let info = McpToolInfo::new(&name, description);
        
        let name_clone = name.clone();
        server.register(info, move |args| {
            McpResult::ok(serde_json::json!({
                "tool": name_clone,
                "args": args,
                "status": "mock_response"
            }))
        });
        
        Ok(())
    }

    /// List MCP tools.
    fn mcp_list_tools<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        use crate::adapters::mcp::global_mcp_server;
        
        let tools = global_mcp_server().list_tools();
        let py_list = PyList::empty(py);
        
        for tool in tools {
            let dict = PyDict::new(py);
            dict.set_item("name", &tool.name)?;
            dict.set_item("description", &tool.description)?;
            dict.set_item("capabilities", tool.capabilities)?;
            py_list.append(dict)?;
        }
        
        Ok(py_list.into())
    }

    /// Call an MCP tool.
    fn mcp_call_tool(&self, name: String, args: String) -> PyResult<String> {
        use crate::adapters::mcp::global_mcp_server;
        
        let args_value: serde_json::Value = serde_json::from_str(&args)
            .map_err(|e| PyRuntimeError::new_err(format!("Invalid JSON: {}", e)))?;
        
        let result = global_mcp_server().call_tool(&name, args_value);
        
        serde_json::to_string(&result)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization error: {}", e)))
    }

    // ===== Phase 6: HITL Approval Gateway =====

    /// Request approval for a sensitive action.
    fn request_approval(&self, run_id: String, node_id: String, reason: String) -> PyResult<String> {
        use crate::governance::approval::{global_gateway, ApprovalRequest};
        
        let request = ApprovalRequest::new(run_id, node_id, reason);
        let id = global_gateway().request_approval(request);
        Ok(id)
    }

    /// Approve a pending request.
    #[pyo3(signature = (request_id, approver=None))]
    fn approve(&self, request_id: String, approver: Option<String>) -> PyResult<bool> {
        use crate::governance::approval::global_gateway;
        Ok(global_gateway().approve(&request_id, approver.as_deref()))
    }

    /// Reject a pending request.
    #[pyo3(signature = (request_id, reason, rejector=None))]
    fn reject(&self, request_id: String, reason: String, rejector: Option<String>) -> PyResult<bool> {
        use crate::governance::approval::global_gateway;
        Ok(global_gateway().reject(&request_id, &reason, rejector.as_deref()))
    }

    /// Check approval status.
    fn check_approval(&self, request_id: String) -> PyResult<Option<String>> {
        use crate::governance::approval::global_gateway;
        Ok(global_gateway().check_status(&request_id).map(|s| s.as_str().to_string()))
    }

    /// List pending approvals.
    fn list_pending_approvals<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        use crate::governance::approval::global_gateway;
        
        let pending = global_gateway().list_pending();
        let py_list = PyList::empty(py);
        
        for req in pending {
            let dict = PyDict::new(py);
            dict.set_item("id", &req.id)?;
            dict.set_item("run_id", &req.run_id)?;
            dict.set_item("node_id", &req.node_id)?;
            dict.set_item("reason", &req.reason)?;
            dict.set_item("created_at", req.created_at)?;
            py_list.append(dict)?;
        }
        
        Ok(py_list.into())
    }

    // ===== Phase 6: Audit Verification =====

    /// Sign a ledger for compliance.
    fn sign_ledger(&self, ledger_json: String, run_id: String) -> PyResult<String> {
        use crate::governance::verification::SignedLedger;
        
        let signed = SignedLedger::new(ledger_json, run_id);
        serde_json::to_string(&signed)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization error: {}", e)))
    }

    /// Verify a signed ledger.
    fn verify_ledger(&self, signed_json: String) -> PyResult<bool> {
        use crate::governance::verification::SignedLedger;
        
        let signed: SignedLedger = serde_json::from_str(&signed_json)
            .map_err(|e| PyRuntimeError::new_err(format!("Invalid JSON: {}", e)))?;
        
        Ok(signed.verify().valid)
    }

    fn __repr__(&self) -> String {
        format!(
            "PyKernel(nodes={}, events={}, seed={:?})",
            self.graph.lock().len(),
            self.ledger.len(),
            *self.seed.lock()
        )
    }
}

