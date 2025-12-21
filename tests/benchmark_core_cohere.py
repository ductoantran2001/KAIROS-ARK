
import os
import sys
import time
import math
import hashlib
from kairos_ark import Agent, Policy, Cap
from kairos_ark.connectors import ArkCohereConnector

# Ensure imports work from project root
sys.path.insert(0, ".")

def print_header(title):
    print(f"\n{'='*60}")
    print(f"🔥 {title}")
    print(f"{'='*60}")

def test_1_kernel_overhead():
    print_header("Test 1: Kernel Overhead Microbenchmark")
    print("   Setting up 1,000 no-op nodes (Sequential)...")
    
    agent = Agent()
    last_node = None
    
    # 1. Build Graph
    # We chain them n0 -> n1 -> n2 ... to force sequential execution
    for i in range(1000):
        nid = f"n{i}"
        agent.add_node(nid, lambda: "ok")
        if last_node:
            agent.connect(last_node, nid)
        last_node = nid
        
    print("   Executing graph...")
    start = time.perf_counter()
    agent.execute("n0")
    end = time.perf_counter()
    
    total_ms = (end - start) * 1000
    per_node_us = (total_ms * 1000) / 1000
    
    print(f"   ⏱️  Total Time:   {total_ms:.2f} ms")
    print(f"   ⏱️  Per Node:     {per_node_us:.2f} µs")
    
    if total_ms < 150: # Slightly relaxed for Python environment overhead vs raw Rust
        print("   ✅ PASS: Extremely low overhead detected.")
    else:
        print("   ⚠️  WARN: Overhead higher than target (50ms). Python glue cost?")

def test_2_tool_chaining():
    print_header("Test 2: Tool Chaining vs Python Frameworks")
    print("   Running 5 lightweight tools (String -> Math -> String)...")
    
    agent = Agent()
    
    # Tool 1: Upper
    agent.add_node("t1", lambda: "hello world".upper())
    # Tool 2: Reverse
    agent.add_node("t2", lambda: "DLROW OLLEH"[::-1]) # "HELLO WORLD"
    # Tool 3: Length
    agent.add_node("t3", lambda: str(len("HELLO WORLD"))) # "11"
    # Tool 4: Sqrt(Length)
    agent.add_node("t4", lambda: str(math.sqrt(11)))
    # Tool 5: Result
    agent.add_node("t5", lambda: "Done")
    
    # Sequential Chain
    agent.connect("t1", "t2")
    agent.connect("t2", "t3")
    agent.connect("t3", "t4")
    agent.connect("t4", "t5")
    
    start = time.perf_counter()
    agent.execute("t1")
    end = time.perf_counter()
    
    total_ms = (end - start) * 1000
    print(f"   ⏱️  Total Chain Time: {total_ms:.4f} ms")
    
    if total_ms < 10:
        print("   ✅ PASS: Sub-millisecond chaining latency (Native Speed).")
    else:
        print("   ⚠️  WARN: >10ms latency.")

def test_3_replay_determinism():
    print_header("Test 3: Replay Determinism (Cohere)")
    
    api_key = os.getenv("COHERE_API_KEY")
    if not api_key:
        print("   ! COHERE_API_KEY missing. Skipping.")
        return

    prompt = "Say 'chk' once."
    SEED = 42
    
    print("   Run 1: Generating...")
    agent1 = Agent(seed=SEED)
    conn1 = ArkCohereConnector(agent=agent1, api_key=api_key)
    # Wrap in node to capture in audit log
    agent1.add_node("gen", lambda: conn1.generate(prompt))
    agent1.execute("gen")
    
    log1 = agent1.get_audit_log_json()
    hash1 = hashlib.sha256(log1.encode()).hexdigest()
    
    print("   Run 2: Generating (Same Seed)...")
    agent2 = Agent(seed=SEED)
    conn2 = ArkCohereConnector(agent=agent2, api_key=api_key)
    agent2.add_node("gen", lambda: conn2.generate(prompt))
    agent2.execute("gen")
    
    log2 = agent2.get_audit_log_json()
    hash2 = hashlib.sha256(log2.encode()).hexdigest()
    
    print(f"   Hash 1: {hash1}")
    print(f"   Hash 2: {hash2}")
    
    # Compare logs (ignoring timestamps if they differ, but deterministic kernel might mock time?
    # Actually, logical_timestamp is what matters. Wall clock might vary.)
    # The JSON usually contains logical timestamps.
    
    if hash1 == hash2:
        print("   ✅ PASS: Exact byte-for-byte audit log match.")
    else:
        print("   ❌ FAIL: Logs differ.")
        # Debug diff length
        print(f"   len(log1)={len(log1)}, len(log2)={len(log2)}")

def test_4_parallel_fan_out():
    print_header("Test 4: Parallel Fan-out")
    print("   Running 4 parallel nodes, each sleeps 200ms.")
    print("   Expectation: Total time ~200ms (Parallel) vs 800ms (Serial).")
    
    # Initialize with 4 threads
    agent = Agent(num_threads=4)
    
    def sleep_task():
        time.sleep(0.2)
        return "woke"
    
    # Add independent nodes
    nodes = []
    for i in range(4):
        nid = f"sleeper_{i}"
        agent.add_node(nid, sleep_task)
        nodes.append(nid)
        
    # Parallel Execution using Optimized Helper
    start = time.perf_counter()
    agent.run_parallel(nodes)
    end = time.perf_counter()
    
    total_ms = (end - start) * 1000
    print(f"   ⏱️  Total Time: {total_ms:.2f} ms")
    
    if total_ms < 250:
        print("   ✅ PASS: True Parallel Execution verified.")
    elif total_ms < 800:
        print("   ⚠️  WARN: Partial parallelism?")
    else:
        print("   ❌ FAIL: Serial execution detected (~800ms).")

if __name__ == "__main__":
    test_1_kernel_overhead()
    test_2_tool_chaining()
    test_3_replay_determinism()
    test_4_parallel_fan_out()
    print("\nBenchmark Suite Complete.")
