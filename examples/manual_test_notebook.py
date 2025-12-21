
print("## 1. Installation (Skipped)")

print("## 2. Hello World Agent")
from kairos_ark import Agent
import json

# 1. Initialize Agent with a fixed seed for determinism
agent = Agent(seed=42)

# 2. Define simple tools (nodes)
# Fixed: removed 'input' parameter from lambdas as they are entry/source nodes in this context or parameterless
agent.add_node("fetch_data", lambda: {"data": "raw_input_from_sensor"})
agent.add_node("process_data", lambda: {"status": "processed", "value": "RAW_INPUT_FROM_SENSOR"})

# 3. Connect the workflow graph
agent.connect("fetch_data", "process_data")

# 4. Execute starting from 'fetch_data'
print("🚀 Executing Workflow...")
results = agent.execute("fetch_data")

print("\n✅ Execution Complete!")
print("Results:", json.dumps(results, indent=2))

print("## 3. True Parallel Execution")
import time

# Define a blocking task that sleeps
def heavy_task(name):
    print(f"[{name}] Starting...")
    time.sleep(1.0) # Sleep for 1 second
    print(f"[{name}] Done!")
    return f"{name}_result"

# Add parallel nodes
agent.add_node("task_a", lambda: heavy_task("A"))
agent.add_node("task_b", lambda: heavy_task("B"))
agent.add_node("task_c", lambda: heavy_task("C"))

# Create a Fork-Join structure
# Note: For Python-bound tasks (like time.sleep), we use the helper run_parallel
# to bypass the GIL via ThreadPoolExecutor found in agent.py
print("\n⚡ Starting Parallel Execution (Expect ~1.0s total time, NOT 3.0s)...")
start_time = time.time()

# Use the threaded helper
results = agent.run_parallel(["task_a", "task_b", "task_c"])

end_time = time.time()
print(f"\n⏱️ Total Time: {end_time - start_time:.4f} seconds")

print("## 4. Kernel-Level Security Policy")
from kairos_ark import Policy, Cap

# 1. Register a tool that *requires* Network Access
agent.register_tool(
    "sensitive_web_tool",
    lambda: "This should be blocked",
    required_capabilities=[Cap.NET_ACCESS]  # Requires Network
)

# 2. Define a Policy that *blocks* Network Access
policy = Policy(
    allowed_capabilities=[Cap.LLM_CALL],  # Only allow LLM, NO Network
    forbidden_content=["API_KEY"]
)

# 3. Enforce the Policy
agent.set_policy(policy)
print("\n🛡️  Policy enforced: Network Access is BLOCKED")

# 4. Check if the tool is allowed
allowed, reason = agent.check_tool_capability("sensitive_web_tool")

if not allowed:
    print(f"✅ Security System Working: {reason}")
else:
    print("❌ Security Check Failed: Tool was allowed (Unexpected)")

print("## 5. Performance Benchmarks")

print("\n### 5.1 High-Throughput Test")
# Benchmarking the raw kernel overhead
# We'll creating 1000 nodes and execute them
agent.clear() # Reset agent
agent = Agent(seed=123)

count = 1000
print(f"Creating {count} nodes...")
start_setup = time.time()
for i in range(count):
    agent.add_node(f"node_{i}", lambda: "ok")
    if i > 0:
        agent.connect(f"node_{i-1}", f"node_{i}")
setup_time = time.time() - start_setup
print(f"Setup Time: {setup_time:.4f}s")

print(f"Executing {count} nodes chain...")
start_exec = time.time()
agent.execute("node_0")
exec_time = time.time() - start_exec

print(f"Execution Time: {exec_time:.4f}s")
print(f"Throughput: {count / exec_time:.2f} nodes/sec")


print("\n### 5.2 Zero-Copy Shared Memory (Advanced)")
import os

# 1. Stats Check
print("Initial Stats:", agent.get_shared_stats())

# 2. Basic Allocation
data_size = 50 * 1024 * 1024 # 50 MB
print(f"Allocating {data_size / 1024 / 1024} MB of data...")
large_data = b"x" * data_size

start_write = time.time()
handle_id = agent.write_shared(large_data)
write_time = time.time() - start_write
print(f"Write Time: {write_time:.6f}s (Handle ID: {handle_id})")

# 3. Read Verification
start_read = time.time()
read_data = agent.read_shared(handle_id)
read_time = time.time() - start_read
print(f"Read Time:  {read_time:.6f}s")
assert len(read_data) == data_size
assert read_data == large_data
print("✅ Data Integrity Verified")

# 4. Stats Check
stats = agent.get_shared_stats()
print("Stats after alloc:", stats)
assert stats["active_handles"] == 1
assert stats["bytes_live"] == data_size

# 5. Manual Free
print("Freeing memory...")
agent.free_shared(handle_id)
stats = agent.get_shared_stats()
print("Stats after free:", stats)
assert stats["active_handles"] == 0
assert stats["bytes_live"] == 0

# 6. Safety Checks (Double Free / Stale Handle)
print("Testing Safety (Double Free)...")
try:
    agent.read_shared(handle_id)
    print("❌ ERROR: Read on freed handle should have failed!")
except Exception as e:
    print(f"✅ Correctly caught stale read: {e}")

try:
    agent.free_shared(handle_id)
    print("❌ ERROR: Double free should have failed!")
except Exception as e:
    print(f"✅ Correctly caught double free: {e}")

# 7. Context Manager
print("Testing Context Manager...")
with agent.shared_buffer(b"temporary_data") as h:
    print(f"Inside context: {agent.get_shared_stats()}")
    data = agent.read_shared(h)
    assert data == b"temporary_data"

print(f"Outside context: {agent.get_shared_stats()}")
assert agent.get_shared_stats()["active_handles"] == 0
print("✅ Context Manager Verified")

# 8. Reset
print("Clearing agent...")
agent.clear()

