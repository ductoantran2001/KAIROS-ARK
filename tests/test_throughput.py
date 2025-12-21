"""
Tests for throughput benchmark.
"""

import time
import pytest


def test_throughput_10k_nodes():
    """
    Test that the kernel can dispatch at least 10,000 nodes per second.
    """
    from kairos_ark import Agent
    
    agent = Agent()
    
    # Create 10,000 lightweight nodes
    for i in range(10000):
        agent.add_node(f"n{i}", lambda: None)
    
    # Dispatch nodes and measure time
    start = time.time()
    for i in range(10000):
        agent.kernel.dispatch_node(f"n{i}")
    elapsed = time.time() - start
    
    rate = 10000 / elapsed
    
    assert rate >= 10000, f"Throughput {rate:.0f} nodes/s, need ≥10,000"
    print(f"✓ Throughput: {rate:,.0f} nodes/second")


def test_throughput_with_logging():
    """
    Test that logging overhead doesn't significantly impact throughput.
    """
    from kairos_ark import Agent
    
    agent = Agent()
    
    # Create nodes
    for i in range(5000):
        agent.add_node(f"n{i}", lambda: "output")
    
    start = time.time()
    for i in range(5000):
        agent.kernel.dispatch_node(f"n{i}")
    elapsed = time.time() - start
    
    rate = 5000 / elapsed
    event_count = agent.event_count()
    
    print(f"✓ Throughput with logging: {rate:,.0f} nodes/second")
    print(f"  Events logged: {event_count}")
    
    assert rate >= 5000, f"Throughput {rate:.0f} nodes/s with logging, need ≥5,000"


def test_audit_log_overhead():
    """
    Measure the overhead of audit logging per event.
    """
    from kairos_ark import Agent
    
    agent = Agent()
    
    # Warm up
    agent.add_node("warmup", lambda: "x")
    agent.kernel.dispatch_node("warmup")
    
    # Clear and measure
    agent.clear()
    
    # Create and dispatch many nodes
    n_nodes = 10000
    for i in range(n_nodes):
        agent.add_node(f"n{i}", lambda: "x")
    
    start = time.time()
    for i in range(n_nodes):
        agent.kernel.dispatch_node(f"n{i}")
    elapsed = time.time() - start
    
    events = agent.event_count()
    overhead_per_event = (elapsed / events) * 1_000_000  # microseconds
    
    print(f"✓ Logging overhead: {overhead_per_event:.2f} μs/event")
    
    # Should be well under 1ms per event
    assert overhead_per_event < 1000, f"Overhead too high: {overhead_per_event:.2f} μs/event"


if __name__ == "__main__":
    test_throughput_10k_nodes()
    test_throughput_with_logging()
    test_audit_log_overhead()
    print("\n✓ All throughput tests passed!")
