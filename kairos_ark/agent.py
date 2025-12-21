"""
KAIROS-ARK Agent: High-level Python API for building and executing workflows.

The Agent class provides a user-friendly interface for:
- Adding task, branch, fork, and join nodes
- Registering Python handlers and conditions
- Executing graphs with deterministic scheduling
- Inspecting the audit log
- Policy enforcement with capability model (Phase 2)
"""

from typing import Any, Callable, Dict, List, Optional, Union
import json
import time
import concurrent.futures
from contextlib import contextmanager


class Cap:
    """
    Capability flags for tool permissions.
    
    These flags define what operations a tool can perform.
    Policies use these to whitelist/blacklist capabilities.
    
    Example:
        ```python
        from kairos_ark import Cap, Policy
        
        # Create a policy that only allows file reads and LLM calls
        policy = Policy(allowed_capabilities=[Cap.FILE_SYSTEM_READ, Cap.LLM_CALL])
        ```
    """
    NET_ACCESS = 0b00000001          # Network/HTTP access
    FILE_SYSTEM_READ = 0b00000010    # Read from filesystem
    FILE_SYSTEM_WRITE = 0b00000100   # Write to filesystem
    SUBPROCESS_EXEC = 0b00001000     # Execute subprocesses
    LLM_CALL = 0b00010000            # Make LLM API calls
    MEMORY_ACCESS = 0b00100000       # Access agent memory
    SENSITIVE_DATA = 0b01000000      # Access sensitive data
    EXTERNAL_API = 0b10000000        # Call external APIs
    CODE_EXEC = 0b100000000          # Execute code
    DATABASE_ACCESS = 0b1000000000   # Access databases
    
    # Presets
    ALL = 0xFFFFFFFF
    NONE = 0
    
    # Aliases for convenience
    DISK_READ = FILE_SYSTEM_READ
    DISK_WRITE = FILE_SYSTEM_WRITE
    
    @classmethod
    def combine(cls, *caps: int) -> int:
        """Combine multiple capabilities into one."""
        result = 0
        for cap in caps:
            result |= cap
        return result


