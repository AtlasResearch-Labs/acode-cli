//! Atlas Daedalus — Token Meter & Cost Telemetry
//! (C) 2026 Atlas Research Labs — MIT License

use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
pub struct SessionCostMetrics {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_cost_usd: f64,
    pub turns_count: usize,
}

#[derive(Clone)]
pub struct CostTracker {
    metrics: Arc<Mutex<SessionCostMetrics>>,
}

impl CostTracker {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(SessionCostMetrics::default())),
        }
    }

    /// Approximate token count from text using character heuristic (~3.8 chars per token for code/text)
    pub fn estimate_tokens(text: &str) -> usize {
        let chars = text.chars().count();
        if chars == 0 {
            0
        } else {
            (chars as f64 / 3.8).ceil() as usize
        }
    }

    /// Get price per million tokens (prompt, completion) in USD
    pub fn get_model_rates(model: &str) -> (f64, f64) {
        let m = model.to_lowercase();
        if m.contains("claude-3-5-sonnet") || m.contains("sonnet") {
            (3.00, 15.00)
        } else if m.contains("opus") {
            (15.00, 75.00)
        } else if m.contains("gpt-4o-mini") {
            (0.15, 0.60)
        } else if m.contains("gpt-4o") || m.contains("gpt-4") {
            (2.50, 10.00)
        } else if m.contains("gemini-2.0-flash") || m.contains("flash") {
            (0.10, 0.40)
        } else if m.contains("gemini-1.5-pro") || m.contains("pro") {
            (1.25, 5.00)
        } else if m.contains("ollama") || m.contains("local") {
            (0.00, 0.00)
        } else {
            // Default general rate (approx Gemini/GPT-4o mini)
            (0.50, 1.50)
        }
    }

    /// Record a turn and calculate cost
    pub fn record_turn(
        &self,
        prompt_text: &str,
        completion_text: &str,
        model: &str,
    ) -> (usize, usize, f64) {
        let p_tokens = Self::estimate_tokens(prompt_text);
        let c_tokens = Self::estimate_tokens(completion_text);
        let (p_rate, c_rate) = Self::get_model_rates(model);

        let turn_cost =
            (p_tokens as f64 / 1_000_000.0) * p_rate + (c_tokens as f64 / 1_000_000.0) * c_rate;

        if let Ok(mut m) = self.metrics.lock() {
            m.prompt_tokens += p_tokens;
            m.completion_tokens += c_tokens;
            m.total_cost_usd += turn_cost;
            m.turns_count += 1;
        }

        (p_tokens, c_tokens, turn_cost)
    }

    /// Retrieve session totals
    pub fn get_metrics(&self) -> SessionCostMetrics {
        self.metrics.lock().map(|m| m.clone()).unwrap_or_default()
    }
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_tracker_calculation() {
        let tracker = CostTracker::new();
        let prompt = "Implement a quicksort algorithm in Rust with generic type T.";
        let completion = "fn quicksort<T: Ord>(slice: &mut [T]) {}";

        let (p, c, cost) = tracker.record_turn(prompt, completion, "gemini-2.0-flash");
        assert!(p > 0);
        assert!(c > 0);
        assert!(cost > 0.0);

        let m = tracker.get_metrics();
        assert_eq!(m.turns_count, 1);
        assert_eq!(m.prompt_tokens, p);
        assert_eq!(m.completion_tokens, c);
    }
}
