use crate::agent::orchestrator::{AgentEvent, DaedalusOrchestrator};
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
            "/key" => {
                let parts: Vec<&str> = arg.split_whitespace().collect();
                if parts.len() < 2 {
                    println!("{}Usage: /key <provider> <api_key>{}\n  Providers: gemini, anthropic, openai, powerbox\n", TuiRenderer::RED, TuiRenderer::RESET);
                    return true;
                }
                let provider = parts[0].to_lowercase();
                let key = parts[1];

                orchestrator.provider.config.api_key = Some(key.to_string());
                orchestrator.provider.config.provider = provider.clone();

                if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
                    let daedalus_dir = std::path::PathBuf::from(home).join(".daedalus");
                    let _ = std::fs::create_dir_all(&daedalus_dir);
                    let env_file = daedalus_dir.join(".env");
                    let var_name = match provider.as_str() {
                        "gemini" => "GEMINI_API_KEY",
                        "anthropic" => "ANTHROPIC_API_KEY",
                        "openai" => "OPENAI_API_KEY",
                        "powerbox" => "POWERBOX_API_KEY",
                        _ => "AI_API_KEY",
                    };
                    let mut existing = std::fs::read_to_string(&env_file).unwrap_or_default();
                    if existing.contains(var_name) {
                        let lines: Vec<String> = existing.lines().map(|l| {
                            if l.starts_with(var_name) {
                                format!("{}={}", var_name, key)
                            } else {
                                l.to_string()
                            }
                        }).collect();
                        existing = lines.join("\n") + "\n";
                    } else {
                        existing.push_str(&format!("{}={}\n", var_name, key));
                    }
                    let _ = std::fs::write(&env_file, existing);
                    println!("{}[OK] Saved '{}' key to ~/.daedalus/.env{}\n", TuiRenderer::GREEN, provider, TuiRenderer::RESET);
                } else {
                    println!("{}[OK] Updated '{}' key for current session.{}\n", TuiRenderer::GREEN, provider, TuiRenderer::RESET);
                }
                true
            }
            "/plan" => {
                if arg.is_empty() {
                    println!("{}Usage: /plan <goal or feature to architect>{}\n  Example: /plan implement JWT authentication in src/auth.rs\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                    return true;
                }
                println!("\n{}[ARCHITECT MODE]{} Generating implementation plan for: {}{}{}\n", TuiRenderer::BOLD, TuiRenderer::RESET, TuiRenderer::CYAN, arg, TuiRenderer::RESET);
                let _ = orchestrator.run_plan(arg, |event| match event {
                    AgentEvent::Thinking(s) => TuiRenderer::print_thinking(s),
                    AgentEvent::ToolStart { name, args } => TuiRenderer::print_tool_start(name, args),
                    AgentEvent::ToolEnd { name, success, output } => TuiRenderer::print_tool_end(name, success, output),
                    AgentEvent::Error(err) => TuiRenderer::print_error(err),
                    _ => {}
                }).await;
                println!("{}[OK] Plan generated. To execute this plan, prompt Daedalus to proceed.{}\n", TuiRenderer::GREEN, TuiRenderer::RESET);
                true
            }
            "/info" => {
                let status = orchestrator.tools.git.get_status().await;
                let traces = orchestrator.chronicle.get_recent_traces(1);
                let last_tokens = traces.first().map(|t| t.tokens.total).unwrap_or(0);
                let last_cost = traces.first().map(|t| t.cost_usd).unwrap_or(0.0);

                println!("\n{}[WORKSPACE SITUATIONAL CONTEXT]{}", TuiRenderer::BOLD, TuiRenderer::RESET);
                println!("  Workspace Root: {}{}{}", TuiRenderer::CYAN, orchestrator.workspace_root.display(), TuiRenderer::RESET);
                println!("  Git Branch:     {}{}{}", TuiRenderer::CYAN, status.branch, TuiRenderer::RESET);
                println!("  Working Tree:   {}", if status.clean { format!("{}[CLEAN]{}", TuiRenderer::GREEN, TuiRenderer::RESET) } else { format!("{}[DIRTY: {} modified]{}", TuiRenderer::YELLOW, status.modified.len(), TuiRenderer::RESET) });
                println!("  Provider/Model: {} ({})", orchestrator.provider.config.provider, orchestrator.provider.config.model);
                println!("  Last Turn Cost: {} tokens (${:.5})\n", last_tokens, last_cost);
                true
            }
            "/undo" => {
                println!("{}[ACODE] Reverting uncommitted working tree changes...{}", TuiRenderer::YELLOW, TuiRenderer::RESET);
                let res = orchestrator.tools.shell.run_command("git checkout -- .", None).await;
                if res.success {
                    println!("{}[OK] Working tree restored to previous clean git commit.{}\n", TuiRenderer::GREEN, TuiRenderer::RESET);
                } else {
                    println!("{}[FAIL] Revert failed: {}{}\n", TuiRenderer::RED, res.stderr, TuiRenderer::RESET);
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
        println!("  {}/plan <goal>{}     - Architect Mode: generate implementation plan before coding", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/info{}            - Display active workspace root, git branch, and context", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/undo{}            - Revert all uncommitted working tree changes", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/diff{}            - Inspect working tree git diff", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/status{}          - Display git repository status", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/commit <msg>{}    - Commit all changes with standard author", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/cost{}            - Display Chronicle token usage and USD costs", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/audit{}           - Run Argus AST code review and secret scan", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/tunnel <port>{}  - Spawn a public HTTPS Beacon tunnel", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/pulse{}           - View live OS host memory and CPU metrics", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/model <name>{}   - Switch active LLM model", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/key <p> <key>{}   - Configure API key (gemini, anthropic, openai, powerbox)", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/clear{}           - Clear console and reset conversational context", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/exit{}            - Terminate REPL session", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}!<command>{}        - Direct shell execution bypass (e.g. !git status, !dir)\n", TuiRenderer::YELLOW, TuiRenderer::RESET);
    }
}
