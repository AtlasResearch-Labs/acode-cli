//! Atlas Daedalus — Self-Healing Test & Verification Engine

use std::path::{Path, PathBuf};
use crate::tools::shell::ShellTools;

#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub has_tests: bool,
    pub command: Option<String>,
    pub passed: bool,
    pub output: String,
    pub duration_ms: u128,
}

pub struct SelfHealingVerifier {
    workspace_root: PathBuf,
    shell: ShellTools,
}

impl SelfHealingVerifier {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        let p = workspace_root.as_ref().to_path_buf();
        Self {
            shell: ShellTools::new(&p),
            workspace_root: p,
        }
    }

    pub fn detect_test_command(&self) -> Option<String> {
        if self.workspace_root.join("Cargo.toml").exists() {
            return Some("cargo test".to_string());
        }

        let pkg_path = self.workspace_root.join("package.json");
        if pkg_path.exists() {
            if let Ok(content) = std::fs::read_to_string(pkg_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(scripts) = json.get("scripts") {
                        if scripts.get("test").is_some() {
                            return Some("npm test".to_string());
                        }
                        if scripts.get("build").is_some() {
                            return Some("npm run build".to_string());
                        }
                    }
                }
            }
        }

        if self.workspace_root.join("go.mod").exists() {
            return Some("go test ./...".to_string());
        }

        if self.workspace_root.join("pytest.ini").exists() || self.workspace_root.join("pyproject.toml").exists() {
            return Some("pytest".to_string());
        }

        None
    }

    pub async fn run_verification(&self) -> VerificationResult {
        let cmd = match self.detect_test_command() {
            Some(c) => c,
            None => {
                return VerificationResult {
                    has_tests: false,
                    command: None,
                    passed: true,
                    output: "No automated test framework detected in workspace.".to_string(),
                    duration_ms: 0,
                };
            }
        };

        let res = self.shell.run_command(&cmd, None).await;
        let mut out = res.stdout;
        if !res.stderr.is_empty() {
            out.push_str("\n");
            out.push_str(&res.stderr);
        }

        VerificationResult {
            has_tests: true,
            command: Some(cmd),
            passed: res.success,
            output: out.trim().to_string(),
            duration_ms: res.duration_ms,
        }
    }
}
