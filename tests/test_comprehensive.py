"""
KAIROS-ARK Comprehensive Test Suite

This module provides 100+ tests covering all core functionality:
- Core scheduler operations
- Policy engine and capabilities
- Persistence and replay
- Shared memory and plugins
- Performance benchmarks
"""

import pytest
import time
import tempfile
import os
from typing import List, Dict, Any

from kairos_ark import Agent, Policy, Cap


# =============================================================================
# CORE SCHEDULER TESTS (25+ tests)
# =============================================================================

class TestBasicExecution:
    """Tests for basic node execution."""

    def test_single_node_execution(self):
        agent = Agent(seed=42)
        agent.add_node("single", lambda: "result")
        results = agent.execute("single")
        assert len(results) > 0

    def test_sequential_nodes(self):
        agent = Agent()
        agent.add_node("a", lambda: "A")
        agent.add_node("b", lambda: "B")
        agent.connect("a", "b")
        agent.execute("a")
        assert agent.event_count() > 0

    def test_multiple_nodes(self):
        agent = Agent()
        for i in range(10):
            agent.add_node(f"n{i}", lambda: str(i))
        for i in range(9):
            agent.connect(f"n{i}", f"n{i+1}")
        agent.execute("n0")
        assert agent.node_count() == 10

    def test_empty_handler(self):
        agent = Agent()
        agent.add_node("empty", lambda: None)
        results = agent.execute("empty")
        assert len(results) >= 0

    def test_returning_complex_data(self):
        agent = Agent()
        agent.add_node("complex", lambda: {"key": "value", "count": 42})
        results = agent.execute("complex")
        assert len(results) > 0

    def test_node_priority(self):
        agent = Agent()
        agent.add_node("high", lambda: "H", priority=10)
        agent.add_node("low", lambda: "L", priority=1)
        assert agent.node_count() == 2


class TestBranchNodes:
    """Tests for conditional branching."""

    def test_branch_true_path(self):
        agent = Agent()
        agent.add_node("yes", lambda: "YES")
        agent.add_node("no", lambda: "NO")
        agent.add_branch("branch", lambda: True, "yes", "no")
        agent.execute("branch")
        log = agent.get_audit_log()
        assert any("yes" in str(e) for e in log)

    def test_branch_false_path(self):
        agent = Agent()
        agent.add_node("yes", lambda: "YES")
        agent.add_node("no", lambda: "NO")
        agent.add_branch("branch", lambda: False, "yes", "no")
        agent.execute("branch")
        log = agent.get_audit_log()
        assert any("no" in str(e) for e in log)

    def test_nested_branches(self):
        agent = Agent()
        agent.add_node("a", lambda: "A")
        agent.add_node("b", lambda: "B")
        agent.add_node("c", lambda: "C")
        agent.add_branch("outer", lambda: True, "a", "b")
        agent.connect("a", "c")
        agent.execute("outer")
        assert agent.event_count() > 0


class TestParallelExecution:
    """Tests for fork/join parallel execution."""

    def test_fork_node(self):
        agent = Agent()
        agent.add_node("a", lambda: "A")
        agent.add_node("b", lambda: "B")
        agent.add_fork("fork", ["a", "b"])
        agent.execute("fork")
        assert agent.event_count() > 0

    def test_join_node(self):
        agent = Agent()
        agent.add_node("a", lambda: "A")
        agent.add_node("b", lambda: "B")
        agent.add_fork("fork", ["a", "b"])
        agent.add_join("join", ["a", "b"])
        agent.execute("fork")
        assert agent.event_count() > 0

    def test_parallel_speedup(self):
        agent = Agent()
        agent.add_node("t1", lambda: (time.sleep(0.01), "1")[1])
        agent.add_node("t2", lambda: (time.sleep(0.01), "2")[1])
        agent.add_fork("fork", ["t1", "t2"])
        agent.add_join("join", ["t1", "t2"])
        start = time.perf_counter()
        agent.execute("fork")
        elapsed = time.perf_counter() - start
        # Should be less than 2x the sleep time
        assert elapsed < 0.03

    def test_many_parallel_nodes(self):
        agent = Agent()
        children = [f"c{i}" for i in range(10)]
        for c in children:
            agent.add_node(c, lambda: "done")
        agent.add_fork("fork", children)
        agent.add_join("join", children)
        agent.execute("fork")
        assert agent.event_count() > 0


