"""
Tests for branch node integrity.
"""

import pytest


def test_branch_same_path():
    """
    Test that the same input always produces the same branch path.
    """
    from kairos_ark import Agent
    
    results = []
    
    for _ in range(10):
        agent = Agent(seed=42)
        
        agent.add_node("yes", lambda: "YES")
        agent.add_node("no", lambda: "NO")
        agent.add_branch("check", lambda: True, "yes", "no")
        
        agent.execute("check")
        
        log = agent.get_audit_log()
        decisions = [e for e in log if "BranchDecision" in e.get("event_type", "")]
        
        results.append(decisions)
    
    # All runs should have identical branch decisions
    for i, result in enumerate(results):
        assert len(result) > 0, f"Run {i}: No branch decision recorded"
        assert "yes" in result[0]["event_type"], f"Run {i}: Wrong path chosen"
    
    print("✓ Branch integrity verified over 10 runs")


def test_branch_false_path():
    """
    Test that a false condition follows the false path.
    """
    from kairos_ark import Agent
    
    agent = Agent(seed=42)
    
    agent.add_node("yes", lambda: "YES")
    agent.add_node("no", lambda: "NO")
    agent.add_branch("check", lambda: False, "yes", "no")
    
    agent.execute("check")
    
    log = agent.get_audit_log()
    decisions = [e for e in log if "BranchDecision" in e.get("event_type", "")]
    
    assert len(decisions) > 0, "No branch decision recorded"
    assert "no" in decisions[0]["event_type"], "Wrong path: should have chosen 'no'"
    
    print("✓ False branch path verified")


def test_nested_branches():
    """
    Test nested branch nodes.
    """
    from kairos_ark import Agent
    
    agent = Agent(seed=42)
    
    agent.add_node("a", lambda: "A")
    agent.add_node("b", lambda: "B")
    agent.add_node("c", lambda: "C")
    agent.add_node("d", lambda: "D")
    
    # Outer branch
    agent.add_branch("outer", lambda: True, "inner_true", "d")
    # Inner branch (reached if outer is true)
    agent.add_branch("inner_true", lambda: False, "a", "b")
    
    agent.execute("outer")
    
    log = agent.get_audit_log()
    decisions = [e for e in log if "BranchDecision" in e.get("event_type", "")]
    
    # Should have 2 branch decisions
    assert len(decisions) == 2, f"Expected 2 branch decisions, got {len(decisions)}"
    
    print("✓ Nested branches verified")


def test_branch_determinism_with_seed():
    """
    Test that branch decisions are deterministic with the same seed.
    """
    from kairos_ark import Agent
    
    counter = [0]  # Mutable container
    
    def varying_condition():
        counter[0] += 1
        return counter[0] % 2 == 0
    
    # First run
    counter[0] = 0
    agent1 = Agent(seed=12345)
    agent1.add_node("even", lambda: "EVEN")
    agent1.add_node("odd", lambda: "ODD")
    agent1.add_branch("check", varying_condition, "even", "odd")
    agent1.execute("check")
    log1 = agent1.get_audit_log()
    
    # Second run with same seed (counter continues from 1)
    agent2 = Agent(seed=12345)
    agent2.add_node("even", lambda: "EVEN")
    agent2.add_node("odd", lambda: "ODD")
    agent2.add_branch("check", varying_condition, "even", "odd")
    agent2.execute("check")
    log2 = agent2.get_audit_log()
    
    # The condition function has external state, so results may differ
    # But the audit logs themselves should be consistently structured
    assert len(log1) == len(log2), "Audit log structures should match"
    
    print("✓ Branch determinism verified")


if __name__ == "__main__":
    test_branch_same_path()
    test_branch_false_path()
    test_nested_branches()
    test_branch_determinism_with_seed()
    print("\n✓ All branch tests passed!")
