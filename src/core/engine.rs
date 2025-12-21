//! Policy Engine for KAIROS-ARK.
//! 
//! The policy engine validates every tool invocation and LLM output
//! before execution, enforcing capability requirements, call limits,
//! and content restrictions.

use regex::Regex;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

use crate::core::{
    AuditLedger, LogicalClock,
    policy::{AgentPolicy, Capability, ContentAction, ContentFilter, ToolMetadata},
};

/// Result of a policy check.
#[derive(Clone, Debug)]
pub enum PolicyDecision {
    /// Execution is allowed
    Allow,
    /// Execution is denied
    Deny {
        /// Human-readable reason
        reason: String,
        /// The specific rule that was triggered
        rule: String,
    },
    /// Content was redacted
    Redact {
        /// Original content
        original: String,
        /// Redacted content
        redacted: String,
        /// Patterns that matched
        patterns: Vec<String>,
    },
}

impl PolicyDecision {
    /// Check if this is an allow decision.
    pub fn is_allow(&self) -> bool {
        matches!(self, Self::Allow)
    }

    /// Check if this is a deny decision.
    pub fn is_deny(&self) -> bool {
        matches!(self, Self::Deny { .. })
    }
}

/// Policy violation details.
#[derive(Clone, Debug)]
pub struct PolicyViolation {
    /// Tool ID that caused the violation (if applicable)
    pub tool_id: Option<String>,
    /// The rule that was violated
    pub rule: String,
    /// Human-readable reason
    pub reason: String,
}

impl std::fmt::Display for PolicyViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(tool_id) = &self.tool_id {
            write!(f, "Policy violation for tool '{}': {} ({})", tool_id, self.reason, self.rule)
        } else {
            write!(f, "Policy violation: {} ({})", self.reason, self.rule)
        }
    }
}

impl std::error::Error for PolicyViolation {}

/// The Policy Engine.
/// 
/// Intercepts execution at two critical points:
/// 1. Pre-Tool Execution: Capability and call-count checks
/// 2. Post-LLM Generation: Content moderation
#[derive(Debug)]
pub struct PolicyEngine {
    /// The active policy
    policy: AgentPolicy,
    /// Call counters for each tool (tool_id -> current count)
    call_counters: Mutex<HashMap<String, u32>>,
    /// Reference to audit ledger for logging
    ledger: Arc<AuditLedger>,
    /// Reference to logical clock for timestamps
    clock: Arc<LogicalClock>,
    /// Compiled regex patterns (cached)
    regex_cache: Vec<(String, Regex)>,
}

impl PolicyEngine {
    /// Create a new policy engine with the given policy.
    pub fn new(
        policy: AgentPolicy,
        ledger: Arc<AuditLedger>,
        clock: Arc<LogicalClock>,
    ) -> Self {
        // Pre-compile regex patterns
        let regex_cache: Vec<_> = policy.forbidden_content.iter()
            .filter_map(|f| {
                if let ContentFilter::Regex(pattern) = f {
                    Regex::new(pattern).ok().map(|r| (pattern.clone(), r))
                } else {
                    None
                }
            })
            .collect();

        Self {
            policy,
            call_counters: Mutex::new(HashMap::new()),
            ledger,
            clock,
            regex_cache,
        }
    }

    /// Get the current policy.
    pub fn policy(&self) -> &AgentPolicy {
        &self.policy
    }

    /// Check if a tool can be executed under current policy.
    /// 
    /// This performs the capability check only.
    #[inline]
    pub fn check_tool_capability(&self, tool: &ToolMetadata) -> Result<(), PolicyViolation> {
        let required = tool.required_capabilities;
        let allowed = self.policy.allowed_capabilities;

        if allowed.contains(required) {
            Ok(())
        } else {
            // Find which capabilities are missing
            let missing = required - allowed;
            let missing_names = missing.names().join(", ");

            Err(PolicyViolation {
                tool_id: Some(tool.id.clone()),
                rule: format!("capability_required:{}", missing_names),
                reason: format!(
                    "Tool '{}' requires capabilities [{}] but policy only allows [{}]",
                    tool.id,
                    required.names().join(", "),
                    allowed.names().join(", ")
                ),
            })
        }
    }

    /// Check and increment call counter for a tool.
    /// 
    /// Returns Ok(current_count) if under limit, Err if exceeded.
    pub fn check_and_increment_call_limit(&self, tool_id: &str) -> Result<u32, PolicyViolation> {
        let limit = match self.policy.get_tool_limit(tool_id) {
            Some(limit) => limit,
            None => return Ok(0), // No limit set
        };

        let mut counters = self.call_counters.lock();
        let count = counters.entry(tool_id.to_string()).or_insert(0);

        if *count >= limit {
            Err(PolicyViolation {
                tool_id: Some(tool_id.to_string()),
                rule: format!("call_limit_exceeded:{}:{}", tool_id, limit),
                reason: format!(
                    "Tool '{}' has reached its call limit of {} (attempted call #{})",
                    tool_id, limit, *count + 1
                ),
            })
        } else {
            *count += 1;
            Ok(*count)
        }
    }

