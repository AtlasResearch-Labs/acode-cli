//! Atlas Daedalus — Slash Command Subsystem
//! Dispatches CLI interactive commands to modular domain handlers:
//! - git: /diff, /status, /undo, /commit, /repos
//! - mcp: /mcp
//! - daemon: /daemon, /daemons, /tunnel, /docker, /container
//! - session: /help, /clear, /web, /fetch, /test, /cost, /audit, /pulse, /host, /swarm, /model, /key, /plan, /info, /cd, /bookmark, /history, /security, /memory, /blackbox, /deps, /symbols, /rename, /tasks, /task, /sandbox, /exit

pub mod daemon;
pub mod git;
pub mod mcp;
pub mod session;

use crate::agent::orchestrator::DaedalusOrchestrator;

pub struct SlashCommandHandler;

impl SlashCommandHandler {
    pub async fn handle(raw_input: &str, orchestrator: &mut DaedalusOrchestrator) -> bool {
        let parts: Vec<&str> = raw_input.split_whitespace().collect();
        if parts.is_empty() {
            return false;
        }

        let cmd = parts[0].to_lowercase();
        let arg = if parts.len() > 1 {
            raw_input.trim()[parts[0].len()..].trim()
        } else {
            ""
        };

        match cmd.as_str() {
            // Git domain
            "/diff" | "/status" | "/undo" | "/commit" | "/repos" => {
                git::handle(&cmd, arg, orchestrator).await
            }
            // MCP domain
            "/mcp" => mcp::handle(arg, orchestrator).await,
            // Daemon & Infrastructure domain
            "/daemon" | "/daemons" | "/tunnel" | "/docker" | "/container" => {
                daemon::handle(&cmd, arg, orchestrator).await
            }
            // Session, Navigation, Observability & Config domain
            _ => session::handle(&cmd, arg, raw_input, orchestrator).await,
        }
    }
}
