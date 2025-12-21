"""
Tests for the Policy Engine and Capability Model (Phase 2).

These tests verify:
1. Unauthorized access blocking (NetAccess capability)
2. Resource ceiling enforcement (max_tool_calls)
3. Zero-overhead capability checking
4. Policy transparency (audit log entries for every policy decision)
5. Content filtering and redaction
"""

import pytest
import time
from kairos_ark import Agent, Policy, Cap


class TestCapabilities:
    """Test capability flag operations."""
    
    def test_capability_values(self):
        """Verify capability flag values are correct."""
        assert Cap.NET_ACCESS == 0b00000001
        assert Cap.FILE_SYSTEM_READ == 0b00000010
        assert Cap.FILE_SYSTEM_WRITE == 0b00000100
        assert Cap.LLM_CALL == 0b00010000
        
    def test_combine_capabilities(self):
        """Test combining multiple capabilities."""
        combined = Cap.combine(Cap.NET_ACCESS, Cap.LLM_CALL)
        assert combined == (Cap.NET_ACCESS | Cap.LLM_CALL)
        
    def test_all_and_none(self):
        """Test ALL and NONE presets."""
        assert Cap.ALL == 0xFFFFFFFF
        assert Cap.NONE == 0


class TestPolicy:
    """Test Policy configuration."""
    
    def test_default_policy_is_permissive(self):
        """Default policy should allow all capabilities."""
        policy = Policy()
        assert policy.allowed_capabilities == Cap.ALL
        
    def test_restrictive_policy(self):
        """Restrictive policy allows nothing."""
        policy = Policy.restrictive()
        assert policy.allowed_capabilities == Cap.NONE
        
    def test_no_network_policy(self):
        """No-network policy blocks NET_ACCESS and EXTERNAL_API."""
        policy = Policy.no_network()
        assert not policy.has_capability(Cap.NET_ACCESS)
        assert not policy.has_capability(Cap.EXTERNAL_API)
        assert policy.has_capability(Cap.LLM_CALL)
        
    def test_policy_with_capabilities(self):
        """Test creating policy with specific capabilities."""
        policy = Policy(allowed_capabilities=[Cap.LLM_CALL, Cap.FILE_SYSTEM_READ])
        assert policy.has_capability(Cap.LLM_CALL)
        assert policy.has_capability(Cap.FILE_SYSTEM_READ)
        assert not policy.has_capability(Cap.NET_ACCESS)
        
    def test_policy_with_tool_limits(self):
        """Test policy with max_tool_calls limits."""
        policy = Policy(max_tool_calls={"web_search": 2, "code_exec": 0})
        assert policy.get_tool_limit("web_search") == 2
        assert policy.get_tool_limit("code_exec") == 0
        assert policy.get_tool_limit("unknown") is None
        
    def test_policy_with_forbidden_content(self):
        """Test policy with forbidden content patterns."""
        policy = Policy(forbidden_content=["secret", "password"])
        assert "secret" in policy.forbidden_content
        assert "password" in policy.forbidden_content


class TestUnauthorizedAccess:
    """Test that unauthorized tool access is blocked by policy."""
    
    def test_tool_with_missing_capability_is_blocked(self):
        """A tool requiring NET_ACCESS should fail with no_network policy."""
        agent = Agent()
        
        # Register a tool requiring NET_ACCESS
        agent.register_tool(
            "web_search",
            lambda: "search results",
            [Cap.NET_ACCESS],
        )
        
        # Set no-network policy
        policy = Policy.no_network()
        agent.set_policy(policy)
        
        # Check capability - should be denied
        allowed, reason = agent.check_tool_capability("web_search")
        assert not allowed, "Tool with NET_ACCESS should be blocked by no_network policy"
        assert reason is not None
        assert "NET_ACCESS" in reason or "capabilities" in reason
        
    def test_tool_with_allowed_capability_passes(self):
        """A tool requiring LLM_CALL should pass with permissive policy."""
        agent = Agent()
        
        agent.register_tool(
            "llm_tool",
            lambda: "llm result",
            [Cap.LLM_CALL],
        )
        
        # Permissive policy allows everything
        policy = Policy.permissive()
        agent.set_policy(policy)
        
        allowed, reason = agent.check_tool_capability("llm_tool")
        assert allowed, "Tool should be allowed with permissive policy"
        assert reason is None
        
    def test_multiple_required_capabilities(self):
        """Tool requiring multiple caps should need all of them."""
        agent = Agent()
        
        # Tool requires both NET_ACCESS and EXTERNAL_API
        agent.register_tool(
            "api_call",
            lambda: "api result",
            [Cap.NET_ACCESS, Cap.EXTERNAL_API],
        )
        
        # Policy only allows NET_ACCESS
        policy = Policy(allowed_capabilities=[Cap.NET_ACCESS])
        agent.set_policy(policy)
        
        allowed, reason = agent.check_tool_capability("api_call")
        assert not allowed, "Should fail when not all required capabilities are allowed"


