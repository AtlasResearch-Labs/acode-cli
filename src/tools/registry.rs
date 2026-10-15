//! Atlas Daedalus — Central Tool Registry

use std::path::Path;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tools::fs::FsTools;
use crate::tools::shell::ShellTools;
use crate::tools::git::GitTools;
use crate::tools::pillars::PillarTools;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

pub struct ToolRegistry {
    pub fs: FsTools,
    pub shell: ShellTools,
    pub git: GitTools,
    pub pillars: PillarTools,
}

impl ToolRegistry {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            fs: FsTools::new(&workspace_root),
            shell: ShellTools::new(&workspace_root),
            git: GitTools::new(&workspace_root),
            pillars: PillarTools::new(&workspace_root),
        }
    }

    pub fn get_definitions(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "view_file".to_string(),
                description: "View the contents of a file with line numbers. Supports slice ranges.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative file path" },
                        "start_line": { "type": "integer", "description": "Start line number" },
                        "end_line": { "type": "integer", "description": "End line number" }
                    },
                    "required": ["path"]
                }),
            },
            ToolDefinition {
                name: "patch_file".to_string(),
                description: "Surgically replace a unique block of text in an existing file.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative file path" },
                        "target_content": { "type": "string", "description": "Exact text to replace" },
                        "replacement_content": { "type": "string", "description": "Replacement text" }
                    },
                    "required": ["path", "target_content", "replacement_content"]
                }),
            },
            ToolDefinition {
                name: "write_file".to_string(),
                description: "Create or overwrite a file with full content.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative file path" },
                        "content": { "type": "string", "description": "Complete file content" }
                    },
                    "required": ["path", "content"]
                }),
            },
            ToolDefinition {
                name: "run_command".to_string(),
                description: "Execute a command line string in the terminal.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": { "type": "string", "description": "Exact command line to execute" },
                        "cwd": { "type": "string", "description": "Working directory" }
                    },
                    "required": ["command"]
                }),
            },
            ToolDefinition {
                name: "list_dir".to_string(),
                description: "List directory contents with automatic ignore filtering.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Directory path" },
                        "recursive": { "type": "boolean", "description": "Whether to recurse" }
                    },
                    "required": []
                }),
            },
            ToolDefinition {
                name: "search_code".to_string(),
                description: "Ripgrep AST regex search for query strings across workspace files.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search pattern or string" },
                        "dir": { "type": "string", "description": "Directory to search" }
                    },
                    "required": ["query"]
                }),
            },
            ToolDefinition {
                name: "git_status".to_string(),
                description: "Get repository branch status and modified/untracked files.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            ToolDefinition {
                name: "git_diff".to_string(),
                description: "View git diff for working tree changes.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "staged": { "type": "boolean", "description": "If true, staged diff only" }
                    },
                    "required": []
                }),
            },
            ToolDefinition {
                name: "argus_audit".to_string(),
                description: "Run Argus AST code review and secret scan on files.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "target": { "type": "string", "description": "Path to audit" }
                    },
                    "required": []
                }),
            },
            ToolDefinition {
                name: "beacon_tunnel".to_string(),
                description: "Expose local port to public HTTPS with Beacon tunnel.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "port": { "type": "integer", "description": "Local port number" },
                        "subdomain": { "type": "string", "description": "Custom subdomain" }
                    },
                    "required": ["port"]
                }),
            },
        ]
    }

    pub async fn execute_tool(&self, name: &str, args: &Value) -> (bool, String) {
        match name {
            "view_file" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let start_line = args.get("start_line").and_then(|v| v.as_u64()).map(|n| n as usize);
                let end_line = args.get("end_line").and_then(|v| v.as_u64()).map(|n| n as usize);

                match self.fs.view_file(path, start_line, end_line) {
                    Ok(c) => (true, c),
                    Err(e) => (false, e),
                }
            }
            "patch_file" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let target = args.get("target_content").and_then(|v| v.as_str()).unwrap_or("");
                let rep = args.get("replacement_content").and_then(|v| v.as_str()).unwrap_or("");

                match self.fs.patch_file(path, target, rep) {
                    Ok(s) => (true, s),
                    Err(e) => (false, e),
                }
            }
            "write_file" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");

                match self.fs.write_file(path, content) {
                    Ok(s) => (true, s),
                    Err(e) => (false, e),
                }
            }
            "run_command" => {
                let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
                let cwd = args.get("cwd").and_then(|v| v.as_str());

                let res = self.shell.run_command(cmd, cwd).await;
                let mut out = String::new();
                if !res.stdout.is_empty() {
                    out.push_str(&format!("STDOUT:\n{}\n", res.stdout));
                }
                if !res.stderr.is_empty() {
                    out.push_str(&format!("STDERR:\n{}\n", res.stderr));
                }
                out.push_str(&format!("Exit code: {} ({}ms)", res.exit_code, res.duration_ms));
                (res.success, out)
            }
            "list_dir" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let recursive = args.get("recursive").and_then(|v| v.as_bool()).unwrap_or(false);

                match self.fs.list_dir(path, recursive) {
                    Ok(files) => (true, files.join("\n")),
                    Err(e) => (false, e),
                }
            }
            "search_code" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let dir = args.get("dir").and_then(|v| v.as_str()).unwrap_or(".");

                match self.fs.search_code(query, dir) {
                    Ok(matches) => {
                        if matches.is_empty() {
                            (true, "No matches found.".to_string())
                        } else {
                            (true, matches.join("\n"))
                        }
                    }
                    Err(e) => (false, e),
                }
            }
            "git_status" => {
                let status = self.git.get_status().await;
                let out = serde_json::json!({
                    "branch": status.branch,
                    "clean": status.clean,
                    "modified": status.modified,
                    "untracked": status.untracked
                });
                (true, out.to_string())
            }
            "git_diff" => {
                let staged = args.get("staged").and_then(|v| v.as_bool()).unwrap_or(false);
                let diff = self.git.get_diff(staged).await;
                (true, if diff.is_empty() { "No diff.".to_string() } else { diff })
            }
            "argus_audit" => {
                let target = args.get("target").and_then(|v| v.as_str()).unwrap_or(".");
                let (clean, summary) = self.pillars.run_argus_audit(target).await;
                (clean, summary)
            }
            "beacon_tunnel" => {
                let port = args.get("port").and_then(|v| v.as_u64()).unwrap_or(3000) as u16;
                let subdomain = args.get("subdomain").and_then(|v| v.as_str());
                let (success, url, inspector) = self.pillars.start_beacon_tunnel(port, subdomain).await;
                (success, format!("Live URL: {}\nInspector: {}", url, inspector))
            }
            _ => (false, format!("Unknown tool: {}", name)),
        }
    }
}
