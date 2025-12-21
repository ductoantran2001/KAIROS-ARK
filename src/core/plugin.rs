//! Native Plugin Loader for KAIROS-ARK.
//!
//! Provides a stable C ABI for loading and executing native plugins.
//! Plugins can be written in any language that exports C functions.

use std::collections::HashMap;

use std::os::raw::c_char;
use std::path::Path;

use parking_lot::RwLock;

/// Result type for plugin operations.
pub type PluginResult<T> = Result<T, PluginError>;

/// Plugin error types.
#[derive(Debug, Clone)]
pub enum PluginError {
    /// Failed to load the library
    LoadFailed(String),
    /// Plugin not found
    NotFound(String),
    /// Function not found in plugin
    FunctionNotFound(String),
    /// Plugin execution failed
    ExecutionFailed(String),
    /// Invalid plugin format
    InvalidFormat(String),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadFailed(msg) => write!(f, "Plugin load failed: {}", msg),
            Self::NotFound(name) => write!(f, "Plugin not found: {}", name),
            Self::FunctionNotFound(name) => write!(f, "Function not found: {}", name),
            Self::ExecutionFailed(msg) => write!(f, "Plugin execution failed: {}", msg),
            Self::InvalidFormat(msg) => write!(f, "Invalid plugin format: {}", msg),
        }
    }
}

impl std::error::Error for PluginError {}

/// C-compatible tool function signature.
///
/// Plugins must export a function with this signature:
/// ```c
/// int invoke_tool(const char* input, char* output, size_t output_len);
/// ```
///
/// Returns 0 on success, non-zero on error.
pub type ToolFn = unsafe extern "C" fn(*const c_char, *mut c_char, usize) -> i32;

/// Plugin metadata function signature.
///
/// Optional function to get plugin info:
/// ```c
/// const char* get_plugin_info();
/// ```
pub type InfoFn = unsafe extern "C" fn() -> *const c_char;

/// Plugin information.
#[derive(Clone, Debug)]
pub struct PluginInfo {
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Required capabilities (bitmask)
    pub capabilities: u32,
    /// Path to the loaded library
    pub path: String,
}

/// A loaded plugin (mock implementation for safety).
///
/// Note: Actual dynamic loading would use libloading crate.
/// This is a simplified version for demonstration.
struct LoadedPlugin {
    info: PluginInfo,
    /// Mock: In real implementation, this would be the actual function pointer
    #[allow(dead_code)]
    handler: Option<Box<dyn Fn(&str) -> Result<String, String> + Send + Sync>>,
}

/// Native plugin loader.
///
/// Manages loading, unloading, and invoking native plugins.
pub struct PluginLoader {
    /// Loaded plugins by name
    plugins: RwLock<HashMap<String, LoadedPlugin>>,
    /// Output buffer size for plugin calls
    output_buffer_size: usize,
}