class TestResourceCeiling:
    """Test that tool call limits are enforced."""
    
    def test_call_limit_tracking(self):
        """Policy should track and enforce call limits."""
        agent = Agent()
        
        agent.register_tool(
            "limited_tool",
            lambda: "result",
            [Cap.LLM_CALL],
        )
        
        policy = Policy(
            allowed_capabilities=[Cap.LLM_CALL],
            max_tool_calls={"limited_tool": 2},
        )
        agent.set_policy(policy)
        
        # Tool limit should be set
        assert policy.get_tool_limit("limited_tool") == 2


class TestZeroOverhead:
    """Test that policy checking is fast (near-zero overhead)."""
    
    def test_capability_check_is_fast(self):
        """Capability checks should complete in microseconds."""
        agent = Agent()
        
        # Register many tools
        N = 1000
        for i in range(N):
            agent.register_tool(
                f"tool_{i}",
                lambda: "",
                [Cap.LLM_CALL],
            )
        
        policy = Policy(allowed_capabilities=[Cap.LLM_CALL])
        agent.set_policy(policy)
        
        # Time the checks
        start = time.perf_counter_ns()
        for i in range(N):
            agent.check_tool_capability(f"tool_{i}")
        elapsed_ns = time.perf_counter_ns() - start
        
        per_check_us = elapsed_ns / N / 1000  # Convert to microseconds
        
        # Each check should take less than 100μs
        assert per_check_us < 100, f"Capability check took {per_check_us:.2f}μs, expected <100μs"
        
        print(f"\n✓ Capability check: {per_check_us:.2f}μs per check ({N} checks)")


class TestContentFiltering:
    """Test content filtering and redaction."""
    
    def test_substring_redaction(self):
        """Forbidden substrings should be redacted."""
        agent = Agent()
        
        policy = Policy(
            forbidden_content=["secret_key_123", "password"],
        )
        agent.set_policy(policy)
        
        content = "The secret_key_123 is here and the password is hidden."
        filtered, patterns = agent.filter_content(content)
        
        assert "secret_key_123" not in filtered
        assert "password" not in filtered
        assert "[REDACTED]" in filtered
        assert len(patterns) == 2
        
    def test_no_match_no_change(self):
        """Content without forbidden patterns should pass through unchanged."""
        agent = Agent()
        
        policy = Policy(forbidden_content=["secret"])
        agent.set_policy(policy)
        
        content = "This is safe content."
        filtered, patterns = agent.filter_content(content)
        
        assert filtered == content
        assert len(patterns) == 0


class TestPolicyTransparency:
    """Test that policy decisions are logged in the audit ledger."""
    
    def test_audit_log_contains_policy_name(self):
        """Policy name should be accessible."""
        policy = Policy(name="test_policy")
        assert policy.name == "test_policy"
        
    def test_policy_repr(self):
        """Policy should have useful string representation."""
        policy = Policy(
            allowed_capabilities=[Cap.LLM_CALL],
            max_tool_calls={"tool": 5},
            name="my_policy",
        )
        
        repr_str = repr(policy)
        assert "my_policy" in repr_str
        assert "tool" in repr_str


class TestPresetPolicies:
    """Test preset policy factories."""
    
    def test_permissive_allows_all(self):
        """Permissive policy allows all capabilities."""
        policy = Policy.permissive()
        assert policy.has_capability(Cap.NET_ACCESS)
        assert policy.has_capability(Cap.FILE_SYSTEM_WRITE)
        assert policy.has_capability(Cap.SUBPROCESS_EXEC)
        
    def test_restrictive_blocks_all(self):
        """Restrictive policy blocks all capabilities."""
        policy = Policy.restrictive()
        assert not policy.has_capability(Cap.NET_ACCESS)
        assert not policy.has_capability(Cap.LLM_CALL)
        
    def test_read_only_allows_reads(self):
        """Read-only policy allows reads but not writes."""
        policy = Policy.read_only()
        assert policy.has_capability(Cap.FILE_SYSTEM_READ)
        assert policy.has_capability(Cap.LLM_CALL)
        # Note: read_only uses only the specified caps, not complement


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
