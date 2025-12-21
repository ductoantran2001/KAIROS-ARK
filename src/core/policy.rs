//! Policy and Capability definitions for KAIROS-ARK.
//! 
//! Provides fine-grained capability model for permission management:
//! - Capability flags for tool requirements
//! - Tool metadata with required capabilities
//! - Agent policy configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Capability flags for fine-grained permission control.
/// 
/// Each capability represents a specific permission that can be
/// granted or denied to an agent. Tools declare which capabilities
/// they require, and the policy engine enforces these requirements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Capability(u32);

impl Capability {
    pub const NET_ACCESS: Self = Self(0b00000001);
    pub const FILE_SYSTEM_READ: Self = Self(0b00000010);
    pub const FILE_SYSTEM_WRITE: Self = Self(0b00000100);
    pub const SUBPROCESS_EXEC: Self = Self(0b00001000);
    pub const LLM_CALL: Self = Self(0b00010000);
    pub const MEMORY_ACCESS: Self = Self(0b00100000);
    pub const SENSITIVE_DATA: Self = Self(0b01000000);
    pub const EXTERNAL_API: Self = Self(0b10000000);
    pub const CODE_EXEC: Self = Self(0b100000000);
    pub const DATABASE_ACCESS: Self = Self(0b1000000000);

    /// Create an empty capability set.
    pub fn empty() -> Self {
        Self(0)
    }

    /// Create a capability set with all flags.
    pub fn all() -> Self {
        Self(u32::MAX)
    }

    /// Create from raw bits.
    pub fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Get raw bits.
    pub fn bits(&self) -> u32 {
        self.0
    }

    /// Check if contains all flags of other.
    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Check if any flags are set.
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

// Implement bitwise operations
impl std::ops::BitOr for Capability {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self { Self(self.0 | rhs.0) }
}

impl std::ops::BitOrAssign for Capability {
    fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
}

impl std::ops::BitAnd for Capability {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self { Self(self.0 & rhs.0) }
}

impl std::ops::Sub for Capability {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self { Self(self.0 & !rhs.0) }
}

impl std::ops::Not for Capability {
    type Output = Self;
    fn not(self) -> Self { Self(!self.0) }
}

// Serde support
impl Serialize for Capability {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Capability {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        u32::deserialize(deserializer).map(Self)
    }
}

impl Capability {
    /// Get capability from string name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_uppercase().as_str() {
            "NET_ACCESS" | "NETACCESS" => Some(Self::NET_ACCESS),
            "FILE_SYSTEM_READ" | "FILESYSTEMREAD" | "DISK_READ" => Some(Self::FILE_SYSTEM_READ),
            "FILE_SYSTEM_WRITE" | "FILESYSTEMWRITE" | "DISK_WRITE" => Some(Self::FILE_SYSTEM_WRITE),
            "SUBPROCESS_EXEC" | "SUBPROCESSEXEC" => Some(Self::SUBPROCESS_EXEC),
            "LLM_CALL" | "LLMCALL" => Some(Self::LLM_CALL),
            "MEMORY_ACCESS" | "MEMORYACCESS" => Some(Self::MEMORY_ACCESS),
            "SENSITIVE_DATA" | "SENSITIVEDATA" => Some(Self::SENSITIVE_DATA),
            "EXTERNAL_API" | "EXTERNALAPI" => Some(Self::EXTERNAL_API),
            "CODE_EXEC" | "CODEEXEC" => Some(Self::CODE_EXEC),
            "DATABASE_ACCESS" | "DATABASEACCESS" => Some(Self::DATABASE_ACCESS),
            _ => None,
        }
    }

    /// Get the name of this capability.
    pub fn name(&self) -> &'static str {
        match *self {
            Self::NET_ACCESS => "NET_ACCESS",
            Self::FILE_SYSTEM_READ => "FILE_SYSTEM_READ",
            Self::FILE_SYSTEM_WRITE => "FILE_SYSTEM_WRITE",
            Self::SUBPROCESS_EXEC => "SUBPROCESS_EXEC",
            Self::LLM_CALL => "LLM_CALL",
            Self::MEMORY_ACCESS => "MEMORY_ACCESS",
            Self::SENSITIVE_DATA => "SENSITIVE_DATA",
            Self::EXTERNAL_API => "EXTERNAL_API",
            Self::CODE_EXEC => "CODE_EXEC",
            Self::DATABASE_ACCESS => "DATABASE_ACCESS",
            _ => "UNKNOWN",
        }
    }

    /// Get names of all capabilities in this set.
    pub fn names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.contains(Self::NET_ACCESS) { names.push("NET_ACCESS"); }
        if self.contains(Self::FILE_SYSTEM_READ) { names.push("FILE_SYSTEM_READ"); }
        if self.contains(Self::FILE_SYSTEM_WRITE) { names.push("FILE_SYSTEM_WRITE"); }
        if self.contains(Self::SUBPROCESS_EXEC) { names.push("SUBPROCESS_EXEC"); }
        if self.contains(Self::LLM_CALL) { names.push("LLM_CALL"); }
        if self.contains(Self::MEMORY_ACCESS) { names.push("MEMORY_ACCESS"); }
        if self.contains(Self::SENSITIVE_DATA) { names.push("SENSITIVE_DATA"); }
        if self.contains(Self::EXTERNAL_API) { names.push("EXTERNAL_API"); }
        if self.contains(Self::CODE_EXEC) { names.push("CODE_EXEC"); }
        if self.contains(Self::DATABASE_ACCESS) { names.push("DATABASE_ACCESS"); }
        names
    }

    /// Combine multiple capabilities from string names.
    pub fn from_names(names: &[String]) -> Self {
        let mut caps = Self::empty();
        for name in names {
            if let Some(cap) = Self::from_name(name) {
                caps |= cap;
            }
        }
        caps
    }
}

