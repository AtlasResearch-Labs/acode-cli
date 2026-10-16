//! Atlas Daedalus — Model Context Protocol (MCP) Slash Commands (/mcp)

use crate::agent::orchestrator::DaedalusOrchestrator;
use crate::cli::tui::TuiRenderer;

pub async fn handle(arg: &str, orchestrator: &mut DaedalusOrchestrator) -> bool {
    let parts: Vec<&str> = arg.split_whitespace().collect();
    if parts.is_empty() || parts[0].eq_ignore_ascii_case("list") {
        let servers = orchestrator.tools.mcp.list_servers();
        println!(
            "\n{}[MODEL CONTEXT PROTOCOL (MCP) REGISTRY]{} (Total: {})",
            TuiRenderer::BOLD,
            TuiRenderer::RESET,
            servers.len()
        );
        if servers.is_empty() {
            println!("  No MCP servers registered in .daedalus/mcp.json or ~/.daedalus/mcp.json.");
            println!("  Add one via: /mcp add <name> <command> [args...]\n");
        } else {
            for (name, cfg, tool_count) in servers {
                let tool_badge = if tool_count > 0 {
                    format!(
                        "{}[{} tools]{}",
                        TuiRenderer::GREEN,
                        tool_count,
                        TuiRenderer::RESET
                    )
                } else {
                    format!("{}[0 tools]{}", TuiRenderer::DIM, TuiRenderer::RESET)
                };
                println!(
                    "  - {}{}{} | {} {}",
                    TuiRenderer::CYAN,
                    name,
                    TuiRenderer::RESET,
                    cfg.command,
                    tool_badge
                );
                if !cfg.args.is_empty() {
                    println!("    Args: {}", cfg.args.join(" "));
                }
            }
            println!(
                "\nCommands: /mcp refresh | /mcp add <name> <cmd> [args] | /mcp remove <name>\n"
            );
        }
        return true;
    }

    match parts[0].to_lowercase().as_str() {
        "refresh" => {
            println!(
                "{}[ACODE] Probing MCP servers and discovering tools...{}",
                TuiRenderer::DIM,
                TuiRenderer::RESET
            );
            orchestrator.tools.mcp.reload_configs();
            orchestrator.tools.mcp.refresh_tools().await;
            let count = orchestrator.tools.mcp.get_definitions().len();
            println!(
                "{}[OK] Discovered {} MCP tools across active servers.{}\n",
                TuiRenderer::GREEN,
                count,
                TuiRenderer::RESET
            );
        }
        "add" => {
            if parts.len() < 3 {
                println!("{}Usage: /mcp add <server_name> <command> [args...]{}\n  Example: /mcp add sqlite uvx mcp-server-sqlite --db-path ./data.db\n", TuiRenderer::RED, TuiRenderer::RESET);
                return true;
            }
            let name = parts[1];
            let command = parts[2];
            let args: Vec<String> = parts[3..].iter().map(|s| s.to_string()).collect();

            println!(
                "{}[ACODE] Registering MCP server '{}' in .daedalus/mcp.json...{}",
                TuiRenderer::DIM,
                name,
                TuiRenderer::RESET
            );
            match orchestrator.tools.mcp.add_server(name, command, args).await {
                Ok(_) => {
                    let defs = orchestrator.tools.mcp.get_definitions();
                    let tool_count = defs
                        .iter()
                        .filter(|d| d.name.starts_with(&format!("mcp__{}__", name)))
                        .count();
                    println!(
                        "{}[OK] MCP server '{}' added and connected! Discovered {} tools.{}\n",
                        TuiRenderer::GREEN,
                        name,
                        tool_count,
                        TuiRenderer::RESET
                    );
                }
                Err(e) => println!(
                    "{}[FAIL] Failed to register MCP server: {}{}\n",
                    TuiRenderer::RED,
                    e,
                    TuiRenderer::RESET
                ),
            }
        }
        "remove" | "rm" => {
            if parts.len() < 2 {
                println!(
                    "{}Usage: /mcp remove <server_name>{}\n",
                    TuiRenderer::RED,
                    TuiRenderer::RESET
                );
                return true;
            }
            let name = parts[1];
            match orchestrator.tools.mcp.remove_server(name).await {
                Ok(true) => println!(
                    "{}[OK] MCP server '{}' removed from .daedalus/mcp.json and disconnected.{}\n",
                    TuiRenderer::GREEN,
                    name,
                    TuiRenderer::RESET
                ),
                Ok(false) => println!(
                    "{}[WARN] MCP server '{}' was not registered in .daedalus/mcp.json.{}\n",
                    TuiRenderer::YELLOW,
                    name,
                    TuiRenderer::RESET
                ),
                Err(e) => println!(
                    "{}[FAIL] Error removing server: {}{}\n",
                    TuiRenderer::RED,
                    e,
                    TuiRenderer::RESET
                ),
            }
        }
        "trust" => {
            println!(
                "{}[ACODE] Approving and pinning project MCP configuration...{}",
                TuiRenderer::DIM,
                TuiRenderer::RESET
            );
            match orchestrator.tools.mcp.trust_project_config() {
                Ok(msg) => {
                    println!("{}[OK] {}{}\n", TuiRenderer::GREEN, msg, TuiRenderer::RESET);
                    orchestrator.tools.mcp.refresh_tools().await;
                }
                Err(e) => {
                    println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET)
                }
            }
        }
        "untrust" => {
            println!(
                "{}[ACODE] Revoking trust for project MCP configuration...{}",
                TuiRenderer::DIM,
                TuiRenderer::RESET
            );
            match orchestrator.tools.mcp.revoke_project_config() {
                Ok(msg) => {
                    println!("{}[OK] {}{}\n", TuiRenderer::GREEN, msg, TuiRenderer::RESET);
                    orchestrator.tools.mcp.refresh_tools().await;
                }
                Err(e) => {
                    println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET)
                }
            }
        }
        _ => {
            println!("{}Usage: /mcp [list | refresh | trust | untrust | add <name> <cmd> [args...] | remove <name>]{}\n", TuiRenderer::CYAN, TuiRenderer::RESET);
        }
    }
    true
}
