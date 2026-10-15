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

        if tool_name == "run_command" {
            let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("").trim();
            if cmd.is_empty() {
                return PermissionDecision::Allow;
            }

            if self.session_allowed_commands.contains(cmd) || self.config.trusted_commands.iter().any(|tc| cmd.starts_with(tc)) {
                return PermissionDecision::Allow;
            }

            if let Some((risk, reason)) = SecurityPolicy::scan_command_risk(cmd) {
                return PermissionDecision::Prompt {
                    reason: reason.to_string(),
                    details: format!("Command: {}", cmd),
                    risk_level: risk,
                    suggest_trust_path: None,
                };
            }

            if self.config.mode == SecurityMode::Strict {
                return PermissionDecision::Prompt {
                    reason: "Strict mode requires explicit confirmation for all terminal commands".to_string(),
                    details: format!("Command: {}", cmd),
                    risk_level: RiskLevel::Medium,
                    suggest_trust_path: None,
                };
            }

            return PermissionDecision::Allow;
        }

        // Tools operating on paths
        let raw_path = match tool_name {
            "view_file" | "patch_file" | "write_file" => {
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
            _ => None,
        };

        if let Some(target_str) = raw_path {
            let target = Path::new(target_str);
            let abs_path = if target.is_absolute() {
                target.to_path_buf()
            } else {
                workspace_root.join(target)
            };

            let canon = abs_path.canonicalize().unwrap_or_else(|_| abs_path.clone());

            // Check sensitive file credentials
            if SecurityPolicy::is_sensitive_path(&canon) {
                if self.config.mode == SecurityMode::Strict {
                    return PermissionDecision::Deny(format!("Strict mode prohibits access to sensitive credential file: {}", canon.display()));
                }
                return PermissionDecision::Prompt {
                    reason: "Accessing sensitive host credential or identity file".to_string(),
                    details: format!("Path: {}", canon.display()),
                    risk_level: RiskLevel::High,
                    suggest_trust_path: None,
                };
            }

            // Check if granted in current session
            if self.session_allowed_paths.iter().any(|sp| canon.starts_with(sp)) {
                return PermissionDecision::Allow;
            }

            let within_trusted = SecurityPolicy::is_within_trusted_boundaries(&canon, workspace_root, &self.config.trusted_paths);

            if within_trusted {
                if self.config.mode == SecurityMode::Strict && (tool_name == "write_file" || tool_name == "patch_file") {
                    return PermissionDecision::Prompt {
                        reason: "Strict mode requires human approval for all file mutations".to_string(),
                        details: format!("Target file: {}", canon.display()),
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
                    canon.display()
                ));
            }

            let suggest_folder = if canon.is_dir() {
                Some(canon.clone())
            } else {
                canon.parent().map(|p| p.to_path_buf())
            };

            return PermissionDecision::Prompt {
                reason: "Operation targets directory or drive outside designated workspace".to_string(),
                details: format!("Path: {}", canon.display()),
                risk_level: RiskLevel::Medium,
                suggest_trust_path: suggest_folder,
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

        let prompt_options = if suggest_path.is_some() { "[y/s/t/d]" } else { "[y/s/d]" };
        print!("\nChoice {} (default: y): ", prompt_options);
        let _ = io::stdout().flush();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            println!("{}\n[FAIL] Failed to read user choice. Denying operation.{}\n", TuiRenderer::RED, TuiRenderer::RESET);
            println!("{}{}{}\n", TuiRenderer::YELLOW, border, TuiRenderer::RESET);
            return false;
        }

        let choice = input.trim().to_lowercase();
        let allowed = match choice.as_str() {
            "" | "y" | "yes" => {
                println!("{}[OK] Permission granted for single execution.{}\n", TuiRenderer::GREEN, TuiRenderer::RESET);
                true
            }
            "s" | "session" => {
                if let Some(p) = suggest_path {
                    self.session_allowed_paths.insert(p.to_path_buf());
                } else if tool_name == "run_command" {
                    let cmd_prefix = details.strip_prefix("Command: ").unwrap_or(details).trim().to_string();
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
                println!("{}[DENIED] Operation aborted by user.{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                false
            }
        };

        println!("{}{}{}\n", TuiRenderer::YELLOW, border, TuiRenderer::RESET);
        allowed
    }
}
