# The KAIROS-ARK Scheduler

## Logical Clocks vs System Clocks

KAIROS-ARK uses **Logical Clocks** instead of system clocks for event ordering. This ensures:

1. **Deterministic Replay** - Same seed → same execution order
2. **No Wall-Clock Dependencies** - Replay doesn't need "fake time"
3. **Bit-for-Bit Identical** - Debugging is reproducible

## How It Works

```
┌─────────────────────────────────────────┐
│         Logical Clock (Lamport)         │
├─────────────────────────────────────────┤
│  Event 1: Start(node_a)     → clock: 1  │
│  Event 2: End(node_a)       → clock: 2  │
│  Event 3: Start(node_b)     → clock: 3  │
│  Event 4: ForkSpawn(c,d)    → clock: 4  │
│  Event 5: Start(node_c)     → clock: 5  │
│  Event 6: Start(node_d)     → clock: 5  │  ← Same clock for parallel
│  Event 7: End(node_c)       → clock: 6  │
└─────────────────────────────────────────┘
```

## Deterministic Seeds

```python
# Same seed = same execution order
agent1 = Agent(seed=12345)
agent2 = Agent(seed=12345)

# Both will produce identical audit logs
```

## Priority-Based Scheduling

Nodes can have priorities (higher = earlier):

```python
agent.add_node("high_priority", handler, priority=100)
agent.add_node("low_priority", handler, priority=1)
```

## Fork/Join Parallelism

```python
# Fork: spawn parallel children
agent.add_fork("split", ["task_a", "task_b"])

# Join: wait for all parents
agent.add_join("merge", ["task_a", "task_b"])
```

The scheduler uses Rayon for true multi-threaded parallel execution.

## Performance

| Metric | Value |
|--------|-------|
| Node dispatch | ~1.4µs |
| Throughput | 720,000+ nodes/sec |
