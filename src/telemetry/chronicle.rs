//! Atlas Daedalus — Chronicle Telemetry Engine
//! High-precision local execution tracing, token computation, and cost attribution.
//! Appends structured spans directly to `.chronicle/traces.jsonl`.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt: usize,
    pub completion: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChronicleSpan {
    pub id: String,
    pub timestamp: String,
    pub provider: String,
    pub model: String,
    pub status: u16,
    pub latency_ms: u64,
    pub tokens: TokenUsage,
    pub cost_usd: f64,
    pub request_messages: usize,
    pub response_chars: usize,
    pub tool_calls_count: usize,
}

pub struct ChronicleTracer {
    trace_file: PathBuf,
}

impl ChronicleTracer {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        let base_dir = workspace_root.as_ref().join(".chronicle");
        let _ = fs::create_dir_all(&base_dir);
        let trace_file = base_dir.join("traces.jsonl");
        Self { trace_file }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_span(
        &self,
        provider: &str,
        model: &str,
        prompt_text: &str,
        completion_text: &str,
        tool_calls_count: usize,
        messages_count: usize,
        latency_ms: u64,
        status: u16,
    ) -> ChronicleSpan {
        let prompt_tokens = (prompt_text.len() / 4).max(1);
        let completion_tokens = completion_text.len() / 4;
        let total_tokens = prompt_tokens + completion_tokens;

        let m = model.to_lowercase();
        let (prompt_rate, completion_rate) =
            if m.contains("claude-3-5-sonnet") || m.contains("gpt-4o") {
                (3.0, 15.0)
            } else if m.contains("flash") || m.contains("mini") || m.contains("haiku") {
                (0.15, 0.6)
            } else if m.contains("ollama") || m.contains("local") {
                (0.0, 0.0)
            } else {
                (1.5, 4.5)
            };

        let cost_usd = ((prompt_tokens as f64 * prompt_rate)
            + (completion_tokens as f64 * completion_rate))
            / 1_000_000.0;

        let id = format!("tr_{:x}", Utc::now().timestamp_nanos_opt().unwrap_or(0));

        let span = ChronicleSpan {
            id,
            timestamp: Utc::now().to_rfc3339(),
            provider: provider.to_string(),
            model: model.to_string(),
            status,
            latency_ms,
            tokens: TokenUsage {
                prompt: prompt_tokens,
                completion: completion_tokens,
                total: total_tokens,
            },
            cost_usd,
            request_messages: messages_count,
            response_chars: completion_text.len(),
            tool_calls_count,
        };

        if let Ok(line) = serde_json::to_string(&span) {
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.trace_file)
            {
                let _ = writeln!(file, "{}", line);
            }
        }

        span
    }

    pub fn get_recent_traces(&self, limit: usize) -> Vec<ChronicleSpan> {
        if !self.trace_file.exists() {
            return Vec::new();
        }

        let content = match fs::read_to_string(&self.trace_file) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut spans = Vec::new();
        for line in content.lines().rev() {
            if spans.len() >= limit {
                break;
            }
            if let Ok(span) = serde_json::from_str::<ChronicleSpan>(line) {
                spans.push(span);
            }
        }
        spans
    }
}
