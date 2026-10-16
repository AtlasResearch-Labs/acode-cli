//! Atlas Daedalus — Self-Healing Test & Verification Engine
//! Polyglot test runner across Rust, Node.js, Go, Python, C/C++, and Java/Kotlin.

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
        // Rust
        if self.workspace_root.join("Cargo.toml").exists() {
            return Some("cargo test".to_string());
        }

        // Node.js / TypeScript
        let pkg_path = self.workspace_root.join("package.json");
        if pkg_path.exists() {
            let runner = if self.workspace_root.join("pnpm-lock.yaml").exists() {
                "pnpm"
            } else if self.workspace_root.join("bun.lockb").exists() || self.workspace_root.join("bun.lock").exists() {
                "bun"
            } else if self.workspace_root.join("yarn.lock").exists() {
                "yarn"
            } else {
                "npm"
            };

            if let Ok(content) = std::fs::read_to_string(pkg_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(scripts) = json.get("scripts") {
                        if scripts.get("test").is_some() {
                            return Some(format!("{} test", runner));
                        }
                        if scripts.get("build").is_some() {
                            return Some(format!("{} run build", runner));
                        }
                    }
                }
            }
        }

        // Go
        if self.workspace_root.join("go.mod").exists() {
            return Some("go test ./...".to_string());
        }

        // Python
        if self.workspace_root.join("pytest.ini").exists() 
            || self.workspace_root.join("pyproject.toml").exists()
            || self.workspace_root.join("tests").exists()
            || self.workspace_root.join("test").exists() {
            return Some("pytest".to_string());
        }

        // C / C++ / Make
        if self.workspace_root.join("Makefile").exists() || self.workspace_root.join("makefile").exists() {
            return Some("make test".to_string());
        }

        // Java / Kotlin
        if self.workspace_root.join("pom.xml").exists() {
            return Some("mvn test".to_string());
        }
        if self.workspace_root.join("build.gradle").exists() || self.workspace_root.join("build.gradle.kts").exists() {
            return Some("./gradlew test".to_string());
        }

        None
    }

    pub async fn run_verification(&self) -> VerificationResult {
        match self.detect_test_command() {
            Some(cmd) => self.run_custom_test(&cmd, None).await,
            None => VerificationResult {
                has_tests: false,
                command: None,
                passed: true,
                output: "No automated test framework detected in workspace.".to_string(),
                duration_ms: 0,
            },
        }
    }

    pub async fn run_custom_test(&self, cmd: &str, cwd: Option<&str>) -> VerificationResult {
        let res = self.shell.run_command(cmd, cwd).await;
        let mut out = res.stdout;
        if !res.stderr.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&res.stderr);
        }

        VerificationResult {
            has_tests: true,
            command: Some(cmd.to_string()),
            passed: res.success,
            output: out.trim().to_string(),
            duration_ms: res.duration_ms,
        }
    }
}
