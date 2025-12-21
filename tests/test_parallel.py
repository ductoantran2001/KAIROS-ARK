"""
Tests for parallel execution speedup.
"""

import time
import pytest


def test_parallel_speedup():
    """
    Test that parallel execution of two 100ms tasks completes in ~100ms, not 200ms.
    """
    from kairos_ark import Agent
    
    agent = Agent()
    
    # Add two tasks that each take 100ms
    agent.add_node("task_a", lambda: (time.sleep(0.1), "A")[1], priority=1)
    agent.add_node("task_b", lambda: (time.sleep(0.1), "B")[1], priority=1)
    
    # Create fork/join pattern
    agent.add_fork("fork", ["task_a", "task_b"])
    agent.add_join("join", ["task_a", "task_b"])
    agent.connect("fork", "join")
    
    # Execute and measure time
    start = time.time()
    agent.execute("fork")
    elapsed = time.time() - start
    
    # Should complete in ~100ms (with some tolerance), not 200ms
    assert elapsed < 0.15, f"Parallel execution took {elapsed:.3f}s, expected ~0.1s"
    print(f"✓ Parallel speedup verified: {elapsed:.3f}s")


def test_parallel_with_many_tasks():
    """
    Test parallel execution with more tasks than threads.
    """
    from kairos_ark import Agent
    
    agent = Agent(num_threads=4)
    
    # Add 8 tasks that each take 50ms
    children = []
    for i in range(8):
        node_id = f"task_{i}"
        agent.add_node(node_id, lambda: (time.sleep(0.05), "done")[1])
        children.append(node_id)
    
    agent.add_fork("fork", children)
    agent.add_join("join", children)
    
    start = time.time()
    agent.execute("fork")
    elapsed = time.time() - start
    
    # With 4 threads and 8 × 50ms tasks, should be ~100ms (2 rounds)
    assert elapsed < 0.2, f"Expected ~0.1s, got {elapsed:.3f}s"
    print(f"✓ Many-task parallel: {elapsed:.3f}s")


if __name__ == "__main__":
    test_parallel_speedup()
    test_parallel_with_many_tasks()
    print("\n✓ All parallel tests passed!")
