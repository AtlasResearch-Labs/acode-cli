//! Atlas Daedalus — Trajectory Compressor

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
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

    pub fn compress(&self, messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
        let total_tokens: usize = messages.iter().map(|m| Self::estimate_tokens(&m.content)).sum();
        if total_tokens <= self.max_tokens || messages.len() <= 8 {
            return messages;
        }

        let system_msg = messages.first().cloned();
        let split_idx = messages.len().saturating_sub(6);

        let middle = &messages[1..split_idx];
        let recent = &messages[split_idx..];

        let mut compressed = Vec::new();
        if let Some(sys) = system_msg {
            compressed.push(sys);
        }

        for msg in middle {
            if msg.role == "tool" && msg.content.len() > 600 {
                let truncated = format!(
                    "{}\n... [TRUNCATED FOR CONTEXT BUDGET] ...\n{}",
                    &msg.content[..300],
                    &msg.content[msg.content.len() - 150..]
                );
                compressed.push(ChatMessage {
                    role: msg.role.clone(),
                    content: truncated,
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