impl PluginLoader {
    /// Create a new plugin loader.
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
            output_buffer_size: 64 * 1024, // 64KB default
        }
    }

    /// Set the output buffer size for plugin calls.
    pub fn with_output_buffer_size(mut self, size: usize) -> Self {
        self.output_buffer_size = size;
        self
    }

    /// Load a plugin from a shared library.
    ///
    /// The library must export:
    /// - `invoke_tool`: Required function for tool execution
    /// - `get_plugin_info`: Optional function for metadata
    ///
    /// # Safety
    /// Loading native code is inherently unsafe. Only load trusted plugins.
    pub fn load(&self, path: &Path) -> PluginResult<PluginInfo> {
        let path_str = path.to_string_lossy().to_string();

        // For safety in this implementation, we create a mock plugin
        // Real implementation would use libloading:
        // let lib = unsafe { libloading::Library::new(path)? };
        // let invoke: Symbol<ToolFn> = unsafe { lib.get(b"invoke_tool")? };

        // Extract name from file
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let info = PluginInfo {
            name: name.clone(),
            version: "1.0.0".to_string(),
            capabilities: 0,
            path: path_str,
        };

        let plugin = LoadedPlugin {
            info: info.clone(),
            handler: None,
        };

        self.plugins.write().insert(name, plugin);

        Ok(info)
    }

    /// Register a Rust-native plugin handler.
    ///
    /// This allows registering plugins implemented in Rust without
    /// going through FFI.
    pub fn register<F>(&self, name: &str, version: &str, handler: F) -> PluginInfo
    where
        F: Fn(&str) -> Result<String, String> + Send + Sync + 'static,
    {
        let info = PluginInfo {
            name: name.to_string(),
            version: version.to_string(),
            capabilities: 0,
            path: "<registered>".to_string(),
        };

        let plugin = LoadedPlugin {
            info: info.clone(),
            handler: Some(Box::new(handler)),
        };

        self.plugins.write().insert(name.to_string(), plugin);

        info
    }

    /// Invoke a loaded plugin.
    pub fn invoke(&self, name: &str, input: &str) -> PluginResult<String> {
        let plugins = self.plugins.read();
        let plugin = plugins
            .get(name)
            .ok_or_else(|| PluginError::NotFound(name.to_string()))?;

        if let Some(handler) = &plugin.handler {
            handler(input).map_err(PluginError::ExecutionFailed)
        } else {
            // Mock response for plugins loaded from disk without handler
            Err(PluginError::ExecutionFailed(
                "Plugin has no handler (native loading not implemented)".to_string(),
            ))
        }
    }

    /// Check if a plugin is loaded.
    pub fn is_loaded(&self, name: &str) -> bool {
        self.plugins.read().contains_key(name)
    }

    /// Get plugin info.
    pub fn get_info(&self, name: &str) -> Option<PluginInfo> {
        self.plugins.read().get(name).map(|p| p.info.clone())
    }

    /// List all loaded plugins.
    pub fn list(&self) -> Vec<PluginInfo> {
        self.plugins
            .read()
            .values()
            .map(|p| p.info.clone())
            .collect()
    }

    /// Unload a plugin.
    pub fn unload(&self, name: &str) -> PluginResult<()> {
        if self.plugins.write().remove(name).is_some() {
            Ok(())
        } else {
            Err(PluginError::NotFound(name.to_string()))
        }
    }

    /// Unload all plugins.
    pub fn unload_all(&self) {
        self.plugins.write().clear();
    }

    /// Get the number of loaded plugins.
    pub fn count(&self) -> usize {
        self.plugins.read().len()
    }
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for PluginLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginLoader")
            .field("plugins", &self.count())
            .field("output_buffer_size", &self.output_buffer_size)
            .finish()
    }
}

/// Global plugin loader.
static GLOBAL_LOADER: std::sync::OnceLock<PluginLoader> = std::sync::OnceLock::new();

/// Get the global plugin loader.
pub fn global_loader() -> &'static PluginLoader {
    GLOBAL_LOADER.get_or_init(PluginLoader::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_invoke() {
        let loader = PluginLoader::new();

        // Register a simple plugin
        loader.register("echo", "1.0.0", |input| Ok(format!("Echo: {}", input)));

        assert!(loader.is_loaded("echo"));

        let result = loader.invoke("echo", "Hello").unwrap();
        assert_eq!(result, "Echo: Hello");
    }

    #[test]
    fn test_plugin_not_found() {
        let loader = PluginLoader::new();
        let result = loader.invoke("nonexistent", "test");
        assert!(matches!(result, Err(PluginError::NotFound(_))));
    }

    #[test]
    fn test_unload() {
        let loader = PluginLoader::new();
        loader.register("temp", "1.0.0", |_| Ok("ok".to_string()));

        assert!(loader.is_loaded("temp"));
        loader.unload("temp").unwrap();
        assert!(!loader.is_loaded("temp"));
    }

    #[test]
    fn test_list_plugins() {
        let loader = PluginLoader::new();
        loader.register("plugin1", "1.0.0", |_| Ok("1".to_string()));
        loader.register("plugin2", "2.0.0", |_| Ok("2".to_string()));

        let list = loader.list();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_plugin_info() {
        let loader = PluginLoader::new();
        loader.register("myplug", "3.2.1", |_| Ok("".to_string()));

        let info = loader.get_info("myplug").unwrap();
        assert_eq!(info.name, "myplug");
        assert_eq!(info.version, "3.2.1");
    }
}