    /// Check call limit without incrementing.
    pub fn check_call_limit(&self, tool_id: &str) -> Result<u32, PolicyViolation> {
        let limit = match self.policy.get_tool_limit(tool_id) {
            Some(limit) => limit,
            None => return Ok(0),
        };

        let counters = self.call_counters.lock();
        let count = counters.get(tool_id).copied().unwrap_or(0);

        if count >= limit {
            Err(PolicyViolation {
                tool_id: Some(tool_id.to_string()),
                rule: format!("call_limit_exceeded:{}:{}", tool_id, limit),
                reason: format!(
                    "Tool '{}' has reached its call limit of {}",
                    tool_id, limit
                ),
            })
        } else {
            Ok(count)
        }
    }

    /// Get the current call count for a tool.
    pub fn get_call_count(&self, tool_id: &str) -> u32 {
        self.call_counters.lock().get(tool_id).copied().unwrap_or(0)
    }

    /// Reset call counters (for new execution session).
    pub fn reset_counters(&self) {
        self.call_counters.lock().clear();
    }

    /// Filter content against forbidden patterns.
    /// 
    /// Returns the (possibly redacted) content and list of matched patterns.
    pub fn filter_content(&self, content: &str) -> (String, Vec<String>) {
        let mut result = content.to_string();
        let mut matched_patterns = Vec::new();

        // Check substring patterns
        for filter in &self.policy.forbidden_content {
            if let ContentFilter::Substring(pattern) = filter {
                if result.contains(pattern) {
                    matched_patterns.push(format!("substring:{}", pattern));
                    result = result.replace(pattern, "[REDACTED]");
                }
            }
        }

        // Check regex patterns
        for (pattern, regex) in &self.regex_cache {
            if regex.is_match(&result) {
                matched_patterns.push(format!("regex:{}", pattern));
                result = regex.replace_all(&result, "[REDACTED]").to_string();
            }
        }

        (result, matched_patterns)
    }

    /// Perform full pre-tool execution check.
    /// 
    /// This checks both capabilities and call limits, logs the decision,
    /// and returns the policy decision.
    pub fn pre_tool_check(&self, tool: &ToolMetadata) -> PolicyDecision {
        // Check capabilities
        if let Err(violation) = self.check_tool_capability(tool) {
            // Log deny event
            self.log_policy_deny(&tool.id, &violation.rule, &violation.reason);
            return PolicyDecision::Deny {
                reason: violation.reason,
                rule: violation.rule,
            };
        }

        // Check call limit
        if let Err(violation) = self.check_and_increment_call_limit(&tool.id) {
            // Log call limit exceeded
            self.log_call_limit_exceeded(&tool.id);
            return PolicyDecision::Deny {
                reason: violation.reason,
                rule: violation.rule,
            };
        }

        // Log allow event
        self.log_policy_allow(&tool.id, &tool.required_capabilities);
        PolicyDecision::Allow
    }

    /// Perform post-generation content check.
    /// 
    /// This scans LLM output for forbidden content and either blocks or redacts.
    pub fn post_generation_check(&self, content: &str) -> PolicyDecision {
        let (filtered, matched_patterns) = self.filter_content(content);

        if matched_patterns.is_empty() {
            PolicyDecision::Allow
        } else {
            match self.policy.content_action {
                ContentAction::Block => {
                    let reason = format!(
                        "Content contains forbidden patterns: {}",
                        matched_patterns.join(", ")
                    );
                    self.log_policy_deny("_content_filter", "forbidden_content", &reason);
                    PolicyDecision::Deny {
                        reason,
                        rule: "forbidden_content".to_string(),
                    }
                }
                ContentAction::Redact => {
                    // Log redaction
                    self.log_content_redacted(content.len(), filtered.len(), &matched_patterns);
                    PolicyDecision::Redact {
                        original: content.to_string(),
                        redacted: filtered,
                        patterns: matched_patterns,
                    }
                }
            }
        }
    }

    // Logging helpers

    fn log_policy_allow(&self, tool_id: &str, capabilities: &Capability) {
        use crate::core::ledger::{Event, EventType};
        
        let ts = self.clock.tick();
        self.ledger.append(Event::new(
            ts,
            tool_id.to_string(),
            EventType::PolicyAllow {
                tool_id: tool_id.to_string(),
                capabilities_checked: capabilities.names().iter().map(|s| s.to_string()).collect(),
            },
            None,
        ));
    }

