//! Atlas Daedalus — Git Tools

use std::path::Path;
use crate::tools::shell::ShellTools;

#[derive(Debug, Clone)]
pub struct GitStatusResult {
    pub branch: String,
    pub clean: bool,
    pub modified: Vec<String>,
    pub untracked: Vec<String>,
}

pub struct GitTools {
    shell: ShellTools,
}

impl GitTools {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            shell: ShellTools::new(workspace_root),
        }
    }

    pub async fn get_status(&self) -> GitStatusResult {
        let res = self.shell.run_command("git status --short --branch", None).await;
        if !res.success {
            return GitStatusResult {
                branch: String::new(),
                clean: true,
                modified: Vec::new(),
                untracked: Vec::new(),
            };
        }

        let lines: Vec<&str> = res.stdout.lines().collect();
        let branch = lines.first()
            .and_then(|l| l.strip_prefix("## "))
            .and_then(|b| b.split("...").next())
            .unwrap_or("HEAD")
            .trim()
            .to_string();

        let mut modified = Vec::new();
        let mut untracked = Vec::new();

        for line in lines.iter().skip(1) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with("??") {
                untracked.push(trimmed[2..].trim().to_string());
            } else if trimmed.len() > 3 {
                modified.push(trimmed[2..].trim().to_string());
            }
        }

        let clean = modified.is_empty() && untracked.is_empty();

        GitStatusResult {
            branch,
            clean,
            modified,
            untracked,
        }
    }

    pub async fn get_diff(&self, staged_only: bool) -> String {
        let cmd = if staged_only {
            "git --no-pager diff --cached"
        } else {
            "git --no-pager diff"
        };
        let res = self.shell.run_command(cmd, None).await;
        res.stdout
    }

    pub async fn commit(&self, message: &str, author: &str) -> Result<String, String> {
        let _ = self.shell.run_command("git add -A", None).await;
        let clean_msg = message.replace('"', "\\\"");
        let cmd = format!("git commit -m \"{}\" --author=\"{}\"", clean_msg, author);
        let res = self.shell.run_command(&cmd, None).await;

        if res.success {
            let hash_res = self.shell.run_command("git rev-parse --short HEAD", None).await;
            Ok(hash_res.stdout.trim().to_string())
        } else {
            Err(res.stderr)
        }
    }
}