class Policy:
    """
    Agent execution policy with capability restrictions.
    
    Policies define what an agent is allowed to do during execution:
    - Which capabilities (NET_ACCESS, FILE_WRITE, etc.) are permitted
    - Maximum number of calls per tool
    - Forbidden content patterns to redact/block
    
    Example:
        ```python
        from kairos_ark import Policy, Cap
        
        # Restrictive policy: only LLM calls, no network, limited tool usage
        policy = Policy(
            allowed_capabilities=[Cap.LLM_CALL, Cap.FILE_SYSTEM_READ],
            max_tool_calls={"web_search": 0, "code_exec": 2},
            forbidden_content=["password", "api_key", "secret"]
        )
        
        agent.run("entry_node", policy=policy)
        ```
    """
    
    def __init__(
        self,
        allowed_capabilities: Optional[List[int]] = None,
        max_tool_calls: Optional[Dict[str, int]] = None,
        forbidden_content: Optional[List[str]] = None,
        content_action: str = "redact",  # "redact" or "block"
        name: str = "default",
    ):
        """
        Initialize a policy.
        
        Args:
            allowed_capabilities: List of capability flags (e.g. [Cap.LLM_CALL]).
                                 Defaults to ALL capabilities if not specified.
            max_tool_calls: Dict mapping tool_id to maximum allowed calls.
                           e.g. {"web_search": 2} limits web_search to 2 calls.
            forbidden_content: List of strings/patterns to redact from outputs.
            content_action: "redact" to replace matched content, "block" to stop execution.
            name: Name of this policy (for logging).
        """
        if allowed_capabilities is None:
            self.allowed_capabilities = Cap.ALL
        else:
            self.allowed_capabilities = Cap.combine(*allowed_capabilities)
        
        self.max_tool_calls = max_tool_calls or {}
        self.forbidden_content = forbidden_content or []
        self.content_action = content_action
        self.name = name
    
    @classmethod
    def permissive(cls) -> "Policy":
        """Create a policy that allows everything."""
        return cls(name="permissive")
    
    @classmethod
    def restrictive(cls) -> "Policy":
        """Create a policy that allows nothing."""
        return cls(allowed_capabilities=[Cap.NONE], name="restrictive")
    
    @classmethod
    def no_network(cls) -> "Policy":
        """Create a policy that blocks network access."""
        all_except_net = Cap.ALL & ~(Cap.NET_ACCESS | Cap.EXTERNAL_API)
        return cls(allowed_capabilities=[all_except_net], name="no_network")
    
    @classmethod
    def read_only(cls) -> "Policy":
        """Create a policy that only allows reading."""
        return cls(
            allowed_capabilities=[Cap.FILE_SYSTEM_READ, Cap.MEMORY_ACCESS, Cap.LLM_CALL],
            name="read_only"
        )
    
    def has_capability(self, cap: int) -> bool:
        """Check if a capability is allowed."""
        return (self.allowed_capabilities & cap) == cap
    
    def get_tool_limit(self, tool_id: str) -> Optional[int]:
        """Get the call limit for a tool, or None if unlimited."""
        return self.max_tool_calls.get(tool_id)
    
    def to_rust(self):
        """Convert to Rust PyPolicy for kernel use."""
        from ._core import PyPolicy
        return PyPolicy(
            allowed_capabilities=[self.allowed_capabilities],
            max_tool_calls=self.max_tool_calls,
            forbidden_content=self.forbidden_content,
            content_action=self.content_action,
            name=self.name,
        )
    
    def __repr__(self) -> str:
        return f"Policy(name={self.name}, caps=0x{self.allowed_capabilities:x}, limits={self.max_tool_calls})"



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
        self._tools: Dict[str, int] = {}  # tool_id -> required_capabilities
        self._policy: Optional[Policy] = None
        
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
        
        This uses a ThreadPoolExecutor to guarantee parallelism regardless of 
        the underlying kernel's configuration.
        
        Args:
            nodes: List of node IDs to execute in parallel.
            
        Returns:
            List of results from each node.
        """
        # Execute independant nodes in parallel threads
        results = [None] * len(nodes)
        
        def run_node(idx, node_id):
            # We treat each node as an isolated entry point
            # This is valid for 'fan-out' of independent tools
            # Note: This bypasses complicated graph dependencies for speed
            node_res = self.execute(node_id)
            # Flatten result if typically single item list
            return node_res[0] if node_res else None

        with concurrent.futures.ThreadPoolExecutor() as executor:
            futures = {executor.submit(run_node, i, nid): i for i, nid in enumerate(nodes)}
            
            for future in concurrent.futures.as_completed(futures):
                idx = futures[future]
                try:
                    results[idx] = future.result()
                except Exception as e:
                    results[idx] = {"error": str(e), "status": "failed"}
        
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
    
    def run(
        self,
        entry_node: Optional[str] = None,
        policy: Optional[Policy] = None,
    ) -> List[Dict[str, Any]]:
        """
        Execute the graph with optional policy enforcement.
        
        This is the preferred way to execute with policies.
        
        Args:
            entry_node: Optional starting node.
            policy: Optional Policy object to enforce during execution.
                    If not provided, execution is unrestricted.
        
        Returns:
            List of node results with status and output.
            
        Example:
            ```python
            policy = Policy(
                allowed_capabilities=[Cap.LLM_CALL],
                max_tool_calls={"web_search": 0},
            )
            results = agent.run("start", policy=policy)
            ```
        """
        if policy is not None:
            self._policy = policy
            self.kernel.set_policy(policy.to_rust())
        
        return self.execute(entry_node)
    
    def set_policy(self, policy: Policy) -> None:
        """
        Set the policy for future executions.
        
        Args:
            policy: Policy object defining capability restrictions.
        """
        self._policy = policy
        self.kernel.set_policy(policy.to_rust())
    
    def get_policy(self) -> Optional[Policy]:
        """Get the current policy, if any."""
        return self._policy
    
    def register_tool(
        self,
        tool_id: str,
        handler: Callable[[], Any],
        required_capabilities: List[int],
        timeout_ms: Optional[int] = None,
        priority: int = 0,
    ) -> str:
        """
        Register a tool with required capabilities.
        
        This extends add_node by also recording the tool's capability
        requirements for policy enforcement.
        
        Args:
            tool_id: Unique identifier for the tool.
            handler: Python callable to execute.
            required_capabilities: List of Cap flags this tool requires.
            timeout_ms: Optional timeout in milliseconds.
            priority: Execution priority.
            
        Returns:
            The tool ID.
            
        Example:
            ```python
            agent.register_tool(
                "web_search",
                lambda: fetch_web_results(),
                [Cap.NET_ACCESS, Cap.EXTERNAL_API],
            )
            ```
        """
        # Register as normal node
        self.add_node(tool_id, handler, timeout_ms, priority)
        
        # Track tool capabilities
        caps = Cap.combine(*required_capabilities)
        self._tools[tool_id] = caps
        self.kernel.register_tool(tool_id, caps)
        
        return tool_id
    
    def check_tool_capability(self, tool_id: str) -> tuple:
        """
        Check if a tool can be executed under current policy.
        
        Args:
            tool_id: The tool to check.
            
        Returns:
            Tuple of (allowed: bool, reason: Optional[str])
        """
        return self.kernel.check_capability(tool_id)

    def check_capability(self, cap_flag: int) -> tuple:
        """
        Check if a raw capability flag is allowed under current policy.
        
        Args:
            cap_flag: The capability integer to check (e.g. Cap.LLM_CALL).
            
        Returns:
            Tuple of (allowed: bool, reason: Optional[str])
        """
        # We need to expose a method on the compiled kernel for raw capability checking
        # If PyKernel.check_raw_capability exists, use it.
        # Otherwise, we can implement Python-side check if we have the policy object locally.
        if self._policy:
            if self._policy.has_capability(cap_flag):
                return (True, None)
            else:
                return (False, f"Capability 0x{cap_flag:x} denied by policy '{self._policy.name}'")
        return (True, None)
    
    def filter_content(self, content: str) -> tuple:
        """
        Filter content through the policy's forbidden patterns.
        
        Args:
            content: The content to filter.
            
        Returns:
            Tuple of (filtered_content, matched_patterns)
        """
        return self.kernel.filter_content(content)

    
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
        Clear the graph, audit log, tools, policy, and shared memory.
        """
        self.kernel.clear_graph()
        self.kernel.clear_audit_log()
        self.kernel.clear_shared_memory()
        self._handlers.clear()
        self._conditions.clear()
        self._node_handlers.clear()
        self._tools.clear()
        self._policy = None
    
    # ===== Persistence & Replay (Phase 3) =====
    
    def save_ledger(self, path: str) -> None:
        """
        Save the audit log to a JSONL file for later replay.
        
        Args:
            path: File path to save the ledger.
            
        Example:
            ```python
            agent.execute("start")
            agent.save_ledger("/tmp/run.jsonl")
            ```
        """
        self.kernel.save_ledger(path)
    
    def load_ledger(self, path: str) -> List[Dict[str, Any]]:
        """
        Load a saved ledger from file.
        
        Args:
            path: Path to the JSONL ledger file.
            
        Returns:
            List of persistent events with metadata.
        """
        events = self.kernel.load_ledger(path)
        return [dict(e) for e in events]
    
    def replay(self, ledger_path: str) -> Dict[str, Any]:
        """
        Replay a saved ledger and return the reconstructed state.
        
        This replays all events to reconstruct the execution state
        without re-invoking handlers.
        
        Args:
            ledger_path: Path to the saved ledger file.
            
        Returns:
            Dict with reconstructed state:
            - clock_value: Final logical timestamp
            - rng_state: Final RNG state
            - last_node: Last completed node
            - node_outputs: Dict of node_id -> output
        """
        return dict(self.kernel.replay_ledger(ledger_path))
    
    def create_snapshot(self, path: str, run_id: Optional[str] = None) -> None:
        """
        Create a state snapshot for fast recovery.
        
        Snapshots capture the current execution state allowing
        faster recovery than replaying the entire ledger.
        
        Args:
            path: File path to save the snapshot.
            run_id: Optional run identifier.
        """
        self.kernel.create_snapshot(path, run_id)
    
    def load_snapshot(self, path: str) -> Dict[str, Any]:
        """
        Load a state snapshot.
        
        Args:
            path: Path to the snapshot file.
            
        Returns:
            Dict with snapshot state.
        """
        return dict(self.kernel.load_snapshot(path))
    
    def has_recovery(self, ledger_dir: str, run_id: str) -> bool:
        """
        Check if a run has a recovery point.
        
        Args:
            ledger_dir: Directory containing ledger files.
            run_id: Run identifier to check.
            
        Returns:
            True if there's a pending (incomplete) run to recover.
        """
        return self.kernel.has_recovery(ledger_dir, run_id)
    
    def get_recovery_point(self, ledger_dir: str, run_id: str) -> Optional[Dict[str, Any]]:
        """
        Get recovery point information.
        
        Args:
            ledger_dir: Directory containing ledger files.
            run_id: Run identifier.
            
        Returns:
            Dict with recovery info, or None if no recovery point.
        """
        result = self.kernel.get_recovery_point(ledger_dir, run_id)
        return dict(result) if result else None
    
    # ===== Phase 10: Shared Memory API =====

    def write_shared(self, data: bytes) -> int:
        """
        Write data to shared memory.
        
        Args:
            data: Bytes to store.
            
        Returns:
            Handle ID (int).
        """
        return self.kernel.write_shared(data)

    def read_shared(self, handle: int) -> bytes:
        """
        Read data from shared memory.
        
        Args:
            handle: Handle ID.
            
        Returns:
            Bytes data.
        """
        return bytes(self.kernel.read_shared(handle))

    def free_shared(self, handle: int) -> bool:
        """
        Free shared memory.
        
        Args:
            handle: Handle ID.
            
        Returns:
            True if freed successfully.
        """
        return self.kernel.free_shared(handle)

    def get_shared_stats(self) -> Dict[str, Any]:
        """
        Get statistics about shared memory usage.
        
        Returns:
            Dict with stats (active_handles, bytes_live, peak_bytes, etc.)
        """
        return dict(self.kernel.shared_memory_stats())

    @contextmanager
    def shared_buffer(self, data: bytes):
        """
        Context manager for temporary shared memory allocation.
        
        Guarantees that memory is freed when the context exits.
        
        Args:
            data: Bytes to write.
            
        Yields:
            Handle ID (int).
            
        Example:
            ```python
            with agent.shared_buffer(b"data") as h:
                process(h)
            ```
        """
        handle = self.write_shared(data)
        try:
            yield handle
        finally:
            self.free_shared(handle)

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
