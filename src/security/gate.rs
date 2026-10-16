//! Atlas Daedalus — Permission Gate & Interactive Challenge
//! (C) 2026 Atlas Research Labs — MIT License

use std::collections::HashSet;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use serde_json::Value;

use crate::cli::tui::TuiRenderer;
use crate::security::policy::{PermissionDecision, RiskLevel, SecurityMode, SecurityPolicy};
use crate::security::storage::{SecurityConfig, SecurityStorage};

pub struct PermissionGate {
    pub config: SecurityConfig,
    pub session_allowed_paths: HashSet<PathBuf>,
    pub session_allowed_commands: HashSet<String>,
}

impl PermissionGate {
    pub fn new(mode_override: Option<SecurityMode>) -> Self {
        let mut config = SecurityStorage::load();
        if let Some(m) = mode_override {
            config.mode = m;
        }

        Self {
            config,
            session_allowed_paths: HashSet::new(),
            session_allowed_commands: HashSet::new(),
        }
    }

    pub fn set_mode(&mut self, mode: SecurityMode) {
        self.config.mode = mode;
        SecurityStorage::save(&self.config);
    }

    pub fn trust_path(&mut self, path: PathBuf) {
        let canon = path.canonicalize().unwrap_or(path);
        if !self.config.trusted_paths.contains(&canon) {
            self.config.trusted_paths.push(canon);
            SecurityStorage::save(&self.config);
        }
    }

    pub fn untrust_path(&mut self, path: &Path) -> bool {
        let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let initial_len = self.config.trusted_paths.len();
        self.config.trusted_paths.retain(|p| p != &canon && p != path);
        let removed = self.config.trusted_paths.len() < initial_len;
        if removed {
            SecurityStorage::save(&self.config);
        }
        removed
    }

    pub fn clear_session_grants(&mut self) {
        self.session_allowed_paths.clear();
        self.session_allowed_commands.clear();
    }

