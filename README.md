# KAIROS-ARK

<div align="center">

**The Operating System for Agentic AI**

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![Python](https://img.shields.io/badge/python-3.8+-blue.svg)](https://www.python.org/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

</div>

## Overview

**KAIROS-ARK** is a high-performance, deterministic execution kernel designed for mission-critical agentic AI workflows. Unlike traditional frameworks that prioritize prompt engineering, KAIROS-ARK prioritizes **system integrity**, **reproducibility**, and **industrial-grade governance**.

It provides a specialized "Operating System" for agents, handling:
- **Scheduling**: Deterministic, multi-threaded task execution.
- **Memory**: Zero-copy shared memory for large datasets.
- **Security**: Kernel-level policy enforcement and sandboxing.
- **Time**: Logical clocks for bit-for-bit identical replay debugging.
- **Governance**: Human-in-the-Loop (HITL) approvals and cryptographic audit logs.

---

## Key Features

| Feature | Description |
|---------|-------------|
| **⚡ High Throughput** | Process **720,000+ nodes/second** with Rust-native execution. |
| **🔒 Policy Engine** | Restrict agent capabilities (Network, FS, Exec) at the kernel level. |
| **⏱️ Time-Travel** | Replay any execution from a ledger with 100% determinism. |
| **🚀 Zero-Copy** | Pass GB-sized payloads between tasks in microseconds. |
| **🤝 Interoperability** | Native adapters for LangGraph, CrewAI, and MCP tools. |
| **🛡️ Governance** | Cryptographically signed audit logs and enforced HITL protocols. |

---

## Installation

```bash
pip install kairos-ark
```

Or build from source for maximum performance:

```bash
git clone https://github.com/YASSERRMD/KAIROS-ARK.git
cd KAIROS-ARK
pip install maturin
maturin develop
```

---

## Quick Start

### 1. Hello World Agent

```python
from kairos_ark import Agent

# Create a deterministic agent
agent = Agent(seed=42)

# Add tasks (nodes)
agent.add_node("fetch", lambda: {"data": "raw data"})
agent.add_node("process", lambda: {"status": "processed"})

# Connect workflow
agent.connect("fetch", "process")

# Execute
results = agent.execute("fetch")
print(f"Executed {len(results)} nodes")
```

### 2. Parallel Execution

KAIROS-ARK uses a Rayon-backed thread pool for true parallelism:

```python
# Fork execution into parallel branches
agent.add_fork("start_parallel", ["scrape_web", "query_db", "check_cache"])

# Join results
agent.add_join("sync_results", ["scrape_web", "query_db", "check_cache"])

agent.execute("start_parallel")
```

---

## Core Capabilities

### 🛡️ Security & Policy Engine

Prevent "excessive agency" by sandboxing tools at the kernel level.

```python
from kairos_ark import Agent, Policy, Cap

# Define a restrictive policy
policy = Policy(
    allowed_capabilities=[Cap.LLM_CALL],       # Only allow LLM calls
    max_tool_calls={"web_search": 5},          # Rate limit specific tools
    forbidden_content=["password", "api_key"]  # Automatic redaction
)

# Run agent with policy
agent.run("start", policy=policy)
```

### 💾 Persistence & Time-Travel Debugging

Debug "Heisenbugs" by replaying execution logs exactly as they happened.

```python
# 1. Save execution ledger
agent.save_ledger("run_001.jsonl")

# 2. Replay later (reconstructs state without re-running side effects)
state = agent.replay("run_001.jsonl")
print(f"Final State: {state['node_outputs']}")

# 3. Create Snapshots for fast recovery
agent.create_snapshot("checkpoint.json", "run_001")
```

### 🚀 Zero-Copy Shared Memory

Pass large objects (images, embeddings, codebases) between Python/Rust without serialization overhead.

```python
# Write 1GB data once (~5µs latency)
handle = agent.kernel.write_shared(large_data_list)

# Pass unique handle to other nodes
result = agent.kernel.read_shared(handle)
```

### 🤝 Interoperability & MCP

KAIROS-ARK acts as a native backend for other frameworks.

```python
# LangGraph-compatible State Store (~4µs access)
agent.kernel.state_set("messages", json.dumps(history))
msgs = agent.kernel.state_get("messages")

# Model Context Protocol (MCP) Support
agent.kernel.mcp_register_tool("search", "Search tool")
result = agent.kernel.mcp_call_tool("search", '{"query": "KAIROS"}')
```

### ⚖️ Governance & HITL

Industrial-grade compliance features built-in.

```python
# 1. Human-in-the-Loop (HITL) Interrupts
req_id = agent.kernel.request_approval("run_1", "deploy", "Deploy to prod?")
# Execution suspends until approved
agent.kernel.approve(req_id, "admin_user")

# 2. Cryptographic Verification
ledger = agent.get_audit_log_json()
signed = agent.kernel.sign_ledger(ledger, "run_1")
is_valid = agent.kernel.verify_ledger(signed)
```

---

## Documentation

- **[Getting Started Guide](docs/getting-started.md)**: Build your first agent in 5 minutes.
- **[The Scheduler](docs/core-concepts/scheduler.md)**: Deep dive into logical clocks and determinism.
- **[Policy Engine](docs/core-concepts/policy-engine.md)**: Configuring capabilities and sandboxes.
- **[Zero-Copy Memory](docs/advanced/zero-copy.md)**: Optimizing for large-scale data.
- **[Time-Travel Debugging](docs/advanced/time-travel.md)**: Mastering the Replay Engine.

---

## Benchmarks

KAIROS-ARK is built for speed.

| Metric | Performance |
|--------|-------------|
| **Node Throughput** | **720,000+ nodes/sec** |
| **Task Dispatch Latency** | ~1.4 µs |
| **Policy Check Overhead** | ~3.0 µs |
| **State Store Access** | ~4.0 µs |
| **Event Logging** | ~7.0 µs |

---

## License

MIT License - see [LICENSE](LICENSE) for details.

## Author

**YASSERRMD**
