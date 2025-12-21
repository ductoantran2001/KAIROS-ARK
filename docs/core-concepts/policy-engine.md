# The Policy Engine

KAIROS-ARK's Policy Engine provides kernel-enforced sandboxing for agent tool calls.

## Capabilities

Every tool requires specific capabilities:

| Capability | Flag | Description |
|------------|------|-------------|
| `Cap.NET_ACCESS` | 0x01 | Network/HTTP access |
| `Cap.FILE_SYSTEM_READ` | 0x02 | Read files |
| `Cap.FILE_SYSTEM_WRITE` | 0x04 | Write files |
| `Cap.SUBPROCESS_EXEC` | 0x08 | Run subprocesses |
| `Cap.LLM_CALL` | 0x10 | Call LLM APIs |
| `Cap.MEMORY_ACCESS` | 0x20 | Access agent memory |
| `Cap.EXTERNAL_API` | 0x40 | External API calls |
| `Cap.CODE_EXEC` | 0x80 | Execute code |

## Creating Policies

```python
from kairos_ark import Policy, Cap

# Allow only specific capabilities
policy = Policy(allowed_capabilities=[Cap.LLM_CALL, Cap.MEMORY_ACCESS])

# Set tool call limits
policy = Policy(max_tool_calls={"web_search": 10})

# Content filtering
policy = Policy(forbidden_content=["password", "secret"])
```

## Preset Policies

```python
# Allow everything (development)
Policy.permissive()

# Block everything (maximum safety)
Policy.restrictive()

# No network access
Policy.no_network()

# Read-only filesystem
Policy.read_only()
```

## Registering Tools

```python
agent.register_tool(
    "web_search",
    search_function,
    [Cap.NET_ACCESS]  # Required capabilities
)

agent.set_policy(Policy.no_network())

# This will be blocked:
allowed, reason = agent.check_tool_capability("web_search")
# allowed = False, reason = "Missing NET_ACCESS"
```

## Content Filtering

```python
agent.set_policy(Policy(forbidden_content=["api_key"]))
filtered, patterns = agent.filter_content("My api_key is xyz")
# filtered = "My [REDACTED] is xyz"
```

## Policy Checks

- **~3µs per check** - Minimal overhead
- **Zero-copy** - No string allocations
- **Audit logging** - All decisions logged
