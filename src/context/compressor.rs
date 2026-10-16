//! Atlas Daedalus — Trajectory Compressor

use crate::providers::types::ToolCall;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
}

pub struct TrajectoryCompressor {
    pub max_tokens: usize,
}

impl TrajectoryCompressor {
    pub fn new(max_tokens: usize) -> Self {
        Self { max_tokens }
    }

    pub fn estimate_tokens(text: &str) -> usize {
        (text.len() / 4).max(1)
    }

    /// Safely truncates a string at a UTF-8 character boundary
    pub fn safe_truncate(s: &str, max_chars: usize) -> &str {
        match s.char_indices().nth(max_chars) {
            Some((idx, _)) => &s[..idx],
            None => s,
        }
    }

    /// Safely slices the trailing N characters of a string at a UTF-8 character boundary
    pub fn safe_slice_end(s: &str, char_count: usize) -> &str {
        let total_chars = s.chars().count();
        if total_chars <= char_count {
            return s;
        }
        let skip = total_chars - char_count;
        match s.char_indices().nth(skip) {
            Some((idx, _)) => &s[idx..],
            None => s,
        }
    }

    pub fn compress(&self, messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
        let total_tokens: usize = messages
            .iter()
            .map(|m| Self::estimate_tokens(&m.content))
            .sum();
        if total_tokens <= self.max_tokens || messages.len() <= 8 {
            return messages;
        }

        let system_msg = messages.first().cloned();
        let mut split_idx = messages.len().saturating_sub(6);

        // Protocol integrity: ensure we do not split between an assistant tool_call and its tool response
        while split_idx < messages.len() && messages[split_idx].role == "tool" {
            if split_idx > 1 {
                split_idx -= 1;
            } else {
                break;
            }
        }

        let middle = &messages[1..split_idx];
        let recent = &messages[split_idx..];

        let mut compressed = Vec::new();
        if let Some(sys) = system_msg {
            compressed.push(sys);
        }

        for msg in middle {
            if msg.role == "tool" && msg.content.chars().count() > 600 {
                let head = Self::safe_truncate(&msg.content, 300);
                let tail = Self::safe_slice_end(&msg.content, 150);
                let truncated =
                    format!("{}\n... [TRUNCATED FOR CONTEXT BUDGET] ...\n{}", head, tail);
                compressed.push(ChatMessage {
                    role: msg.role.clone(),
                    content: truncated,
                    tool_calls: msg.tool_calls.clone(),
                    tool_call_id: msg.tool_call_id.clone(),
                    name: msg.name.clone(),
                });
            } else {
                compressed.push(msg.clone());
            }
        }

        compressed.extend_from_slice(recent);
        compressed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_truncate_non_ascii() {
        // Multi-byte characters (Japanese, Greek, emojis)
        let s = "こんにちは世界！Hello world. 🚀 Rust sovereign.";
        let truncated = TrajectoryCompressor::safe_truncate(s, 5);
        assert_eq!(truncated, "こんにちは");

        let tail = TrajectoryCompressor::safe_slice_end(s, 10);
        assert_eq!(tail, "sovereign.");
    }
}