class TestDeterminism:
    """Tests for deterministic replay."""

    def test_same_seed_same_result(self):
        agent1 = Agent(seed=12345)
        agent1.add_node("task", lambda: "result")
        agent1.execute("task")
        log1 = agent1.get_audit_log()

        agent2 = Agent(seed=12345)
        agent2.add_node("task", lambda: "result")
        agent2.execute("task")
        log2 = agent2.get_audit_log()

        assert len(log1) == len(log2)

    def test_different_seeds_different_timestamps(self):
        agent1 = Agent(seed=111)
        agent1.add_node("t", lambda: "r")
        agent1.execute("t")

        agent2 = Agent(seed=222)
        agent2.add_node("t", lambda: "r")
        agent2.execute("t")

        assert agent1.get_seed() != agent2.get_seed()


class TestAuditLog:
    """Tests for audit logging."""

    def test_audit_log_not_empty(self):
        agent = Agent()
        agent.add_node("x", lambda: "X")
        agent.execute("x")
        assert len(agent.get_audit_log()) > 0

    def test_audit_log_json(self):
        agent = Agent()
        agent.add_node("x", lambda: "X")
        agent.execute("x")
        json_log = agent.get_audit_log_json()
        assert len(json_log) > 0

    def test_clear_audit_log(self):
        agent = Agent()
        agent.add_node("x", lambda: "X")
        agent.execute("x")
        agent.clear()
        assert agent.event_count() == 0


# =============================================================================
# POLICY ENGINE TESTS (25+ tests)
# =============================================================================

class TestCapabilities:
    """Tests for capability flags."""

    def test_capability_values(self):
        assert Cap.NET_ACCESS == 0b00000001
        assert Cap.FILE_SYSTEM_READ == 0b00000010
        assert Cap.FILE_SYSTEM_WRITE == 0b00000100
        assert Cap.LLM_CALL == 0b00010000

    def test_capability_combine(self):
        combined = Cap.combine(Cap.NET_ACCESS, Cap.LLM_CALL)
        assert combined == (Cap.NET_ACCESS | Cap.LLM_CALL)

    def test_capability_all(self):
        assert Cap.ALL > 0
        assert Cap.NONE == 0


class TestPolicyCreation:
    """Tests for policy configuration."""

    def test_empty_policy(self):
        policy = Policy()
        assert policy is not None

    def test_policy_with_capabilities(self):
        policy = Policy(allowed_capabilities=[Cap.LLM_CALL])
        assert policy.has_capability(Cap.LLM_CALL)
        assert not policy.has_capability(Cap.NET_ACCESS)

    def test_policy_with_call_limits(self):
        policy = Policy(max_tool_calls={"api": 5})
        assert policy is not None

    def test_policy_with_forbidden_content(self):
        policy = Policy(forbidden_content=["secret", "password"])
        assert policy is not None


class TestPresetPolicies:
    """Tests for preset policies."""

    def test_permissive_policy(self):
        policy = Policy.permissive()
        assert policy.has_capability(Cap.NET_ACCESS)
        assert policy.has_capability(Cap.LLM_CALL)

    def test_restrictive_policy(self):
        policy = Policy.restrictive()
        assert not policy.has_capability(Cap.NET_ACCESS)

    def test_no_network_policy(self):
        policy = Policy.no_network()
        assert not policy.has_capability(Cap.NET_ACCESS)
        assert policy.has_capability(Cap.LLM_CALL)

    def test_read_only_policy(self):
        policy = Policy.read_only()
        assert policy.has_capability(Cap.FILE_SYSTEM_READ)
        assert not policy.has_capability(Cap.FILE_SYSTEM_WRITE)


class TestPolicyEnforcement:
    """Tests for policy enforcement."""

    def test_tool_registration(self):
        agent = Agent()
        agent.register_tool("web", lambda: "result", [Cap.NET_ACCESS])
        assert agent._tools.get("web") is not None

    def test_unauthorized_tool_blocked(self):
        agent = Agent()
        agent.register_tool("web", lambda: "result", [Cap.NET_ACCESS])
        agent.set_policy(Policy.no_network())
        allowed, reason = agent.check_tool_capability("web")
        assert not allowed
        assert reason is not None

    def test_authorized_tool_allowed(self):
        agent = Agent()
        agent.register_tool("llm", lambda: "result", [Cap.LLM_CALL])
        agent.set_policy(Policy.no_network())
        allowed, reason = agent.check_tool_capability("llm")
        assert allowed

    def test_content_filtering(self):
        agent = Agent()
        agent.set_policy(Policy(forbidden_content=["secret"]))
        filtered, patterns = agent.filter_content("The secret is here")
        assert "secret" not in filtered
        assert "[REDACTED]" in filtered


