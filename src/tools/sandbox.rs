//! Atlas Daedalus — Shadow Workspace Git Worktree Sandbox (Ironclad Architecture)
//! Creates isolated, transient git worktrees in `.daedalus/shadow/` for zero-risk
//! autonomous coding, refactoring, and test healing without dirtying the user's working tree.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowSession {
    pub session_id: String,
    pub branch_name: String,
    pub worktree_path: PathBuf,
    pub original_root: PathBuf,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowSessionInfo {
    pub session_id: String,
    pub branch: String,
    pub path: String,
    pub is_clean: bool,
}

pub struct ShadowSandbox {
    pub workspace_root: PathBuf,
}

impl ShadowSandbox {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
    }

    /// Verifies if workspace is inside an initialized git repository
    pub fn is_git_repo(&self) -> bool {
        let out = Self::run_git(&self.workspace_root, &["rev-parse", "--is-inside-work-tree"]);
        out.map(|s| s.trim() == "true").unwrap_or(false)
    }

    /// Creates an isolated shadow git worktree
    pub fn create_sandbox(&self, custom_name: Option<&str>) -> Result<ShadowSession, String> {
        if !self.is_git_repo() {
            return Err("Cannot create sandbox: active workspace is not a git repository.".to_string());
        }

        let shadow_dir = self.workspace_root.join(".daedalus").join("shadow");
        let _ = fs::create_dir_all(&shadow_dir);

        // Ensure .daedalus/shadow is ignored locally
        self.ensure_shadow_git_ignored();

        let id = custom_name
            .map(|s| s.replace(|c: char| !c.is_alphanumeric() && c != '-', "_"))
            .unwrap_or_else(|| {
                use std::time::{SystemTime, UNIX_EPOCH};
                let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                format!("wt_{:x}", now)
            });

        let branch_name = format!("daedalus-shadow-{}", id);
        let worktree_path = shadow_dir.join(&id);

        // Remove old worktree folder if leftover
        if worktree_path.exists() {
            let _ = Self::run_git(&self.workspace_root, &["worktree", "remove", "--force", &worktree_path.to_string_lossy()]);
            let _ = fs::remove_dir_all(&worktree_path);
        }

        // Delete leftover branch if exists
        let _ = Self::run_git(&self.workspace_root, &["branch", "-D", &branch_name]);

        // Create worktree with dedicated branch
        let wt_path_str = worktree_path.to_string_lossy().to_string();
        let res = Self::run_git(
            &self.workspace_root,
            &["worktree", "add", "-b", &branch_name, &wt_path_str],
        )?;

        let session = ShadowSession {
            session_id: id,
            branch_name,
            worktree_path: worktree_path.canonicalize().unwrap_or(worktree_path),
            original_root: self.workspace_root.clone(),
            is_active: true,
        };

        let _ = res;
        Ok(session)
    }

    /// Merges changes from shadow worktree back into primary workspace,
    /// preserving all untracked files, multiple commits, modifications, and renames.
    pub fn merge_sandbox(&self, session: &ShadowSession) -> Result<String, String> {
        if !session.worktree_path.exists() {
            return Err(format!("Shadow worktree path '{}' does not exist.", session.worktree_path.display()));
        }

        // 1. Stage all untracked and modified files inside the shadow worktree
        let _ = Self::run_git(&session.worktree_path, &["add", "-A"]);

        // 2. Commit any staged changes in the shadow worktree so all work is preserved on session.branch_name
        let status = Self::run_git(&session.worktree_path, &["status", "--porcelain"])?;
        if !status.trim().is_empty() {
            let _ = Self::run_git(
                &session.worktree_path,
                &[
                    "-c", "user.name=Atlas Daedalus",
                    "-c", "user.email=daedalus@atlasresearchlabs.online",
                    "commit", "-m", "daedalus shadow worktree auto-commit",
                    "--no-verify",
                ],
            );
        }

        // 3. Generate full binary diff between workspace HEAD and the shadow branch
        // This diff captures all changes across all commits and untracked files added in the shadow worktree
        let diff = Self::run_git(
            &self.workspace_root,
            &["--no-pager", "diff", "--binary", "HEAD", &session.branch_name],
        )?;

        if diff.trim().is_empty() {
            self.discard_sandbox(session)?;
            return Ok("No modifications detected in shadow sandbox. Teardown complete.".to_string());
        }

        // 4. Apply the comprehensive diff to primary workspace root
        let daedalus_dir = self.workspace_root.join(".daedalus");
        let _ = fs::create_dir_all(&daedalus_dir);
        let patch_file = daedalus_dir.join("shadow.patch");
        fs::write(&patch_file, diff.as_bytes()).map_err(|e| format!("Failed to write patch: {}", e))?;

        let apply_res = Self::run_git(
            &self.workspace_root,
            &["apply", "--whitespace=nowarn", "--binary", &patch_file.to_string_lossy()],
        );

        let _ = fs::remove_file(&patch_file);

        match apply_res {
            Ok(_) => {
                let _ = self.discard_sandbox(session);
                Ok("Successfully merged and applied all shadow changes to active working tree.".to_string())
            }
            Err(e) => Err(format!(
                "Git apply failed: {}. Shadow worktree preserved at {}",
                e,
                session.worktree_path.display()
            )),
        }
    }

    /// Tears down shadow worktree and deletes shadow branch without modifying original workspace
    pub fn discard_sandbox(&self, session: &ShadowSession) -> Result<String, String> {
        let wt_str = session.worktree_path.to_string_lossy().to_string();
        let _ = Self::run_git(&self.workspace_root, &["worktree", "remove", "--force", &wt_str]);

        if session.worktree_path.exists() {
            let _ = fs::remove_dir_all(&session.worktree_path);
        }

        let _ = Self::run_git(&self.workspace_root, &["branch", "-D", &session.branch_name]);
        let _ = Self::run_git(&self.workspace_root, &["worktree", "prune"]);

        Ok(format!("Shadow sandbox '{}' discarded cleanly.", session.session_id))
    }

    /// Lists active shadow worktrees
    pub fn list_sandboxes(&self) -> Result<Vec<ShadowSessionInfo>, String> {
        if !self.is_git_repo() {
            return Ok(Vec::new());
        }

        let raw = Self::run_git(&self.workspace_root, &["worktree", "list", "--porcelain"])?;
        let mut list = Vec::new();
        let mut cur_path = String::new();
        let mut cur_branch = String::new();

        for line in raw.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                cur_path = p.trim().to_string();
            } else if let Some(b) = line.strip_prefix("branch ") {
                cur_branch = b.trim().to_string();
            } else if line.is_empty() {
                if cur_branch.contains("daedalus-shadow-") {
                    let id = cur_branch.replace("refs/heads/daedalus-shadow-", "").replace("daedalus-shadow-", "");
                    list.push(ShadowSessionInfo {
                        session_id: id,
                        branch: cur_branch.clone(),
                        path: cur_path.clone(),
                        is_clean: true,
                    });
                }
                cur_path.clear();
                cur_branch.clear();
            }
        }

        if cur_branch.contains("daedalus-shadow-") {
            let id = cur_branch.replace("refs/heads/daedalus-shadow-", "").replace("daedalus-shadow-", "");
            list.push(ShadowSessionInfo {
                session_id: id,
                branch: cur_branch,
                path: cur_path,
                is_clean: true,
            });
        }

        Ok(list)
    }

    fn ensure_shadow_git_ignored(&self) {
        let exclude_file = self.workspace_root.join(".git").join("info").join("exclude");
        if exclude_file.exists() {
            if let Ok(content) = fs::read_to_string(&exclude_file) {
                if !content.contains(".daedalus/shadow") {
                    use std::io::Write;
                    if let Ok(mut f) = fs::OpenOptions::new().append(true).open(&exclude_file) {
                        let _ = writeln!(f, "\n.daedalus/shadow");
                    }
                }
            }
        }
    }

    fn run_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
        let mut cmd = Command::new("git");
        cmd.args(args);
        cmd.current_dir(cwd);

        match cmd.output() {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                if out.status.success() {
                    Ok(stdout)
                } else {
                    let msg = if !stderr.is_empty() { stderr } else { stdout };
                    Err(msg.trim().to_string())
                }
            }
            Err(e) => {
                #[cfg(target_os = "windows")]
                {
                    let mut ps_cmd = Command::new("powershell.exe");
                    let full = format!("git {}", args.join(" "));
                    ps_cmd.arg("-NoProfile").arg("-Command").arg(&full);
                    ps_cmd.current_dir(cwd);
                    if let Ok(out) = ps_cmd.output() {
                        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                        if out.status.success() {
                            return Ok(stdout);
                        } else {
                            let msg = if !stderr.is_empty() { stderr } else { stdout };
                            return Err(msg.trim().to_string());
                        }
                    }
                }
                Err(format!("Git execution error: {}", e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_instantiation() {
        let sandbox = ShadowSandbox::new(".");
        let is_repo = sandbox.is_git_repo();
        // In this repository root, git status should be true
        assert!(is_repo);
    }

    #[test]
    fn test_sandbox_lifecycle() {
        let sandbox = ShadowSandbox::new(".");
        if !sandbox.is_git_repo() {
            return;
        }

        // Test create
        let session = sandbox.create_sandbox(Some("ci_test_unit")).expect("Failed to create test sandbox");
        assert_eq!(session.session_id, "ci_test_unit");
        assert!(session.worktree_path.exists());

        // Test list
        let list = sandbox.list_sandboxes().expect("Failed to list sandboxes");
        assert!(list.iter().any(|s| s.session_id == "ci_test_unit"));

        // Test discard
        let res = sandbox.discard_sandbox(&session);
        assert!(res.is_ok());
        assert!(!session.worktree_path.exists());

        // Test list after discard
        let list_after = sandbox.list_sandboxes().expect("Failed to list sandboxes after discard");
        assert!(!list_after.iter().any(|s| s.session_id == "ci_test_unit"));
    }

    #[test]
    fn test_sandbox_merge_with_untracked_files() {
        let sandbox = ShadowSandbox::new(".");
        if !sandbox.is_git_repo() {
            return;
        }

        let session = sandbox.create_sandbox(Some("merge_test_unit")).expect("Failed to create test sandbox");
        assert!(session.worktree_path.exists());

        // Create a new untracked file in the shadow worktree
        let test_file = session.worktree_path.join("shadow_untracked_probe.tmp");
        fs::write(&test_file, b"daedalus shadow worktree test content").expect("Failed to write probe file");
        assert!(test_file.exists());

        // Merge sandbox - must stage untracked files, auto-commit, diff, and apply to main workspace
        let merge_res = sandbox.merge_sandbox(&session);
        assert!(merge_res.is_ok(), "Merge failed: {:?}", merge_res);

        // Verify the untracked file was applied to main workspace
        let main_applied = Path::new(".").join("shadow_untracked_probe.tmp");
        assert!(main_applied.exists(), "Merged untracked file was not applied to main workspace");
        let content = fs::read_to_string(&main_applied).unwrap_or_default();
        assert_eq!(content, "daedalus shadow worktree test content");

        // Clean up probe file
        let _ = fs::remove_file(main_applied);
        assert!(!session.worktree_path.exists());
    }
}
