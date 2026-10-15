//! Atlas Daedalus — Slash Command Router

use crate::agent::orchestrator::DaedalusOrchestrator;
use crate::cli::tui::TuiRenderer;

pub struct SlashCommandHandler;

impl SlashCommandHandler {
    pub async fn handle(raw_input: &str, orchestrator: &mut DaedalusOrchestrator) -> bool {
        let parts: Vec<&str> = raw_input.trim().split_whitespace().collect();
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
            "/help" => {
                Self::print_help();
                true
            }
            "/clear" => {
                print!("\x1B[2J\x1B[1;1H");
                orchestrator.clear_history();
                println!("{}[ACODE] Terminal cleared and session context reset.{}\n", TuiRenderer::DIM, TuiRenderer::RESET);
                true
            }
            "/diff" => {
                let diff = orchestrator.tools.git.get_diff(false).await;
                if diff.is_empty() {
                    println!("{}No working tree changes.{}\n", TuiRenderer::DIM, TuiRenderer::RESET);
                } else {
                    TuiRenderer::print_diff(&diff);
                }
                true
            }
            "/status" => {
                let status = orchestrator.tools.git.get_status().await;
                println!(
                    "\n{}[GIT STATUS]{} Branch: {}{}{}",
                    TuiRenderer::BOLD, TuiRenderer::RESET, TuiRenderer::CYAN, status.branch, TuiRenderer::RESET
                );
                if status.clean {
                    println!("{}[OK] Working tree clean.{}\n", TuiRenderer::GREEN, TuiRenderer::RESET);
                } else {
                    if !status.modified.is_empty() {
                        println!("{}Modified files:{}", TuiRenderer::YELLOW, TuiRenderer::RESET);
                        for m in &status.modified {
                            println!("  M {}", m);
                        }
                    }
                    if !status.untracked.is_empty() {
                        println!("{}Untracked files:{}", TuiRenderer::RED, TuiRenderer::RESET);
                        for u in &status.untracked {
                            println!("  ? {}", u);
                        }
                    }
                    println!();
                }
                true
            }
            "/cost" => {
                let traces = orchestrator.chronicle.get_recent_traces(10);
                if traces.is_empty() {
                    println!("{}No telemetry traces recorded in current session.{}\n", TuiRenderer::DIM, TuiRenderer::RESET);
                } else {
                    let total_cost: f64 = traces.iter().map(|t| t.cost_usd).sum();
                    let total_tokens: usize = traces.iter().map(|t| t.tokens.total).sum();
                    println!("\n{}[CHRONICLE TELEMETRY]{}", TuiRenderer::BOLD, TuiRenderer::RESET);
                    println!(
                        "Total Tokens: {}{}{} · Total Estimated Cost: {}${:.6}{}",
                        TuiRenderer::CYAN, total_tokens, TuiRenderer::RESET,
                        TuiRenderer::GREEN, total_cost, TuiRenderer::RESET
                    );
                    println!("Recent Spans:");
                    for t in &traces {
                        let time = if t.timestamp.len() >= 19 { &t.timestamp[11..19] } else { &t.timestamp };
                        println!("  [{}] {} - {} tokens (${:.5})", time, t.model, t.tokens.total, t.cost_usd);
                    }
                    println!();
                }
                true
            }
            "/audit" => {
                println!("{}[ACODE] Launching Argus AST security audit...{}", TuiRenderer::DIM, TuiRenderer::RESET);
                let target = if arg.is_empty() { "." } else { arg };
                let (clean, summary) = orchestrator.tools.pillars.run_argus_audit(target).await;
                println!("\n{}[ARGUS AUDIT]{} {}", TuiRenderer::BOLD, TuiRenderer::RESET, summary);
                if clean {
                    println!("{}[OK] All files verified clean. Zero vulnerabilities detected.{}\n", TuiRenderer::GREEN, TuiRenderer::RESET);
                }
                true
            }
            "/tunnel" => {
                let port: u16 = match arg.parse() {
                    Ok(p) => p,
                    Err(_) => {
                        println!("{}Usage: /tunnel <port>{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                        return true;
                    }
                };
                println!("{}[ACODE] Starting Beacon HTTPS tunnel on port {}...{}", TuiRenderer::DIM, port, TuiRenderer::RESET);
                let (success, url, inspector) = orchestrator.tools.pillars.start_beacon_tunnel(port, None).await;
                if success {
                    println!("\n{}[BEACON TUNNEL]{} Public URL: {}{}{}", TuiRenderer::BOLD, TuiRenderer::RESET, TuiRenderer::GREEN, url, TuiRenderer::RESET);
                    println!("Traffic Inspector: {}{}{}\n", TuiRenderer::CYAN, inspector, TuiRenderer::RESET);
                }
                true
            }
            "/pulse" => {
                let metrics = orchestrator.tools.pillars.get_pulse_metrics();
                println!("\n{}[PULSE OBSERVABILITY]{}", TuiRenderer::BOLD, TuiRenderer::RESET);
                println!("CPU Cores: {} · System Uptime: {}s\n", metrics.cpu_count, metrics.uptime_secs);
                true
            }
            "/commit" => {
                if arg.is_empty() {
                    println!("{}Usage: /commit <commit message>{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                    return true;
                }
                match orchestrator.tools.git.commit(arg, "Eren Yeager <victor@atlasresearchlabs.online>").await {
                    Ok(hash) => {
                        println!("{}[OK] Committed as [{}]: {}{}\n", TuiRenderer::GREEN, hash, arg, TuiRenderer::RESET);
                    }
                    Err(e) => {
                        println!("{}[FAIL] Git commit failed: {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET);
                    }
                }
                true
            }
            "/model" => {
                if arg.is_empty() {
                    println!("{}Current model: {} ({}){}\n", TuiRenderer::CYAN, orchestrator.provider.config.model, orchestrator.provider.config.provider, TuiRenderer::RESET);
                } else {
                    orchestrator.provider.config.model = arg.to_string();
                    println!("{}[OK] Switched active model to: {}{}\n", TuiRenderer::GREEN, arg, TuiRenderer::RESET);
                }
                true
            }
            "/exit" | "/quit" => {
                println!("\n{}[ACODE] Exiting Atlas Daedalus. Sovereign Rust runtime stopped.{}\n", TuiRenderer::DIM, TuiRenderer::RESET);
                std::process::exit(0);
            }
            _ => false,
        }
    }

    fn print_help() {
        println!("\n{}[ATLAS DAEDALUS COMMANDS]{}", TuiRenderer::BOLD, TuiRenderer::RESET);
        println!("  {}/help{}            - Show this command reference", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/diff{}            - Inspect working tree git diff", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/status{}          - Display git repository status", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/commit <msg>{}    - Commit all changes with standard author", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/cost{}            - Display Chronicle token usage and USD costs", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/audit{}           - Run Argus AST code review and secret scan", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/tunnel <port>{}  - Spawn a public HTTPS Beacon tunnel", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/pulse{}           - View live OS host memory and CPU metrics", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/model <name>{}   - Switch active LLM model", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/clear{}           - Clear console and reset conversational context", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/exit{}            - Terminate REPL session\n", TuiRenderer::CYAN, TuiRenderer::RESET);
    }
}
