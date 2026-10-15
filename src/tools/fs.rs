//! Atlas Daedalus — Filesystem Tools (Ripgrep-grade traversal)

use std::fs;
use std::path::{Path, PathBuf};
use ignore::WalkBuilder;
use regex::Regex;
use similar::TextDiff;

pub struct FsTools {
    workspace_root: PathBuf,
}

impl FsTools {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
    }

    fn resolve_path(&self, target_path: &str) -> PathBuf {
        let p = Path::new(target_path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.workspace_root.join(p)
        }
    }

    pub fn view_file(&self, target_path: &str, start_line: Option<usize>, end_line: Option<usize>) -> Result<String, String> {
        let full_path = self.resolve_path(target_path);
        if !full_path.exists() {
            return Err(format!("File not found: {}", target_path));
        }

        let content = fs::read_to_string(&full_path).map_err(|e| e.to_string())?;
        let lines: Vec<&str> = content.lines().collect();

        let s = start_line.unwrap_or(1).max(1);
        let e = end_line.unwrap_or(lines.len()).min(lines.len());

        if s > lines.len() {
            return Ok(String::new());
        }

        let mut out = String::new();
        for (idx, line) in lines[s - 1..e].iter().enumerate() {
            out.push_str(&format!("{}: {}\n", s + idx, line));
        }

        Ok(out)
    }

    pub fn patch_file(&self, target_path: &str, target_content: &str, replacement_content: &str) -> Result<String, String> {
        let full_path = self.resolve_path(target_path);
        if !full_path.exists() {
            return Err(format!("File not found: {}", target_path));
        }

        let original = fs::read_to_string(&full_path).map_err(|e| e.to_string())?;

        let count = original.matches(target_content).count();
        if count == 0 {
            return Err(format!("Target content block not found in {}. Ensure whitespace matches exactly.", target_path));
        }
        if count > 1 {
            return Err(format!("Target content found {} times in {}. Must match uniquely.", count, target_path));
        }

        let updated = original.replacen(target_content, replacement_content, 1);
        fs::write(&full_path, &updated).map_err(|e| e.to_string())?;

        // Generate unified diff
        let diff = TextDiff::from_lines(&original, &updated);
        let unified = diff.unified_diff().header("original", "patched").to_string();

        Ok(format!("Successfully patched {}.\n{}", target_path, unified))
    }

    pub fn write_file(&self, target_path: &str, content: &str) -> Result<String, String> {
        let full_path = self.resolve_path(target_path);
        if let Some(parent) = full_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        fs::write(&full_path, content).map_err(|e| e.to_string())?;
        Ok(format!("Wrote {} bytes to {}", content.len(), target_path))
    }

    pub fn list_dir(&self, target_path: &str, recursive: bool) -> Result<Vec<String>, String> {
        let full_path = self.resolve_path(target_path);
        if !full_path.exists() {
            return Err(format!("Directory not found: {}", target_path));
        }

        let max_depth = if recursive { 6 } else { 1 };

        let walker = WalkBuilder::new(&full_path)
            .max_depth(Some(max_depth))
            .git_ignore(true)
            .hidden(false)
            .build();

        let mut results = Vec::new();
        for entry in walker.flatten() {
            let p = entry.path();
            if p == full_path {
                continue;
            }

            let rel_str = if let Ok(rel) = p.strip_prefix(&full_path) {
                rel.to_string_lossy().replace('\\', "/")
            } else if let Ok(rel) = p.strip_prefix(&self.workspace_root) {
                rel.to_string_lossy().replace('\\', "/")
            } else {
                p.to_string_lossy().replace('\\', "/")
            };

            if p.is_dir() {
                results.push(format!("{}/", rel_str));
            } else {
                results.push(rel_str);
            }

            if results.len() >= 200 {
                break;
            }
        }

        Ok(results)
    }

    pub fn search_code(&self, query: &str, target_dir: &str) -> Result<Vec<String>, String> {
        let full_path = self.resolve_path(target_dir);
        let regex = Regex::new(query).map_err(|e| format!("Invalid regex: {}", e))?;

        let walker = WalkBuilder::new(&full_path)
            .git_ignore(true)
            .hidden(false)
            .build();

        let mut matches = Vec::new();

        for entry in walker.flatten() {
            if matches.len() >= 50 {
                break;
            }

            let p = entry.path();
            if !p.is_file() {
                continue;
            }

            if let Ok(content) = fs::read_to_string(p) {
                let file_str = if let Ok(rel) = p.strip_prefix(&full_path) {
                    rel.to_string_lossy().replace('\\', "/")
                } else if let Ok(rel) = p.strip_prefix(&self.workspace_root) {
                    rel.to_string_lossy().replace('\\', "/")
                } else {
                    p.to_string_lossy().replace('\\', "/")
                };

                for (idx, line) in content.lines().enumerate() {
                    if regex.is_match(line) {
                        matches.push(format!("{}:{}: {}", file_str, idx + 1, line.trim()));
                        if matches.len() >= 50 {
                            break;
                        }
                    }
                }
            }
        }

        Ok(matches)
    }
}
