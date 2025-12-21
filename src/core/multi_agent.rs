//! Multi-Agent Scheduler for KAIROS-ARK.
//!
//! Provides concurrent execution of multiple agent graphs with
//! priority-based scheduling and work stealing.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};


use parking_lot::{Mutex};
use rayon::prelude::*;

/// Agent ID type.
pub type AgentId = u64;

/// Priority levels for Quality of Service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Priority {
    /// Real-time tasks (voice, immediate interaction)
    RealTime = 0,
    /// High priority tasks
    High = 1,
    /// Normal priority (default)
    Normal = 2,
    /// Background/batch tasks
    Background = 3,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

impl From<u8> for Priority {
    fn from(v: u8) -> Self {
        match v {
            0 => Priority::RealTime,
            1 => Priority::High,
            3 => Priority::Background,
            _ => Priority::Normal,
        }
    }
}

/// Result of running an agent.
#[derive(Clone, Debug)]
pub struct AgentResult {
    /// Agent ID
    pub agent_id: AgentId,
    /// Whether execution succeeded
    pub success: bool,
    /// Number of nodes executed
    pub nodes_executed: usize,
    /// Execution time in microseconds
    pub execution_time_us: u64,
    /// Error message if failed
    pub error: Option<String>,
}

/// Agent task for scheduling.
pub struct AgentTask {
    /// Unique agent ID
    pub id: AgentId,
    /// Priority level
    pub priority: Priority,
    /// Task handler
    handler: Box<dyn FnOnce() -> AgentResult + Send>,
}

impl AgentTask {
    /// Create a new agent task.
    pub fn new<F>(priority: Priority, handler: F) -> Self
    where
        F: FnOnce() -> AgentResult + Send + 'static,
    {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::SeqCst),
            priority,
            handler: Box::new(handler),
        }
    }

    /// Execute the task.
    pub fn execute(self) -> AgentResult {
        (self.handler)()
    }
}

/// Multi-agent scheduler with priority queues.
pub struct MultiAgentScheduler {
    /// Priority queues (ordered by priority level)
    queues: [Mutex<VecDeque<AgentTask>>; 4],
    /// Number of worker threads
    num_threads: usize,
    /// Running task count
    running_count: AtomicUsize,
    /// Total tasks completed
    completed_count: AtomicU64,
    /// Preemption flag for real-time tasks
    preempt_flag: AtomicUsize,
}

impl MultiAgentScheduler {
    /// Create a new multi-agent scheduler.
    pub fn new(num_threads: usize) -> Self {
        Self {
            queues: Default::default(),
            num_threads: num_threads.max(1),
            running_count: AtomicUsize::new(0),
            completed_count: AtomicU64::new(0),
            preempt_flag: AtomicUsize::new(0),
        }
    }

    /// Add an agent task with priority.
    pub fn add_task(&self, task: AgentTask) -> AgentId {
        let id = task.id;
        let priority_idx = task.priority as usize;
        self.queues[priority_idx].lock().push_back(task);
        id
    }

    /// Add multiple tasks at once.
    pub fn add_tasks(&self, tasks: Vec<AgentTask>) -> Vec<AgentId> {
        tasks.into_iter().map(|t| self.add_task(t)).collect()
    }

    /// Get the next task to run (respects priority).
    fn pop_next(&self) -> Option<AgentTask> {
        // Check queues in priority order
        for queue in &self.queues {
            if let Some(task) = queue.lock().pop_front() {
                return Some(task);
            }
        }
        None
    }

    /// Get pending task count.
    pub fn pending_count(&self) -> usize {
        self.queues.iter().map(|q| q.lock().len()).sum()
    }

    /// Get running task count.
    pub fn running_count(&self) -> usize {
        self.running_count.load(Ordering::SeqCst)
    }

    /// Get completed task count.
    pub fn completed_count(&self) -> u64 {
        self.completed_count.load(Ordering::SeqCst)
    }

    /// Run all pending tasks and return results.
    pub fn run_all(&self) -> Vec<AgentResult> {
        let mut results = Vec::new();

        // Collect all tasks
        let mut all_tasks = Vec::new();
        for queue in &self.queues {
            all_tasks.extend(queue.lock().drain(..));
        }

        if all_tasks.is_empty() {
            return results;
        }

        // Sort by priority (lower = higher priority)
        all_tasks.sort_by_key(|t| t.priority);

        // Execute in parallel using rayon
        results = all_tasks
            .into_par_iter()
            .map(|task| {
                self.running_count.fetch_add(1, Ordering::SeqCst);
                let result = task.execute();
                self.running_count.fetch_sub(1, Ordering::SeqCst);
                self.completed_count.fetch_add(1, Ordering::SeqCst);
                result
            })
            .collect();

        results
    }