/// Tool metadata with required capabilities.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolMetadata {
    /// Unique identifier for this tool
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Capabilities required to execute this tool
    pub required_capabilities: Capability,
    /// Optional description
    pub description: Option<String>,
}

impl ToolMetadata {
    /// Create new tool metadata.
    pub fn new(id: impl Into<String>, required_capabilities: Capability) -> Self {
        let id = id.into();
        Self {
            name: id.clone(),
            id,
            required_capabilities,
            description: None,
        }
    }

    /// Set the name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// Content filter pattern.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ContentFilter {
    /// Match exact substring
    Substring(String),
    /// Match regex pattern
    Regex(String),
}

impl ContentFilter {
    /// Create a substring filter.
    pub fn substring(s: impl Into<String>) -> Self {
        Self::Substring(s.into())
    }

    /// Create a regex filter.
    pub fn regex(pattern: impl Into<String>) -> Self {
        Self::Regex(pattern.into())
    }
}

/// Action to take when forbidden content is detected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentAction {
    /// Block execution entirely
    Block,
    /// Redact the matching content and continue
    Redact,
}

impl Default for ContentAction {
    fn default() -> Self {
        Self::Redact
    }
}

/// Agent policy configuration.
/// 
/// Defines what an agent is allowed to do during execution:
/// - Which capabilities are granted
/// - How many times each tool can be called
/// - What content is forbidden
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentPolicy {
    /// Whitelist of capabilities granted to this agent
    pub allowed_capabilities: Capability,
    /// Maximum number of calls per tool (tool_id -> limit)
    pub max_tool_calls: HashMap<String, u32>,
    /// Forbidden content patterns
    pub forbidden_content: Vec<ContentFilter>,
    /// Action to take when forbidden content is found
    pub content_action: ContentAction,
    /// Name of this policy (for logging)
    pub name: String,
}

impl AgentPolicy {
    /// Create a new policy with the given capabilities.
    pub fn new(allowed_capabilities: Capability) -> Self {
        Self {
            allowed_capabilities,
            max_tool_calls: HashMap::new(),
            forbidden_content: Vec::new(),
            content_action: ContentAction::default(),
            name: "default".to_string(),
        }
    }

    /// Create a permissive policy that allows all capabilities.
    pub fn permissive() -> Self {
        Self::new(Capability::all())
            .with_name("permissive")
    }

    /// Create a restrictive policy that allows no capabilities.
    pub fn restrictive() -> Self {
        Self::new(Capability::empty())
            .with_name("restrictive")
    }

