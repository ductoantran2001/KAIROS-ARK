# Getting Started with KAIROS-ARK

Welcome to KAIROS-ARK, the high-performance execution kernel for agentic AI workflows.

## Installation

```bash
pip install kairos-ark
```

Or build from source:

```bash
git clone https://github.com/YASSERRMD/KAIROS-ARK.git
cd KAIROS-ARK
pip install maturin
maturin develop
```

## Your First Agent (5 minutes)

### 1. Create a Simple Agent

```python
from kairos_ark import Agent

# Create an agent with a deterministic seed
agent = Agent(seed=42)

# Add nodes (tasks)
agent.add_node("fetch_data", lambda: {"status": "fetched"})
agent.add_node("process", lambda: {"status": "processed"})
agent.add_node("save", lambda: {"status": "saved"})

# Connect nodes
agent.connect("fetch_data", "process")
agent.connect("process", "save")

# Execute
results = agent.execute("fetch_data")
print(f"Executed {len(results)} nodes")
```

### 2. Add Conditional Branching

```python
agent.add_branch(
    "check_quality",
    lambda: True,  # Condition
    "publish",     # If true
    "retry"        # If false
)
```

### 3. Add Parallel Execution

```python
# Fork to run tasks in parallel
agent.add_fork("parallel_start", ["task_a", "task_b", "task_c"])

# Join to wait for all
agent.add_join("parallel_end", ["task_a", "task_b", "task_c"])
```

### 4. View the Audit Log

```python
# See what happened
agent.print_audit_log()

# Get as JSON
log_json = agent.get_audit_log_json()
```

## Next Steps

- [Core Concepts: The Scheduler](core-concepts/scheduler.md)
- [Core Concepts: Policy Engine](core-concepts/policy-engine.md)
- [Advanced: Zero-Copy Memory](advanced/zero-copy.md)
- [Advanced: Time-Travel Debugging](advanced/time-travel.md)
