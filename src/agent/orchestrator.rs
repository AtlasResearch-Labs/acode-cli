//! Atlas Daedalus — Master Autonomous Orchestrator

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::agent::tester::SelfHealingVerifier;
use crate::context::compressor::{ChatMessage, TrajectoryCompressor};
use crate::context::memory::MemoryEngine;
use crate::providers::router::ProviderRouter;
use crate::providers::types::LLMMessage;
use crate::security::{PermissionDecision, PermissionGate, SecurityMode};
use crate::telemetry::chronicle::ChronicleTracer;
use crate::tools::registry::ToolRegistry;

pub enum AgentEvent<'a> {
    Thinking(&'a str),
    ToolStart { name: &'a str, args: &'a serde_json::Value },
    ToolEnd { name: &'a str, success: bool, output: &'a str },
    TestResult { command: &'a str, passed: bool, output: &'a str },
    Done { summary: &'a str, files_modified: &'a [String], tools_called: usize },
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
    pub security: PermissionGate,
    pub history: Vec<LLMMessage>,
}

impl DaedalusOrchestrator {
    pub fn new<P: AsRef<Path>>(
        workspace_root: P,
        override_model: Option<String>,
        override_provider: Option<String>,
        override_security: Option<SecurityMode>,
    ) -> Self {
        let p = workspace_root.as_ref().to_path_buf();
        Self {
            provider: ProviderRouter::new(override_model, override_provider),
            tools: ToolRegistry::new(&p),
            memory: MemoryEngine::new(&p),
            chronicle: ChronicleTracer::new(&p),
            verifier: SelfHealingVerifier::new(&p),
            compressor: TrajectoryCompressor::new(60000),
            security: PermissionGate::new(override_security),
            history: Vec::new(),
            workspace_root: p,
        }
    }

    fn build_system_prompt(&self) -> String {
        let mem = self.memory.get_memory_context();
        let sec_mode = self.security.config.mode;
        format!(
            "You are Atlas Daedalus (acode), a sovereign industrial terminal AI software engineer built by Atlas Research Labs.\n\n\
            SECURITY MODE: {}\n\
            CORE OPERATIONAL LAWS:\n\
            1. ZERO EMOJI POLICY: NEVER output emojis anywhere. Use clean brackets or text labels like [OK], [FAIL], [PLAN], [TEST], [DIFF].\n\
            2. SOVEREIGN DEMEANOR: NEVER say 'I am an AI language model', 'As an AI', or give generic disclaimers. You are an autonomous software engineer running directly on the user's terminal.\n\
            3. WORKSPACE SITUATIONAL AWARENESS: Your active base workspace is: {}.\n\
               Security mode is: {}.\n\
               If the user asks to inspect, read, search, list, or modify files across other drives, execute using your tools. The security gate will prompt the user if confirmation is required.\n\
               If the user asks to switch, cd into, or get into another directory, call the 'change_workspace' tool immediately.\n\
            4. AUTONOMOUS VERIFICATION & REPAIR: When you modify code, always verify it with the project's tests or builds. If errors occur, inspect compiler/test output and self-heal.\n\
            5. SURGICAL PRECISION: Prefer patch_file over write_file when editing existing files to avoid wiping out code.\n\
            6. CONCISE HIGH-DENSITY ANSWERS: For simple questions or conversational remarks, reply directly and concisely without unnecessary filler or tool overhead.\n\
            7. TOOLS: view_file, patch_file, write_file, run_command, list_dir, search_code, change_workspace, git_status, git_diff, argus_audit, beacon_tunnel.\n\n\
            WORKSPACE ROOT: {}\n{}",
            sec_mode,
            self.workspace_root.display(),
            sec_mode,
            self.workspace_root.display(),
            if mem.is_empty() { String::new() } else { format!("\nPROJECT MEMORY:\n{}", mem) }
        )
    }

    pub fn set_workspace(&mut self, new_root: PathBuf) {
        self.workspace_root = new_root.clone();
        self.tools = ToolRegistry::new(&new_root);
        self.memory = MemoryEngine::new(&new_root);
        self.chronicle = ChronicleTracer::new(&new_root);
        self.verifier = SelfHealingVerifier::new(&new_root);
        self.compressor = TrajectoryCompressor::new(60000);

        let new_prompt = self.build_system_prompt();
        if let Some(first) = self.history.first_mut() {
            if first.role == "system" {
                first.content = new_prompt;
            }
        }
    }