    fn log_policy_deny(&self, tool_id: &str, rule: &str, reason: &str) {
        use crate::core::ledger::{Event, EventType};
        
        let ts = self.clock.tick();
        self.ledger.append(Event::new(
            ts,
            tool_id.to_string(),
            EventType::PolicyDeny {
                tool_id: tool_id.to_string(),
                rule: rule.to_string(),
                reason: reason.to_string(),
            },
            Some(reason.to_string()),
        ));
    }

    fn log_call_limit_exceeded(&self, tool_id: &str) {
        use crate::core::ledger::{Event, EventType};
        
        let limit = self.policy.get_tool_limit(tool_id).unwrap_or(0);
        let attempted = self.get_call_count(tool_id);
        
        let ts = self.clock.tick();
        self.ledger.append(Event::new(
            ts,
            tool_id.to_string(),
            EventType::CallLimitExceeded {
                tool_id: tool_id.to_string(),
                limit,
                attempted,
            },
            Some(format!("limit={}, attempted={}", limit, attempted)),
        ));
    }

    fn log_content_redacted(&self, original_len: usize, redacted_len: usize, patterns: &[String]) {
        use crate::core::ledger::{Event, EventType};
        
        let ts = self.clock.tick();
        self.ledger.append(Event::new(
            ts,
            "_content_filter".to_string(),
            EventType::ContentRedacted {
                original_length: original_len,
                redacted_length: redacted_len,
                patterns_matched: patterns.to_vec(),
            },
            Some(format!("{}→{} bytes, {} patterns", original_len, redacted_len, patterns.len())),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::policy::AgentPolicy;

    fn make_engine(policy: AgentPolicy) -> PolicyEngine {
        let ledger = Arc::new(AuditLedger::new());
        let clock = Arc::new(LogicalClock::new());
        PolicyEngine::new(policy, ledger, clock)
    }

    #[test]
    fn test_capability_check_pass() {
        let policy = AgentPolicy::new(Capability::NET_ACCESS | Capability::LLM_CALL);
        let engine = make_engine(policy);

        let tool = ToolMetadata::new("web_search", Capability::NET_ACCESS);
        assert!(engine.check_tool_capability(&tool).is_ok());
    }

    #[test]
    fn test_capability_check_fail() {
        let policy = AgentPolicy::new(Capability::LLM_CALL); // No NET_ACCESS
        let engine = make_engine(policy);

        let tool = ToolMetadata::new("web_search", Capability::NET_ACCESS);
        let result = engine.check_tool_capability(&tool);
        assert!(result.is_err());
        assert!(result.unwrap_err().reason.contains("NET_ACCESS"));
    }

    #[test]
    fn test_call_limit_enforcement() {
        let policy = AgentPolicy::new(Capability::all())
            .with_tool_limit("api_call", 2);
        let engine = make_engine(policy);

        // First two calls succeed
        assert!(engine.check_and_increment_call_limit("api_call").is_ok());
        assert!(engine.check_and_increment_call_limit("api_call").is_ok());

        // Third call fails
        assert!(engine.check_and_increment_call_limit("api_call").is_err());
    }

    #[test]
    fn test_content_filter_substring() {
        let policy = AgentPolicy::new(Capability::all())
            .with_forbidden_substring("secret_key_123");
        let engine = make_engine(policy);

        let (filtered, patterns) = engine.filter_content("The secret_key_123 is here.");
        assert_eq!(filtered, "The [REDACTED] is here.");
        assert_eq!(patterns.len(), 1);
    }

    #[test]
    fn test_content_filter_regex() {
        let policy = AgentPolicy::new(Capability::all())
            .with_forbidden_regex(r"\b\d{3}-\d{2}-\d{4}\b"); // SSN pattern
        let engine = make_engine(policy);

        let (filtered, patterns) = engine.filter_content("SSN: 123-45-6789");
        assert_eq!(filtered, "SSN: [REDACTED]");
        assert_eq!(patterns.len(), 1);
    }

    #[test]
    fn test_pre_tool_check() {
        let policy = AgentPolicy::new(Capability::NET_ACCESS)
            .with_tool_limit("search", 1);
        let engine = make_engine(policy);

        let tool = ToolMetadata::new("search", Capability::NET_ACCESS);

        // First check: allow
        assert!(engine.pre_tool_check(&tool).is_allow());

        // Second check: deny (limit exceeded)
        assert!(engine.pre_tool_check(&tool).is_deny());
    }
}
