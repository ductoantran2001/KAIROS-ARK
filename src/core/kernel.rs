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

/// Thread-safe storage for Python callbacks.
struct CallbackStore {
    handlers: HashMap<String, PyObject>,
    conditions: HashMap<String, PyObject>,
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

    /// Execute the graph and return results.
    fn execute<'py>(&self, py: Python<'py>, entry_node: Option<String>) -> PyResult<&'py PyList> {
        // Clone data we need, releasing locks before execution
        let mut graph = self.graph.lock().clone();
        let seed = *self.seed.lock();
        let num_threads = *self.num_threads.lock();
        
        // Set entry if provided
        if let Some(ref entry) = entry_node {
            graph.set_entry(entry);
        }
        
        // Clone handlers and conditions to avoid holding lock during execution
        let (handlers, conditions) = {
            let callbacks = self.callbacks.lock();
            (callbacks.handlers.clone(), callbacks.conditions.clone())
        };
        
        let scheduler = Scheduler::with_config(graph, seed, num_threads);

        // Register handlers (cloned, so no lock held)
        for (handler_id, py_handler) in handlers.iter() {
            let handler_clone = py_handler.clone();
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
        for (condition_id, py_condition) in conditions.iter() {
            let condition_clone = py_condition.clone();
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
                Ok(py_results)
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
    fn get_audit_log<'py>(&self, py: Python<'py>) -> PyResult<&'py PyList> {
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
            };

            let dict = PyDict::new(py);
            dict.set_item("logical_timestamp", event.logical_timestamp)?;
            dict.set_item("node_id", &event.node_id)?;
            dict.set_item("event_type", event_type_str)?;
            dict.set_item("payload", &event.payload)?;
            py_list.append(dict)?;
        }
        
        Ok(py_list)
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
    fn list_nodes<'py>(&self, py: Python<'py>) -> PyResult<&'py PyList> {
        let graph = self.graph.lock();
        let nodes: Vec<_> = graph.node_ids().cloned().collect();
        Ok(PyList::new(py, nodes))
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

    fn __repr__(&self) -> String {
        format!(
            "PyKernel(nodes={}, events={}, seed={:?})",
            self.graph.lock().len(),
            self.ledger.len(),
            *self.seed.lock()
        )
    }
}
