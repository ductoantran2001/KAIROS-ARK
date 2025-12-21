//! Agent Profiler for KAIROS-ARK.
//!
//! Provides detailed timing breakdowns for agent execution,
//! including kernel overhead, tool execution, and LLM latency.

use std::collections::HashMap;
use std::time::Instant;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

/// Timing category for profiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimingCategory {
    /// Kernel dispatch/scheduling overhead
    Kernel,
    /// Tool/handler execution
    ToolExecution,
    /// LLM API latency
    LlmLatency,
    /// Serialization/deserialization
    Serialization,
    /// Policy checking
    PolicyCheck,
    /// State management
    StateManagement,
    /// Other
    Other,
}

impl TimingCategory {
    /// Get display name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Kernel => "kernel",
            Self::ToolExecution => "tool",
            Self::LlmLatency => "llm",
            Self::Serialization => "serde",
            Self::PolicyCheck => "policy",
            Self::StateManagement => "state",
            Self::Other => "other",
        }
    }
}

/// Single timing record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimingRecord {
    /// Record name (node or operation)
    pub name: String,
    /// Category
    pub category: TimingCategory,
    /// Duration in microseconds
    pub duration_us: u64,
    /// Timestamp relative to profile start
    pub offset_us: u64,
}

/// Aggregated profile data.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProfileData {
    /// Total execution time in microseconds
    pub total_time_us: u64,
    /// Time spent in kernel overhead
    pub kernel_overhead_us: u64,
    /// Time spent in tool execution
    pub tool_execution_us: u64,
    /// Time spent waiting for LLM
    pub llm_latency_us: u64,
    /// Time per node
    pub node_timings: HashMap<String, u64>,
    /// Category breakdown
    pub category_breakdown: HashMap<String, u64>,
    /// Individual timing records
    pub records: Vec<TimingRecord>,
}

impl ProfileData {
    /// Get percentage for a category.
    pub fn category_percent(&self, category: &str) -> f64 {
        if self.total_time_us == 0 {
            return 0.0;
        }
        let cat_time = self.category_breakdown.get(category).copied().unwrap_or(0);
        (cat_time as f64 / self.total_time_us as f64) * 100.0
    }

    /// Get flamegraph-compatible format.
    pub fn to_flamegraph_lines(&self) -> Vec<String> {
        self.records
            .iter()
            .map(|r| format!("{};{} {}", r.category.as_str(), r.name, r.duration_us))
            .collect()
    }
}

/// Agent profiler for timing analysis.
pub struct AgentProfiler {
    /// Start time
    start: Option<Instant>,
    /// Recorded timings
    records: Mutex<Vec<TimingRecord>>,
    /// Current node being profiled
    current_node: Mutex<Option<(String, Instant)>>,
}

impl AgentProfiler {
    /// Create a new profiler.
    pub fn new() -> Self {
        Self {
            start: None,
            records: Mutex::new(Vec::new()),
            current_node: Mutex::new(None),
        }
    }

    /// Start profiling.
    pub fn start(&mut self) {
        self.start = Some(Instant::now());
        self.records.lock().clear();
    }

    /// Begin timing a node.
    pub fn begin_node(&self, name: impl Into<String>) {
        *self.current_node.lock() = Some((name.into(), Instant::now()));
    }

    /// End timing current node.
    pub fn end_node(&self, category: TimingCategory) {
        if let Some((name, start)) = self.current_node.lock().take() {
            let duration = start.elapsed();
            let offset = self.start.map(|s| s.elapsed()).unwrap_or_default();

            self.records.lock().push(TimingRecord {
                name,
                category,
                duration_us: duration.as_micros() as u64,
                offset_us: offset.as_micros() as u64,
            });
        }
    }

    /// Record a timing directly.
    pub fn record(&self, name: impl Into<String>, category: TimingCategory, duration_us: u64) {
        let offset = self.start.map(|s| s.elapsed()).unwrap_or_default();
        
        self.records.lock().push(TimingRecord {
            name: name.into(),
            category,
            duration_us,
            offset_us: offset.as_micros() as u64,
        });
    }

    /// Finish profiling and return data.
    pub fn finish(&self) -> ProfileData {
        let total_time = self.start.map(|s| s.elapsed()).unwrap_or_default();
        let records = self.records.lock().clone();

        let mut data = ProfileData {
            total_time_us: total_time.as_micros() as u64,
            records: records.clone(),
            ..Default::default()
        };

        // Aggregate by category
        for record in &records {
            let cat_key = record.category.as_str().to_string();
            *data.category_breakdown.entry(cat_key).or_insert(0) += record.duration_us;
            *data.node_timings.entry(record.name.clone()).or_insert(0) += record.duration_us;

            match record.category {
                TimingCategory::Kernel => data.kernel_overhead_us += record.duration_us,
                TimingCategory::ToolExecution => data.tool_execution_us += record.duration_us,
                TimingCategory::LlmLatency => data.llm_latency_us += record.duration_us,
                _ => {}
            }
        }

        data
    }

    /// Reset profiler.
    pub fn reset(&mut self) {
        self.start = None;
        self.records.lock().clear();
        *self.current_node.lock() = None;
    }
}

impl Default for AgentProfiler {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for AgentProfiler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentProfiler")
            .field("running", &self.start.is_some())
            .field("records", &self.records.lock().len())
            .finish()
    }
}

/// Utility to measure execution time.
pub struct Timer {
    start: Instant,
    name: String,
}

impl Timer {
    /// Start a new timer.
    pub fn start(name: impl Into<String>) -> Self {
        Self {
            start: Instant::now(),
            name: name.into(),
        }
    }

    /// Get elapsed microseconds.
    pub fn elapsed_us(&self) -> u64 {
        self.start.elapsed().as_micros() as u64
    }

    /// Stop and return a timing record.
    pub fn stop(self, category: TimingCategory) -> TimingRecord {
        let duration_us = self.start.elapsed().as_micros() as u64;
        TimingRecord {
            name: self.name,
            category,
            duration_us,
            offset_us: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_profiler_basic() {
        let mut profiler = AgentProfiler::new();
        profiler.start();
        
        profiler.begin_node("node1");
        thread::sleep(Duration::from_millis(10));
        profiler.end_node(TimingCategory::ToolExecution);

        let data = profiler.finish();
        assert!(data.total_time_us >= 10000); // At least 10ms
        assert!(data.tool_execution_us > 0);
    }

    #[test]
    fn test_record_direct() {
        let mut profiler = AgentProfiler::new();
        profiler.start();
        
        profiler.record("kernel_dispatch", TimingCategory::Kernel, 100);
        profiler.record("llm_call", TimingCategory::LlmLatency, 5000);

        let data = profiler.finish();
        assert_eq!(data.kernel_overhead_us, 100);
        assert_eq!(data.llm_latency_us, 5000);
    }

    #[test]
    fn test_category_percent() {
        let data = ProfileData {
            total_time_us: 1000,
            category_breakdown: [("kernel".to_string(), 100)].into(),
            ..Default::default()
        };

        assert!((data.category_percent("kernel") - 10.0).abs() < 0.01);
    }
}
