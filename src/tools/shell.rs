//! Atlas Daedalus — Async Shell Tools

use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct ExecResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u128,
}

pub struct ShellTools {
    workspace_root: PathBuf,
}

impl ShellTools {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
    }

    pub async fn run_command(&self, command: &str, cwd: Option<&str>) -> ExecResult {
        if let Err(threat) = crate::security::threat_shield::ThreatShield::audit_command(command) {
            return ExecResult {
                success: false,
                stdout: String::new(),
                stderr: format!(
                    "[THREAT_SHIELD INTERCEPT: {}] {}",
                    threat.threat_id, threat.description
                ),
                exit_code: 126,
                duration_ms: 0,
            };
        }

        let start = Instant::now();
        let target_cwd = match cwd {
            Some(d) => Path::new(d).to_path_buf(),
            None => self.workspace_root.clone(),
        };

        let is_win = cfg!(target_os = "windows");
        let mut cmd = if is_win {
            let mut c = Command::new("powershell.exe");
            c.arg("-NoProfile").arg("-Command").arg(command);
            c
        } else {
            // Check if bubblewrap sandbox containment is available on Linux host
            let bwrap_bin = if !cfg!(target_os = "android") {
                ["/usr/bin/bwrap", "/bin/bwrap", "/usr/local/bin/bwrap"]
                    .iter()
                    .find(|p| Path::new(p).exists())
                    .map(|p| (*p).to_string())
            } else {
                None
            };

            if let Some(bwrap) = bwrap_bin {
                let mut c = Command::new(bwrap);
                let ws_str = self.workspace_root.to_string_lossy().to_string();
                c.arg("--ro-bind")
                    .arg("/")
                    .arg("/")
                    .arg("--dev")
                    .arg("/dev")
                    .arg("--proc")
                    .arg("/proc")
                    .arg("--tmpfs")
                    .arg("/tmp")
                    .arg("--bind")
                    .arg(&ws_str)
                    .arg(&ws_str)
                    .arg("--die-with-parent")
                    .arg("/bin/sh")
                    .arg("-c")
                    .arg(command);
                c
            } else {
                let mut c = Command::new("/bin/sh");
                c.arg("-c").arg(command);
                c
            }
        };

        cmd.current_dir(target_cwd);

        match tokio::time::timeout(std::time::Duration::from_secs(120), cmd.output()).await {
            Ok(Ok(output)) => {
                let duration_ms = start.elapsed().as_millis();
                let exit_code = output.status.code().unwrap_or(1);
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                ExecResult {
                    success: exit_code == 0,
                    stdout,
                    stderr,
                    exit_code,
                    duration_ms,
                }
            }
            Ok(Err(e)) => {
                let duration_ms = start.elapsed().as_millis();
                ExecResult {
                    success: false,
                    stdout: String::new(),
                    stderr: format!("Failed to spawn process: {}", e),
                    exit_code: 1,
                    duration_ms,
                }
            }
            Err(_) => {
                let duration_ms = start.elapsed().as_millis();
                ExecResult {
                    success: false,
                    stdout: String::new(),
                    stderr: "Process execution timed out after 120 seconds.".to_string(),
                    exit_code: 124,
                    duration_ms,
                }
            }
        }
    }
}
