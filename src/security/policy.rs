//! Atlas Daedalus — Security Policies & Risk Scanners
//! (C) 2026 Atlas Research Labs — MIT License

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityMode {
    #[serde(rename = "full")]
    FullAccess,
    #[serde(rename = "sandboxed")]
    Sandboxed,
    #[serde(rename = "strict")]
    Strict,
}

impl Default for SecurityMode {
    fn default() -> Self {
        SecurityMode::Sandboxed
    }
}

impl std::str::FromStr for SecurityMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "full" | "fullaccess" | "full-access" | "unrestricted" => Ok(SecurityMode::FullAccess),
            "sandboxed" | "sandbox" | "standard" => Ok(SecurityMode::Sandboxed),
            "strict" | "paranoid" | "zero-trust" => Ok(SecurityMode::Strict),
            _ => Err(format!("Unknown security mode: '{}'. Valid options: full, sandboxed, strict", s)),
        }
    }
}

impl std::fmt::Display for SecurityMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityMode::FullAccess => write!(f, "full"),
            SecurityMode::Sandboxed => write!(f, "sandboxed"),
            SecurityMode::Strict => write!(f, "strict"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "LOW"),
            RiskLevel::Medium => write!(f, "MEDIUM"),
            RiskLevel::High => write!(f, "HIGH"),
            RiskLevel::Critical => write!(f, "CRITICAL"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    Allow,
    Deny(String),
    Prompt {
        reason: String,
        details: String,
        risk_level: RiskLevel,
        suggest_trust_path: Option<PathBuf>,
    },
}

pub struct SecurityPolicy;

impl SecurityPolicy {
    /// Detects if a command is inherently high risk or destructive
    pub fn scan_command_risk(cmd: &str) -> Option<(RiskLevel, &'static str)> {
        let trimmed = cmd.trim();
        let lower = trimmed.to_lowercase();

        // Critical destructive commands
        if lower.contains("rm -rf /") || lower.contains("rm -rf *") || lower.contains("del /f /s /q c:\\") {
            return Some((RiskLevel::Critical, "Mass destructive filesystem deletion"));
        }
        if lower.contains("mkfs") || lower.contains("format ") || lower.contains("dd if=") {
            return Some((RiskLevel::Critical, "Disk format or low-level block write"));
        }

        // High risk commands
        if lower.contains("rm -rf") || lower.contains("rm -r ") || lower.contains("rmdir /s") {
            return Some((RiskLevel::High, "Recursive directory deletion"));
        }
        if lower.contains("git push") && (lower.contains("--force") || lower.contains("-f ")) {
            return Some((RiskLevel::High, "Force push altering remote Git history"));
        }
        if lower.contains("git reset --hard") || lower.contains("git clean -fdx") {
            return Some((RiskLevel::High, "Destructive Git reset discarding uncommitted code"));
        }
        if lower.contains("npm publish") || lower.contains("cargo publish") {
            return Some((RiskLevel::High, "Publishing package to public package registry"));
        }
        if lower.contains("shutdown") || lower.contains("reboot") {
            return Some((RiskLevel::High, "System power state modification"));
        }

        // Medium risk network / privilege escalation commands
        if lower.starts_with("curl ") || lower.starts_with("wget ") || lower.contains("| sh") || lower.contains("| bash") {
            return Some((RiskLevel::Medium, "Remote script execution"));
        }

        None
    }

    /// Checks if a path targets sensitive credentials, ssh keys, or configuration
    pub fn is_sensitive_path(path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase().replace('\\', "/");

        let sensitive_fragments = [
            "/.ssh/",
            "/id_rsa",
            "/id_ed25519",
            "/id_ecdsa",
            "/.aws/credentials",
            "/.aws/config",
            "/.kube/config",
            "/.gnupg/",
            "/.docker/config.json",
            "/.bash_history",
            "/.zsh_history",
        ];

        for frag in &sensitive_fragments {
            if path_str.contains(frag) || path_str.ends_with(frag.trim_start_matches('/')) {
                return true;
            }
        }

        false
    }

    /// Resolves whether a path is within the workspace root or any explicitly trusted directory
    pub fn is_within_trusted_boundaries(
        target: &Path,
        workspace_root: &Path,
        trusted_paths: &[PathBuf],
    ) -> bool {
        let abs_target = if target.is_absolute() {
            target.to_path_buf()
        } else {
            workspace_root.join(target)
        };

        let canon_target = abs_target.canonicalize().unwrap_or(abs_target);
        let canon_root = workspace_root.canonicalize().unwrap_or_else(|_| workspace_root.to_path_buf());

        if canon_target.starts_with(&canon_root) {
            return true;
        }

        for trusted in trusted_paths {
            let canon_trusted = trusted.canonicalize().unwrap_or_else(|_| trusted.clone());
            if canon_target.starts_with(&canon_trusted) {
                return true;
            }
        }

        false
    }
}
