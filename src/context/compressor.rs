//! Atlas Daedalus — Trajectory Compressor

use crate::providers::types::ToolCall;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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

    /// Generates a structured semantic summary of intermediate execution turns,
    /// capturing workspace files, tool operations, errors, and intermediate reasoning.
    pub fn generate_semantic_summary(messages: &[ChatMessage]) -> String {
        let mut actions = Vec::new();
        let mut files_touched = BTreeSet::new();
        let mut last_assistant_thought = String::new();

        for msg in messages {
            match msg.role.as_str() {
                "assistant" => {
                    if let Some(tool_calls) = &msg.tool_calls {
                        for tc in tool_calls {
                            let name = &tc.name;
                            let args_summary = match name.as_str() {
                                "view_file" | "write_file" | "patch_file" => {
                                    if let Some(path) =
                                        tc.arguments.get("path").and_then(|v| v.as_str())
                                    {
                                        files_touched.insert(path.to_string());
                                        format!("file: {}", path)
                                    } else {
                                        "file".to_string()
                                    }
                                }
                                "run_command" | "run_tests" => {
                                    if let Some(cmd) =
                                        tc.arguments.get("command").and_then(|v| v.as_str())
                                    {
                                        format!("cmd: {}", Self::safe_truncate(cmd, 50))
                                    } else {
                                        "command".to_string()
                                    }
                                }
                                _ => format!("call {}", name),
                            };
                            actions.push(format!("{}({})", name, args_summary));
                        }
                    }
                    if !msg.content.trim().is_empty() {
                        last_assistant_thought =
                            Self::safe_truncate(msg.content.trim(), 160).to_string();
                    }
                }
                "tool" => {
                    let is_error = msg.content.contains("error")
                        || msg.content.contains("failed")
                        || msg.content.contains("FAIL");
                    if is_error {
                        let first_line = msg.content.lines().next().unwrap_or("error");
                        actions.push(format!(
                            "result: error ({})",
                            Self::safe_truncate(first_line, 80)
                        ));
                    }
                }
                _ => {}
            }
        }

        let mut summary = String::from("[COMPACTED EXECUTION TRAJECTORY SUMMARY]\n");
        if !files_touched.is_empty() {
            summary.push_str(&format!(
                "- Files modified/referenced: {}\n",
                files_touched.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
        if !actions.is_empty() {
            summary.push_str("- Prior sequence of actions:\n");
            for act in actions.iter().take(12) {
                summary.push_str(&format!("  * {}\n", act));
            }
            if actions.len() > 12 {
                summary.push_str(&format!(
                    "  * ... (and {} more operations)\n",
                    actions.len() - 12
                ));
            }
        }
        if !last_assistant_thought.is_empty() {
            summary.push_str(&format!(
                "- Intermediate reasoning state: \"{}\"\n",
                last_assistant_thought
            ));
        }
        summary.push_str("[END COMPACTED TRAJECTORY - Recent detailed context follows]");
        summary
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
        let user_goal = if messages.len() > 1 && messages[1].role == "user" {
            Some(messages[1].clone())
        } else {
            None
        };

        // Determine slice point for recent messages (preserving last 6 turns)
        let mut split_idx = messages.len().saturating_sub(6);

        // Protocol integrity: ensure we do not split between an assistant tool_call and its tool response
        while split_idx < messages.len() && messages[split_idx].role == "tool" {
            if split_idx > 2 {
                split_idx -= 1;
            } else {
                break;
            }
        }

        let middle_start = if user_goal.is_some() { 2 } else { 1 };
        let mut compressed = Vec::new();

        if let Some(sys) = system_msg {
            compressed.push(sys);
        }
        if let Some(goal) = user_goal {
            compressed.push(goal);
        }

        // If there are intermediate turns to compact, generate semantic summary
        if split_idx > middle_start {
            let middle = &messages[middle_start..split_idx];
            let summary_text = Self::generate_semantic_summary(middle);
            compressed.push(ChatMessage {
                role: "user".to_string(),
                content: summary_text,
                tool_calls: None,
                tool_call_id: None,
                name: Some("trajectory_summary".to_string()),
            });
        }

        // Truncate any oversized tool messages in recent turns to protect budget
        for msg in &messages[split_idx..] {
            if msg.role == "tool" && msg.content.chars().count() > 800 {
                let head = Self::safe_truncate(&msg.content, 400);
                let tail = Self::safe_slice_end(&msg.content, 200);
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

        compressed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_safe_truncate_non_ascii() {
        // Multi-byte characters (Japanese, Greek, emojis)
        let s = "こんにちは世界！Hello world. 🚀 Rust sovereign.";
        let truncated = TrajectoryCompressor::safe_truncate(s, 5);
        assert_eq!(truncated, "こんにちは");

        let tail = TrajectoryCompressor::safe_slice_end(s, 10);
        assert_eq!(tail, "sovereign.");
    }

    #[test]
    fn test_semantic_trajectory_summarization() {
        let compressor = TrajectoryCompressor::new(100); // Low token threshold to trigger compression

        let mut messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You are Daedalus.".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            ChatMessage {
                role: "user".to_string(),
                content: "Fix the bug in src/main.rs".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ];

        // Add 10 intermediate turns
        for i in 0..10 {
            messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: format!("Turn {}: reading files", i),
                tool_calls: Some(vec![ToolCall {
                    id: format!("call_{}", i),
                    name: "view_file".to_string(),
                    arguments: json!({ "path": format!("src/file_{}.rs", i) }),
                }]),
                tool_call_id: None,
                name: None,
            });
            messages.push(ChatMessage {
                role: "tool".to_string(),
                content: format!("File content of file {}", i),
                tool_calls: None,
                tool_call_id: Some(format!("call_{}", i)),
                name: Some("view_file".to_string()),
            });
        }

        let compressed = compressor.compress(messages);
        // Verify system message and original user goal are preserved
        assert_eq!(compressed[0].role, "system");
        assert_eq!(compressed[1].role, "user");
        assert_eq!(compressed[1].content, "Fix the bug in src/main.rs");

        // Verify that a semantic summary turn was inserted
        let has_summary = compressed.iter().any(|m| {
            m.content
                .contains("[COMPACTED EXECUTION TRAJECTORY SUMMARY]")
        });
        assert!(
            has_summary,
            "Compressed messages must include structured semantic summary"
        );

        // Verify summary contains files referenced
        let summary_msg = compressed
            .iter()
            .find(|m| {
                m.content
                    .contains("[COMPACTED EXECUTION TRAJECTORY SUMMARY]")
            })
            .unwrap();
        assert!(summary_msg.content.contains("src/file_"));
    }
}