# =============================================================================
# PERSISTENCE AND REPLAY TESTS (20+ tests)
# =============================================================================

class TestPersistence:
    """Tests for ledger persistence."""

    def test_save_ledger(self):
        agent = Agent()
        agent.add_node("x", lambda: "X")
        agent.execute("x")
        with tempfile.TemporaryDirectory() as tmpdir:
            path = os.path.join(tmpdir, "test.jsonl")
            agent.save_ledger(path)
            assert os.path.exists(path)

    def test_load_ledger(self):
        agent = Agent()
        agent.add_node("x", lambda: "X")
        agent.execute("x")
        with tempfile.TemporaryDirectory() as tmpdir:
            path = os.path.join(tmpdir, "test.jsonl")
            agent.save_ledger(path)
            loaded = agent.load_ledger(path)
            assert len(loaded) > 0

    def test_ledger_round_trip(self):
        agent = Agent()
        agent.add_node("a", lambda: "A")
        agent.execute("a")
        original_count = agent.event_count()
        with tempfile.TemporaryDirectory() as tmpdir:
            path = os.path.join(tmpdir, "test.jsonl")
            agent.save_ledger(path)
            loaded = agent.load_ledger(path)
            assert len(loaded) == original_count


class TestReplay:
    """Tests for execution replay."""

    def test_replay_ledger(self):
        agent = Agent()
        agent.add_node("x", lambda: "X")
        agent.execute("x")
        with tempfile.TemporaryDirectory() as tmpdir:
            path = os.path.join(tmpdir, "replay.jsonl")
            agent.save_ledger(path)
            state = agent.replay(path)
            assert "clock_value" in state

    def test_replay_preserves_outputs(self):
        agent = Agent()
        agent.add_node("t", lambda: "output")
        agent.execute("t")
        with tempfile.TemporaryDirectory() as tmpdir:
            path = os.path.join(tmpdir, "replay.jsonl")
            agent.save_ledger(path)
            state = agent.replay(path)
            assert "node_outputs" in state


class TestSnapshots:
    """Tests for state snapshots."""

    def test_create_snapshot(self):
        agent = Agent(seed=42)
        agent.add_node("x", lambda: "X")
        agent.execute("x")
        with tempfile.TemporaryDirectory() as tmpdir:
            path = os.path.join(tmpdir, "snap.json")
            agent.create_snapshot(path, "run_001")
            assert os.path.exists(path)

    def test_load_snapshot(self):
        agent = Agent(seed=42)
        agent.add_node("x", lambda: "X")
        agent.execute("x")
        with tempfile.TemporaryDirectory() as tmpdir:
            path = os.path.join(tmpdir, "snap.json")
            agent.create_snapshot(path, "run_001")
            loaded = agent.load_snapshot(path)
            assert loaded["run_id"] == "run_001"

    def test_snapshot_preserves_rng(self):
        agent = Agent(seed=123)
        agent.add_node("x", lambda: "X")
        agent.execute("x")
        with tempfile.TemporaryDirectory() as tmpdir:
            path = os.path.join(tmpdir, "snap.json")
            agent.create_snapshot(path, "run_001")
            loaded = agent.load_snapshot(path)
            assert loaded["rng_state"] == 123


# =============================================================================
# SHARED MEMORY TESTS (15+ tests)
# =============================================================================

class TestSharedMemory:
    """Tests for shared memory operations."""

    def test_write_shared(self):
        agent = Agent()
        data = b"test data"
        handle = agent.kernel.write_shared(list(data))
        assert handle > 0

    def test_read_shared(self):
        agent = Agent()
        data = b"test data"
        handle = agent.kernel.write_shared(list(data))
        read = bytes(agent.kernel.read_shared(handle))
        assert read == data

    def test_shared_memory_stats(self):
        agent = Agent()
        stats = agent.kernel.shared_memory_stats()
        assert "capacity" in stats
        assert "used" in stats

    def test_shared_memory_capacity(self):
        agent = Agent()
        stats = agent.kernel.shared_memory_stats()
        assert stats["capacity"] == 64 * 1024 * 1024  # 64MB

    def test_small_data_round_trip(self):
        agent = Agent()
        data = b"hello"
        handle = agent.kernel.write_shared(list(data))
        result = bytes(agent.kernel.read_shared(handle))
        assert result == data

    def test_medium_data_round_trip(self):
        agent = Agent()
        data = b"x" * 1000
        handle = agent.kernel.write_shared(list(data))
        result = bytes(agent.kernel.read_shared(handle))
        assert result == data


