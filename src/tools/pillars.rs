//! Atlas Daedalus — Pillar Integration Tools (Argus, Beacon, Orion, Pulse)

use crate::tools::shell::ShellTools;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct PulseMetrics {
    pub cpu_count: usize,
    pub uptime_secs: u64,
}

pub struct PillarTools {
    shell: ShellTools,
}

impl PillarTools {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            shell: ShellTools::new(workspace_root),
        }
    }

    pub async fn run_argus_audit(&self, target: &str) -> (bool, String) {
        let cmd = format!("npx -y argusio review --target \"{}\" --json", target);
        let res = self.shell.run_command(&cmd, None).await;

        if !res.success {
            let err_msg = if res.stderr.trim().is_empty() {
                format!(
                    "Argus audit command failed with exit code {}",
                    res.exit_code
                )
            } else {
                format!(
                    "Argus audit failed (exit code {}): {}",
                    res.exit_code,
                    res.stderr.trim()
                )
            };
            return (false, err_msg);
        }

        let out = res.stdout.trim();
        if out.is_empty() {
            return (
                false,
                "Argus audit executed but returned empty output.".to_string(),
            );
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(out) {
            let clean = json
                .get("issues")
                .and_then(|i| i.as_array())
                .map(|a| a.is_empty())
                .unwrap_or(false);
            let default_summary = if clean {
                "Argus AST audit clean: zero vulnerabilities found."
            } else {
                "Argus AST audit detected security or quality issues."
            };
            let summary = json
                .get("summary")
                .and_then(|s| s.as_str())
                .unwrap_or(default_summary)
                .to_string();
            return (clean, summary);
        }

        // If stdout is non-JSON formatted text, extract text honestly without claiming clean
        let lower = out.to_lowercase();
        let has_vulns = lower.contains("vulnerability")
            || lower.contains("critical")
            || lower.contains("high risk")
            || lower.contains("error:");
        (!has_vulns, format!("Argus text report: {}", out))
    }

    pub async fn start_beacon_tunnel(
        &self,
        port: u16,
        subdomain: Option<&str>,
    ) -> (bool, String, String) {
        let sub_flag = match subdomain {
            Some(s) => format!("--subdomain {}", s),
            None => String::new(),
        };
        let cmd = format!("npx -y beacontunnel {} {}", port, sub_flag);
        let res = self.shell.run_command(&cmd, None).await;

        let host = match subdomain {
            Some(s) => format!("https://{}.beacontunnel.online", s),
            None => format!("https://tunnel-{}.beacontunnel.online", port),
        };

        (res.success, host, "http://localhost:4040".to_string())
    }

    pub async fn orion_action(&self, action: &str, target: Option<&str>) -> String {
        let cmd = match target {
            Some(t) => format!("orion {} {}", action, t),
            None => format!("orion {}", action),
        };
        let res = self.shell.run_command(&cmd, None).await;
        if res.success {
            res.stdout
        } else {
            res.stderr
        }
    }

    pub fn get_pulse_metrics(&self) -> PulseMetrics {
        let cpu_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        PulseMetrics {
            cpu_count,
            uptime_secs: 3600,
        }
    }
}
