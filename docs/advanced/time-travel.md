# Time-Travel Debugging

KAIROS-ARK's Replay Engine enables debugging "Heisenbugs" by reconstructing execution state.

## The Problem

Non-deterministic bugs are hard to reproduce:
- Different LLM responses
- Timing-dependent behavior
- Race conditions

## The Solution

1. **Save the ledger** after execution
2. **Replay** without re-invoking tools
3. **Step through** to find the bug

## Quick Start

### 1. Save Execution

```python
agent.execute("start")
agent.save_ledger("/path/to/run.jsonl")
```

### 2. Replay Later

```python
state = agent.replay("/path/to/run.jsonl")
print(state["clock_value"])    # Final timestamp
print(state["node_outputs"])   # All outputs
print(state["last_node"])      # Where it stopped
```

### 3. Create Checkpoints

```python
agent.create_snapshot("/path/to/snap.json", "run_001")

# Later: fast-forward to checkpoint
loaded = agent.load_snapshot("/path/to/snap.json")
```

## Ledger Format (JSONL)

Each line is a JSON event:

```json
{"logical_timestamp":1,"node_id":"fetch","event_type":"Start","run_id":"run_001"}
{"logical_timestamp":2,"node_id":"fetch","event_type":"ToolOutput","output":"data"}
{"logical_timestamp":3,"node_id":"fetch","event_type":"End"}
```

## Nondeterministic Capture

The ledger captures:
- `wall_clock_ms` - Real timestamp
- `rng_state` - Random seed
- `outputs` - Tool results

This allows perfect replay even for randomized logic.

## Performance

| Operation | Latency |
|-----------|---------|
| Event logging | ~7µs |
| Ledger save | <50µs/event |
| Replay | 100K events/sec |
