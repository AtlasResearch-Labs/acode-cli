//! Atlas Daedalus — Timer, Countdown & Scheduling Tool
//! (C) 2026 Atlas Research Labs — MIT License

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerOutcome {
    pub label: String,
    pub requested_seconds: u64,
    pub elapsed_millis: u128,
    pub status: String,
}

pub struct TimerTools {
    active_stopwatches: Arc<Mutex<HashMap<String, Instant>>>,
}

impl TimerTools {
    pub fn new() -> Self {
        Self {
            active_stopwatches: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Asynchronously wait for a given duration with an optional descriptive label
    pub async fn set_timer(
        &self,
        seconds: u64,
        label: Option<String>,
    ) -> Result<TimerOutcome, String> {
        if seconds > 3600 {
            return Err(
                "Timer limit exceeded: maximum allowed duration is 3600 seconds (1 hour)."
                    .to_string(),
            );
        }

        let name = label.unwrap_or_else(|| format!("{}s timer", seconds));
        let start = Instant::now();

        tokio::time::sleep(Duration::from_secs(seconds)).await;

        let elapsed = start.elapsed();
        Ok(TimerOutcome {
            label: name,
            requested_seconds: seconds,
            elapsed_millis: elapsed.as_millis(),
            status: "completed".to_string(),
        })
    }

    /// Sleep for a specific duration
    pub async fn sleep(&self, seconds: u64) -> Result<String, String> {
        let outcome = self.set_timer(seconds, Some("pause".to_string())).await?;
        Ok(format!(
            "[OK] Paused execution for {:.1}s",
            outcome.elapsed_millis as f64 / 1000.0
        ))
    }

    /// Start a named stopwatch
    pub fn start_stopwatch(&self, name: &str) {
        if let Ok(mut map) = self.active_stopwatches.lock() {
            map.insert(name.to_string(), Instant::now());
        }
    }

    /// Stop a named stopwatch and get elapsed duration
    pub fn stop_stopwatch(&self, name: &str) -> Option<Duration> {
        if let Ok(mut map) = self.active_stopwatches.lock() {
            map.remove(name).map(|start| start.elapsed())
        } else {
            None
        }
    }
}

impl Default for TimerTools {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_timer_countdown_and_elapsed() {
        let tools = TimerTools::new();
        let outcome = tools
            .set_timer(1, Some("test_build".to_string()))
            .await
            .expect("Timer should succeed");

        assert_eq!(outcome.label, "test_build");
        assert_eq!(outcome.requested_seconds, 1);
        assert!(outcome.elapsed_millis >= 900);
        assert_eq!(outcome.status, "completed");
    }

    #[test]
    fn test_stopwatch_lifecycle() {
        let tools = TimerTools::new();
        tools.start_stopwatch("phase1");
        std::thread::sleep(Duration::from_millis(10));
        let elapsed = tools
            .stop_stopwatch("phase1")
            .expect("Stopwatch should exist");
        assert!(elapsed.as_millis() >= 8);
    }
}
