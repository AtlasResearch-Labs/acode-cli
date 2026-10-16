//! Atlas Daedalus — Central Tool Registry

use std::path::Path;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tools::fs::FsTools;
use crate::tools::shell::ShellTools;
use crate::tools::git::GitTools;
use crate::tools::pillars::PillarTools;
use crate::tools::web::WebTools;
use crate::tools::daemon::DaemonRegistry;
use crate::tools::deps::DependencyManager;
use crate::tools::container::ContainerManager;
use crate::tools::symbols::SymbolEngine;

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
    pub web: WebTools,
    pub daemons: DaemonRegistry,
    pub deps: DependencyManager,
    pub containers: ContainerManager,
    pub symbols: SymbolEngine,
    pub workspace_root: std::path::PathBuf,
}

impl ToolRegistry {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        let p = workspace_root.as_ref().to_path_buf();
        Self {
            fs: FsTools::new(&p),
            shell: ShellTools::new(&p),
            git: GitTools::new(&p),
            pillars: PillarTools::new(&p),
            web: WebTools::new(),
            daemons: DaemonRegistry::new(),
            deps: DependencyManager::new(&p),
            containers: ContainerManager::new(),
            symbols: SymbolEngine::new(&p),
            workspace_root: p,
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
                name: "change_workspace".to_string(),
                description: "Switch active workspace root directory across any drive (e.g. F:\\projects\\Atlas-Research-Labs or relative path). Updates project tools, git context, memory, and verification to the target directory.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute or relative target directory path to switch into" }
                    },
                    "required": ["path"]
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
            ToolDefinition {
                name: "web_search".to_string(),
                description: "Multi-source web search across DuckDuckGo, HackerNews, and technical documentation.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query or keyword" },
                        "max_results": { "type": "integer", "description": "Maximum results to return (1-10)" }
                    },
                    "required": ["query"]
                }),
            },
            ToolDefinition {
                name: "web_fetch".to_string(),
                description: "Extract readable Markdown from any web page, Reddit post/thread, arXiv paper, YouTube video transcript, or documentation.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "url": { "type": "string", "description": "Full HTTP/HTTPS URL" },
                        "max_chars": { "type": "integer", "description": "Max characters to extract (default: 8000)" }
                    },
                    "required": ["url"]
                }),
            },
            ToolDefinition {
                name: "run_tests".to_string(),
                description: "Run automated test suite (cargo test, npm test, pytest, go test, make test, etc.) or custom test command.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": { "type": "string", "description": "Optional specific test command to execute" },
                        "cwd": { "type": "string", "description": "Optional directory to run tests in" }
                    },
                    "required": []
                }),
            },
            ToolDefinition {
                name: "list_workspaces".to_string(),
                description: "List sibling projects and workspace directories in parent folder.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            ToolDefinition {
                name: "update_memory".to_string(),
                description: "Update persistent user preferences (~/.daedalus/USER.md) or project architectural memory (.daedalus/MEMORY.md).".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "scope": { "type": "string", "enum": ["user", "project"], "description": "Memory scope: 'user' for ~/.daedalus/USER.md, 'project' for .daedalus/MEMORY.md" },
                        "content": { "type": "string", "description": "Markdown text note, guideline, or preference to record" }
                    },
                    "required": ["scope", "content"]
                }),
            },
            ToolDefinition {
                name: "start_daemon".to_string(),
                description: "Start a long-running background process (dev server, test watcher, build) without blocking the agent.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": { "type": "string", "description": "Shell command to run in the background" },
                        "cwd": { "type": "string", "description": "Optional working directory" }
                    },
                    "required": ["command"]
                }),
            },
            ToolDefinition {
                name: "list_daemons".to_string(),
                description: "List all active and recent background daemon processes with PID, uptime, and status.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            ToolDefinition {
                name: "daemon_logs".to_string(),
                description: "Retrieve recent stdout/stderr output lines from a background daemon process.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "description": "Daemon ID (e.g. 'd_01')" },
                        "lines": { "type": "integer", "description": "Number of lines to read (default: 25)" }
                    },
                    "required": ["id"]
                }),
            },
            ToolDefinition {
                name: "kill_daemon".to_string(),
                description: "Terminate a running background daemon process by ID.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "description": "Daemon ID (e.g. 'd_01')" }
                    },
                    "required": ["id"]
                }),
            },
            ToolDefinition {
                name: "manage_dependencies".to_string(),
                description: "Cross-ecosystem dependency management for Cargo, NPM, PNPM, Bun, Yarn, Pip, UV, Poetry, and GoMod. List, add, remove, or audit project dependencies.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["list", "add", "remove", "audit"],
                            "description": "Action to execute"
                        },
                        "package": { "type": "string", "description": "Package name (for add/remove)" },
                        "version": { "type": "string", "description": "Package version specifier (optional)" },
                        "is_dev": { "type": "boolean", "description": "Install as dev dependency (optional)" }
                    },
                    "required": ["action"]
                }),
            },
            ToolDefinition {
                name: "manage_containers".to_string(),
                description: "Direct control over Docker and Podman container lifecycles, log streaming, status discovery, and compose orchestration.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["status", "list", "logs", "inspect", "compose_up", "compose_down", "stop", "restart"],
                            "description": "Container action to execute"
                        },
                        "target": { "type": "string", "description": "Container ID or name (for logs, inspect, stop, restart)" },
                        "lines": { "type": "integer", "description": "Number of log lines to retrieve (default: 30)" }
                    },
                    "required": ["action"]
                }),
            },
            ToolDefinition {
                name: "extract_symbols".to_string(),
                description: "Extract function, struct, class, trait, interface, and enum symbols from a source file (Rust, TypeScript/JavaScript, Python, Go) with line numbers and signatures.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative file path" }
                    },
                    "required": ["path"]
                }),
            },
            ToolDefinition {
                name: "find_symbols".to_string(),
                description: "Search workspace source tree to locate symbol definitions matching a specific identifier name.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Symbol/identifier name to find" }
                    },
                    "required": ["name"]
                }),
            },
            ToolDefinition {
                name: "rename_symbol".to_string(),
                description: "Safely rename an identifier across matching workspace files using word-boundary tokens, avoiding comment and string false positives.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "old_name": { "type": "string", "description": "Existing identifier name" },
                        "new_name": { "type": "string", "description": "New identifier name" },
                        "path_filter": { "type": "string", "description": "Optional substring path filter to limit affected files" }
                    },
                    "required": ["old_name", "new_name"]
                }),
            },
            ToolDefinition {
                name: "scaffold_test".to_string(),
                description: "Proactively generate an idiomatic unit test scaffold for a source file (Rust, TypeScript/JavaScript, Python, Go).".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative source file path" },
                        "write_to_file": { "type": "boolean", "description": "If true, writes scaffold directly to test destination file (optional)" }
                    },
                    "required": ["path"]
                }),
            },
        ]
    }

    pub async fn execute_tool(&self, name: &str, args: &Value) -> (bool, String) {
        match name {
            "view_file" => {
                let path = args.get("path")
                    .or_else(|| args.get("file"))
                    .or_else(|| args.get("file_path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let start_line = args.get("start_line").and_then(|v| v.as_u64()).map(|n| n as usize);
                let end_line = args.get("end_line").and_then(|v| v.as_u64()).map(|n| n as usize);

                match self.fs.view_file(path, start_line, end_line) {
                    Ok(c) => (true, c),
                    Err(e) => (false, e),
                }
            }
            "patch_file" => {
                let path = args.get("path")
                    .or_else(|| args.get("file"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let target = args.get("target_content").and_then(|v| v.as_str()).unwrap_or("");
                let rep = args.get("replacement_content").and_then(|v| v.as_str()).unwrap_or("");

                match self.fs.patch_file(path, target, rep) {
                    Ok(s) => (true, s),
                    Err(e) => (false, e),
                }
            }
            "write_file" => {
                let path = args.get("path")
                    .or_else(|| args.get("file"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
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
                let path = args.get("path")
                    .or_else(|| args.get("dir"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");
                let recursive = args.get("recursive").and_then(|v| v.as_bool()).unwrap_or(false);

                match self.fs.list_dir(path, recursive) {
                    Ok(files) => {
                        if files.is_empty() {
                            (true, format!("(directory is empty or all items ignored: {})", path))
                        } else {
                            (true, files.join("\n"))
                        }
                    }
                    Err(e) => (false, e),
                }
            }
            "search_code" => {
                let query = args.get("query")
                    .or_else(|| args.get("pattern"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let dir = args.get("dir")
                    .or_else(|| args.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");

                match self.fs.search_code(query, dir) {
                    Ok(matches) => {
                        if matches.is_empty() {
                            (true, format!("No matches found for '{}' in {}.", query, dir))
                        } else {
                            (true, matches.join("\n"))
                        }
                    }
                    Err(e) => (false, e),
                }
            }
            "change_workspace" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                (true, format!("Workspace switch requested for: {}", path))
            }
            "git_status" => {
                let status = self.git.get_status().await;
                let out = if !status.is_repo {
                    serde_json::json!({
                        "is_git_repo": false,
                        "status": "Not a git repository. Workspace is an unversioned directory."
                    })
                } else {
                    serde_json::json!({
                        "is_git_repo": true,
                        "branch": status.branch,
                        "clean": status.clean,
                        "modified": status.modified,
                        "untracked": status.untracked
                    })
                };
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
            "web_search" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let max_results = args.get("max_results").and_then(|v| v.as_u64()).map(|n| n as usize).unwrap_or(5);
                match self.web.web_search(query, max_results).await {
                    Ok(res) => (true, res),
                    Err(e) => (false, e),
                }
            }
            "web_fetch" => {
                let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
                let max_chars = args.get("max_chars").and_then(|v| v.as_u64()).map(|n| n as usize).unwrap_or(8000);
                match self.web.web_fetch(url, max_chars).await {
                    Ok(res) => (true, res),
                    Err(e) => (false, e),
                }
            }
            "run_tests" => {
                let cmd = args.get("command").and_then(|v| v.as_str());
                let cwd = args.get("cwd").and_then(|v| v.as_str());
                let test_cmd = match cmd {
                    Some(c) => c.to_string(),
                    None => {
                        crate::agent::tester::SelfHealingVerifier::new(&self.workspace_root)
                            .detect_test_command()
                            .unwrap_or_else(|| "cargo test".to_string())
                    }
                };
                let res = self.shell.run_command(&test_cmd, cwd).await;
                let mut out = format!("Test Command: {}\nExit code: {}\nPassed: {}\n\n", test_cmd, res.exit_code, res.success);
                if !res.stdout.is_empty() { out.push_str(&format!("STDOUT:\n{}\n", res.stdout)); }
                if !res.stderr.is_empty() { out.push_str(&format!("STDERR:\n{}\n", res.stderr)); }
                (res.success, out)
            }
            "list_workspaces" => {
                if let Some(parent) = self.workspace_root.parent() {
                    if let Ok(entries) = std::fs::read_dir(parent) {
                        let mut dirs = Vec::new();
                        for entry in entries.flatten() {
                            if let Ok(ft) = entry.file_type() {
                                if ft.is_dir() {
                                    let name = entry.file_name().to_string_lossy().to_string();
                                    if !name.starts_with('.') {
                                        dirs.push(name);
                                    }
                                }
                            }
                        }
                        dirs.sort();
                        if dirs.is_empty() {
                            (true, "No sibling directories found in parent.".to_string())
                        } else {
                            (true, format!("Parent Directory: {}\nWorkspaces:\n{}", parent.display(), dirs.iter().map(|d| format!("- {}", d)).collect::<Vec<_>>().join("\n")))
                        }
                    } else {
                        (false, format!("Cannot read parent directory of {}", self.workspace_root.display()))
                    }
                } else {
                    (false, "Active workspace has no parent directory.".to_string())
                }
            }
            "start_daemon" => {
                let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
                let cwd_override = args.get("cwd").and_then(|v| v.as_str()).map(Path::new);
                let cwd = cwd_override.unwrap_or(&self.workspace_root);
                match self.daemons.start_daemon(command, cwd) {
                    Ok(info) => (true, format!("[DAEMON STARTED] ID: {} · PID: {} · Command: '{}'", info.id, info.pid, info.command)),
                    Err(e) => (false, format!("Failed to start daemon: {}", e)),
                }
            }
            "list_daemons" => {
                let list = self.daemons.list_daemons();
                if list.is_empty() {
                    (true, "No background daemons currently registered.".to_string())
                } else {
                    let mut out = format!("[ACTIVE DAEMONS (Total: {})]\n\n", list.len());
                    for d in &list {
                        let status_str = if d.is_alive { "RUNNING".to_string() } else { format!("EXITED ({:?})", d.exit_code) };
                        out.push_str(&format!("- [{}] PID: {} · {} (Uptime: {}s) · Command: '{}'\n", d.id, d.pid, status_str, d.uptime_secs, d.command));
                    }
                    (true, out)
                }
            }
            "daemon_logs" => {
                let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let lines = args.get("lines").and_then(|v| v.as_u64()).map(|n| n as usize).unwrap_or(25);
                match self.daemons.get_daemon_logs(id, lines) {
                    Some(logs) => {
                        if logs.is_empty() {
                            (true, format!("[DAEMON LOGS: {}] (No output recorded yet)", id))
                        } else {
                            (true, format!("[DAEMON LOGS: {} (Last {} lines)]\n\n{}", id, logs.len(), logs.join("\n")))
                        }
                    }
                    None => (false, format!("Daemon with ID '{}' not found.", id)),
                }
            }
            "kill_daemon" => {
                let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
                match self.daemons.kill_daemon(id) {
                    Ok(true) => (true, format!("[DAEMON KILLED] Process '{}' terminated.", id)),
                    Ok(false) => (false, format!("Daemon '{}' was not active.", id)),
                    Err(e) => (false, e),
                }
            }
            "manage_dependencies" => {
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");
                let package = args.get("package").and_then(|v| v.as_str()).unwrap_or("");
                let version = args.get("version").and_then(|v| v.as_str());
                let is_dev = args.get("is_dev").and_then(|v| v.as_bool()).unwrap_or(false);

                match action {
                    "list" => {
                        match self.deps.list_dependencies(&self.workspace_root) {
                            Ok(list) => {
                                if list.is_empty() {
                                    (true, "[DEPENDENCIES] No direct dependencies detected in project manifest.".to_string())
                                } else {
                                    let mut out = format!("[PROJECT DEPENDENCIES (Total: {})]\n\n", list.len());
                                    for dep in &list {
                                        let badge = if dep.is_dev { " (dev)" } else { "" };
                                        out.push_str(&format!("  - {}: {}{}\n", dep.name, dep.version, badge));
                                    }
                                    (true, out)
                                }
                            }
                            Err(e) => (false, e),
                        }
                    }
                    "add" => {
                        if package.is_empty() {
                            (false, "Package name required for 'add' action.".to_string())
                        } else {
                            match self.deps.add_dependency(&self.workspace_root, package, version, is_dev) {
                                Ok(out) => (true, format!("[DEPENDENCY ADDED] {}\n{}", package, out)),
                                Err(e) => (false, format!("Failed to add dependency '{}': {}", package, e)),
                            }
                        }
                    }
                    "remove" => {
                        if package.is_empty() {
                            (false, "Package name required for 'remove' action.".to_string())
                        } else {
                            match self.deps.remove_dependency(&self.workspace_root, package) {
                                Ok(out) => (true, format!("[DEPENDENCY REMOVED] {}\n{}", package, out)),
                                Err(e) => (false, format!("Failed to remove dependency '{}': {}", package, e)),
                            }
                        }
                    }
                    "audit" => {
                        match self.deps.audit_dependencies(&self.workspace_root) {
                            Ok(out) => (true, format!("[DEPENDENCY AUDIT]\n{}", out)),
                            Err(e) => (false, format!("Audit error: {}", e)),
                        }
                    }
                    _ => (false, format!("Unknown dependency action: '{}'. Use list, add, remove, or audit.", action)),
                }
            }
            "manage_containers" => {
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("status");
                let target = args.get("target").and_then(|v| v.as_str()).unwrap_or("");
                let lines = args.get("lines").and_then(|v| v.as_u64()).map(|n| n as usize).unwrap_or(30);

                match action {
                    "status" => {
                        match self.containers.check_status() {
                            Ok(s) => (true, format!("[CONTAINER STATUS] {}", s)),
                            Err(e) => (false, format!("[CONTAINER STATUS ERROR] {}", e)),
                        }
                    }
                    "list" => {
                        match self.containers.list_containers(true) {
                            Ok(list) => {
                                if list.is_empty() {
                                    (true, "[CONTAINERS] Zero active or stopped containers found.".to_string())
                                } else {
                                    let mut out = format!("[CONTAINERS (Total: {})]\n\n", list.len());
                                    for c in &list {
                                        out.push_str(&format!("  - [{}] {} | Image: {} | State: {} | Ports: {}\n", c.id, c.name, c.image, c.state, c.ports));
                                    }
                                    (true, out)
                                }
                            }
                            Err(e) => (false, format!("Failed to list containers: {}", e)),
                        }
                    }
                    "logs" => {
                        if target.is_empty() {
                            (false, "Target container ID or name required for 'logs'.".to_string())
                        } else {
                            match self.containers.get_logs(target, lines) {
                                Ok(l) => (true, format!("[CONTAINER LOGS: {} (Last {} lines)]\n\n{}", target, lines, l)),
                                Err(e) => (false, format!("Failed to get logs for '{}': {}", target, e)),
                            }
                        }
                    }
                    "inspect" => {
                        if target.is_empty() {
                            (false, "Target container ID or name required for 'inspect'.".to_string())
                        } else {
                            match self.containers.inspect_container(target) {
                                Ok(info) => (true, format!("[CONTAINER INSPECT: {}]\n{}", target, info)),
                                Err(e) => (false, format!("Inspect failed: {}", e)),
                            }
                        }
                    }
                    "compose_up" => {
                        match self.containers.compose_up(&self.workspace_root) {
                            Ok(out) => (true, format!("[COMPOSE UP]\n{}", out)),
                            Err(e) => (false, format!("Compose up failed: {}", e)),
                        }
                    }
                    "compose_down" => {
                        match self.containers.compose_down(&self.workspace_root) {
                            Ok(out) => (true, format!("[COMPOSE DOWN]\n{}", out)),
                            Err(e) => (false, format!("Compose down failed: {}", e)),
                        }
                    }
                    "stop" => {
                        if target.is_empty() {
                            (false, "Target container required for 'stop'.".to_string())
                        } else {
                            match self.containers.stop_container(target) {
                                Ok(out) => (true, format!("[CONTAINER STOPPED: {}]\n{}", target, out)),
                                Err(e) => (false, format!("Stop failed: {}", e)),
                            }
                        }
                    }
                    "restart" => {
                        if target.is_empty() {
                            (false, "Target container required for 'restart'.".to_string())
                        } else {
                            match self.containers.restart_container(target) {
                                Ok(out) => (true, format!("[CONTAINER RESTARTED: {}]\n{}", target, out)),
                                Err(e) => (false, format!("Restart failed: {}", e)),
                            }
                        }
                    }
                    _ => (false, format!("Unknown container action: '{}'", action)),
                }
            }
            "extract_symbols" => {
                let path = args.get("path")
                    .or_else(|| args.get("file"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let target_path = self.workspace_root.join(path);
                match self.symbols.extract_symbols_from_file(&target_path) {
                    Ok(list) => {
                        if list.is_empty() {
                            (true, format!("[SYMBOLS: {}] No symbols recognized.", path))
                        } else {
                            let mut out = format!("[SYMBOLS: {} (Total: {})]\n\n", path, list.len());
                            for s in &list {
                                let pub_badge = if s.is_public { "pub " } else { "" };
                                out.push_str(&format!("  L{:4} | {:9} | {}{}\n", s.line_number, s.kind, pub_badge, s.name));
                            }
                            (true, out)
                        }
                    }
                    Err(e) => (false, e),
                }
            }
            "find_symbols" => {
                let name_to_find = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if name_to_find.is_empty() {
                    (false, "Symbol name required for 'find_symbols'.".to_string())
                } else {
                    let list = self.symbols.find_symbol_definitions(name_to_find);
                    if list.is_empty() {
                        (true, format!("[SYMBOL SEARCH] No definitions found for '{}'.", name_to_find))
                    } else {
                        let mut out = format!("[SYMBOL SEARCH: '{}' (Matches: {})]\n\n", name_to_find, list.len());
                        for s in &list {
                            out.push_str(&format!("  - {} (Line {}): {}\n", s.file_path, s.line_number, s.signature));
                        }
                        (true, out)
                    }
                }
            }
            "rename_symbol" => {
                let old_name = args.get("old_name").and_then(|v| v.as_str()).unwrap_or("");
                let new_name = args.get("new_name").and_then(|v| v.as_str()).unwrap_or("");
                let path_filter = args.get("path_filter").and_then(|v| v.as_str());

                match self.symbols.rename_symbol(old_name, new_name, path_filter) {
                    Ok(modified) => {
                        if modified.is_empty() {
                            (true, format!("[RENAME] Symbol '{}' not found in any matched source files.", old_name))
                        } else {
                            let total_occ: usize = modified.iter().map(|(_, c)| *c).sum();
                            let mut out = format!("[RENAME SUCCESS: '{}' -> '{}' ({} replacements in {} files)]\n\n", old_name, new_name, total_occ, modified.len());
                            for (file, count) in &modified {
                                out.push_str(&format!("  - {}: {} occurrences replaced\n", file, count));
                            }
                            (true, out)
                        }
                    }
                    Err(e) => (false, format!("Rename failed: {}", e)),
                }
            }
            "scaffold_test" => {
                let path = args.get("path")
                    .or_else(|| args.get("file"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let write_to_file = args.get("write_to_file").and_then(|v| v.as_bool()).unwrap_or(false);

                match crate::agent::tester::SelfHealingVerifier::scaffold_test(&self.workspace_root, path) {
                    Ok((test_file, code)) => {
                        if write_to_file {
                            let target_p = self.workspace_root.join(&test_file);
                            if let Some(parent) = target_p.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if target_p.exists() && test_file == path {
                                // Append for Rust inline test module
                                if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(&target_p) {
                                    use std::io::Write;
                                    let _ = writeln!(f, "{}", code);
                                    (true, format!("[TEST SCAFFOLD APPENDED] Added unit test module to {}", test_file))
                                } else {
                                    (false, format!("Failed to append to {}", test_file))
                                }
                            } else {
                                match std::fs::write(&target_p, &code) {
                                    Ok(_) => (true, format!("[TEST SCAFFOLD CREATED] Wrote test suite to {}", test_file)),
                                    Err(e) => (false, format!("Failed to write test file: {}", e)),
                                }
                            }
                        } else {
                            (true, format!("[TEST SCAFFOLD FOR: {} (Target: {})]\n\n{}", path, test_file, code))
                        }
                    }
                    Err(e) => (false, e),
                }
            }
            _ => (false, format!("Unknown tool: {}", name)),
        }
    }
}
