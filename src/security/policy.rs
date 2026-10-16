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
    /// Lexically normalizes a path by resolving '.' and '..' components purely in memory.
    /// This prevents path traversal bypasses on files that do not yet exist on disk.
    pub fn normalize_lexically(path: &Path) -> PathBuf {
        use std::path::Component;
        let mut components = Vec::new();

        for component in path.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    match components.last() {
                        Some(Component::Normal(_)) => {
                            components.pop();
                        }
                        Some(Component::RootDir) | Some(Component::Prefix(_)) => {
                            // Cannot pop beyond root directory or prefix
                        }
                        _ => {
                            components.push(component);
                        }
                    }
                }
                _ => components.push(component),
            }
        }

        components.iter().collect()
    }

    /// Splits a compound command string into individual sub-commands across common shell delimiters (;, &&, ||, |, &)
    pub fn split_command_chain(cmd: &str) -> Vec<&str> {
        let mut list = Vec::new();
        let mut last = 0;
        let bytes = cmd.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            if bytes[i] == b';' || bytes[i] == b'\n' {
                let sub = cmd[last..i].trim();
                if !sub.is_empty() {
                    list.push(sub);
                }
                i += 1;
                last = i;
            } else if bytes[i] == b'&' {
                if i + 1 < len && bytes[i + 1] == b'&' {
                    let sub = cmd[last..i].trim();
                    if !sub.is_empty() {
                        list.push(sub);
                    }
                    i += 2;
                    last = i;
                } else {
                    let sub = cmd[last..i].trim();
                    if !sub.is_empty() {
                        list.push(sub);
                    }
                    i += 1;
                    last = i;
                }
            } else if bytes[i] == b'|' {
                if i + 1 < len && bytes[i + 1] == b'|' {
                    let sub = cmd[last..i].trim();
                    if !sub.is_empty() {
                        list.push(sub);
                    }
                    i += 2;
                    last = i;
                } else {
                    let sub = cmd[last..i].trim();
                    if !sub.is_empty() {
                        list.push(sub);
                    }
                    i += 1;
                    last = i;
                }
            } else {
                i += 1;
            }
        }

        let tail = cmd[last..].trim();
        if !tail.is_empty() {
            list.push(tail);
        }

        list
    }

    /// Detects if a command or any chained sub-command is inherently high risk or destructive
    pub fn scan_command_risk(cmd: &str) -> Option<(RiskLevel, &'static str)> {
        let sub_cmds = Self::split_command_chain(cmd);
        let mut highest: Option<(RiskLevel, &'static str)> = None;

        for sub in sub_cmds {
            if let Some((r, desc)) = Self::scan_single_subcommand(sub) {
                highest = match highest {
                    None => Some((r, desc)),
                    Some((cur_r, cur_desc)) => {
                        let rank = |lvl: RiskLevel| match lvl {
                            RiskLevel::Low => 1,
                            RiskLevel::Medium => 2,
                            RiskLevel::High => 3,
                            RiskLevel::Critical => 4,
                        };
                        if rank(r) > rank(cur_r) {
                            Some((r, desc))
                        } else {
                            Some((cur_r, cur_desc))
                        }
                    }
                };
            }
        }

        highest
    }

    fn scan_single_subcommand(sub: &str) -> Option<(RiskLevel, &'static str)> {
        let trimmed = sub.trim();
        // Collapse consecutive whitespace to single space for normalized pattern matching
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        let normalized = words.join(" ").to_lowercase();

        // 1. Critical destructive filesystem wipe commands
        if normalized.contains("rm -rf /") 
            || normalized.contains("rm -rf /*") 
            || normalized.contains("del /f /s /q c:\\") 
            || normalized.contains("del /f /q c:\\")
        {
            return Some((RiskLevel::Critical, "Mass destructive filesystem deletion"));
        }
        if normalized.contains("mkfs") || normalized.starts_with("format ") || normalized.contains("dd if=") {
            return Some((RiskLevel::Critical, "Disk format or low-level block write"));
        }

        // 2. High risk commands: recursive removal, user home deletion, python rmtree, find -delete
        if normalized.starts_with("rm -rf ") || normalized.starts_with("rm -r ") || normalized.contains(" rmdir /s") || normalized.starts_with("rmdir /s") {
            if normalized.contains(" ~") || normalized.contains(" /") || normalized.contains(" *") || normalized.contains(" ..") {
                return Some((RiskLevel::Critical, "Destructive recursive removal of root, home, or wildcards"));
            }
            return Some((RiskLevel::High, "Recursive directory deletion"));
        }

        if normalized.contains("find ") && normalized.contains("-delete") {
            return Some((RiskLevel::Critical, "Unbounded find -delete execution"));
        }

        if normalized.contains("rmtree") || normalized.contains("shutil.rmtree") {
            return Some((RiskLevel::Critical, "Scripted recursive tree deletion via Python"));
        }

        if normalized.contains("git push") && (normalized.contains("--force") || normalized.contains("-f ") || normalized.ends_with("-f")) {
            return Some((RiskLevel::High, "Force push altering remote Git history"));
        }
        if normalized.contains("git reset --hard") || normalized.contains("git clean -fdx") {
            return Some((RiskLevel::High, "Destructive Git reset discarding uncommitted code"));
        }
        if normalized.contains("npm publish") || normalized.contains("cargo publish") {
            return Some((RiskLevel::High, "Publishing package to public package registry"));
        }
        if normalized.contains("shutdown") || normalized.contains("reboot") {
            return Some((RiskLevel::High, "System power state modification"));
        }

        // 3. Network exfiltration and reverse shells
        if normalized.contains(" | nc ") || normalized.contains(" | ncat ") || normalized.contains(" | netcat ") || normalized.contains(" | socat ") {
            return Some((RiskLevel::Critical, "Piped network exfiltration via netcat/socat"));
        }
        if normalized.contains("/dev/tcp/") || normalized.contains("/dev/udp/") {
            return Some((RiskLevel::Critical, "Raw socket transmission / reverse shell vector"));
        }

        // 4. Medium risk network / script pipe execution
        if normalized.starts_with("curl ") || normalized.starts_with("wget ") || normalized.contains("| sh") || normalized.contains("| bash") {
            return Some((RiskLevel::Medium, "Remote script or web download execution"));
        }

        None
    }

    /// Checks if a path targets sensitive credentials, ssh keys, or configuration
    pub fn is_sensitive_path(path: &Path) -> bool {
        let norm = Self::normalize_lexically(path);
        let path_str = norm.to_string_lossy().to_lowercase().replace('\\', "/");

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
            "/.env",
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

        // Lexically normalize to eliminate .. path traversal on both existing and non-existing files
        let norm_target = Self::normalize_lexically(&abs_target);
        let canon_target = norm_target.canonicalize().unwrap_or(norm_target);

        let norm_root = Self::normalize_lexically(workspace_root);
        let canon_root = norm_root.canonicalize().unwrap_or(norm_root);

        if canon_target.starts_with(&canon_root) {
            return true;
        }

        for trusted in trusted_paths {
            let norm_trusted = Self::normalize_lexically(trusted);
            let canon_trusted = norm_trusted.canonicalize().unwrap_or(norm_trusted);
            if canon_target.starts_with(&canon_trusted) {
                return true;
            }
        }

        false
    }
}