    /// Run tasks sequentially (for single-threaded mode).
    pub fn run_sequential(&self) -> Vec<AgentResult> {
        let mut results = Vec::new();

        while let Some(task) = self.pop_next() {
            self.running_count.fetch_add(1, Ordering::SeqCst);
            let result = task.execute();
            self.running_count.fetch_sub(1, Ordering::SeqCst);
            self.completed_count.fetch_add(1, Ordering::SeqCst);
            results.push(result);
        }

        results
    }

    /// Signal preemption for real-time task.
    pub fn signal_preempt(&self) {
        self.preempt_flag.fetch_add(1, Ordering::SeqCst);
    }

    /// Check if preemption was signaled.
    pub fn check_preempt(&self) -> bool {
        self.preempt_flag.swap(0, Ordering::SeqCst) > 0
    }

    /// Clear all queues.
    pub fn clear(&self) {
        for queue in &self.queues {
            queue.lock().clear();
        }
    }
}

impl Default for MultiAgentScheduler {
    fn default() -> Self {
        Self::new(num_cpus())
    }
}

impl std::fmt::Debug for MultiAgentScheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiAgentScheduler")
            .field("num_threads", &self.num_threads)
            .field("pending", &self.pending_count())
            .field("running", &self.running_count())
            .field("completed", &self.completed_count())
            .finish()
    }
}

/// Get number of CPUs.
fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Global multi-agent scheduler.
static GLOBAL_SCHEDULER: std::sync::OnceLock<MultiAgentScheduler> = std::sync::OnceLock::new();

/// Get the global multi-agent scheduler.
pub fn global_scheduler() -> &'static MultiAgentScheduler {
    GLOBAL_SCHEDULER.get_or_init(MultiAgentScheduler::default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::RealTime < Priority::High);
        assert!(Priority::High < Priority::Normal);
        assert!(Priority::Normal < Priority::Background);
    }

    #[test]
    fn test_add_and_run_tasks() {
        let scheduler = MultiAgentScheduler::new(2);

        let task1 = AgentTask::new(Priority::Normal, || AgentResult {
            agent_id: 1,
            success: true,
            nodes_executed: 10,
            execution_time_us: 100,
            error: None,
        });

        let task2 = AgentTask::new(Priority::High, || AgentResult {
            agent_id: 2,
            success: true,
            nodes_executed: 5,
            execution_time_us: 50,
            error: None,
        });

        scheduler.add_task(task1);
        scheduler.add_task(task2);

        assert_eq!(scheduler.pending_count(), 2);

        let results = scheduler.run_all();
        assert_eq!(results.len(), 2);
        assert_eq!(scheduler.completed_count(), 2);
    }

    #[test]
    fn test_parallel_execution() {
        let scheduler = MultiAgentScheduler::new(4);

        // Add 100 tasks
        for i in 0..100 {
            let task = AgentTask::new(Priority::Normal, move || AgentResult {
                agent_id: i as u64,
                success: true,
                nodes_executed: 1,
                execution_time_us: 0,
                error: None,
            });
            scheduler.add_task(task);
        }

        let start = Instant::now();
        let results = scheduler.run_all();
        let elapsed = start.elapsed();

        assert_eq!(results.len(), 100);
        println!("100 tasks completed in {:?}", elapsed);
    }

    #[test]
    fn test_priority_execution_order() {
        let scheduler = MultiAgentScheduler::new(1);
        let order = Arc::new(Mutex::new(Vec::new()));

        // Add tasks in reverse priority order
        let order_clone = order.clone();
        scheduler.add_task(AgentTask::new(Priority::Background, move || {
            order_clone.lock().push("bg");
            AgentResult {
                agent_id: 1,
                success: true,
                nodes_executed: 0,
                execution_time_us: 0,
                error: None,
            }
        }));

        let order_clone = order.clone();
        scheduler.add_task(AgentTask::new(Priority::RealTime, move || {
            order_clone.lock().push("rt");
            AgentResult {
                agent_id: 2,
                success: true,
                nodes_executed: 0,
                execution_time_us: 0,
                error: None,
            }
        }));

        // Run sequentially to check order
        scheduler.run_sequential();

        let executed = order.lock().clone();
        assert_eq!(executed, vec!["rt", "bg"]);
    }
}
