"""
KAIROS-ARK Agent: High-level Python API for building and executing workflows.

The Agent class provides a user-friendly interface for:
- Adding task, branch, fork, and join nodes
- Registering Python handlers and conditions
- Executing graphs with deterministic scheduling
- Inspecting the audit log
"""

from typing import Any, Callable, Dict, List, Optional, Union
import json
import time


class Agent:
    """
    High-level agent for building and executing KAIROS-ARK workflows.
    
    The Agent wraps the low-level Kernel and provides convenient helpers
    for common workflow patterns like branching and parallel execution.
    
    Example:
        ```python
        from kairos_ark import Agent
        
        agent = Agent(seed=42)
        
        # Add task nodes
        agent.add_node("fetch_data", lambda: fetch_from_api())
        agent.add_node("process", lambda: transform_data())
        
        # Connect them
        agent.connect("fetch_data", "process")
        
        # Execute
        results = agent.execute("fetch_data")
        
        # Inspect trace
        agent.print_audit_log()
        ```
    """
    
    def __init__(self, seed: Optional[int] = None, num_threads: Optional[int] = None):
        """
        Initialize a new Agent.
        
        Args:
            seed: Optional RNG seed for deterministic execution. If not provided,
                  a seed will be generated and recorded in the audit log.
            num_threads: Optional number of threads for the worker pool.
                         Defaults to the number of CPU cores.
        """
        from ._core import PyKernel
        
        self.kernel = PyKernel(seed=seed, num_threads=num_threads)
        self._handlers: Dict[str, Callable] = {}
        self._conditions: Dict[str, Callable] = {}
        self._node_handlers: Dict[str, str] = {}  # node_id -> handler_id mapping
        
    def add_node(
        self,
        node_id: str,
        handler: Callable[[], Any],
        timeout_ms: Optional[int] = None,
        priority: int = 0,
    ) -> str:
        """
        Add a task node to the graph.
        
        Args:
            node_id: Unique identifier for the node.
            handler: Python callable to execute. Should return a string or
                     JSON-serializable value.
            timeout_ms: Optional timeout in milliseconds.
            priority: Execution priority (higher = execute first).
            
        Returns:
            The node ID (for chaining).
        """
        handler_id = f"_handler_{node_id}"
        
        # Wrap handler to accept node_id
        def wrapped_handler(nid: str) -> str:
            result = handler()
            if isinstance(result, str):
                return result
            return json.dumps(result) if result is not None else ""
        
        self._handlers[handler_id] = wrapped_handler
        self._node_handlers[node_id] = handler_id
        
        self.kernel.add_task(node_id, handler_id, priority, timeout_ms)
        self.kernel.register_handler(handler_id, wrapped_handler)
        
        return node_id
    
    def add_branch(
        self,
        node_id: str,
        condition_func: Callable[[], bool],
        true_node: str,
        false_node: str,
    ) -> str:
        """
        Add a conditional branch node.
        
        The condition function is evaluated at execution time, and exactly
        one outgoing edge is followed based on the result.
        
        Args:
            node_id: Unique identifier for the branch node.
            condition_func: Callable that returns True or False.
            true_node: Node to execute if condition is True.
            false_node: Node to execute if condition is False.
            
        Returns:
            The node ID (for chaining).
        """
        condition_id = f"_condition_{node_id}"
        
        self._conditions[condition_id] = condition_func
        
        self.kernel.add_branch(node_id, condition_id, true_node, false_node)
        self.kernel.register_condition(condition_id, condition_func)
        
        return node_id
    
    def add_fork(self, node_id: str, children: List[str]) -> str:
        """
        Add a parallel fork node.
        
        All child nodes will be executed concurrently using the thread pool.
        
        Args:
            node_id: Unique identifier for the fork node.
            children: List of node IDs to execute in parallel.
            
        Returns:
            The node ID (for chaining).
        """
        self.kernel.add_fork(node_id, children)
        return node_id
    
    def add_join(
        self,
        node_id: str,
        parents: List[str],
        next_node: Optional[str] = None,
    ) -> str:
        """
        Add a join node that waits for multiple parents.
        
        The join node will only execute after all parent nodes have completed.
        Parent outputs are collected in deterministic order (sorted by node ID).
        
        Args:
            node_id: Unique identifier for the join node.
            parents: List of parent node IDs to wait for.
            next_node: Optional node to execute after join completes.
            
        Returns:
            The node ID (for chaining).
        """
        self.kernel.add_join(node_id, parents, next_node)
        return node_id
    
    def connect(self, from_node: str, to_node: str) -> bool:
        """
        Add an edge between two nodes.
        
        Args:
            from_node: Source node ID.
            to_node: Target node ID.
            
        Returns:
            True if the edge was added successfully.
        """
        return self.kernel.add_edge(from_node, to_node)
    
    def set_entry(self, node_id: str) -> None:
        """
        Set the entry point for graph execution.
        
        Args:
            node_id: The node to start execution from.
        """
        self.kernel.set_entry(node_id)
    
    def run_parallel(self, nodes: List[str]) -> List[Any]:
        """
        Execute multiple nodes in parallel.
        
        This is a convenience method that creates a temporary fork/join
        structure and executes it.
        
        Args:
            nodes: List of node IDs to execute in parallel.
            
        Returns:
            List of results from each node.
        """
        # Create temporary fork/join
        fork_id = "_parallel_fork"
        join_id = "_parallel_join"
        
        self.add_fork(fork_id, nodes)
        self.add_join(join_id, nodes)
        self.connect(fork_id, join_id)
        
        # Execute
        results = self.execute(fork_id)
        
        return results
    
    def execute(self, entry_node: Optional[str] = None) -> List[Dict[str, Any]]:
        """
        Execute the graph.
        
        Args:
            entry_node: Optional starting node. Uses the set_entry() node
                        if not specified.
                        
        Returns:
            List of node results with status and output.
        """
        results = self.kernel.execute(entry_node)
        return [dict(r) for r in results]
    
    def get_audit_log(self) -> List[Dict[str, Any]]:
        """
        Get the execution audit log.
        
        Returns:
            List of events sorted by logical timestamp.
        """
        events = self.kernel.get_audit_log()
        return [dict(e) for e in events]
    
    def get_audit_log_json(self) -> str:
        """
        Get the audit log as a JSON string.
        
        Returns:
            JSON-formatted audit log.
        """
        return self.kernel.get_audit_log_json()
    
    def print_audit_log(self) -> None:
        """
        Pretty-print the execution audit log.
        """
        events = self.get_audit_log()
        
        print("\n" + "=" * 60)
        print("KAIROS-ARK Execution Trace")
        print("=" * 60)
        
        for event in events:
            ts = event.get("logical_timestamp", "?")
            node = event.get("node_id", "?")
            event_type = event.get("event_type", "?")
            payload = event.get("payload", "")
            
            # Colorize based on event type
            if "Start" in event_type:
                prefix = "▶"
            elif "End" in event_type:
                prefix = "■"
            elif "Branch" in event_type:
                prefix = "◇"
            elif "Fork" in event_type:
                prefix = "⊕"
            elif "Join" in event_type:
                prefix = "⊗"
            elif "Error" in event_type:
                prefix = "✗"
            else:
                prefix = "●"
            
            print(f"[{ts:04}] {prefix} {node:20} | {event_type}")
            
            if payload:
                print(f"       └─ {payload[:60]}...")
        
        print("=" * 60)
        print(f"Total events: {len(events)}")
        print(f"Seed: {self.kernel.get_seed()}")
        print("=" * 60 + "\n")
    
    def get_seed(self) -> Optional[int]:
        """
        Get the RNG seed used for this execution.
        
        Returns:
            The seed value, or None if not yet executed.
        """
        return self.kernel.get_seed()
    
    def get_clock_value(self) -> int:
        """
        Get the current logical clock value.
        
        Returns:
            The current clock value.
        """
        return self.kernel.get_clock_value()
    
    def node_count(self) -> int:
        """
        Get the number of nodes in the graph.
        
        Returns:
            Node count.
        """
        return self.kernel.node_count()
    
    def event_count(self) -> int:
        """
        Get the number of events in the audit log.
        
        Returns:
            Event count.
        """
        return self.kernel.event_count()
    
    def list_nodes(self) -> List[str]:
        """
        List all node IDs in the graph.
        
        Returns:
            List of node IDs.
        """
        return list(self.kernel.list_nodes())
    
    def get_node(self, node_id: str) -> Optional[Dict[str, Any]]:
        """
        Get information about a specific node.
        
        Args:
            node_id: The node ID to look up.
            
        Returns:
            Node info dict, or None if not found.
        """
        node = self.kernel.get_node(node_id)
        if node:
            return {
                "id": node.id,
                "node_type": node.node_type,
                "priority": node.priority,
                "timeout_ms": node.timeout_ms,
            }
        return None
    
    def clear(self) -> None:
        """
        Clear the graph and audit log.
        """
        self.kernel.clear_graph()
        self.kernel.clear_audit_log()
        self._handlers.clear()
        self._conditions.clear()
        self._node_handlers.clear()
    
    def __repr__(self) -> str:
        return f"Agent(nodes={self.node_count()}, events={self.event_count()}, seed={self.get_seed()})"


# Convenience function for quick parallel execution
def run_parallel(tasks: List[Callable[[], Any]], seed: Optional[int] = None) -> List[Any]:
    """
    Execute multiple tasks in parallel and return results.
    
    This is a convenience function for simple parallel execution without
    building a full graph.
    
    Args:
        tasks: List of callables to execute.
        seed: Optional RNG seed for determinism.
        
    Returns:
        List of results from each task.
    """
    agent = Agent(seed=seed)
    
    node_ids = []
    for i, task in enumerate(tasks):
        node_id = f"task_{i}"
        agent.add_node(node_id, task)
        node_ids.append(node_id)
    
    return agent.run_parallel(node_ids)