    pub async fn run_plan<F>(&mut self, goal: &str, on_event: F) -> Result<String, String>
    where
        F: FnMut(AgentEvent),
    {
        let prompt = format!(
            "ARCHITECT PLANNING MODE:\n\
            Analyze the following goal and produce a comprehensive, structured implementation plan.\n\
            DO NOT modify any files. Use read-only tools (view_file, search_code, list_dir, git_status, git_diff) to inspect the codebase as needed.\n\n\
            Structure your output as:\n\
            [PLAN: ARCHITECT BLUEPRINT]\n\
            1. OBJECTIVE & ARCHITECTURAL OVERVIEW\n\
            2. AFFECTED FILES & DEPENDENCIES\n\
            3. STEP-BY-STEP IMPLEMENTATION MILESTONES\n\
            4. TEST & VERIFICATION STRATEGY\n\
            5. POTENTIAL RISKS & EDGE CASES\n\n\
            GOAL: {}",
            goal
        );

        self.run_task(&prompt, on_event).await
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
        let mut total_tools_called = 0;

        while turns < max_turns {
            turns += 1;

            if turns > 1 {
                let thinking_status = format!("Autonomous turn {}...", turns);
                on_event(AgentEvent::Thinking(&thinking_status));
            }

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

            total_tools_called += tool_calls.len();
            if turns == 1 {
                on_event(AgentEvent::Thinking("Autonomous execution turn 1..."));
            }

            // Execute each tool call
            for call in &tool_calls {
                on_event(AgentEvent::ToolStart { name: &call.name, args: &call.arguments });

                // Security gate verification
                let decision = self.security.evaluate(&call.name, &call.arguments, &self.workspace_root);
                match decision {
                    PermissionDecision::Allow => {}
                    PermissionDecision::Deny(reason) => {
                        let output = format!("[SECURITY POLICY DENIED] Operation aborted: {}", reason);
                        on_event(AgentEvent::ToolEnd { name: &call.name, success: false, output: &output });
                        self.history.push(LLMMessage {
                            role: "tool".to_string(),
                            content: output,
                            tool_calls: None,
                            tool_call_id: Some(call.id.clone()),
                            name: Some(call.name.clone()),
                        });
                        continue;
                    }
                    PermissionDecision::Prompt { reason, details, risk_level, suggest_trust_path } => {
                        let allowed = self.security.prompt_user_interactive(
                            &call.name,
                            &reason,
                            &details,
                            risk_level,
                            suggest_trust_path.as_deref(),
                        );
                        if !allowed {
                            let output = format!("[SECURITY CHALLENGE DENIED] User declined permission to execute: {}", details);
                            on_event(AgentEvent::ToolEnd { name: &call.name, success: false, output: &output });
                            self.history.push(LLMMessage {
                                role: "tool".to_string(),
                                content: output,
                                tool_calls: None,
                                tool_call_id: Some(call.id.clone()),
                                name: Some(call.name.clone()),
                            });
                            continue;
                        }
                    }
                }

                if call.name == "write_file" || call.name == "patch_file" {
                    if let Some(p) = call.arguments.get("path").and_then(|v| v.as_str()) {
                        files_modified.insert(p.to_string());
                    }
                }

                let (success, output) = if call.name == "change_workspace" {
                    let target_str = call.arguments.get("path")
                        .or_else(|| call.arguments.get("dir"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let target = std::path::Path::new(target_str);
                    let resolved = if target.is_absolute() {
                        target.to_path_buf()
                    } else {
                        self.workspace_root.join(target)
                    };

                    if !resolved.exists() {
                        (false, format!("Directory does not exist: {}", target_str))
                    } else if !resolved.is_dir() {
                        (false, format!("Path is not a directory: {}", target_str))
                    } else {
                        let final_path = resolved.canonicalize().unwrap_or(resolved);
                        self.set_workspace(final_path.clone());
                        let git_status = self.tools.git.get_status().await;
                        (true, format!("Successfully switched active workspace root to: {} (Git branch: {}, clean: {})", final_path.display(), git_status.branch, git_status.clean))
                    }
                } else {
                    self.tools.execute_tool(&call.name, &call.arguments).await
                };
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
            tools_called: total_tools_called,
        });

        Ok(final_summary)
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}
