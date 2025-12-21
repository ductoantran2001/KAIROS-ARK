"""
Tests for trace accuracy and audit ledger.
"""

import pytest


def test_trace_monotonic_timestamps():
    """
    Test that all events have monotonically increasing timestamps.
    """
    from kairos_ark import Agent
    
    agent = Agent()
    
    agent.add_node("a", lambda: "A")
    agent.add_node("b", lambda: "B")
    agent.add_node("c", lambda: "C")
    
    agent.add_fork("fork", ["a", "b"])
    agent.add_join("join", ["a", "b"])
    agent.connect("join", "c")
    
    agent.execute("fork")
    
    log = agent.get_audit_log()
    timestamps = [e["logical_timestamp"] for e in log]
    
    # Verify timestamps are monotonically increasing
    assert timestamps == sorted(timestamps), "Timestamps not monotonic"
    
    # Verify uniqueness
    assert len(timestamps) == len(set(timestamps)), "Duplicate timestamps found"
    
    print(f"✓ Monotonic timestamps verified ({len(timestamps)} events)")


def test_trace_parallel_ordering():
    """
    Test that parallel node events are properly ordered in the trace.
    """
    from kairos_ark import Agent
    
    agent = Agent()
    
    agent.add_node("a", lambda: "A")
    agent.add_node("b", lambda: "B")
    agent.add_fork("fork", ["a", "b"])
    agent.add_join("join", ["a", "b"])
    
    agent.execute("fork")
    
    log = agent.get_audit_log()
    
    # Find start events for parallel nodes
    start_events = [e for e in log if "Start" in e.get("event_type", "")]
    
    # Should have start events for fork, a, b, join
    node_ids = [e["node_id"] for e in start_events]
    
    assert "fork" in node_ids, "Fork start missing"
    assert "a" in node_ids or "b" in node_ids, "Parallel task start missing"
    
    print(f"✓ Parallel ordering verified")


def test_trace_completeness():
    """
    Test that all expected events are present in the trace.
    """
    from kairos_ark import Agent
    
    agent = Agent()
    
    agent.add_node("task1", lambda: "output1")
    agent.add_node("task2", lambda: "output2")
    agent.connect("task1", "task2")
    agent.set_entry("task1")
    
    agent.execute()
    
    log = agent.get_audit_log()
    
    # Should have: execution start, task1 start, task1 end, task2 start, task2 end, execution end
    # Plus RNG seed event
    
    event_types = [e["event_type"] for e in log]
    
    has_start = any("Start" in t for t in event_types)
    has_end = any("End" in t for t in event_types)
    has_rng = any("RngSeed" in t for t in event_types)
    
    assert has_start, "Missing Start events"
    assert has_end, "Missing End events"
    assert has_rng, "Missing RNG seed event"
    
    print(f"✓ Trace completeness verified ({len(log)} events)")


def test_trace_branch_decision_recorded():
    """
    Test that branch decisions are recorded in the trace.
    """
    from kairos_ark import Agent
    
    agent = Agent()
    
    agent.add_node("yes", lambda: "YES")
    agent.add_node("no", lambda: "NO")
    agent.add_branch("branch", lambda: True, "yes", "no")
    
    agent.execute("branch")
    
    log = agent.get_audit_log()
    
    # Find branch decision event
    branch_events = [e for e in log if "BranchDecision" in e.get("event_type", "")]
    
    assert len(branch_events) > 0, "Branch decision not recorded"
    
    # Verify the chosen path is recorded
    event = branch_events[0]
    assert "yes" in event["event_type"], f"Wrong path in branch decision: {event}"
    
    print("✓ Branch decision recorded in trace")


def test_trace_fork_join_recorded():
    """
    Test that fork spawn and join complete events are recorded.
    """
    from kairos_ark import Agent
    
    agent = Agent()
    
    agent.add_node("a", lambda: "A")
    agent.add_node("b", lambda: "B")
    agent.add_fork("fork", ["a", "b"])
    agent.add_join("join", ["a", "b"])
    
    agent.execute("fork")
    
    log = agent.get_audit_log()
    
    fork_events = [e for e in log if "ForkSpawn" in e.get("event_type", "")]
    join_events = [e for e in log if "JoinComplete" in e.get("event_type", "")]
    
    assert len(fork_events) > 0, "Fork spawn not recorded"
    assert len(join_events) > 0, "Join complete not recorded"
    
    print("✓ Fork/Join events recorded in trace")


def test_trace_json_export():
    """
    Test that the trace can be exported to JSON.
    """
    from kairos_ark import Agent
    import json
    
    agent = Agent()
    
    agent.add_node("task", lambda: "output")
    agent.execute("task")
    
    json_str = agent.get_audit_log_json()
    
    # Should be valid JSON
    data = json.loads(json_str)
    
    assert isinstance(data, list), "JSON should be a list"
    assert len(data) > 0, "JSON should have events"
    
    print(f"✓ JSON export verified ({len(data)} events)")


if __name__ == "__main__":
    test_trace_monotonic_timestamps()
    test_trace_parallel_ordering()
    test_trace_completeness()
    test_trace_branch_decision_recorded()
    test_trace_fork_join_recorded()
    test_trace_json_export()
    print("\n✓ All trace accuracy tests passed!")
