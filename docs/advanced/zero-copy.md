# Zero-Copy Advanced Shared Memory

KAIROS-ARK's Shared Memory Subsystem is a high-performance, safe arena for passing large data between Python and Rust without serialization overhead.

## Architecture

The system uses a **Generational Bench Allocator** (Slab-based) to provide O(1) allocation and deallocation while guaranteeing memory safety.

### 1. Generational Handles
Handles are not raw pointers. They are 64-bit composite IDs:
- **High 32 bits**: Index in the slab.
- **Low 32 bits**: Generation counter.

**Safety**: When a slot is freed and reused, its generation increments. Accessing an old handle will fail deterministically with a `Stale Handle` error, preventing Use-After-Free vulnerabilities.

### 2. Strict Budgeting
To ensure stability in production, the kernel enforces two levels of limits:

| Limit Type | Threshold | Consequence |
|------------|-----------|-------------|
| **Hard Limit (Global)** | 1 GB | Allocation Fails (`Global budget exceeded`) |
| **Hard Limit (Single)** | 100 MB | Allocation Fails (`Allocation too large`) |
| **Soft Limit** | 850 MB (85%) | Increments `soft_limit_hits` counter |

### 3. Debugging & Observability
The kernel provides deep introspection tools for debugging memory leaks.

```python
# 1. Get Detailed Stats
stats = agent.get_shared_stats()
# {
#   'active_handles': 5,
#   'bytes_live': 1048576,
#   'peak_bytes': 2097152,
#   'alloc_count': 10,
#   'free_count': 5,
#   'errors': 0,
#   'soft_limit_hits': 0,  # >85% usage
#   'hard_limit_hits': 0   # Blocked allocs
# }

# 2. List Live Handles (Debug)
live = agent.list_live_shared()
# [(handle_id, size_bytes), ...]
```

## Usage

### Basic Allocation
```python
# Write data (returns integer handle)
handle = agent.write_shared(b"large_payload")

# Read data
data = agent.read_shared(handle)

# Manual Free (Important!)
agent.free_shared(handle)
```

### Context Manager (Recommended)
Use the context manager to ensure handles are automatically freed, preventing leaks even if errors occur.

```python
with agent.shared_buffer(b"temporary_image_data") as h:
    # 'h' is the handle ID
    process_data(h)
    
# Memory is automatically freed here
```

### Resetting Memory
For repeated test runs or isolating executions:

```python
agent.reset_execution_memory() 
# Instantly clears the entire arena (O(1)) and invalidates all handles.
```
