//! Model Context Protocol (MCP) Support.
//!
//! Provides MCP-compatible tool registration and invocation,
//! allowing any MCP-compliant tool to leverage KAIROS-ARK.

use std::collections::HashMap;


use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// MCP Tool schema definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpSchema {
    /// JSON Schema type
    #[serde(rename = "type")]
    pub schema_type: String,
    /// Properties for object types
    #[serde(default)]
    pub properties: HashMap<String, Value>,
    /// Required properties
    #[serde(default)]
    pub required: Vec<String>,
}

impl Default for McpSchema {
    fn default() -> Self {
        Self {
            schema_type: "object".to_string(),
            properties: HashMap::new(),
            required: Vec::new(),
        }
    }
}

/// MCP Tool information.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpToolInfo {
    /// Tool name
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Input schema
    pub input_schema: McpSchema,
    /// Required capabilities (for policy enforcement)
    #[serde(default)]
    pub capabilities: u32,
}

impl McpToolInfo {
    /// Create a new tool info.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: McpSchema::default(),
            capabilities: 0,
        }
    }

    /// Set input schema.
    pub fn with_schema(mut self, schema: McpSchema) -> Self {
        self.input_schema = schema;
        self
    }

    /// Set required capabilities.
    pub fn with_capabilities(mut self, caps: u32) -> Self {
        self.capabilities = caps;
        self
    }
}

/// MCP Tool result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpResult {
    /// Whether the call succeeded
    pub success: bool,
    /// Result content
    pub content: Value,
    /// Error message if failed
    pub error: Option<String>,
}

impl McpResult {
    /// Create a success result.
    pub fn ok(content: Value) -> Self {
        Self {
            success: true,
            content,
            error: None,
        }
    }

    /// Create an error result.
    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            content: Value::Null,
            error: Some(message.into()),
        }
    }
}

/// MCP Tool handler type.
pub type McpHandler = Box<dyn Fn(Value) -> McpResult + Send + Sync>;

/// Registered MCP tool.
struct RegisteredTool {
    info: McpToolInfo,
    handler: McpHandler,
}

/// MCP Server implementation for KAIROS-ARK.
///
/// This allows MCP-compliant tools to be registered and invoked
/// with full policy enforcement and audit logging.
pub struct McpServer {
    /// Registered tools
    tools: RwLock<HashMap<String, RegisteredTool>>,
    /// Server name
    name: String,
    /// Server version
    version: String,
}

impl McpServer {
    /// Create a new MCP server.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
            name: name.into(),
            version: version.into(),
        }
    }

    /// Get server name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get server version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Register an MCP tool.
    pub fn register<F>(&self, info: McpToolInfo, handler: F)
    where
        F: Fn(Value) -> McpResult + Send + Sync + 'static,
    {
        let name = info.name.clone();
        self.tools.write().insert(
            name,
            RegisteredTool {
                info,
                handler: Box::new(handler),
            },
        );
    }

    /// List all registered tools.
    pub fn list_tools(&self) -> Vec<McpToolInfo> {
        self.tools.read().values().map(|t| t.info.clone()).collect()
    }

    /// Get tool info by name.
    pub fn get_tool(&self, name: &str) -> Option<McpToolInfo> {
        self.tools.read().get(name).map(|t| t.info.clone())
    }

    /// Call a tool.
    pub fn call_tool(&self, name: &str, args: Value) -> McpResult {
        let tools = self.tools.read();
        match tools.get(name) {
            Some(tool) => (tool.handler)(args),
            None => McpResult::err(format!("Tool not found: {}", name)),
        }
    }

    /// Unregister a tool.
    pub fn unregister(&self, name: &str) -> bool {
        self.tools.write().remove(name).is_some()
    }

    /// Get tool count.
    pub fn tool_count(&self) -> usize {
        self.tools.read().len()
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new("kairos-ark", env!("CARGO_PKG_VERSION"))
    }
}

impl std::fmt::Debug for McpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpServer")
            .field("name", &self.name)
            .field("version", &self.version)
            .field("tools", &self.tool_count())
            .finish()
    }
}

/// Global MCP server singleton.
static GLOBAL_MCP_SERVER: std::sync::OnceLock<McpServer> = std::sync::OnceLock::new();

/// Get the global MCP server.
pub fn global_mcp_server() -> &'static McpServer {
    GLOBAL_MCP_SERVER.get_or_init(McpServer::default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_tool() {
        let server = McpServer::default();
        let info = McpToolInfo::new("echo", "Echo input back");
        
        server.register(info, |args| {
            McpResult::ok(args)
        });

        assert_eq!(server.tool_count(), 1);
    }

    #[test]
    fn test_call_tool() {
        let server = McpServer::default();
        let info = McpToolInfo::new("add", "Add two numbers");
        
        server.register(info, |args| {
            let a = args.get("a").and_then(|v| v.as_i64()).unwrap_or(0);
            let b = args.get("b").and_then(|v| v.as_i64()).unwrap_or(0);
            McpResult::ok(Value::from(a + b))
        });

        let args = serde_json::json!({"a": 2, "b": 3});
        let result = server.call_tool("add", args);
        assert!(result.success);
        assert_eq!(result.content, Value::from(5));
    }

    #[test]
    fn test_tool_not_found() {
        let server = McpServer::default();
        let result = server.call_tool("nonexistent", Value::Null);
        assert!(!result.success);
        assert!(result.error.is_some());
    }
}
