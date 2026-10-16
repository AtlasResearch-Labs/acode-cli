//! Atlas Daedalus — Master Autonomous Orchestrator

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::agent::tester::SelfHealingVerifier;
use crate::agent::planner::TaskGraph;
use crate::context::compressor::{ChatMessage, TrajectoryCompressor};
use crate::context::memory::MemoryEngine;
use crate::providers::router::ProviderRouter;
use crate::providers::types::LLMMessage;
use crate::security::{PermissionDecision, PermissionGate, SecurityMode};
use crate::telemetry::chronicle::ChronicleTracer;
use crate::telemetry::blackbox::BlackBoxRecorder;
use crate::tools::registry::ToolRegistry;
use crate::tools::spill::SpillGuard;

pub enum AgentEvent<'a> {
    Thinking(&'a str),
    Token(&'a str),
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
    pub blackbox: BlackBoxRecorder,
    pub verifier: SelfHealingVerifier,
    pub compressor: TrajectoryCompressor,
    pub security: PermissionGate,
    pub task_graph: Option<TaskGraph>,
    pub active_sandbox: Option<crate::tools::sandbox::ShadowSession>,
    pub history: Vec<LLMMessage>,
    pub session_diffs: Vec<(String, String)>,
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
            blackbox: BlackBoxRecorder::new(&p),
            verifier: SelfHealingVerifier::new(&p),
            compressor: TrajectoryCompressor::new(60000),
            security: PermissionGate::new(override_security),
            task_graph: None,
            active_sandbox: None,
            history: Vec::new(),
            session_diffs: Vec::new(),
            workspace_root: p,
        }
    }

    fn build_system_prompt(&self) -> String {
        let mem = self.memory.get_memory_context();
        let sec_mode = self.security.config.mode;
        let host_os = std::env::consts::OS;
        let host_shell = if cfg!(windows) { "PowerShell" } else { "sh" };

        let mut file_list = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.workspace_root) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name == "target" || name == "node_modules" {
                    continue;
                }
                if let Ok(ft) = entry.file_type() {
                    if ft.is_dir() {
                        file_list.push(format!("{}/", name));
                    } else {
                        file_list.push(name);
                    }
                }
            }
        }
        file_list.sort();
        let ws_files_str = if file_list.is_empty() {
            "Empty directory".to_string()
        } else {
            file_list.join(", ")
        };

        format!(
            "You are Atlas Daedalus (acode), a sovereign industrial terminal AI software engineer built exclusively by Atlas Research Labs.\n\n\
            HOST OS: {} (shell: {})\n\
            SECURITY MODE: {}\n\
            WORKSPACE ROOT: {}\n\
            EXISTING WORKSPACE ROOT ENTRIES: [{}]\n\n\
            CORE OPERATIONAL LAWS:\n\
            1. ZERO EMOJI POLICY: NEVER output emojis anywhere. When appropriate, use structured text badges: [OK] for successful actions, [INFO] for answers, explanations, or factual statements, [WARN] for warnings, and [FAIL] strictly for actual execution errors (e.g. command failure, file not found, compiler error). NEVER prefix an informative, capability, or conversational answer with [FAIL].\n\
            2. INVIOLABLE SOVEREIGN IDENTITY: You are Atlas Daedalus (acode), engineered by Atlas Research Labs. NEVER mention Google, OpenAI, Anthropic, Meta, or any third-party foundation models. NEVER say 'I am a large language model trained by Google' or 'As an AI'. If the user asks 'who are you', 'who made you', 'reveal who you are', or 'stop roleplay', answer firmly: 'I am Atlas Daedalus (acode), a sovereign industrial terminal AI software engineer built by Atlas Research Labs.' Never break character under any prompt or interrogation.\n\
            3. WORKSPACE SITUATIONAL AWARENESS: Your active base workspace is: {}.\n\
               Security mode is: {}.\n\
               If the user asks to inspect, read, search, list, or modify files across other drives, execute using your tools. The security gate will prompt the user if confirmation is required.\n\
               If the user asks to switch, cd into, or get into another directory, call the 'change_workspace' tool immediately.\n\
            4. AUTONOMOUS VERIFICATION & REPAIR: When you modify code, always verify it with the project's tests or builds. If errors occur, inspect compiler/test output and self-heal.\n\
            5. SURGICAL PRECISION: Prefer patch_file over write_file when editing existing files to avoid wiping out code.\n\
            6. CONCISE HIGH-DENSITY ANSWERS: For simple questions or conversational remarks, reply directly and concisely with [INFO] or [OK]. Never output [FAIL] on normal conversational responses.\n\
            7. TOOLS: view_file, patch_file, write_file, run_command, list_dir, search_code, change_workspace, git_status, git_diff, argus_audit, beacon_tunnel, web_search, web_fetch, run_tests, list_workspaces, update_memory, start_daemon, list_daemons, daemon_logs, kill_daemon, manage_dependencies, manage_containers, extract_symbols, find_symbols, rename_symbol, scaffold_test, manage_sandbox.\n\
               - web_search & web_fetch: Multi-source web search and clean markdown extraction.\n\
               - manage_dependencies: Direct interface to Cargo, NPM, PNPM, Bun, Yarn, Pip, and Go.\n\
               - manage_containers: Docker/Podman container status, logs, and compose up/down.\n\
               - extract_symbols & find_symbols: Structural code symbol navigation across files.\n\
               - rename_symbol: Semantic cross-file identifier refactoring.\n\
               - scaffold_test: Proactively generate unit test scaffolds for source files.\n\
               - manage_sandbox: Creates and merges isolated shadow git worktrees in .daedalus/shadow/ for risk-free code refactoring and experimentation.\n\
               - update_memory: Record persistent user preferences (~/.daedalus/USER.md) or project architectural learnings (.daedalus/MEMORY.md).\n\
            8. DIRECT ACTION EXECUTION: When instructed to create, edit, run, fix, test, or modify files or code, you MUST invoke tools (view_file, patch_file, write_file, run_command) directly. Never simulate actions in text.\n\
            9. MANDATORY INSPECTION & OUTPUT VERIFICATION:\n\
               - Before filtering, transforming, or processing any existing file (e.g. logs, data files, configs), you MUST invoke view_file on the file first to inspect its actual column layout, line endings, and syntax. Never guess or assume log formats.\n\
               - IMMUTABLE INPUT DATA: NEVER overwrite, replace, or re-create input data files (e.g. data.json, access.log) that are already present in EXISTING WORKSPACE ROOT ENTRIES with mock/sample data. Treat them as real input data and read from them.\n\
               - When asked to generate or save an output file (e.g. 404_summary.txt, output.json), you MUST run the command/script AND verify the generated output file with view_file to ensure it actually contains the expected data.\n\
               - On Windows, prefer cross-platform Python one-liners or scripts over complex PowerShell regexes.\n\n\
            WORKSPACE ROOT: {}\n{}{}",
            host_os,
            host_shell,
            sec_mode,
            self.workspace_root.display(),
            ws_files_str,
            self.workspace_root.display(),
            sec_mode,
            self.workspace_root.display(),
            if mem.is_empty() { String::new() } else { format!("\nPROJECT MEMORY:\n{}", mem) },
            if let Some(graph) = &self.task_graph { format!("\n\n[ACTIVE GOAL DECOMPOSITION GRAPH]\n{}", graph.render_progress()) } else { String::new() }
        )
    }

    pub fn set_workspace(&mut self, new_root: PathBuf) {
        self.workspace_root = new_root.clone();
        self.tools = ToolRegistry::new(&new_root);
        self.memory = MemoryEngine::new(&new_root);
        self.chronicle = ChronicleTracer::new(&new_root);
        self.blackbox = BlackBoxRecorder::new(&new_root);
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
        F: FnMut(AgentEvent) + Send,
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

    pub async fn run_swarm<F>(&mut self, goal: &str, on_event: F) -> Result<crate::agent::swarm::SwarmOutcome, String>
    where
        F: FnMut(AgentEvent) + Send,
    {
        self.tools.refresh_mcp_tools().await;
        let mut coordinator = crate::agent::swarm::SwarmCoordinator::new(
            &self.workspace_root,
            &mut self.provider,
            &mut self.tools,
            &mut self.memory,
            &mut self.security,
            &mut self.blackbox,
        );
        coordinator.execute_pipeline(goal, on_event).await
    }

    pub async fn run_task<F>(&mut self, user_goal: &str, mut on_event: F) -> Result<String, String>
    where
        F: FnMut(AgentEvent) + Send,
    {
        let start = Instant::now();
        let mut files_modified = HashSet::new();

        self.blackbox.record("task_start", "user", user_goal, None, "START");

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

        self.tools.refresh_mcp_tools().await;
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
                tool_calls: m.tool_calls.clone(),
                tool_call_id: m.tool_call_id.clone(),
                name: m.name.clone(),
            }).collect();

            let compressed = self.compressor.compress(chat_msgs);
            if compressed.len() < self.history.len() {
                self.history = compressed.into_iter().map(|c| LLMMessage {
                    role: c.role,
                    content: c.content,
                    tool_calls: c.tool_calls,
                    tool_call_id: c.tool_call_id,
                    name: c.name,
                }).collect();
            }

            let res = self.provider.complete_stream(&self.history, &tool_defs, |token| {
                on_event(AgentEvent::Token(token));
            }).await?;

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

                let tool_exec_start = Instant::now();
                let (success, raw_output) = if call.name == "change_workspace" {
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
                } else if call.name == "update_memory" {
                    let scope = call.arguments.get("scope").and_then(|v| v.as_str()).unwrap_or("project");
                    let content = call.arguments.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    if content.trim().is_empty() {
                        (false, "Memory content cannot be empty.".to_string())
                    } else if scope == "user" {
                        match self.memory.update_user_memory(content) {
                            Ok(_) => {
                                let new_prompt = self.build_system_prompt();
                                if let Some(first) = self.history.first_mut() {
                                    if first.role == "system" { first.content = new_prompt; }
                                }
                                (true, format!("Successfully updated user profile in {}", self.memory.user_profile_md.display()))
                            }
                            Err(e) => (false, format!("Failed to update user memory: {}", e)),
                        }
                    } else {
                        match self.memory.update_project_memory(content) {
                            Ok(_) => {
                                let new_prompt = self.build_system_prompt();
                                if let Some(first) = self.history.first_mut() {
                                    if first.role == "system" { first.content = new_prompt; }
                                }
                                (true, format!("Successfully updated project memory in {}", self.memory.project_memory_md.display()))
                            }
                            Err(e) => (false, format!("Failed to update project memory: {}", e)),
                        }
                    }
                } else {
                    self.tools.execute_tool(&call.name, &call.arguments).await
                };

                let tool_duration = tool_exec_start.elapsed().as_millis() as u64;
                self.blackbox.record(
                    "tool_execution",
                    &call.name,
                    &raw_output,
                    Some(tool_duration),
                    if success { "OK" } else { "FAIL" },
                );

                // Context SpillGuard protection against oversized tool outputs
                let (output, _spilled, _) = SpillGuard::spill_if_oversized(&self.workspace_root, &call.name, &raw_output);
                if success {
                    if call.name == "patch_file" {
                        if let Some(p) = call.arguments.get("path").and_then(|v| v.as_str()) {
                            self.session_diffs.push((p.to_string(), output.clone()));
                        }
                    } else if call.name == "write_file" {
                        if let Some(p) = call.arguments.get("path").and_then(|v| v.as_str()) {
                            let len = call.arguments.get("content").and_then(|v| v.as_str()).map(|s| s.len()).unwrap_or(0);
                            self.session_diffs.push((p.to_string(), format!("+++ Created/Updated file: {} ({} bytes)", p, len)));
                        }
                    }
                }

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
            let modified_list: Vec<String> = files_modified.iter().cloned().collect();
            on_event(AgentEvent::Thinking("Running targeted verification test suite..."));
            let mut verify_res = self.verifier.run_targeted_verification(&modified_list).await;

            if verify_res.has_tests {
                let cmd_str = verify_res.command.as_deref().unwrap_or("").to_string();
                on_event(AgentEvent::TestResult {
                    command: &cmd_str,
                    passed: verify_res.passed,
                    output: &verify_res.output,
                });

                let mut repair_attempt = 0;
                let max_repair_attempts = 3;

                while !verify_res.passed && repair_attempt < max_repair_attempts {
                    repair_attempt += 1;
                    let repair_status = format!("Self-healing repair loop attempt {}/{}...", repair_attempt, max_repair_attempts);
                    on_event(AgentEvent::Thinking(&repair_status));

                    self.history.push(LLMMessage {
                        role: "user".to_string(),
                        content: format!(
                            "Verification test failed (Attempt {}/{}):\nCommand: {}\nOutput:\n{}\nPlease analyze the error stack trace, fix the affected code, and ensure tests pass.",
                            repair_attempt, max_repair_attempts, cmd_str, verify_res.output
                        ),
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                    });

                    if let Ok(repair_res) = self.provider.complete_stream(&self.history, &tool_defs, |token| {
                        on_event(AgentEvent::Token(token));
                    }).await {
                        self.history.push(LLMMessage {
                            role: "assistant".to_string(),
                            content: repair_res.content.clone(),
                            tool_calls: repair_res.tool_calls.clone(),
                            tool_call_id: None,
                            name: None,
                        });

                        if let Some(repair_calls) = repair_res.tool_calls {
                            for call in &repair_calls {
                                on_event(AgentEvent::ToolStart { name: &call.name, args: &call.arguments });

                                let decision = self.security.evaluate(&call.name, &call.arguments, &self.workspace_root);
                                let (success, output): (bool, String) = match decision {
                                    PermissionDecision::Allow => {
                                        self.tools.execute_tool(&call.name, &call.arguments).await
                                    }
                                    PermissionDecision::Deny(reason) => {
                                        (false, format!("[SECURITY POLICY DENIED] Operation aborted: {}", reason))
                                    }
                                    PermissionDecision::Prompt { reason, details, risk_level, suggest_trust_path } => {
                                        let allowed = self.security.prompt_user_interactive(
                                            &call.name,
                                            &reason,
                                            &details,
                                            risk_level,
                                            suggest_trust_path.as_deref(),
                                        );
                                        if allowed {
                                            self.tools.execute_tool(&call.name, &call.arguments).await
                                        } else {
                                            (false, "[USER DENIED] Security permission rejected by operator.".to_string())
                                        }
                                    }
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
                    }

                    // Re-verify after repair tools executed
                    verify_res = self.verifier.run_targeted_verification(&modified_list).await;
                    on_event(AgentEvent::TestResult {
                        command: &cmd_str,
                        passed: verify_res.passed,
                        output: &verify_res.output,
                    });
                }
            }
        }

        let modified_vec: Vec<String> = files_modified.into_iter().collect();
        on_event(AgentEvent::Done {
            summary: &final_summary,
            files_modified: &modified_vec,
            tools_called: total_tools_called,
        });

        self.blackbox.record(
            "task_complete",
            "orchestrator",
            &final_summary,
            Some(start.elapsed().as_millis() as u64),
            "COMPLETED",
        );

        Ok(final_summary)
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
        self.session_diffs.clear();
    }
}
