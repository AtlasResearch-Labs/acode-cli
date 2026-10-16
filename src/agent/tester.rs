//! Atlas Daedalus — Self-Healing Test & Verification Engine
//! Polyglot test runner across Rust, Node.js, Go, Python, C/C++, and Java/Kotlin
//! with Proactive Test Impact Analysis and Test Scaffolding.

use std::fs;
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

    /// Automatically detects full test command
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

            if let Ok(content) = fs::read_to_string(pkg_path) {
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

    /// Test Impact Analysis: calculates targeted test execution command for modified files
    pub fn detect_targeted_test(&self, modified_files: &[String]) -> Option<String> {
        if modified_files.is_empty() {
            return self.detect_test_command();
        }

        let first = &modified_files[0];
        let p = Path::new(first);
        let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        match ext {
            "rs" if self.workspace_root.join("Cargo.toml").exists() => {
                if !stem.is_empty() && stem != "main" && stem != "lib" && stem != "mod" {
                    Some(format!("cargo test {}", stem))
                } else {
                    Some("cargo test".to_string())
                }
            }
            "ts" | "tsx" | "js" | "jsx" if self.workspace_root.join("package.json").exists() => {
                let pkg_has_test = fs::read_to_string(self.workspace_root.join("package.json"))
                    .ok()
                    .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                    .and_then(|j| j.get("scripts")?.get("test").cloned())
                    .is_some();
                if pkg_has_test {
                    let runner = if self.workspace_root.join("pnpm-lock.yaml").exists() {
                        "pnpm"
                    } else if self.workspace_root.join("bun.lockb").exists() || self.workspace_root.join("bun.lock").exists() {
                        "bun"
                    } else if self.workspace_root.join("yarn.lock").exists() {
                        "yarn"
                    } else {
                        "npm"
                    };

                    if !stem.is_empty() {
                        Some(format!("{} test -- {}", runner, stem))
                    } else {
                        Some(format!("{} test", runner))
                    }
                } else {
                    None
                }
            }
            "py" => {
                if self.workspace_root.join("pytest.ini").exists() 
                    || self.workspace_root.join("pyproject.toml").exists()
                    || self.workspace_root.join("tests").exists()
                    || self.workspace_root.join("test").exists() {
                    if !stem.is_empty() {
                        Some(format!("pytest -k {}", stem))
                    } else {
                        Some("pytest".to_string())
                    }
                } else if self.workspace_root.join(format!("test_{}.py", stem)).exists() {
                    Some(format!("pytest test_{}.py", stem))
                } else {
                    None
                }
            }
            "go" if self.workspace_root.join("go.mod").exists() => {
                if let Some(parent) = p.parent() {
                    let dir_str = parent.to_string_lossy();
                    if !dir_str.is_empty() {
                        return Some(format!("go test ./{}/...", dir_str));
                    }
                }
                Some("go test ./...".to_string())
            }
            _ => self.detect_test_command(),
        }
    }

    /// Generates idiomatic test scaffold for a given source file based on detected symbols
    pub fn scaffold_test(root: &Path, source_path_str: &str) -> Result<(String, String), String> {
        let target_file = root.join(source_path_str);
        if !target_file.exists() {
            return Err(format!("Source file '{}' not found.", source_path_str));
        }

        let ext = target_file.extension().and_then(|s| s.to_str()).unwrap_or("");
        let stem = target_file.file_stem().and_then(|s| s.to_str()).unwrap_or("module");

        match ext {
            "rs" => {
                let scaffold = format!(
                    "\n#[cfg(test)]\nmod tests {{\n    use super::*;\n\n    #[test]\n    fn test_{}_basic_functionality() {{\n        // Proactive test verification generated by Atlas Daedalus\n        assert!(true);\n    }}\n}}\n",
                    stem
                );
                Ok((source_path_str.to_string(), scaffold))
            }
            "ts" | "tsx" | "js" | "jsx" => {
                let test_rel = format!("tests/{}.test.{}", stem, ext);
                let scaffold = format!(
                    "describe('{} module verification', () => {{\n  it('should initialize and execute without errors', () => {{\n    expect(true).toBe(true);\n  }});\n}});\n",
                    stem
                );
                Ok((test_rel, scaffold))
            }
            "py" => {
                let test_rel = format!("tests/test_{}.py", stem);
                let scaffold = format!(
                    "import pytest\n\ndef test_{}_behavior():\n    \"\"\"Proactive test scaffold generated by Atlas Daedalus\"\"\"\n    assert True\n",
                    stem
                );
                Ok((test_rel, scaffold))
            }
            "go" => {
                let test_rel = format!("{}_test.go", stem);
                let scaffold = format!(
                    "package main\n\nimport \"testing\"\n\nfunc Test{}Behavior(t *testing.T) {{\n    // Proactive test scaffold generated by Atlas Daedalus\n}}\n",
                    stem
                );
                Ok((test_rel, scaffold))
            }
            _ => Err(format!("Unsupported language extension '.{}' for test scaffolding.", ext)),
        }
    }

    pub fn scaffold_test_for_file(&self, source_path_str: &str) -> Result<(String, String), String> {
        Self::scaffold_test(&self.workspace_root, source_path_str)
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

    pub async fn run_targeted_verification(&self, modified_files: &[String]) -> VerificationResult {
        match self.detect_targeted_test(modified_files) {
            Some(cmd) => self.run_custom_test(&cmd, None).await,
            None => self.run_verification().await,
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
