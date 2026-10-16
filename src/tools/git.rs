//! Atlas Daedalus — Git Tools

use crate::tools::shell::ShellTools;
use std::path::{Path, PathBuf};
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct GitStatusResult {
    pub is_repo: bool,
    pub branch: String,
    pub clean: bool,
    pub modified: Vec<String>,
    pub untracked: Vec<String>,
}

pub struct GitTools {
    shell: ShellTools,
    workspace_root: PathBuf,
}

impl GitTools {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        let p = workspace_root.as_ref().to_path_buf();
        Self {
            shell: ShellTools::new(&p),
            workspace_root: p,
        }
    }

    pub async fn get_status(&self) -> GitStatusResult {
        let res = self
            .shell
            .run_command("git status --short --branch", None)
            .await;
        if !res.success {
            return GitStatusResult {
                is_repo: false,
                branch: String::new(),
                clean: false,
                modified: Vec::new(),
                untracked: Vec::new(),
            };
        }

        let lines: Vec<&str> = res.stdout.lines().collect();
        let branch = lines
            .first()
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
            if let Some(stripped) = trimmed.strip_prefix("??") {
                untracked.push(stripped.trim().to_string());
            } else if trimmed.len() > 3 {
                let rest = trimmed.chars().skip(2).collect::<String>();
                modified.push(rest.trim().to_string());
            }
        }

        let clean = modified.is_empty() && untracked.is_empty();

        GitStatusResult {
            is_repo: true,
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
        // Verify there are staged changes before committing (never run blind git add behind user's back)
        let staged_diff = self.get_diff(true).await;
        if staged_diff.trim().is_empty() {
            return Err(
                "No staged changes to commit. Stage your changes with 'git add <path>' first."
                    .to_string(),
            );
        }

        let mut cmd = Command::new("git");
        cmd.arg("commit")
            .arg("-m")
            .arg(message)
            .arg(format!("--author={}", author))
            .current_dir(&self.workspace_root);

        match cmd.output().await {
            Ok(output) => {
                if output.status.success() {
                    let mut rev_cmd = Command::new("git");
                    rev_cmd
                        .arg("rev-parse")
                        .arg("--short")
                        .arg("HEAD")
                        .current_dir(&self.workspace_root);
                    if let Ok(rev_out) = rev_cmd.output().await {
                        Ok(String::from_utf8_lossy(&rev_out.stdout).trim().to_string())
                    } else {
                        Ok("HEAD".to_string())
                    }
                } else {
                    let err = String::from_utf8_lossy(&output.stderr).to_string();
                    if err.is_empty() {
                        Err(String::from_utf8_lossy(&output.stdout).to_string())
                    } else {
                        Err(err)
                    }
                }
            }
            Err(e) => Err(format!("Failed to execute git commit: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_git_tools_isolated_repo() {
        let temp = tempfile::tempdir().expect("Failed to create tempdir");
        let root = temp.path();

        let git_tools = GitTools::new(root);
        let status_pre = git_tools.get_status().await;
        assert!(!status_pre.is_repo);

        let _ = tokio::process::Command::new("git")
            .arg("init")
            .current_dir(root)
            .output()
            .await;
        let _ = tokio::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(root)
            .output()
            .await;
        let _ = tokio::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(root)
            .output()
            .await;

        let status_init = git_tools.get_status().await;
        assert!(status_init.is_repo);
        assert!(status_init.clean);

        let file_path = root.join("test.txt");
        let _ = std::fs::write(&file_path, b"hello");

        let status_untracked = git_tools.get_status().await;
        assert!(status_untracked.is_repo);
        assert!(!status_untracked.clean);
        assert!(status_untracked.untracked.contains(&"test.txt".to_string()));

        let _ = tokio::process::Command::new("git")
            .args(["add", "test.txt"])
            .current_dir(root)
            .output()
            .await;
        let commit_res = git_tools
            .commit("Initial commit", "Test <test@example.com>")
            .await;
        assert!(commit_res.is_ok());

        let status_clean = git_tools.get_status().await;
        assert!(status_clean.clean);
    }
}
