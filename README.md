# KAIROS-ARK

<div align="center">

**A Deterministic Multi-Threaded Scheduler for Agentic AI Workflows**

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![Python](https://img.shields.io/badge/python-3.8+-blue.svg)](https://www.python.org/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

</div>

## Overview

KAIROS-ARK is a high-performance execution kernel designed for agentic AI workflows. It provides:

- **🔀 Conditional Branching**: Branch nodes that evaluate conditions and choose execution paths
- **⚡ Parallel Execution**: Fork/Join semantics for concurrent task processing
- **🔄 Deterministic Replay**: Logical clocks ensure bit-for-bit identical replayability
- **📝 System-Level Tracing**: Comprehensive audit ledger for debugging and analysis
- **🚀 High Throughput**: 10,000+ nodes/second with minimal overhead

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Python Layer                           │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                    Agent API                         │   │
│  │   add_node() | add_branch() | run_parallel()        │   │
│  └─────────────────────────────────────────────────────┘   │
│                           │                                 │
│  ┌─────────────────────────────────────────────────────┐   │
│  │               PyO3 Kernel Wrapper                    │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                            │
┌─────────────────────────────────────────────────────────────┐
│                       Rust Core                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  Scheduler   │  │ Logical Clock│  │ Audit Ledger │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│         │                                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Thread Pool  │  │ Task Queue   │  │ RNG Manager  │      │
│  │   (Rayon)    │  │ (Priority)   │  │ (ChaCha8)    │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

## Installation

### Prerequisites

- Rust 1.70+
- Python 3.8+
- [Maturin](https://github.com/PyO3/maturin) for building

### Build from Source

```bash
# Clone the repository
git clone https://github.com/YASSERRMD/KAIROS-ARK.git
cd KAIROS-ARK

# Build and install the Python package
pip install maturin
maturin develop

# Verify installation
python -c "from kairos_ark import Agent; print('✓ KAIROS-ARK installed successfully')"
```

## Quick Start

### Basic Task Execution

```python
from kairos_ark import Agent

# Create an agent with a fixed seed for reproducibility
agent = Agent(seed=42)

# Add task nodes
agent.add_node("fetch", lambda: "data from API")
agent.add_node("process", lambda: "processed result")

# Connect nodes
agent.connect("fetch", "process")

# Execute starting from "fetch"
agent.set_entry("fetch")
results = agent.execute()

# View the execution trace
agent.print_audit_log()
```

### Conditional Branching

```python
from kairos_ark import Agent

agent = Agent(seed=42)

# Define nodes
agent.add_node("success_path", lambda: "Success!")
agent.add_node("failure_path", lambda: "Failed!")

# Add branch that evaluates a condition
agent.add_branch(
    "check_condition",
    condition_func=lambda: True,  # Your condition here
    true_node="success_path",
    false_node="failure_path"
)

# Execute - will follow true_node since condition returns True
agent.execute("check_condition")
```

### Parallel Execution

```python
from kairos_ark import Agent
import time

agent = Agent()

# Add parallel tasks (each takes 100ms)
agent.add_node("task_a", lambda: (time.sleep(0.1), "A done")[1])
agent.add_node("task_b", lambda: (time.sleep(0.1), "B done")[1])
agent.add_node("task_c", lambda: (time.sleep(0.1), "C done")[1])

# Fork to run all tasks in parallel
agent.add_fork("parallel_start", ["task_a", "task_b", "task_c"])

# Join to wait for all tasks
agent.add_join("parallel_end", ["task_a", "task_b", "task_c"])

# Execute - all three 100ms tasks complete in ~100ms total
start = time.time()
agent.execute("parallel_start")
elapsed = time.time() - start

print(f"Completed in {elapsed:.2f}s (parallel speedup!)")
```

### Deterministic Replay

```python
from kairos_ark import Agent

# First execution
agent1 = Agent(seed=12345)
agent1.add_node("random_task", lambda: "result")
agent1.execute("random_task")
log1 = agent1.get_audit_log()

# Second execution with same seed
agent2 = Agent(seed=12345)
agent2.add_node("random_task", lambda: "result")
agent2.execute("random_task")
log2 = agent2.get_audit_log()

# Logs are identical!
assert log1 == log2
print("✓ Deterministic replay verified")
```

### Policy Engine (Phase 2)

KAIROS-ARK includes a powerful policy engine for capability-based access control:

```python
from kairos_ark import Agent, Policy, Cap

agent = Agent()

# Register tools with required capabilities
agent.register_tool("web_search", lambda: fetch_web(), [Cap.NET_ACCESS])
agent.register_tool("read_file", lambda: read_file(), [Cap.FILE_SYSTEM_READ])
agent.register_tool("llm_call", lambda: call_llm(), [Cap.LLM_CALL])

# Create a restrictive policy
policy = Policy(
    allowed_capabilities=[Cap.LLM_CALL, Cap.FILE_SYSTEM_READ],
    max_tool_calls={"web_search": 0},  # Block web searches entirely
    forbidden_content=["password", "api_key", "secret"]  # Redact sensitive data
)

# Run with policy enforcement
results = agent.run("entry_node", policy=policy)

# Check if a tool would be allowed
allowed, reason = agent.check_tool_capability("web_search")
if not allowed:
    print(f"Blocked: {reason}")

# Filter content through policy
filtered, patterns = agent.filter_content("The api_key is secret123")
print(filtered)  # "The [REDACTED] is [REDACTED]"
```

#### Capability Flags

| Capability | Description |
|------------|-------------|
| `Cap.NET_ACCESS` | Network/HTTP access |
| `Cap.FILE_SYSTEM_READ` | Read from filesystem |
| `Cap.FILE_SYSTEM_WRITE` | Write to filesystem |
| `Cap.SUBPROCESS_EXEC` | Execute subprocesses |
| `Cap.LLM_CALL` | Make LLM API calls |
| `Cap.MEMORY_ACCESS` | Access agent memory |
| `Cap.EXTERNAL_API` | Call external APIs |
| `Cap.CODE_EXEC` | Execute code |

#### Preset Policies

```python
Policy.permissive()   # Allows everything
Policy.restrictive()  # Blocks everything
Policy.no_network()   # Blocks NET_ACCESS and EXTERNAL_API
Policy.read_only()    # Only FILE_SYSTEM_READ, MEMORY_ACCESS, LLM_CALL
```

## API Reference

### Agent Class

| Method | Description |
|--------|-------------|
| `add_node(id, handler, timeout_ms, priority)` | Add a task node |
| `add_branch(id, condition_func, true_node, false_node)` | Add conditional branch |
| `add_fork(id, children)` | Add parallel fork |
| `add_join(id, parents, next_node)` | Add parallel join |
| `connect(from, to)` | Add edge between nodes |
| `execute(entry_node)` | Execute the graph |
| `run(entry_node, policy)` | Execute with policy |
| `register_tool(id, handler, capabilities)` | Register tool with capabilities |
| `set_policy(policy)` | Set execution policy |
| `check_tool_capability(tool_id)` | Check if tool is allowed |
| `filter_content(content)` | Filter forbidden content |
| `get_audit_log()` | Get execution trace |
| `print_audit_log()` | Pretty-print trace |

### Event Types

| Event | Description |
|-------|-------------|
| `Start` | Node execution began |
| `End` | Node execution completed |
| `BranchDecision` | Branch condition evaluated |
| `ForkSpawn` | Parallel children spawned |
| `JoinComplete` | All parents finished |
| `ToolOutput` | Handler produced output |
| `PolicyAllow` | Tool allowed by policy |
| `PolicyDeny` | Tool blocked by policy |
| `ContentRedacted` | Content was redacted |
| `CallLimitExceeded` | Tool call limit reached |
| `Error` | Execution error occurred |

## Benchmarks

| Metric | Result |
|--------|--------|
| Parallel Speedup | 2× 100ms tasks → ~100ms total |
| Node Throughput | **720,000+ nodes/second** |
| Policy Check | ~3μs per capability check |
| Event Logging | ~7μs per event |

---

## Phase 3: Persistence & Replay

KAIROS-ARK supports durable execution with disk-based event logging and state snapshots.

### Save and Load Ledger

```python
from kairos_ark import Agent

agent = Agent(seed=42)
agent.add_node("task", lambda: "result")
agent.execute("task")

# Save execution log
agent.save_ledger("/path/to/run.jsonl")

# Load later for analysis
events = agent.load_ledger("/path/to/run.jsonl")
```

### Replay Execution

```python
# Reconstruct state without re-executing handlers
state = agent.replay("/path/to/run.jsonl")
print(state["clock_value"])   # Final timestamp
print(state["node_outputs"])  # All outputs
```

### State Snapshots

```python
# Create checkpoint for fast recovery
agent.create_snapshot("/path/to/snapshot.json", "run_001")

# Load snapshot
snapshot = agent.load_snapshot("/path/to/snapshot.json")
```

---

## Phase 4: Zero-Copy IPC & Plugins

Extreme performance optimizations for large data and native plugins.

### Shared Memory

```python
# Write large data once
handle = agent.kernel.write_shared(list(data))

# Read by handle (avoids serialization)
result = bytes(agent.kernel.read_shared(handle))

# Pool statistics
stats = agent.kernel.shared_memory_stats()
# {'capacity': 67108864, 'used': 1000, 'allocations': 1}
```

### Native Plugins

```python
# Register plugin
agent.kernel.register_plugin("calculator", "1.0")

# Invoke plugin
result = agent.kernel.invoke_plugin("calculator", "2+2")

# List all plugins
plugins = agent.kernel.list_plugins()
```

---

## Phase 6: Governance & HITL

Human-in-the-Loop approval nodes and audit verification.

### Approval Gateway

```python
# Request approval for sensitive action
request_id = agent.kernel.request_approval("run_001", "delete_node", "Delete user data?")

# Check status (in another process/API)
status = agent.kernel.check_approval(request_id)  # "pending"

# Approve or reject
agent.kernel.approve(request_id, approver="admin")
agent.kernel.reject(request_id, "Not authorized", rejector="admin")

# List all pending approvals
pending = agent.kernel.list_pending_approvals()
```

### Audit Verification

```python
# Sign ledger for compliance
ledger_json = agent.get_audit_log_json()
signed = agent.kernel.sign_ledger(ledger_json, "run_001")

# Verify integrity
is_valid = agent.kernel.verify_ledger(signed)
```

---

## Documentation

- [Getting Started](docs/getting-started.md)
- [Core Concepts: Scheduler](docs/core-concepts/scheduler.md)
- [Core Concepts: Policy Engine](docs/core-concepts/policy-engine.md)
- [Advanced: Zero-Copy Memory](docs/advanced/zero-copy.md)
- [Advanced: Time-Travel Debugging](docs/advanced/time-travel.md)

---

## Testing

```bash
# Run comprehensive test suite (59+ tests)
pytest tests/test_comprehensive.py -v

# Run all tests
pytest tests/ -v
```

---

## License

MIT License - see [LICENSE](LICENSE) for details.

## Author

**YASSERRMD**