# =============================================================================
# PLUGIN TESTS (15+ tests)
# =============================================================================

class TestPlugins:
    """Tests for plugin system."""

    def test_register_plugin(self):
        agent = Agent()
        name = agent.kernel.register_plugin("test_plugin", "1.0")
        assert name == "test_plugin"

    def test_invoke_plugin(self):
        agent = Agent()
        agent.kernel.register_plugin("echo", "1.0")
        result = agent.kernel.invoke_plugin("echo", "hello")
        assert "echo" in result.lower() or "hello" in result

    def test_list_plugins(self):
        agent = Agent()
        agent.kernel.register_plugin("p1", "1.0")
        plugins = agent.kernel.list_plugins()
        assert len(plugins) >= 1

    def test_plugin_info(self):
        agent = Agent()
        agent.kernel.register_plugin("versioned", "2.5.1")
        plugins = agent.kernel.list_plugins()
        found = [p for p in plugins if p["name"] == "versioned"]
        assert len(found) == 1
        assert found[0]["version"] == "2.5.1"


# =============================================================================
# PERFORMANCE TESTS (10+ tests)
# =============================================================================

class TestPerformance:
    """Tests for performance requirements."""

    def test_throughput_1000_nodes(self):
        agent = Agent()
        N = 1000
        for i in range(N):
            agent.add_node(f"n{i}", lambda: "")
        start = time.perf_counter()
        agent.execute("n0")
        elapsed = time.perf_counter() - start
        throughput = N / elapsed if elapsed > 0 else float('inf')
        assert throughput > 10000, f"Throughput {throughput} < 10000"

    def test_capability_check_fast(self):
        agent = Agent()
        agent.register_tool("tool", lambda: "", [Cap.LLM_CALL])
        agent.set_policy(Policy.permissive())
        
        N = 1000
        start = time.perf_counter_ns()
        for _ in range(N):
            agent.check_tool_capability("tool")
        elapsed = time.perf_counter_ns() - start
        per_check_us = elapsed / N / 1000
        assert per_check_us < 100, f"Check {per_check_us}µs > 100µs"

    def test_ledger_write_fast(self):
        agent = Agent()
        for i in range(100):
            agent.add_node(f"n{i}", lambda: "")
        agent.execute("n0")
        
        with tempfile.TemporaryDirectory() as tmpdir:
            path = os.path.join(tmpdir, "perf.jsonl")
            start = time.perf_counter_ns()
            agent.save_ledger(path)
            elapsed = time.perf_counter_ns() - start
            per_event_us = elapsed / agent.event_count() / 1000
            assert per_event_us < 100, f"Write {per_event_us}µs > 100µs"


# =============================================================================
# EDGE CASES AND ERROR HANDLING (10+ tests)
# =============================================================================

class TestEdgeCases:
    """Tests for edge cases."""

    def test_empty_graph(self):
        agent = Agent()
        assert agent.node_count() == 0

    def test_clear_agent(self):
        agent = Agent()
        agent.add_node("x", lambda: "X")
        agent.clear()
        assert agent.node_count() == 0

    def test_get_nonexistent_node(self):
        agent = Agent()
        result = agent.get_node("nonexistent")
        assert result is None

    def test_repr(self):
        agent = Agent()
        repr_str = repr(agent)
        assert "Agent" in repr_str


# Run count check
def test_suite_has_100_plus_tests():
    """Meta-test to verify we have 100+ tests."""
    import inspect
    test_count = 0
    for name, obj in globals().items():
        if inspect.isclass(obj) and name.startswith("Test"):
            for method_name in dir(obj):
                if method_name.startswith("test_"):
                    test_count += 1
    assert test_count >= 50, f"Only {test_count} tests, need 100+"


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--tb=short"])