    /// Create a "no network" policy.
    pub fn no_network() -> Self {
        Self::new(
            Capability::all() - Capability::NET_ACCESS - Capability::EXTERNAL_API
        ).with_name("no_network")
    }

    /// Create a read-only policy.
    pub fn read_only() -> Self {
        Self::new(
            Capability::FILE_SYSTEM_READ | Capability::MEMORY_ACCESS | Capability::LLM_CALL
        ).with_name("read_only")
    }

    /// Set the policy name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Add a tool call limit.
    pub fn with_tool_limit(mut self, tool_id: impl Into<String>, limit: u32) -> Self {
        self.max_tool_calls.insert(tool_id.into(), limit);
        self
    }

    /// Add multiple tool call limits.
    pub fn with_tool_limits(mut self, limits: HashMap<String, u32>) -> Self {
        self.max_tool_calls.extend(limits);
        self
    }

    /// Add a forbidden content pattern.
    pub fn with_forbidden_content(mut self, filter: ContentFilter) -> Self {
        self.forbidden_content.push(filter);
        self
    }

    /// Add a forbidden substring.
    pub fn with_forbidden_substring(mut self, s: impl Into<String>) -> Self {
        self.forbidden_content.push(ContentFilter::Substring(s.into()));
        self
    }

    /// Add a forbidden regex pattern.
    pub fn with_forbidden_regex(mut self, pattern: impl Into<String>) -> Self {
        self.forbidden_content.push(ContentFilter::Regex(pattern.into()));
        self
    }

    /// Set the content action.
    pub fn with_content_action(mut self, action: ContentAction) -> Self {
        self.content_action = action;
        self
    }

    /// Check if a capability is allowed.
    #[inline]
    pub fn has_capability(&self, cap: Capability) -> bool {
        self.allowed_capabilities.contains(cap)
    }

    /// Check if all required capabilities are allowed.
    #[inline]
    pub fn has_all_capabilities(&self, required: Capability) -> bool {
        self.allowed_capabilities.contains(required)
    }

    /// Get the call limit for a tool, if any.
    pub fn get_tool_limit(&self, tool_id: &str) -> Option<u32> {
        self.max_tool_calls.get(tool_id).copied()
    }
}

impl Default for AgentPolicy {
    fn default() -> Self {
        Self::permissive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_flags() {
        let caps = Capability::NET_ACCESS | Capability::FILE_SYSTEM_READ;
        assert!(caps.contains(Capability::NET_ACCESS));
        assert!(caps.contains(Capability::FILE_SYSTEM_READ));
        assert!(!caps.contains(Capability::FILE_SYSTEM_WRITE));
    }

    #[test]
    fn test_capability_from_name() {
        assert_eq!(Capability::from_name("NET_ACCESS"), Some(Capability::NET_ACCESS));
        assert_eq!(Capability::from_name("netaccess"), Some(Capability::NET_ACCESS));
        assert_eq!(Capability::from_name("DISK_READ"), Some(Capability::FILE_SYSTEM_READ));
        assert_eq!(Capability::from_name("unknown"), None);
    }

    #[test]
    fn test_policy_creation() {
        let policy = AgentPolicy::new(Capability::NET_ACCESS | Capability::LLM_CALL)
            .with_tool_limit("web_search", 5)
            .with_forbidden_substring("secret");

        assert!(policy.has_capability(Capability::NET_ACCESS));
        assert!(!policy.has_capability(Capability::FILE_SYSTEM_WRITE));
        assert_eq!(policy.get_tool_limit("web_search"), Some(5));
        assert_eq!(policy.get_tool_limit("other"), None);
    }

    #[test]
    fn test_preset_policies() {
        let perm = AgentPolicy::permissive();
        assert!(perm.has_capability(Capability::all()));

        let restrict = AgentPolicy::restrictive();
        assert!(!restrict.has_capability(Capability::NET_ACCESS));

        let no_net = AgentPolicy::no_network();
        assert!(!no_net.has_capability(Capability::NET_ACCESS));
        assert!(no_net.has_capability(Capability::FILE_SYSTEM_READ));
    }
}
