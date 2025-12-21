# Zero-Copy Memory

KAIROS-ARK's Shared Memory Store eliminates serialization overhead for large data.

## The Problem

Passing large payloads (images, codebases) between Python and Rust involves:

1. Serialize Python → bytes
2. Copy bytes to Rust
3. Parse in Rust
4. **Result: ~14ms for 10MB**

## The Solution

```python
# Write data once to shared memory
handle = agent.kernel.write_shared(list(data))

# Pass handle (8 bytes) instead of data (10MB)
result = agent.kernel.read_shared(handle)
```

## Performance

| Operation | Latency |
|-----------|---------|
| write_shared (10KB) | ~5µs |
| read_shared (10KB) | ~4µs |
| Pool capacity | 64MB |

## Memory Pool Stats

```python
stats = agent.kernel.shared_memory_stats()
# {
#   'capacity': 67108864,  # 64MB
#   'used': 10240,
#   'available': 67098624,
#   'allocations': 1
# }
```

## Reference Counting

Handles are reference-counted for safe multi-use:

```python
handle = agent.kernel.write_shared(data)
# ref_count = 1

# Pass to multiple nodes (each adds ref)
# Data freed when all refs released
```

## Best Practices

1. **Use for large data** (>1KB)
2. **Free handles** when done
3. **Monitor pool usage** via stats
4. **Pre-allocate** for known workloads
