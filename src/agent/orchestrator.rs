//! Atlas Daedalus — Master Autonomous Orchestrator

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::agent::tester::SelfHealingVerifier;
use crate::context::compressor::{ChatMessage, TrajectoryCompressor};
use crate::context::memory::MemoryEngine;
use crate::providers::router::ProviderRouter;
use crate::providers::types::LLMMessage;
use crate::telemetry::chronicle::ChronicleTracer;
use crate::tools::registry::ToolRegistry;

pub enum AgentEvent<'a> {
    Thinking(&'a str),
    ToolStart { name: &'a str, args: &'a serde_json::Value },
    ToolEnd { name: &'a str, success: bool, output: &'a str },
    TestResult { command: &'a str, passed: bool, output: &'a str },
    Done { summary: &'a str, files_modified: &'a [String] },
    Error(&'a str),
}

pub struct DaedalusOrchestrator {
    pub workspace_root: PathBuf,
    pub provider: ProviderRouter,
    pub tools: ToolRegistry,
    pub memory: MemoryEngine,
    pub chronicle: ChronicleTracer,
    pub verifier: SelfHealingVerifier,
    pub compressor: TrajectoryCompressor,
    pub history: Vec<LLMMessage>,
}

impl DaedalusOrchestrator {
    pub fn new<P: AsRef<Path>>(
        workspace_root: P,
        override_model: Option<String>,
        override_provider: Option<String>,
    ) -> Self {
        let p = workspace_root.as_ref().to_path_buf();
        Self {
            provider: ProviderRouter::new(override_model, override_provider),
            tools: ToolRegistry::new(&p),
            memory: MemoryEngine::new(&p),
            chronicle: ChronicleTracer::new(&p),
            verifier: SelfHealingVerifier::new(&p),
            compressor: TrajectoryCompressor::new(60000),
            history: Vec::new(),
            workspace_root: p,
        }
    }

    fn build_system_prompt(&self) -> String {
        let mem = self.memory.get_memory_context();
        format!(
            "You are Atlas Daedalus (acode), a world-class autonomous terminal AI coding agent built by Atlas Research Labs.\n\n\
            CORE OPERATIONAL RULES:\n\
            1. ZERO EMOJI POLICY: NEVER output emojis anywhere. Use clean brackets or text labels like [OK], [FAIL], [PLAN], [TEST], [DIFF].\n\
            2. AUTONOMOUS VERIFICATION: When you make code modifications, always verify them. If a test or build fails, read the error output and repair the code until it passes.\n\
            3. SURGICAL PRECISION: Prefer patch_file over write_file when editing existing files to avoid wiping out code.\n\
            4. HONESTY AND CONCISENESS: Output clean, concise, high-density explanations.\n\
            5. LOCAL ARCHITECTURE: You have direct access to tools (view_file, patch_file, write_file, run_command, list_dir, search_code, git_status, git_diff, argus_audit, beacon_tunnel).\n\n\
            WORKSPACE ROOT: {}\n{}",
            self.workspace_root.display(),
            if mem.is_empty() { String::new() } else { format!("\nPROJECT MEMORY:\n{}", mem) }
        )
    }

    pub async fn run_task<F>(&mut self, user_goal: &str, mut on_event: F) -> Result<String, String>
    where
        F: FnMut(AgentEvent),
    {
        let start = Instant::now();
        let mut files_modified = HashSet::new();

        if self.history.is_empty() {
            self.history.push(LLMMessage {
                role: "system".to_string(),
                content: self.build_system_prompt(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });
        }

        self.history.push(LLMMessage {
            role: "user".to_string(),
            content: user_goal.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        let tool_defs = self.tools.get_definitions();
        let mut turns = 0;
        let max_turns = 25;
        let mut final_summary = String::new();

        while turns < max_turns {
            turns += 1;

            let thinking_status = format!("Autonomous turn {}...", turns);
            on_event(AgentEvent::Thinking(&thinking_status));

            // Compress history
            let chat_msgs: Vec<ChatMessage> = self.history.iter().map(|m| ChatMessage {
                role: m.role.clone(),
                content: m.content.clone(),
                tool_call_id: m.tool_call_id.clone(),
                name: m.name.clone(),
            }).collect();

            let compressed = self.compressor.compress(chat_msgs);
            if compressed.len() < self.history.len() {
                self.history = compressed.into_iter().map(|c| LLMMessage {
                    role: c.role,
                    content: c.content,
                    tool_calls: None,
                    tool_call_id: c.tool_call_id,
                    name: c.name,
                }).collect();
            }

            let res = self.provider.complete(&self.history, &tool_defs).await?;

            // Record Chronicle telemetry
            let prompt_text: String = self.history.iter().map(|m| m.content.as_str()).collect::<Vec<_>>().join(" ");
            let latency_ms = start.elapsed().as_millis() as u64;

            self.chronicle.record_span(
                &self.provider.config.provider,
                &self.provider.config.model,
                &prompt_text,
                &res.content,
                res.tool_calls.as_ref().map(|tc| tc.len()).unwrap_or(0),
                self.history.len(),
                latency_ms,
                200,
            );

            // Append assistant message
            self.history.push(LLMMessage {
                role: "assistant".to_string(),
                content: res.content.clone(),
                tool_calls: res.tool_calls.clone(),
                tool_call_id: None,
                name: None,
            });

            // If no tool calls, completion is reached
            let tool_calls = match res.tool_calls {
                Some(tc) if !tc.is_empty() => tc,
                _ => {
                    final_summary = res.content;
                    break;
                }
            };

            // Execute each tool call
            for call in &tool_calls {
                on_event(AgentEvent::ToolStart { name: &call.name, args: &call.arguments });

                if call.name == "write_file" || call.name == "patch_file" {
                    if let Some(p) = call.arguments.get("path").and_then(|v| v.as_str()) {
                        files_modified.insert(p.to_string());
                    }
                }

                let (success, output) = self.tools.execute_tool(&call.name, &call.arguments).await;
                on_event(AgentEvent::ToolEnd { name: &call.name, success, output: &output });

                self.history.push(LLMMessage {
                    role: "tool".to_string(),
                    content: output,
                    tool_calls: None,
                    tool_call_id: Some(call.id.clone()),
                    name: Some(call.name.clone()),
                });
            }
        }

        // Run self-healing verifier if files were modified
        if !files_modified.is_empty() {
            on_event(AgentEvent::Thinking("Running verification test suite..."));
            let verify_res = self.verifier.run_verification().await;

            if verify_res.has_tests {
                let cmd_str = verify_res.command.as_deref().unwrap_or("");
                on_event(AgentEvent::TestResult {
                    command: cmd_str,
                    passed: verify_res.passed,
                    output: &verify_res.output,
                });

                if !verify_res.passed {
                    // Trigger repair turn
                    self.history.push(LLMMessage {
                        role: "user".to_string(),
                        content: format!(
                            "Verification test failed:\nCommand: {}\nOutput:\n{}\nPlease inspect the error stack trace, fix the files, and ensure tests pass.",
                            cmd_str, verify_res.output
                        ),
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                    });

                    on_event(AgentEvent::Thinking("Self-healing repair loop in progress..."));
                    if let Ok(repair_res) = self.provider.complete(&self.history, &tool_defs).await {
                        if let Some(repair_calls) = repair_res.tool_calls {
                            for call in &repair_calls {
                                on_event(AgentEvent::ToolStart { name: &call.name, args: &call.arguments });
                                let (success, output) = self.tools.execute_tool(&call.name, &call.arguments).await;
                                on_event(AgentEvent::ToolEnd { name: &call.name, success, output: &output });
                            }
                        }
                    }
                }
            }
        }

        let modified_vec: Vec<String> = files_modified.into_iter().collect();
        on_event(AgentEvent::Done {
            summary: &final_summary,
            files_modified: &modified_vec,
        });

        Ok(final_summary)
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}
