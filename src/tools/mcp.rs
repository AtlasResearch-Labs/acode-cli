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

use crate::tools::registry::ToolDefinition;

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

    pub fn discover_configs(workspace_root: &Path) -> HashMap<String, McpServerConfig> {
        let mut configs = HashMap::new();

        // 1. User global ~/.daedalus/mcp.json
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

        // 2. Project-level .daedalus/mcp.json (takes precedence)
        let project_daedalus = workspace_root.join(".daedalus").join("mcp.json");
        if project_daedalus.exists() {
            if let Ok(content) = std::fs::read_to_string(&project_daedalus) {
                if let Ok(parsed) = serde_json::from_str::<McpConfigFile>(&content) {
                    for (k, v) in parsed.mcp_servers {
                        configs.insert(k, v);
                    }
                }
            }
        }

        // 3. Project-level mcp.json
        let project_root = workspace_root.join("mcp.json");
        if project_root.exists() {
            if let Ok(content) = std::fs::read_to_string(&project_root) {
                if let Ok(parsed) = serde_json::from_str::<McpConfigFile>(&content) {
                    for (k, v) in parsed.mcp_servers {
                        configs.insert(k, v);
                    }
                }
            }
        }

        configs
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
}
