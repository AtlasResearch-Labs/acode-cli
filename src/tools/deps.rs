//! Atlas Daedalus — Unified Dependency Manager (Hephaestus Architecture)
//! Direct interface to project ecosystems (Cargo, NPM, PNPM, Bun, Yarn, Pip, UV, Poetry, GoMod)
//! for instant dependency listing, addition, removal, and vulnerability audits.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ecosystem {
    Cargo,
    Npm,
    Pnpm,
    Bun,
    Yarn,
    Pip,
    Uv,
    Poetry,
    GoMod,
}

impl Ecosystem {
    pub fn as_str(&self) -> &'static str {
        match self {
            Ecosystem::Cargo => "cargo",
            Ecosystem::Npm => "npm",
            Ecosystem::Pnpm => "pnpm",
            Ecosystem::Bun => "bun",
            Ecosystem::Yarn => "yarn",
            Ecosystem::Pip => "pip",
            Ecosystem::Uv => "uv",
            Ecosystem::Poetry => "poetry",
            Ecosystem::GoMod => "go",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInfo {
    pub name: String,
    pub version: String,
    pub is_dev: bool,
}

pub struct DependencyManager {
    pub workspace_root: PathBuf,
}

impl DependencyManager {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
    }

    pub fn detect_ecosystem(&self, root: &Path) -> Option<Ecosystem> {
        if root.join("Cargo.toml").exists() {
            return Some(Ecosystem::Cargo);
        }
        if root.join("package.json").exists() {
            if root.join("bun.lockb").exists() || root.join("bun.lock").exists() {
                return Some(Ecosystem::Bun);
            }
            if root.join("pnpm-lock.yaml").exists() {
                return Some(Ecosystem::Pnpm);
            }
            if root.join("yarn.lock").exists() {
                return Some(Ecosystem::Yarn);
            }
            return Some(Ecosystem::Npm);
        }
        if root.join("go.mod").exists() {
            return Some(Ecosystem::GoMod);
        }
        if root.join("pyproject.toml").exists() {
            if root.join("uv.lock").exists() {
                return Some(Ecosystem::Uv);
            }
            if root.join("poetry.lock").exists() {
                return Some(Ecosystem::Poetry);
            }
            return Some(Ecosystem::Pip);
        }
        if root.join("requirements.txt").exists() {
            return Some(Ecosystem::Pip);
        }
        None
    }

    pub fn list_dependencies(&self, root: &Path) -> Result<Vec<DependencyInfo>, String> {
        let eco = self.detect_ecosystem(root)
            .ok_or_else(|| "No recognized package manifest (Cargo.toml, package.json, requirements.txt, go.mod) found.".to_string())?;

        match eco {
            Ecosystem::Cargo => Self::parse_cargo_deps(&root.join("Cargo.toml")),
            Ecosystem::Npm | Ecosystem::Pnpm | Ecosystem::Bun | Ecosystem::Yarn => {
                Self::parse_npm_deps(&root.join("package.json"))
            }
            Ecosystem::Pip | Ecosystem::Uv | Ecosystem::Poetry => {
                if root.join("requirements.txt").exists() {
                    Self::parse_pip_requirements(&root.join("requirements.txt"))
                } else if root.join("pyproject.toml").exists() {
                    Self::parse_pyproject_deps(&root.join("pyproject.toml"))
                } else {
                    Ok(Vec::new())
                }
            }
            Ecosystem::GoMod => Self::parse_go_deps(&root.join("go.mod")),
        }
    }

    pub fn add_dependency(
        &self,
        root: &Path,
        package: &str,
        version: Option<&str>,
        is_dev: bool,
    ) -> Result<String, String> {
        let eco = self
            .detect_ecosystem(root)
            .ok_or_else(|| "Cannot add dependency: unknown project ecosystem.".to_string())?;

        let spec = match version {
            Some(v) if !v.is_empty() => format!("{}@{}", package, v),
            _ => package.to_string(),
        };

        let (cmd, args) = match eco {
            Ecosystem::Cargo => {
                let mut a = vec!["add".to_string(), spec];
                if is_dev {
                    a.push("--dev".to_string());
                }
                ("cargo", a)
            }
            Ecosystem::Bun => {
                let mut a = vec!["add".to_string(), spec];
                if is_dev {
                    a.push("-d".to_string());
                }
                ("bun", a)
            }
            Ecosystem::Pnpm => {
                let mut a = vec!["add".to_string(), spec];
                if is_dev {
                    a.push("-D".to_string());
                }
                ("pnpm", a)
            }
            Ecosystem::Yarn => {
                let mut a = vec!["add".to_string(), spec];
                if is_dev {
                    a.push("--dev".to_string());
                }
                ("yarn", a)
            }
            Ecosystem::Npm => {
                let mut a = vec!["install".to_string(), spec];
                if is_dev {
                    a.push("--save-dev".to_string());
                }
                ("npm", a)
            }
            Ecosystem::Uv => {
                let mut a = vec!["add".to_string(), spec];
                if is_dev {
                    a.push("--dev".to_string());
                }
                ("uv", a)
            }
            Ecosystem::Poetry => {
                let mut a = vec!["add".to_string(), spec];
                if is_dev {
                    a.push("--group=dev".to_string());
                }
                ("poetry", a)
            }
            Ecosystem::Pip => ("pip", vec!["install".to_string(), spec]),
            Ecosystem::GoMod => ("go", vec!["get".to_string(), spec]),
        };

        Self::execute_cli(cmd, &args, root)
    }

    pub fn remove_dependency(&self, root: &Path, package: &str) -> Result<String, String> {
        let eco = self
            .detect_ecosystem(root)
            .ok_or_else(|| "Cannot remove dependency: unknown project ecosystem.".to_string())?;

        let (cmd, args) = match eco {
            Ecosystem::Cargo => ("cargo", vec!["rm".to_string(), package.to_string()]),
            Ecosystem::Bun => ("bun", vec!["remove".to_string(), package.to_string()]),
            Ecosystem::Pnpm => ("pnpm", vec!["remove".to_string(), package.to_string()]),
            Ecosystem::Yarn => ("yarn", vec!["remove".to_string(), package.to_string()]),
            Ecosystem::Npm => ("npm", vec!["uninstall".to_string(), package.to_string()]),
            Ecosystem::Uv => ("uv", vec!["remove".to_string(), package.to_string()]),
            Ecosystem::Poetry => ("poetry", vec!["remove".to_string(), package.to_string()]),
            Ecosystem::Pip => (
                "pip",
                vec![
                    "uninstall".to_string(),
                    "-y".to_string(),
                    package.to_string(),
                ],
            ),
            Ecosystem::GoMod => ("go", vec!["mod".to_string(), "tidy".to_string()]),
        };

        Self::execute_cli(cmd, &args, root)
    }

    pub fn audit_dependencies(&self, root: &Path) -> Result<String, String> {
        let eco = self
            .detect_ecosystem(root)
            .ok_or_else(|| "Cannot audit dependencies: unknown project ecosystem.".to_string())?;

        let (cmd, args) = match eco {
            Ecosystem::Cargo => ("cargo", vec!["check".to_string()]),
            Ecosystem::Npm => ("npm", vec!["audit".to_string()]),
            Ecosystem::Pnpm => ("pnpm", vec!["audit".to_string()]),
            Ecosystem::Bun => ("bun", vec!["pm".to_string(), "audit".to_string()]),
            Ecosystem::Yarn => ("yarn", vec!["audit".to_string()]),
            Ecosystem::Pip | Ecosystem::Uv | Ecosystem::Poetry => {
                ("pip", vec!["check".to_string()])
            }
            Ecosystem::GoMod => ("go", vec!["vet".to_string(), "./...".to_string()]),
        };

        Self::execute_cli(cmd, &args, root)
    }

    fn execute_cli(cmd_name: &str, args: &[String], cwd: &Path) -> Result<String, String> {
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
                        combined.push('\n');
                        combined.push_str(&stderr);
                    }
                    Ok(combined.trim().to_string())
                } else {
                    Err(format!(
                        "Exit code {}: {}",
                        out.status.code().unwrap_or(1),
                        stderr.trim()
                    ))
                }
            }
            Err(e) => Err(format!("Failed to execute '{}': {}", cmd_name, e)),
        }
    }

    fn parse_cargo_deps(path: &Path) -> Result<Vec<DependencyInfo>, String> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read Cargo.toml: {}", e))?;

        let mut results = Vec::new();
        let mut current_section = "";

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                current_section = trimmed;
                continue;
            }

            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let is_dep_sec = current_section == "[dependencies]";
            let is_dev_sec = current_section == "[dev-dependencies]"
                || current_section == "[build-dependencies]";

            if (is_dep_sec || is_dev_sec) && trimmed.contains('=') {
                let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
                let name = parts[0].trim().to_string();
                let val = parts[1].trim();

                let version = if val.starts_with('"') && val.ends_with('"') {
                    val.trim_matches('"').to_string()
                } else if val.contains("version") {
                    if let Some(start) = val.find("version = \"") {
                        let sub = &val[start + 11..];
                        if let Some(end) = sub.find('"') {
                            sub[..end].to_string()
                        } else {
                            "workspace/path".to_string()
                        }
                    } else {
                        "workspace/path".to_string()
                    }
                } else {
                    "workspace/path".to_string()
                };

                results.push(DependencyInfo {
                    name,
                    version,
                    is_dev: is_dev_sec,
                });
            }
        }

        Ok(results)
    }

    fn parse_npm_deps(path: &Path) -> Result<Vec<DependencyInfo>, String> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read package.json: {}", e))?;

        let json: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("Invalid package.json JSON: {}", e))?;

        let mut results = Vec::new();

        if let Some(deps) = json.get("dependencies").and_then(|d| d.as_object()) {
            for (name, val) in deps {
                results.push(DependencyInfo {
                    name: name.clone(),
                    version: val.as_str().unwrap_or("latest").to_string(),
                    is_dev: false,
                });
            }
        }

        if let Some(dev_deps) = json.get("devDependencies").and_then(|d| d.as_object()) {
            for (name, val) in dev_deps {
                results.push(DependencyInfo {
                    name: name.clone(),
                    version: val.as_str().unwrap_or("latest").to_string(),
                    is_dev: true,
                });
            }
        }

        Ok(results)
    }

    fn parse_pip_requirements(path: &Path) -> Result<Vec<DependencyInfo>, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read requirements.txt: {}", e))?;

        let mut results = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let (name, ver) = if let Some(idx) = trimmed.find("==") {
                (&trimmed[..idx], &trimmed[idx + 2..])
            } else if let Some(idx) = trimmed.find(">=") {
                (&trimmed[..idx], &trimmed[idx + 2..])
            } else {
                (trimmed, "latest")
            };

            results.push(DependencyInfo {
                name: name.trim().to_string(),
                version: ver.trim().to_string(),
                is_dev: false,
            });
        }

        Ok(results)
    }

    fn parse_pyproject_deps(path: &Path) -> Result<Vec<DependencyInfo>, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read pyproject.toml: {}", e))?;

        let mut results = Vec::new();
        let mut in_deps = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == "dependencies = [" {
                in_deps = true;
                continue;
            }
            if in_deps {
                if trimmed.starts_with(']') {
                    in_deps = false;
                    continue;
                }
                let clean = trimmed
                    .trim_matches('"')
                    .trim_matches('\'')
                    .trim_matches(',');
                if !clean.is_empty() {
                    let parts: Vec<&str> = clean.split(['=', '>', '<']).collect();
                    let name = parts[0].trim();
                    let ver = if parts.len() > 1 {
                        clean[name.len()..].trim()
                    } else {
                        "latest"
                    };
                    results.push(DependencyInfo {
                        name: name.to_string(),
                        version: ver.to_string(),
                        is_dev: false,
                    });
                }
            }
        }

        Ok(results)
    }

    fn parse_go_deps(path: &Path) -> Result<Vec<DependencyInfo>, String> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read go.mod: {}", e))?;

        let mut results = Vec::new();
        let mut in_require = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == "require (" {
                in_require = true;
                continue;
            }
            if in_require {
                if trimmed == ")" {
                    in_require = false;
                    continue;
                }
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    results.push(DependencyInfo {
                        name: parts[0].to_string(),
                        version: parts[1].to_string(),
                        is_dev: false,
                    });
                }
            } else if let Some(stripped) = trimmed.strip_prefix("require ") {
                let parts: Vec<&str> = stripped.split_whitespace().collect();
                if parts.len() >= 2 {
                    results.push(DependencyInfo {
                        name: parts[0].to_string(),
                        version: parts[1].to_string(),
                        is_dev: false,
                    });
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cargo_deps() {
        let sample = "[package]\nname = \"sample\"\nversion = \"0.1.0\"\n\n[dependencies]\ntokio = \"1.43\"\nreqwest = { version = \"0.12\", features = [\"json\"] }\n\n[dev-dependencies]\ntestcrate = \"3.8\"\n";
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("test_daedalus_cargo.toml");
        fs::write(&file_path, sample).unwrap();

        let deps = DependencyManager::parse_cargo_deps(&file_path).unwrap();
        let _ = fs::remove_file(&file_path);

        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].name, "tokio");
        assert_eq!(deps[0].version, "1.43");
        assert!(!deps[0].is_dev);

        assert_eq!(deps[1].name, "reqwest");
        assert_eq!(deps[1].version, "0.12");
        assert!(!deps[1].is_dev);

        assert_eq!(deps[2].name, "testcrate");
        assert_eq!(deps[2].version, "3.8");
        assert!(deps[2].is_dev);
    }

    #[test]
    fn test_parse_npm_deps() {
        let sample = "{\"name\":\"sample\",\"dependencies\":{\"express\":\"^4.18.2\"},\"devDependencies\":{\"typescript\":\"^5.3.0\"}}";
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("test_daedalus_pkg.json");
        fs::write(&file_path, sample).unwrap();

        let deps = DependencyManager::parse_npm_deps(&file_path).unwrap();
        let _ = fs::remove_file(&file_path);

        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].name, "express");
        assert_eq!(deps[0].version, "^4.18.2");
        assert!(!deps[0].is_dev);

        assert_eq!(deps[1].name, "typescript");
        assert_eq!(deps[1].version, "^5.3.0");
        assert!(deps[1].is_dev);
    }
}
