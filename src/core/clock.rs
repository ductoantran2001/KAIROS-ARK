//! Logical Clock implementation for deterministic ordering.
//! 
//! The logical clock provides a monotonic counter that increments with every
//! event, ensuring consistent ordering during replay regardless of wall-clock time.

use std::sync::atomic::{AtomicU64, Ordering};

/// A thread-safe monotonic logical clock.
/// 
/// Each `tick()` increments the counter and returns the new value,
/// ensuring total ordering of events across all threads.
#[derive(Debug)]
pub struct LogicalClock {
    counter: AtomicU64,
}

impl LogicalClock {
    /// Create a new logical clock starting at 0.
    pub fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
        }
    }

    /// Create a new logical clock starting at a specific value.
    /// Useful for replay scenarios.
    pub fn with_start(start: u64) -> Self {
        Self {
            counter: AtomicU64::new(start),
        }
    }

    /// Atomically increment the clock and return the new timestamp.
    /// 
    /// This is the primary method for generating event timestamps.
    /// The returned value is guaranteed to be unique and monotonically
    /// increasing across all concurrent callers.
    #[inline]
    pub fn tick(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Get the current clock value without incrementing.
    /// 
    /// Note: This is primarily for debugging/inspection. For event
    /// ordering, always use `tick()`.
    #[inline]
    pub fn current(&self) -> u64 {
        self.counter.load(Ordering::SeqCst)
    }

    /// Reset the clock to zero.
    /// 
    /// Warning: Only use this between complete graph executions.
    pub fn reset(&self) {
        self.counter.store(0, Ordering::SeqCst);
    }
}

impl Default for LogicalClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for LogicalClock {
    fn clone(&self) -> Self {
        Self::with_start(self.current())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_monotonic_increment() {
        let clock = LogicalClock::new();
        assert_eq!(clock.current(), 0);
        assert_eq!(clock.tick(), 1);
        assert_eq!(clock.tick(), 2);
        assert_eq!(clock.tick(), 3);
        assert_eq!(clock.current(), 3);
    }

    #[test]
    fn test_concurrent_ticks() {
        let clock = Arc::new(LogicalClock::new());
        let mut handles = vec![];

        for _ in 0..10 {
            let clock_clone = Arc::clone(&clock);
            handles.push(thread::spawn(move || {
                let mut ticks = vec![];
                for _ in 0..100 {
                    ticks.push(clock_clone.tick());
                }
                ticks
            }));
        }

        let mut all_ticks: Vec<u64> = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();

        // All ticks should be unique
        all_ticks.sort();
        let original_len = all_ticks.len();
        all_ticks.dedup();
        assert_eq!(all_ticks.len(), original_len);

        // Should have 1000 unique ticks (10 threads * 100 ticks)
        assert_eq!(all_ticks.len(), 1000);
        
        // Clock should be at 1000
        assert_eq!(clock.current(), 1000);
    }

    #[test]
    fn test_reset() {
        let clock = LogicalClock::new();
        clock.tick();
        clock.tick();
        clock.reset();
        assert_eq!(clock.current(), 0);
        assert_eq!(clock.tick(), 1);
    }
}