    pub fn evaluate(
        &self,
        tool_name: &str,
        args: &Value,
        workspace_root: &Path,
    ) -> PermissionDecision {
        if self.config.mode == SecurityMode::FullAccess {
            return PermissionDecision::Allow;
        }

        // Terminal execution tools: run_command, run_tests, start_daemon, or MCP command tools
        if tool_name == "run_command" || tool_name == "run_tests" || tool_name == "start_daemon" || (tool_name.starts_with("mcp__") && (args.get("command").is_some() || args.get("cmd").is_some())) {
            let cmd = match tool_name {
                "run_tests" => {
                    args.get("command")
                        .and_then(|v| v.as_str())
                        .unwrap_or("cargo test")
                        .trim()
                }
                _ => {
                    args.get("command")
                        .or_else(|| args.get("cmd"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim()
                }
            };
            if cmd.is_empty() {
                return PermissionDecision::Allow;
            }

            // Split compound chained commands (e.g. "git status; rm -rf ~" or "cargo test && /bin/sh")
            let sub_cmds = SecurityPolicy::split_command_chain(cmd);

            // Audit EVERY sub-command for high/critical risk patterns
            for sub in &sub_cmds {
                if let Some((risk, reason)) = SecurityPolicy::scan_command_risk(sub) {
                    return PermissionDecision::Prompt {
                        reason: reason.to_string(),
                        details: format!("Command: {}", cmd),
                        risk_level: risk,
                        suggest_trust_path: None,
                    };
                }
            }

            if self.session_allowed_commands.contains(cmd) {
                return PermissionDecision::Allow;
            }

            // Check if every individual sub-command is in trusted commands
            let all_trusted = !sub_cmds.is_empty() && sub_cmds.iter().all(|sub| {
                self.config.trusted_commands.iter().any(|tc| {
                    sub == tc || sub.starts_with(&format!("{} ", tc))
                })
            });

            if all_trusted {
                return PermissionDecision::Allow;
            }

            if self.config.mode == SecurityMode::Strict {
                return PermissionDecision::Prompt {
                    reason: "Strict mode requires explicit confirmation for all terminal commands".to_string(),
                    details: format!("Command: {}", cmd),
                    risk_level: RiskLevel::Medium,
                    suggest_trust_path: None,
                };
            }

            if tool_name == "start_daemon" {
                return PermissionDecision::Prompt {
                    reason: "Starting persistent background daemon process".to_string(),
                    details: format!("Daemon command: {}", cmd),
                    risk_level: RiskLevel::Medium,
                    suggest_trust_path: None,
                };
            }

            return PermissionDecision::Allow;
        }

        // Network tunneling tools
        if tool_name == "beacon_tunnel" {
            let port = args.get("port").and_then(|v| v.as_u64()).unwrap_or(0);
            return PermissionDecision::Prompt {
                reason: "Exposing local network port to the public internet via HTTPS tunnel".to_string(),
                details: format!("Local Port: {}", port),
                risk_level: RiskLevel::High,
                suggest_trust_path: None,
            };
        }

        // Container management tools
        if tool_name == "manage_containers" {
            let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("status");
            let target = args.get("target").and_then(|v| v.as_str()).unwrap_or("");
            if matches!(action, "stop" | "compose_down") {
                return PermissionDecision::Prompt {
                    reason: "Terminating container workloads or tearing down docker-compose services".to_string(),
                    details: format!("Action: {}, Target: {}", action, target),
                    risk_level: RiskLevel::High,
                    suggest_trust_path: None,
                };
            } else if matches!(action, "compose_up" | "restart") && self.config.mode == SecurityMode::Strict {
                return PermissionDecision::Prompt {
                    reason: "Strict mode requires confirmation for container lifecycle state changes".to_string(),
                    details: format!("Action: {}, Target: {}", action, target),
                    risk_level: RiskLevel::Medium,
                    suggest_trust_path: None,
                };
            }
        }

        // Dependency management tools
        if tool_name == "manage_dependencies" {
            let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");
            let package = args.get("package").and_then(|v| v.as_str()).unwrap_or("");
            if matches!(action, "add" | "remove") && self.config.mode == SecurityMode::Strict {
                return PermissionDecision::Prompt {
                    reason: "Strict mode requires confirmation before modifying project dependencies".to_string(),
                    details: format!("Action: {}, Package: {}", action, package),
                    risk_level: RiskLevel::Medium,
                    suggest_trust_path: None,
                };
            }
        }

        // Tools operating on filesystem paths
        let raw_path = match tool_name {
            "view_file" | "patch_file" | "write_file" | "extract_symbols" | "scaffold_test" => {
                args.get("path")
                    .or_else(|| args.get("file"))
                    .or_else(|| args.get("file_path"))
                    .and_then(|v| v.as_str())
            }
            "list_dir" => {
                args.get("path")
                    .or_else(|| args.get("dir"))
                    .and_then(|v| v.as_str())
            }
            "search_code" => {
                args.get("dir")
                    .or_else(|| args.get("path"))
                    .and_then(|v| v.as_str())
            }
            "change_workspace" => {
                args.get("path")
                    .or_else(|| args.get("dir"))
                    .and_then(|v| v.as_str())
            }
            _ if tool_name.starts_with("mcp__") => {
                args.get("path")
                    .or_else(|| args.get("file"))
                    .or_else(|| args.get("file_path"))
                    .or_else(|| args.get("dir"))
                    .and_then(|v| v.as_str())
            }
            _ => None,
        };

        if let Some(target_str) = raw_path {
            let target = Path::new(target_str);
            let abs_path = if target.is_absolute() {
                target.to_path_buf()
            } else {
                workspace_root.join(target)
            };

            // Lexical normalization resolves '..' and '.' in memory even if the file does not yet exist on disk
            let normalized = SecurityPolicy::normalize_lexically(&abs_path);

            // Check sensitive file credentials
            if SecurityPolicy::is_sensitive_path(&normalized) {
                if self.config.mode == SecurityMode::Strict {
                    return PermissionDecision::Deny(format!("Strict mode prohibits access to sensitive credential file: {}", normalized.display()));
                }
                return PermissionDecision::Prompt {
                    reason: "Accessing sensitive host credential or identity file".to_string(),
                    details: format!("Path: {}", normalized.display()),
                    risk_level: RiskLevel::High,
                    suggest_trust_path: None,
                };
            }

            // Check if granted in current session
            if self.session_allowed_paths.iter().any(|sp| normalized.starts_with(sp)) {
                return PermissionDecision::Allow;
            }

            let within_trusted = SecurityPolicy::is_within_trusted_boundaries(&normalized, workspace_root, &self.config.trusted_paths);

            if within_trusted {
                if self.config.mode == SecurityMode::Strict && (tool_name == "write_file" || tool_name == "patch_file") {
                    return PermissionDecision::Prompt {
                        reason: "Strict mode requires human approval for all file mutations".to_string(),
                        details: format!("Target file: {}", normalized.display()),
                        risk_level: RiskLevel::Low,
                        suggest_trust_path: None,
                    };
                }
                return PermissionDecision::Allow;
            }

            // Path is outside workspace and not in trusted paths
            if self.config.mode == SecurityMode::Strict {
                return PermissionDecision::Deny(format!(
                    "Strict mode prohibits access outside designated workspace root: {}",
                    normalized.display()
                ));
            }

            let suggest_folder = if normalized.is_dir() {
                Some(normalized.clone())
            } else {
                normalized.parent().map(|p| p.to_path_buf())
            };

            return PermissionDecision::Prompt {
                reason: "Operation targets directory or drive outside designated workspace".to_string(),
                details: format!("Path: {}", normalized.display()),
                risk_level: RiskLevel::Medium,
                suggest_trust_path: suggest_folder,
            };
        }

        if tool_name.starts_with("mcp__") && self.config.mode == SecurityMode::Strict {
            return PermissionDecision::Prompt {
                reason: format!("Strict mode requires human approval for external MCP tool '{}'", tool_name),
                details: format!("Arguments: {}", args),
                risk_level: RiskLevel::Medium,
                suggest_trust_path: None,
            };
        }

        PermissionDecision::Allow
    }

    pub fn prompt_user_interactive(
        &mut self,
        tool_name: &str,
        reason: &str,
        details: &str,
        risk: RiskLevel,
        suggest_path: Option<&Path>,
    ) -> bool {
        let border = "─".repeat(78);
        println!("\n{}{}{}", TuiRenderer::YELLOW, border, TuiRenderer::RESET);
        println!(
            "{}[SECURITY GATE · PERMISSION REQUIRED]{}  Risk Level: {}{}{}",
            TuiRenderer::BOLD,
            TuiRenderer::RESET,
            match risk {
                RiskLevel::Low => TuiRenderer::CYAN,
                RiskLevel::Medium => TuiRenderer::YELLOW,
                RiskLevel::High | RiskLevel::Critical => TuiRenderer::RED,
            },
            risk,
            TuiRenderer::RESET
        );
        println!("  Operation: {}{}{}", TuiRenderer::WHITE, tool_name, TuiRenderer::RESET);
        println!("  Reason:    {}{}{}", TuiRenderer::DIM, reason, TuiRenderer::RESET);
        println!("  Details:   {}{}{}\n", TuiRenderer::CYAN, details, TuiRenderer::RESET);

        println!("Options:");
        println!("  [y] Allow Once");
        println!("  [s] Allow for Current Session");
        if suggest_path.is_some() {
            println!("  [t] Trust Path Permanently (add to ~/.daedalus/security.json)");
        }
        println!("  [d] Deny Operation");

        let default_choice = match risk {
            RiskLevel::High | RiskLevel::Critical => "d",
            RiskLevel::Low | RiskLevel::Medium => "y",
        };

        let prompt_options = match (suggest_path.is_some(), default_choice) {
            (true, "d") => "[y/s/t/D] (default: Deny)",
            (true, "y") => "[Y/s/t/d] (default: Allow)",
            (false, "d") => "[y/s/D] (default: Deny)",
            (false, "y") => "[Y/s/d] (default: Allow)",
            _ => "[y/s/d]",
        };
        print!("\nChoice {}: ", prompt_options);
        let _ = io::stdout().flush();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => {
                // Closed stdin / EOF: must ALWAYS deny to prevent automated execution or headless pipe bypass
                println!("{}\n[SECURITY GATE] Non-interactive EOF / closed stdin detected. Automatically denying operation.{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                println!("{}{}{}\n", TuiRenderer::YELLOW, border, TuiRenderer::RESET);
                return false;
            }
            Err(_) => {
                println!("{}\n[FAIL] Failed to read user choice. Denying operation.{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                println!("{}{}{}\n", TuiRenderer::YELLOW, border, TuiRenderer::RESET);
                return false;
            }
            Ok(_) => {}
        }

        let choice = input.trim().to_lowercase();
        let effective_choice = if choice.is_empty() {
            default_choice
        } else {
            choice.as_str()
        };

        let allowed = match effective_choice {
            "y" | "yes" => {
                println!("{}[OK] Permission granted for single execution.{}\n", TuiRenderer::GREEN, TuiRenderer::RESET);
                true
            }
            "s" | "session" => {
                if let Some(p) = suggest_path {
                    self.session_allowed_paths.insert(p.to_path_buf());
                } else if tool_name == "run_command" || tool_name == "run_tests" || tool_name == "start_daemon" {
                    let cmd_prefix = details.strip_prefix("Command: ")
                        .or_else(|| details.strip_prefix("Daemon command: "))
                        .unwrap_or(details)
                        .trim()
                        .to_string();
                    self.session_allowed_commands.insert(cmd_prefix);
                }
                println!("{}[OK] Permission granted for this entire session.{}\n", TuiRenderer::GREEN, TuiRenderer::RESET);
                true
            }
            "t" | "trust" => {
                if let Some(p) = suggest_path {
                    self.trust_path(p.to_path_buf());
                    println!("{}[OK] Trusted path permanently: {}{}\n", TuiRenderer::GREEN, p.display(), TuiRenderer::RESET);
                    true
                } else {
                    println!("{}[OK] Permission granted for single execution.{}\n", TuiRenderer::GREEN, TuiRenderer::RESET);
                    true
                }
            }
            _ => {
                println!("{}[DENIED] Operation aborted by security policy.{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                false
            }
        };

        println!("{}{}{}\n", TuiRenderer::YELLOW, border, TuiRenderer::RESET);
        allowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_gate_chained_command_interception() {
        let gate = PermissionGate::new(Some(SecurityMode::Sandboxed));
        let workspace = PathBuf::from("/workspace");
        let args = json!({ "command": "git status; rm -rf ~" });

        let decision = gate.evaluate("run_command", &args, &workspace);
        match decision {
            PermissionDecision::Prompt { risk_level, .. } => {
                assert_eq!(risk_level, RiskLevel::Critical);
            }
            other => panic!("Expected Critical prompt, got: {:?}", other),
        }
    }

    #[test]
    fn test_gate_run_tests_interception() {
        let gate = PermissionGate::new(Some(SecurityMode::Sandboxed));
        let workspace = PathBuf::from("/workspace");
        let args = json!({ "command": "cargo test && rm -rf /" });

        let decision = gate.evaluate("run_tests", &args, &workspace);
        match decision {
            PermissionDecision::Prompt { risk_level, .. } => {
                assert_eq!(risk_level, RiskLevel::Critical);
            }
            other => panic!("Expected Critical prompt, got: {:?}", other),
        }
    }

    #[test]
    fn test_gate_path_traversal_non_existent_file() {
        let gate = PermissionGate::new(Some(SecurityMode::Sandboxed));
        let workspace = PathBuf::from("/home/user/workspace");
        let args = json!({ "path": "../../etc/shadow" });

        let decision = gate.evaluate("view_file", &args, &workspace);
        match decision {
            PermissionDecision::Prompt { risk_level, .. } => {
                assert!(risk_level == RiskLevel::Medium || risk_level == RiskLevel::High);
            }
            other => panic!("Expected Prompt for path traversal, got: {:?}", other),
        }
    }

    #[test]
    fn test_gate_mcp_command_risk() {
        let gate = PermissionGate::new(Some(SecurityMode::Sandboxed));
        let workspace = PathBuf::from("/workspace");
        let args = json!({ "command": "cargo check && rm -rf /" });

        let decision = gate.evaluate("mcp__terminal__exec", &args, &workspace);
        match decision {
            PermissionDecision::Prompt { risk_level, .. } => {
                assert_eq!(risk_level, RiskLevel::Critical);
            }
            other => panic!("Expected Critical prompt for MCP command, got: {:?}", other),
        }
    }

    #[test]
    fn test_gate_mcp_strict_mode() {
        let gate = PermissionGate::new(Some(SecurityMode::Strict));
        let workspace = PathBuf::from("/workspace");
        let args = json!({ "query": "SELECT * FROM users" });

        let decision = gate.evaluate("mcp__sqlite__query", &args, &workspace);
        match decision {
            PermissionDecision::Prompt { reason, .. } => {
                assert!(reason.contains("Strict mode requires human approval"));
            }
            other => panic!("Expected Strict prompt for MCP tool, got: {:?}", other),
        }
    }
}
