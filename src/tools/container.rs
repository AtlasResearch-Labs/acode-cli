//! Atlas Daedalus — Container & Microservice Engine (Hephaestus Architecture)
//! Direct control over Docker and Podman container lifecycles, log streaming,
//! port discovery, and compose orchestrations without manual terminal guesswork.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerSummary {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: String,
    pub ports: String,
}

pub struct ContainerManager;

impl ContainerManager {
    pub fn new() -> Self {
        Self
    }

    /// Detects whether `docker` or `podman` CLI is accessible
    pub fn detect_cli() -> Option<&'static str> {
        let is_win = cfg!(target_os = "windows");
        let check = |cli: &str| -> bool {
            let mut cmd = if is_win {
                let mut c = Command::new("powershell.exe");
                c.arg("-NoProfile")
                    .arg("-Command")
                    .arg(format!("Get-Command {} -ErrorAction SilentlyContinue", cli));
                c
            } else {
                let mut c = Command::new("which");
                c.arg(cli);
                c
            };
            cmd.output()
                .map(|o| o.status.success() && !o.stdout.is_empty())
                .unwrap_or(false)
        };

        if check("docker") {
            Some("docker")
        } else if check("podman") {
            Some("podman")
        } else {
            None
        }
    }

    /// Checks daemon ping/status
    pub fn check_status(&self) -> Result<String, String> {
        let cli = Self::detect_cli().ok_or_else(|| {
            "Neither 'docker' nor 'podman' executable found in system PATH.".to_string()
        })?;

        let res = Self::run_raw(
            cli,
            &["info", "--format", "{{.ServerVersion}}"],
            Path::new("."),
        );
        match res {
            Ok(ver) if !ver.is_empty() => Ok(format!(
                "{} daemon active (Server Version: {})",
                cli,
                ver.trim()
            )),
            _ => {
                // Fallback check
                let fallback = Self::run_raw(cli, &["version"], Path::new("."));
                match fallback {
                    Ok(_) => Ok(format!("{} CLI is available and responsive.", cli)),
                    Err(e) => Err(format!(
                        "{} is installed but daemon is unreachable: {}",
                        cli, e
                    )),
                }
            }
        }
    }

    /// Lists running and stopped containers
    pub fn list_containers(&self, all: bool) -> Result<Vec<ContainerSummary>, String> {
        let cli = Self::detect_cli()
            .ok_or_else(|| "No container runtime (docker/podman) available.".to_string())?;

        let mut args = vec![
            "ps",
            "--format",
            "{{.ID}}|{{.Names}}|{{.Image}}|{{.Status}}|{{.State}}|{{.Ports}}",
        ];
        if all {
            args.insert(1, "-a");
        }

        let output = Self::run_raw(cli, &args, Path::new("."))?;
        let mut list = Vec::new();

        for line in output.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 6 {
                list.push(ContainerSummary {
                    id: parts[0].trim().to_string(),
                    name: parts[1].trim().to_string(),
                    image: parts[2].trim().to_string(),
                    status: parts[3].trim().to_string(),
                    state: parts[4].trim().to_string(),
                    ports: parts[5].trim().to_string(),
                });
            } else if parts.len() >= 4 {
                list.push(ContainerSummary {
                    id: parts[0].trim().to_string(),
                    name: parts[1].trim().to_string(),
                    image: parts[2].trim().to_string(),
                    status: parts[3].trim().to_string(),
                    state: "unknown".to_string(),
                    ports: String::new(),
                });
            }
        }

        Ok(list)
    }

    /// Fetches recent stdout/stderr logs from a container
    pub fn get_logs(&self, target: &str, line_count: usize) -> Result<String, String> {
        let cli =
            Self::detect_cli().ok_or_else(|| "No container runtime available.".to_string())?;

        let lines_str = line_count.to_string();
        let args = ["logs", "--tail", &lines_str, target];
        Self::run_raw(cli, &args, Path::new("."))
    }

    /// Inspects container configuration in concise summary
    pub fn inspect_container(&self, target: &str) -> Result<String, String> {
        let cli =
            Self::detect_cli().ok_or_else(|| "No container runtime available.".to_string())?;

        let args = [
            "inspect",
            "--format",
            "ID: {{.Id}}\nName: {{.Name}}\nState: {{.State.Status}}\nIP: {{range .NetworkSettings.Networks}}{{.IPAddress}} {{end}}\nPorts: {{json .NetworkSettings.Ports}}",
            target,
        ];

        Self::run_raw(cli, &args, Path::new("."))
    }

    /// Launches docker-compose or podman-compose up
    pub fn compose_up(&self, cwd: &Path) -> Result<String, String> {
        let cli =
            Self::detect_cli().ok_or_else(|| "No container runtime available.".to_string())?;

        if cli == "docker" {
            // Try 'docker compose up -d' first
            match Self::run_raw("docker", &["compose", "up", "-d"], cwd) {
                Ok(out) => Ok(out),
                Err(_) => Self::run_raw("docker-compose", &["up", "-d"], cwd),
            }
        } else {
            Self::run_raw("podman-compose", &["up", "-d"], cwd)
        }
    }

    /// Tears down docker-compose or podman-compose
    pub fn compose_down(&self, cwd: &Path) -> Result<String, String> {
        let cli =
            Self::detect_cli().ok_or_else(|| "No container runtime available.".to_string())?;

        if cli == "docker" {
            match Self::run_raw("docker", &["compose", "down"], cwd) {
                Ok(out) => Ok(out),
                Err(_) => Self::run_raw("docker-compose", &["down"], cwd),
            }
        } else {
            Self::run_raw("podman-compose", &["down"], cwd)
        }
    }

    /// Stops a specific container
    pub fn stop_container(&self, target: &str) -> Result<String, String> {
        let cli =
            Self::detect_cli().ok_or_else(|| "No container runtime available.".to_string())?;

        Self::run_raw(cli, &["stop", target], Path::new("."))
    }

    /// Restarts a specific container
    pub fn restart_container(&self, target: &str) -> Result<String, String> {
        let cli =
            Self::detect_cli().ok_or_else(|| "No container runtime available.".to_string())?;

        Self::run_raw(cli, &["restart", target], Path::new("."))
    }

    fn run_raw(cmd_name: &str, args: &[&str], cwd: &Path) -> Result<String, String> {
        let is_win = cfg!(target_os = "windows");
        let mut command = if is_win {
            let mut c = Command::new("powershell.exe");
            let full = format!("{} {}", cmd_name, args.join(" "));
            c.arg("-NoProfile").arg("-Command").arg(&full);
            c
        } else {
            let mut c = Command::new(cmd_name);
            c.args(args);
            c
        };

        command.current_dir(cwd);

        match command.output() {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                if out.status.success() {
                    let mut combined = stdout;
                    if !stderr.is_empty() {
                        if !combined.is_empty() {
                            combined.push('\n');
                        }
                        combined.push_str(&stderr);
                    }
                    Ok(combined.trim().to_string())
                } else {
                    let msg = if !stderr.is_empty() { stderr } else { stdout };
                    Err(msg.trim().to_string())
                }
            }
            Err(e) => Err(format!("Execution failed: {}", e)),
        }
    }
}

impl Default for ContainerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_manager_instantiation() {
        let mgr = ContainerManager::new();
        // Verifies graceful return when docker or podman is evaluated
        let _ = mgr.check_status();
    }
}
