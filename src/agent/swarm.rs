//! Atlas Daedalus — Multi-Agent Swarm & Hierarchical Delegation Engine
//! (C) 2026 Atlas Research Labs — MIT License
//! Coordinates specialized sub-agents: Explorer (Reconnaissance), Coder (Implementation),
//! and Verifier (Quality Assurance & Test Self-Healing).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::agent::orchestrator::AgentEvent;
use crate::context::memory::MemoryEngine;
use crate::providers::router::ProviderRouter;
use crate::providers::types::LLMMessage;
use crate::security::{PermissionDecision, PermissionGate};
use crate::telemetry::blackbox::BlackBoxRecorder;
use crate::tools::registry::{ToolDefinition, ToolRegistry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubAgentRole {
    Explorer,
    Coder,
    Verifier,
}

impl SubAgentRole {
    pub fn name(&self) -> &'static str {
        match self {
            SubAgentRole::Explorer => "EXPLORER",
            SubAgentRole::Coder => "CODER",
            SubAgentRole::Verifier => "VERIFIER",
        }
    }

    pub fn system_instructions(&self, workspace_root: &Path) -> String {
        match self {
            SubAgentRole::Explorer => format!(
                "You are the EXPLORER sub-agent in the Atlas Daedalus sovereign swarm.\n\
                WORKSPACE: {}\n\
                MISSION: Perform read-only codebase reconnaissance, dependency mapping, symbol discovery, and architectural analysis.\n\
                CONSTRAINTS:\n\
                - DO NOT attempt to edit, patch, or write files.\n\
                - Use view_file, list_dir, search_code, extract_symbols, find_symbols, git_status, git_diff, and web_search.\n\
                - Output a crisp, structured reconnaissance brief listing relevant files, exact line numbers, architectural patterns, and implementation guidance for the Coder agent.\n\
                - Zero emojis. Use structured tags like [INFO] and [RECON].",
                workspace_root.display()
            ),
            SubAgentRole::Coder => format!(
                "You are the CODER sub-agent in the Atlas Daedalus sovereign swarm.\n\
                WORKSPACE: {}\n\
                MISSION: Implement surgical, minimal-diff code changes to achieve the objective based on the reconnaissance brief.\n\
                CONSTRAINTS:\n\
                - Prefer patch_file over write_file to avoid erasing existing code.\n\
                - Follow the codebase's existing conventions and style.\n\
                - Modify only what is necessary.\n\
                - Zero emojis. Output structured badges like [OK], [MODIFIED], [FAIL].",
                workspace_root.display()
            ),
            SubAgentRole::Verifier => format!(
                "You are the VERIFIER sub-agent in the Atlas Daedalus sovereign swarm.\n\
                WORKSPACE: {}\n\
                MISSION: Execute project tests, run build checks, run Argus AST security audits, and validate correctness.\n\
                CONSTRAINTS:\n\
                - Run run_tests, run_command, and argus_audit.\n\
                - If verification passes, confirm with [VERIFICATION PASSED].\n\
                - If verification fails, diagnose the exact failure, culprit file, and lines so the Coder can heal.\n\
                - Zero emojis.",
                workspace_root.display()
            ),
        }
    }

    pub fn filter_tool_definitions(&self, all_defs: &[ToolDefinition]) -> Vec<ToolDefinition> {
        let allowed: &[&str] = match self {
            SubAgentRole::Explorer => &[
                "view_file",
                "list_dir",
                "search_code",
                "extract_symbols",
                "find_symbols",
                "git_status",
                "git_diff",
                "web_search",
                "web_fetch",
                "list_workspaces",
            ],
            SubAgentRole::Coder => &[
                "view_file",
                "patch_file",
                "write_file",
                "search_code",
                "list_dir",
                "extract_symbols",
                "find_symbols",
                "manage_dependencies",
                "run_command",
            ],
            SubAgentRole::Verifier => &[
                "run_tests",
                "run_command",
                "view_file",
                "argus_audit",
                "git_status",
                "git_diff",
            ],
        };

        all_defs
            .iter()
            .filter(|d| allowed.contains(&d.name.as_str()) || d.name.starts_with("mcp__"))
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmOutcome {
    pub goal: String,
    pub explorer_brief: String,
    pub coder_summary: String,
    pub verifier_report: String,
    pub files_modified: Vec<String>,
    pub verification_passed: bool,
    pub duration_ms: u64,
}

pub struct SwarmCoordinator<'a> {
    pub workspace_root: &'a PathBuf,
    pub provider: &'a mut ProviderRouter,
    pub tools: &'a mut ToolRegistry,
    pub memory: &'a mut MemoryEngine,
    pub security: &'a mut PermissionGate,
    pub blackbox: &'a mut BlackBoxRecorder,
}

impl<'a> SwarmCoordinator<'a> {
    pub fn new(
        workspace_root: &'a PathBuf,
        provider: &'a mut ProviderRouter,
        tools: &'a mut ToolRegistry,
        memory: &'a mut MemoryEngine,
        security: &'a mut PermissionGate,
        blackbox: &'a mut BlackBoxRecorder,
    ) -> Self {
        Self {
            workspace_root,
            provider,
            tools,
            memory,
            security,
            blackbox,
        }
    }

    /// Run single specialized sub-agent
    pub async fn run_subagent<F>(
        &mut self,
        role: SubAgentRole,
        objective: &str,
        max_turns: usize,
        mut on_event: F,
    ) -> Result<(String, Vec<String>), String>
    where
        F: FnMut(AgentEvent) + Send,
    {
        self.blackbox
            .record("subagent_start", role.name(), objective, None, "START");

        let system_prompt = role.system_instructions(self.workspace_root);
        let mut history = vec![
            LLMMessage {
                role: "system".to_string(),
                content: system_prompt,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            LLMMessage {
                role: "user".to_string(),
                content: objective.to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ];

        let all_defs = self.tools.get_definitions();
        let scoped_defs = role.filter_tool_definitions(&all_defs);
        let mut modified_files = HashSet::new();
        let mut summary = String::new();
        let mut turn = 0;

        while turn < max_turns {
            turn += 1;
            let status_msg = format!("{} turn {}...", role.name(), turn);
            on_event(AgentEvent::Thinking(&status_msg));

            let res = self
                .provider
                .complete_stream(&history, &scoped_defs, |tok| {
                    on_event(AgentEvent::Token(tok));
                })
                .await?;

            history.push(LLMMessage {
                role: "assistant".to_string(),
                content: res.content.clone(),
                tool_calls: res.tool_calls.clone(),
                tool_call_id: None,
                name: None,
            });

            let tool_calls = match res.tool_calls {
                Some(tc) if !tc.is_empty() => tc,
                _ => {
                    summary = res.content;
                    break;
                }
            };

            for call in &tool_calls {
                on_event(AgentEvent::ToolStart {
                    name: &call.name,
                    args: &call.arguments,
                });

                let decision =
                    self.security
                        .evaluate(&call.name, &call.arguments, self.workspace_root);
                match decision {
                    PermissionDecision::Allow => {}
                    PermissionDecision::Deny(r) => {
                        let out = format!("[SECURITY POLICY DENIED] {}", r);
                        on_event(AgentEvent::ToolEnd {
                            name: &call.name,
                            success: false,
                            output: &out,
                        });
                        history.push(LLMMessage {
                            role: "tool".to_string(),
                            content: out,
                            tool_calls: None,
                            tool_call_id: Some(call.id.clone()),
                            name: Some(call.name.clone()),
                        });
                        continue;
                    }
                    PermissionDecision::Prompt {
                        reason,
                        details,
                        risk_level,
                        suggest_trust_path,
                    } => {
                        let allowed = self.security.prompt_user_interactive(
                            &call.name,
                            &reason,
                            &details,
                            risk_level,
                            suggest_trust_path.as_deref(),
                        );
                        if !allowed {
                            let out = format!(
                                "[SECURITY CHALLENGE DENIED] Execution rejected: {}",
                                details
                            );
                            on_event(AgentEvent::ToolEnd {
                                name: &call.name,
                                success: false,
                                output: &out,
                            });
                            history.push(LLMMessage {
                                role: "tool".to_string(),
                                content: out,
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
                        modified_files.insert(p.to_string());
                    }
                }

                let (success, output) = self.tools.execute_tool(&call.name, &call.arguments).await;
                on_event(AgentEvent::ToolEnd {
                    name: &call.name,
                    success,
                    output: &output,
                });

                history.push(LLMMessage {
                    role: "tool".to_string(),
                    content: output,
                    tool_calls: None,
                    tool_call_id: Some(call.id.clone()),
                    name: Some(call.name.clone()),
                });
            }
        }

        self.blackbox
            .record("subagent_end", role.name(), &summary, None, "COMPLETE");
        let files: Vec<String> = modified_files.into_iter().collect();
        Ok((summary, files))
    }

    /// Executes the full 3-phase autonomous swarm pipeline:
    /// Phase 1: Explorer -> Phase 2: Coder -> Phase 3: Verifier -> (Self-Healing Loop if needed)
    pub async fn execute_pipeline<F>(
        &mut self,
        goal: &str,
        mut on_event: F,
    ) -> Result<SwarmOutcome, String>
    where
        F: FnMut(AgentEvent) + Send,
    {
        let start = Instant::now();

        // 1. Phase 1: Explorer Reconnaissance
        on_event(AgentEvent::Thinking(
            "Phase 1/3: Dispatching EXPLORER sub-agent for reconnaissance...",
        ));
        let explorer_objective = format!(
            "Analyze the codebase to prepare for this goal: {}\n\
            Identify: 1) Affected files and line numbers 2) Existing patterns and symbol definitions 3) Risks and caveats.",
            goal
        );
        let (explorer_brief, _) = self
            .run_subagent(
                SubAgentRole::Explorer,
                &explorer_objective,
                8,
                &mut on_event,
            )
            .await?;

        // 2. Phase 2: Coder Implementation
        on_event(AgentEvent::Thinking(
            "Phase 2/3: Dispatching CODER sub-agent for surgical implementation...",
        ));
        let coder_objective = format!(
            "OBJECTIVE: {}\n\nRECONNAISSANCE BRIEF:\n{}\n\nExecute the minimal surgical edits required.",
            goal, explorer_brief
        );
        let (coder_summary, mut all_modified) = self
            .run_subagent(SubAgentRole::Coder, &coder_objective, 12, &mut on_event)
            .await?;

        // 3. Phase 3: Verifier Quality Assurance & Self-Healing
        on_event(AgentEvent::Thinking(
            "Phase 3/3: Dispatching VERIFIER sub-agent for validation & test audit...",
        ));
        let verifier_objective = format!(
            "Verify the implementation for: {}\n\
            Modified files: {}\n\
            Run project tests and inspect build output. Report [VERIFICATION PASSED] if clean or diagnose failures.",
            goal, all_modified.join(", ")
        );
        let (mut verifier_report, _) = self
            .run_subagent(
                SubAgentRole::Verifier,
                &verifier_objective,
                6,
                &mut on_event,
            )
            .await?;

        let mut verification_passed = verifier_report.contains("[VERIFICATION PASSED]")
            || (!verifier_report.contains("FAILED") && !verifier_report.contains("error[E"));

        // If verification failed and files were modified, run 1 self-healing repair cycle
        if !verification_passed && !all_modified.is_empty() {
            on_event(AgentEvent::Thinking(
                "Self-Healing: Verification failed. Dispatching CODER sub-agent to repair...",
            ));
            let repair_objective = format!(
                "REPAIR REQUIRED: Verification reported failures:\n{}\n\n\
                Inspect compiler/test errors and apply fixes to affected files: {}",
                verifier_report,
                all_modified.join(", ")
            );
            let (_repair_summary, repair_files) = self
                .run_subagent(SubAgentRole::Coder, &repair_objective, 8, &mut on_event)
                .await?;

            for rf in repair_files {
                if !all_modified.contains(&rf) {
                    all_modified.push(rf);
                }
            }

            // Re-verify
            on_event(AgentEvent::Thinking("Re-verifying repaired changes..."));
            let (reverify_report, _) = self
                .run_subagent(SubAgentRole::Verifier, &verifier_objective, 4, on_event)
                .await?;

            verifier_report = reverify_report;
            verification_passed = verifier_report.contains("[VERIFICATION PASSED]")
                || (!verifier_report.contains("FAILED") && !verifier_report.contains("error[E"));
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(SwarmOutcome {
            goal: goal.to_string(),
            explorer_brief,
            coder_summary,
            verifier_report,
            files_modified: all_modified,
            verification_passed,
            duration_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subagent_role_names() {
        assert_eq!(SubAgentRole::Explorer.name(), "EXPLORER");
        assert_eq!(SubAgentRole::Coder.name(), "CODER");
        assert_eq!(SubAgentRole::Verifier.name(), "VERIFIER");
    }

    #[test]
    fn test_subagent_role_system_instructions() {
        let p = PathBuf::from("/test/workspace");
        let exp_inst = SubAgentRole::Explorer.system_instructions(&p);
        assert!(exp_inst.contains("EXPLORER"));
        assert!(exp_inst.contains("DO NOT attempt to edit"));
        assert!(!exp_inst.contains("Google"));
        assert!(!exp_inst.contains("OpenAI"));

        let coder_inst = SubAgentRole::Coder.system_instructions(&p);
        assert!(coder_inst.contains("CODER"));
        assert!(coder_inst.contains("surgical"));

        let verifier_inst = SubAgentRole::Verifier.system_instructions(&p);
        assert!(verifier_inst.contains("VERIFIER"));
        assert!(verifier_inst.contains("run_tests"));
    }

    #[test]
    fn test_subagent_role_tool_filtering() {
        let defs = vec![
            ToolDefinition {
                name: "view_file".to_string(),
                description: "".to_string(),
                parameters: serde_json::json!({}),
            },
            ToolDefinition {
                name: "search_code".to_string(),
                description: "".to_string(),
                parameters: serde_json::json!({}),
            },
            ToolDefinition {
                name: "patch_file".to_string(),
                description: "".to_string(),
                parameters: serde_json::json!({}),
            },
            ToolDefinition {
                name: "write_file".to_string(),
                description: "".to_string(),
                parameters: serde_json::json!({}),
            },
            ToolDefinition {
                name: "run_tests".to_string(),
                description: "".to_string(),
                parameters: serde_json::json!({}),
            },
            ToolDefinition {
                name: "argus_audit".to_string(),
                description: "".to_string(),
                parameters: serde_json::json!({}),
            },
            ToolDefinition {
                name: "mcp__db__query".to_string(),
                description: "".to_string(),
                parameters: serde_json::json!({}),
            },
        ];

        let exp_defs = SubAgentRole::Explorer.filter_tool_definitions(&defs);
        let exp_names: Vec<&str> = exp_defs.iter().map(|d| d.name.as_str()).collect();
        assert!(exp_names.contains(&"view_file"));
        assert!(exp_names.contains(&"search_code"));
        assert!(exp_names.contains(&"mcp__db__query"));
        assert!(!exp_names.contains(&"patch_file"));
        assert!(!exp_names.contains(&"write_file"));
        assert!(!exp_names.contains(&"run_tests"));

        let coder_defs = SubAgentRole::Coder.filter_tool_definitions(&defs);
        let coder_names: Vec<&str> = coder_defs.iter().map(|d| d.name.as_str()).collect();
        assert!(coder_names.contains(&"patch_file"));
        assert!(coder_names.contains(&"write_file"));
        assert!(!coder_names.contains(&"run_tests"));
        assert!(!coder_names.contains(&"argus_audit"));

        let verifier_defs = SubAgentRole::Verifier.filter_tool_definitions(&defs);
        let verifier_names: Vec<&str> = verifier_defs.iter().map(|d| d.name.as_str()).collect();
        assert!(verifier_names.contains(&"run_tests"));
        assert!(verifier_names.contains(&"argus_audit"));
        assert!(!verifier_names.contains(&"patch_file"));
        assert!(!verifier_names.contains(&"write_file"));
    }
}
