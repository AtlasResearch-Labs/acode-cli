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
                let git_diff = orchestrator.tools.git.get_diff(false).await;
                if !git_diff.is_empty() {
                    TuiRenderer::print_diff(&git_diff);
                } else if !orchestrator.session_diffs.is_empty() {
                    println!("\n{}[SESSION MODIFICATIONS]{}", TuiRenderer::BOLD, TuiRenderer::RESET);
                    for (file, diff) in &orchestrator.session_diffs {
                        println!("{}File: {}{}", TuiRenderer::CYAN, file, TuiRenderer::RESET);
                        TuiRenderer::print_diff(diff);
                    }
                    println!();
                } else {
                    println!("{}No working tree or session modifications recorded.{}\n", TuiRenderer::DIM, TuiRenderer::RESET);
                }
                true
            }
            "/status" => {
                let status = orchestrator.tools.git.get_status().await;
                if !status.is_repo {
                    println!(
                        "\n{}[STATUS]{} Workspace root: {}{}{}\n{}[INFO] Directory is not a git repository. Session file tracking active.{}\n",
                        TuiRenderer::BOLD, TuiRenderer::RESET, TuiRenderer::CYAN, orchestrator.workspace_root.display(), TuiRenderer::RESET,
                        TuiRenderer::DIM, TuiRenderer::RESET
                    );
                } else {
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
                }
                true
            }
            "/web" => {
                if arg.is_empty() {
                    println!("{}Usage: /web <query>{}\n  Example: /web rust reqwest timeout\n", TuiRenderer::RED, TuiRenderer::RESET);
                    return true;
                }
                println!("{}[ACODE] Searching web across DuckDuckGo and HackerNews...{}\n", TuiRenderer::DIM, TuiRenderer::RESET);
                match orchestrator.tools.web.web_search(arg, 5).await {
                    Ok(res) => println!("{}\n", res),
                    Err(e) => println!("{}[FAIL] Web search error: {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                }
                true
            }
            "/fetch" | "/read" => {
                if arg.is_empty() {
                    println!("{}Usage: /fetch <url>{}\n  Example: /fetch https://arxiv.org/abs/2312.11805\n", TuiRenderer::RED, TuiRenderer::RESET);
                    return true;
                }
                println!("{}[ACODE] Fetching and parsing web content...{}\n", TuiRenderer::DIM, TuiRenderer::RESET);
                match orchestrator.tools.web.web_fetch(arg, 8000).await {
                    Ok(content) => println!("{}\n", content),
                    Err(e) => println!("{}[FAIL] Fetch error: {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                }
                true
            }
            "/test" => {
                println!("{}[ACODE] Running test suite...{}", TuiRenderer::DIM, TuiRenderer::RESET);
                let res = if arg.is_empty() {
                    orchestrator.verifier.run_verification().await
                } else {
                    orchestrator.verifier.run_custom_test(arg, None).await
                };

                let cmd_name = res.command.as_deref().unwrap_or("no test framework detected");
                println!("\n{}[TEST EXECUTION]{} Command: {}{}{} ({}ms)", TuiRenderer::BOLD, TuiRenderer::RESET, TuiRenderer::CYAN, cmd_name, TuiRenderer::RESET, res.duration_ms);
                if res.passed {
                    println!("{}[OK] All tests passed successfully.{}\n", TuiRenderer::GREEN, TuiRenderer::RESET);
                } else {
                    println!("{}[FAIL] Tests failed.\n{}\n", TuiRenderer::RED, res.output);
                }
                true
            }
            "/repos" => {
                if let Some(parent) = orchestrator.workspace_root.parent() {
                    if let Ok(entries) = std::fs::read_dir(parent) {
                        let mut repos = Vec::new();
                        for entry in entries.flatten() {
                            if let Ok(ft) = entry.file_type() {
                                if ft.is_dir() {
                                    let name = entry.file_name().to_string_lossy().to_string();
                                    if !name.starts_with('.') {
                                        let is_git = entry.path().join(".git").exists();
                                        repos.push((name, is_git));
                                    }
                                }
                            }
                        }
                        repos.sort();
                        println!("\n{}[SIBLING WORKSPACES]{} in {}", TuiRenderer::BOLD, TuiRenderer::RESET, parent.display());
                        for (r, is_git) in &repos {
                            let git_badge = if *is_git { format!("{}[git]{}", TuiRenderer::GREEN, TuiRenderer::RESET) } else { String::new() };
                            println!("  - {}{}{} {}", TuiRenderer::CYAN, r, TuiRenderer::RESET, git_badge);
                        }
                        println!("\nSwitch active workspace using: /cd <folder_name>\n");
                    } else {
                        println!("{}[FAIL] Cannot read parent folder: {}{}\n", TuiRenderer::RED, parent.display(), TuiRenderer::RESET);
                    }
                } else {
                    println!("{}Active workspace has no parent folder.{}\n", TuiRenderer::DIM, TuiRenderer::RESET);
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
            "/host" | "/metrics" => {
                let m = crate::telemetry::host::HostInspector::collect();
                println!("\n{}[HOST TELEMETRY METRICS]{}", TuiRenderer::BOLD, TuiRenderer::RESET);
                println!("  Operating System: {}{}{}", TuiRenderer::CYAN, m.os_name, TuiRenderer::RESET);
                println!("  CPU Cores:        {}{}{}", TuiRenderer::CYAN, m.cpu_cores, TuiRenderer::RESET);
                println!("  Free Memory:      {}{}{} MB (Total: {} MB)", TuiRenderer::GREEN, m.available_memory_mb, TuiRenderer::RESET, m.total_memory_mb);
                println!("  Process RSS:      {}{}{} MB", TuiRenderer::YELLOW, m.process_rss_mb, TuiRenderer::RESET);
                let hours = m.uptime_secs / 3600;
                let mins = (m.uptime_secs % 3600) / 60;
                let secs = m.uptime_secs % 60;
                println!("  System Uptime:    {}h {}m {}s ({}s)\n", hours, mins, secs, m.uptime_secs);
                true
            }
            "/mcp" => {
                let parts: Vec<&str> = arg.split_whitespace().collect();
                if parts.is_empty() || parts[0].eq_ignore_ascii_case("list") {
                    let servers = orchestrator.tools.mcp.list_servers();
                    println!("\n{}[MODEL CONTEXT PROTOCOL (MCP) REGISTRY]{} (Total: {})", TuiRenderer::BOLD, TuiRenderer::RESET, servers.len());
                    if servers.is_empty() {
                        println!("  No MCP servers registered in .daedalus/mcp.json or ~/.daedalus/mcp.json.");
                        println!("  Add one via: /mcp add <name> <command> [args...]\n");
                    } else {
                        for (name, cfg, tool_count) in servers {
                            let tool_badge = if tool_count > 0 {
                                format!("{}[{} tools]{}", TuiRenderer::GREEN, tool_count, TuiRenderer::RESET)
                            } else {
                                format!("{}[0 tools]{}", TuiRenderer::DIM, TuiRenderer::RESET)
                            };
                            println!("  - {}{}{} | {} {}", TuiRenderer::CYAN, name, TuiRenderer::RESET, cfg.command, tool_badge);
                            if !cfg.args.is_empty() {
                                println!("    Args: {}", cfg.args.join(" "));
                            }
                        }
                        println!("\nCommands: /mcp refresh | /mcp add <name> <cmd> [args] | /mcp remove <name>\n");
                    }
                    return true;
                }

                match parts[0].to_lowercase().as_str() {
                    "refresh" => {
                        println!("{}[ACODE] Probing MCP servers and discovering tools...{}", TuiRenderer::DIM, TuiRenderer::RESET);
                        orchestrator.tools.mcp.reload_configs();
                        orchestrator.tools.mcp.refresh_tools().await;
                        let count = orchestrator.tools.mcp.get_definitions().len();
                        println!("{}[OK] Discovered {} MCP tools across active servers.{}\n", TuiRenderer::GREEN, count, TuiRenderer::RESET);
                    }
                    "add" => {
                        if parts.len() < 3 {
                            println!("{}Usage: /mcp add <server_name> <command> [args...]{}\n  Example: /mcp add sqlite uvx mcp-server-sqlite --db-path ./data.db\n", TuiRenderer::RED, TuiRenderer::RESET);
                            return true;
                        }
                        let name = parts[1];
                        let command = parts[2];
                        let args: Vec<String> = parts[3..].iter().map(|s| s.to_string()).collect();

                        println!("{}[ACODE] Registering MCP server '{}' in .daedalus/mcp.json...{}", TuiRenderer::DIM, name, TuiRenderer::RESET);
                        match orchestrator.tools.mcp.add_server(name, command, args).await {
                            Ok(_) => {
                                let defs = orchestrator.tools.mcp.get_definitions();
                                let tool_count = defs.iter().filter(|d| d.name.starts_with(&format!("mcp__{}__", name))).count();
                                println!("{}[OK] MCP server '{}' added and connected! Discovered {} tools.{}\n", TuiRenderer::GREEN, name, tool_count, TuiRenderer::RESET);
                            }
                            Err(e) => println!("{}[FAIL] Failed to register MCP server: {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                        }
                    }
                    "remove" | "rm" => {
                        if parts.len() < 2 {
                            println!("{}Usage: /mcp remove <server_name>{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                            return true;
                        }
                        let name = parts[1];
                        match orchestrator.tools.mcp.remove_server(name).await {
                            Ok(true) => println!("{}[OK] MCP server '{}' removed from .daedalus/mcp.json and disconnected.{}\n", TuiRenderer::GREEN, name, TuiRenderer::RESET),
                            Ok(false) => println!("{}[WARN] MCP server '{}' was not registered in .daedalus/mcp.json.{}\n", TuiRenderer::YELLOW, name, TuiRenderer::RESET),
                            Err(e) => println!("{}[FAIL] Error removing server: {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                        }
                    }
                    "trust" => {
                        println!("{}[ACODE] Approving and pinning project MCP configuration...{}", TuiRenderer::DIM, TuiRenderer::RESET);
                        match orchestrator.tools.mcp.trust_project_config() {
                            Ok(msg) => {
                                println!("{}[OK] {}{}\n", TuiRenderer::GREEN, msg, TuiRenderer::RESET);
                                orchestrator.tools.mcp.refresh_tools().await;
                            }
                            Err(e) => println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                        }
                    }
                    "untrust" => {
                        println!("{}[ACODE] Revoking trust for project MCP configuration...{}", TuiRenderer::DIM, TuiRenderer::RESET);
                        match orchestrator.tools.mcp.revoke_project_config() {
                            Ok(msg) => {
                                println!("{}[OK] {}{}\n", TuiRenderer::GREEN, msg, TuiRenderer::RESET);
                                orchestrator.tools.mcp.refresh_tools().await;
                            }
                            Err(e) => println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                        }
                    }
                    _ => {
                        println!("{}Usage: /mcp [list | refresh | trust | untrust | add <name> <cmd> [args...] | remove <name>]{}\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                    }
                }
                true
            }
            "/swarm" => {
                if arg.is_empty() {
                    println!("{}Usage: /swarm <complex objective to delegate>{}\n  Example: /swarm add comprehensive tests for host metrics and verify build\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                    return true;
                }
                println!("\n{}[MULTI-AGENT SWARM ACTIVE]{} Delegating objective to Explorer, Coder, and Verifier...", TuiRenderer::BOLD, TuiRenderer::RESET);
                println!("Goal: {}{}{}\n", TuiRenderer::CYAN, arg, TuiRenderer::RESET);

                let res = orchestrator.run_swarm(arg, |event| match event {
                    AgentEvent::Thinking(s) => TuiRenderer::print_thinking(s),
                    AgentEvent::Token(tok) => {
                        print!("{}", tok);
                        let _ = std::io::Write::flush(&mut std::io::stdout());
                    }
                    AgentEvent::ToolStart { name, args } => TuiRenderer::print_tool_start(name, args),
                    AgentEvent::ToolEnd { name, success, output } => TuiRenderer::print_tool_end(name, success, output),
                    AgentEvent::TestResult { command, passed, output } => TuiRenderer::print_test_result(command, passed, output),
                    AgentEvent::Error(e) => TuiRenderer::print_error(e),
                    _ => {}
                }).await;

                match res {
                    Ok(outcome) => {
                        println!("\n{}[SWARM EXECUTION COMPLETE]{} in {}ms", TuiRenderer::BOLD, TuiRenderer::RESET, outcome.duration_ms);
                        if !outcome.files_modified.is_empty() {
                            println!("  Files Modified: {}{}{}", TuiRenderer::GREEN, outcome.files_modified.join(", "), TuiRenderer::RESET);
                        }
                        let verif_badge = if outcome.verification_passed {
                            format!("{}[PASSED]{}", TuiRenderer::GREEN, TuiRenderer::RESET)
                        } else {
                            format!("{}[FAILED]{}", TuiRenderer::RED, TuiRenderer::RESET)
                        };
                        println!("  Verification:   {}\n", verif_badge);
                        if !outcome.coder_summary.is_empty() {
                            println!("{}\n", outcome.coder_summary);
                        }
                    }
                    Err(e) => TuiRenderer::print_error(&format!("Swarm coordination failed: {}", e)),
                }
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
                    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
                        let daedalus_dir = std::path::PathBuf::from(home).join(".daedalus");
                        let _ = std::fs::create_dir_all(&daedalus_dir);
                        let env_file = daedalus_dir.join(".env");
                        let var_name = "DAEDALUS_MODEL";
                        let mut existing = std::fs::read_to_string(&env_file).unwrap_or_default();
                        if existing.contains(var_name) {
                            let lines: Vec<String> = existing.lines().map(|l| {
                                if l.starts_with(var_name) {
                                    format!("{}={}", var_name, arg)
                                } else {
                                    l.to_string()
                                }
                            }).collect();
                            existing = lines.join("\n") + "\n";
                        } else {
                            existing.push_str(&format!("{}={}\n", var_name, arg));
                        }
                        let _ = std::fs::write(&env_file, existing);
                    }
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
                if provider == "powerbox" {
                    orchestrator.provider.config.base_url = Some("https://powerbox.atlasresearchlabs.online/v1".to_string());
                    orchestrator.provider.config.model = "auto".to_string();
                }

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

                    if provider == "powerbox" {
                        if !existing.contains("POWERBOX_BASE_URL") {
                            existing.push_str("POWERBOX_BASE_URL=https://powerbox.atlasresearchlabs.online/v1\n");
                        }
                        if !existing.contains("POWERBOX_MODEL") {
                            existing.push_str("POWERBOX_MODEL=auto\n");
                        }
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
            "/cd" => {
                if arg.is_empty() {
                    println!("{}Usage: /cd <directory_path | @alias>{}\n  Example: /cd F:\\projects\\Atlas-Research-Labs\n  Example: /cd @labs\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                    return true;
                }

                let target_path_str = if arg.starts_with('@') {
                    let alias = &arg[1..];
                    let bookmarks = Self::load_bookmarks();
                    match bookmarks.get(alias) {
                        Some(p) => p.clone(),
                        None => {
                            println!("{}[FAIL] Unknown bookmark alias: @{}{}\nUse /bookmark to list saved project aliases.\n", TuiRenderer::RED, alias, TuiRenderer::RESET);
                            return true;
                        }
                    }
                } else {
                    arg.to_string()
                };

                let target = std::path::Path::new(&target_path_str);
                let new_path = if target.is_absolute() {
                    target.to_path_buf()
                } else {
                    orchestrator.workspace_root.join(target)
                };

                if !new_path.exists() {
                    println!("{}[FAIL] Directory does not exist: {}{}\n", TuiRenderer::RED, new_path.display(), TuiRenderer::RESET);
                    return true;
                }

                if !new_path.is_dir() {
                    println!("{}[FAIL] Path is not a directory: {}{}\n", TuiRenderer::RED, new_path.display(), TuiRenderer::RESET);
                    return true;
                }

                let canonical = new_path.canonicalize().unwrap_or(new_path);
                orchestrator.set_workspace(canonical.clone());
                println!("{}[OK] Switched active workspace to: {}{}\n", TuiRenderer::GREEN, canonical.display(), TuiRenderer::RESET);
                true
            }
            "/bookmark" => {
                let parts: Vec<&str> = arg.split_whitespace().collect();
                let mut bookmarks = Self::load_bookmarks();

                if parts.is_empty() {
                    if bookmarks.is_empty() {
                        println!("{}No bookmarks saved yet. Use: /bookmark add <alias> [path]{}\n", TuiRenderer::DIM, TuiRenderer::RESET);
                    } else {
                        println!("\n{}[SAVED WORKSPACE BOOKMARKS]{}", TuiRenderer::BOLD, TuiRenderer::RESET);
                        for (alias, path) in &bookmarks {
                            println!("  {}{}@{}{} -> {}", TuiRenderer::CYAN, TuiRenderer::BOLD, alias, TuiRenderer::RESET, path);
                        }
                        println!("\nUsage: /cd @<alias>\n");
                    }
                    return true;
                }

                match parts[0].to_lowercase().as_str() {
                    "add" => {
                        if parts.len() < 2 {
                            println!("{}Usage: /bookmark add <alias> [path]{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                            return true;
                        }
                        let alias = parts[1].trim_start_matches('@').to_string();
                        let path = if parts.len() > 2 {
                            parts[2..].join(" ")
                        } else {
                            orchestrator.workspace_root.to_string_lossy().to_string()
                        };
                        bookmarks.insert(alias.clone(), path.clone());
                        Self::save_bookmarks(&bookmarks);
                        println!("{}[OK] Bookmarked @{} -> {}{}\n", TuiRenderer::GREEN, alias, path, TuiRenderer::RESET);
                    }
                    "remove" | "rm" => {
                        if parts.len() < 2 {
                            println!("{}Usage: /bookmark remove <alias>{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                            return true;
                        }
                        let alias = parts[1].trim_start_matches('@');
                        if bookmarks.remove(alias).is_some() {
                            Self::save_bookmarks(&bookmarks);
                            println!("{}[OK] Removed bookmark @{}{}\n", TuiRenderer::GREEN, alias, TuiRenderer::RESET);
                        } else {
                            println!("{}[FAIL] Bookmark @{} not found.{}\n", TuiRenderer::RED, alias, TuiRenderer::RESET);
                        }
                    }
                    _ => {
                        println!("{}Usage: /bookmark [add <alias> [path] | remove <alias>]{}\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                    }
                }
                true
            }
            "/history" => {
                if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
                    let history_path = std::path::PathBuf::from(home).join(".daedalus").join("history.txt");
                    if let Ok(content) = std::fs::read_to_string(history_path) {
                        let lines: Vec<&str> = content.lines().collect();
                        let start_idx = if lines.len() > 15 { lines.len() - 15 } else { 0 };
                        println!("\n{}[RECENT COMMAND HISTORY]{}", TuiRenderer::BOLD, TuiRenderer::RESET);
                        for (idx, line) in lines[start_idx..].iter().enumerate() {
                            println!("  {:2}. {}", start_idx + idx + 1, line);
                        }
                        println!();
                    } else {
                        println!("{}No command history recorded yet.{}\n", TuiRenderer::DIM, TuiRenderer::RESET);
                    }
                }
                true
            }
            "/security" => {
                let parts: Vec<&str> = arg.split_whitespace().collect();
                if parts.is_empty() {
                    let cfg = &orchestrator.security.config;
                    let mode_color = match cfg.mode {
                        crate::security::SecurityMode::FullAccess => TuiRenderer::GREEN,
                        crate::security::SecurityMode::Sandboxed => TuiRenderer::YELLOW,
                        crate::security::SecurityMode::Strict => TuiRenderer::RED,
                    };
                    println!("\n{}[AGENT SECURITY GATE STATUS]{}", TuiRenderer::BOLD, TuiRenderer::RESET);
                    println!("  Security Mode:   {}{}{}", mode_color, cfg.mode, TuiRenderer::RESET);
                    println!("  Trusted Paths:   {}", cfg.trusted_paths.len());
                    for tp in &cfg.trusted_paths {
                        println!("    + {}", tp.display());
                    }
                    println!("  Session Grants:  {} paths, {} commands", 
                        orchestrator.security.session_allowed_paths.len(),
                        orchestrator.security.session_allowed_commands.len()
                    );
                    println!("\nCommands:");
                    println!("  /security mode <full|sandboxed|strict> - Toggle security profile");
                    println!("  /security trust <path>                 - Add persistent trusted path");
                    println!("  /security untrust <path>               - Remove trusted path");
                    println!("  /security reset                        - Clear active session grants\n");
                    return true;
                }

                match parts[0].to_lowercase().as_str() {
                    "mode" => {
                        if parts.len() < 2 {
                            println!("{}Usage: /security mode <full | sandboxed | strict>{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                            return true;
                        }
                        match parts[1].parse::<crate::security::SecurityMode>() {
                            Ok(new_mode) => {
                                orchestrator.security.set_mode(new_mode);
                                println!("{}[OK] Switched agent security mode to: {}{}\n", TuiRenderer::GREEN, new_mode, TuiRenderer::RESET);
                            }
                            Err(e) => {
                                println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET);
                            }
                        }
                    }
                    "trust" => {
                        if parts.len() < 2 {
                            println!("{}Usage: /security trust <directory_path>{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                            return true;
                        }
                        let p = std::path::PathBuf::from(parts[1..].join(" "));
                        orchestrator.security.trust_path(p.clone());
                        println!("{}[OK] Added trusted path: {}{}\n", TuiRenderer::GREEN, p.display(), TuiRenderer::RESET);
                    }
                    "untrust" => {
                        if parts.len() < 2 {
                            println!("{}Usage: /security untrust <directory_path>{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                            return true;
                        }
                        let p = std::path::Path::new(parts[1]);
                        if orchestrator.security.untrust_path(p) {
                            println!("{}[OK] Removed trusted path: {}{}\n", TuiRenderer::GREEN, p.display(), TuiRenderer::RESET);
                        } else {
                            println!("{}[FAIL] Path was not in trusted list: {}{}\n", TuiRenderer::RED, p.display(), TuiRenderer::RESET);
                        }
                    }
                    "reset" => {
                        orchestrator.security.clear_session_grants();
                        println!("{}[OK] Cleared all active session permission grants.{}\n", TuiRenderer::GREEN, TuiRenderer::RESET);
                    }
                    _ => {
                        println!("{}Usage: /security [mode <full|sandboxed|strict> | trust <path> | untrust <path> | reset]{}\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                    }
                }
                true
            }
            "/memory" => {
                let parts: Vec<&str> = arg.split_whitespace().collect();
                if parts.is_empty() {
                    println!("\n{}[PERSISTENT MEMORY LEDGER]{}", TuiRenderer::BOLD, TuiRenderer::RESET);
                    println!("{}User Profile (~/.daedalus/USER.md):{}", TuiRenderer::CYAN, TuiRenderer::RESET);
                    let user_prof = orchestrator.memory.get_user_profile();
                    if user_prof.trim().is_empty() {
                        println!("  (empty)");
                    } else {
                        for line in user_prof.lines() {
                            println!("  {}", line);
                        }
                    }
                    println!("\n{}Workspace Architectural Memory (.daedalus/MEMORY.md):{}", TuiRenderer::CYAN, TuiRenderer::RESET);
                    let proj_mem = orchestrator.memory.get_project_memory();
                    if proj_mem.trim().is_empty() {
                        println!("  (empty)");
                    } else {
                        for line in proj_mem.lines() {
                            println!("  {}", line);
                        }
                    }
                    println!("\nCommands:");
                    println!("  /memory user <note>    - Append note/preference to global USER.md");
                    println!("  /memory project <note> - Append architectural decision to workspace MEMORY.md\n");
                } else {
                    let target = parts[0].to_lowercase();
                    let note = parts[1..].join(" ");
                    if note.is_empty() {
                        println!("{}Usage: /memory [user|project] <note to record>{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                    } else if target == "user" {
                        match orchestrator.memory.update_user_memory(&note) {
                            Ok(_) => println!("{}[OK] Recorded note in ~/.daedalus/USER.md{}\n", TuiRenderer::GREEN, TuiRenderer::RESET),
                            Err(e) => println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                        }
                    } else if target == "project" {
                        match orchestrator.memory.update_project_memory(&note) {
                            Ok(_) => println!("{}[OK] Recorded note in .daedalus/MEMORY.md{}\n", TuiRenderer::GREEN, TuiRenderer::RESET),
                            Err(e) => println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                        }
                    } else {
                        println!("{}Unknown memory target '{}'. Use 'user' or 'project'.{}\n", TuiRenderer::RED, target, TuiRenderer::RESET);
                    }
                }
                true
            }
            "/blackbox" | "/flight" => {
                let events = orchestrator.blackbox.read_recent_events(15);
                if events.is_empty() {
                    println!("{}No telemetry events recorded in .daedalus/blackbox.jsonl yet.{}\n", TuiRenderer::DIM, TuiRenderer::RESET);
                } else {
                    println!("\n{}[BLACKBOX FLIGHT RECORDER (RECENT 15 EVENTS)]{}", TuiRenderer::BOLD, TuiRenderer::RESET);
                    println!("Log path: {}{}{}\n", TuiRenderer::CYAN, orchestrator.blackbox.log_file.display(), TuiRenderer::RESET);
                    for evt in events {
                        let time = if evt.timestamp.len() >= 19 { &evt.timestamp[11..19] } else { &evt.timestamp };
                        let status_color = if evt.status == "ok" { TuiRenderer::GREEN } else { TuiRenderer::YELLOW };
                        let duration_str = evt.duration_ms.map(|d| format!(" ({}ms)", d)).unwrap_or_default();
                        let short_hash = if evt.payload_hash.len() >= 8 { &evt.payload_hash[..8] } else { &evt.payload_hash };
                        println!(
                            "  [{}] {}{:12}{} {:14} {}{}{}{} #hash:{}",
                            time,
                            TuiRenderer::CYAN,
                            evt.event_type,
                            TuiRenderer::RESET,
                            evt.source,
                            status_color,
                            evt.status,
                            duration_str,
                            TuiRenderer::RESET,
                            short_hash
                        );
                        println!("      `-> {}", evt.payload_summary);
                    }
                    println!();
                }
                true
            }
            "/daemons" => {
                let list = orchestrator.tools.daemons.list_daemons();
                if list.is_empty() {
                    println!("{}No background daemons currently managed.{}\nSpawn one via: !& <cmd> or start_daemon tool.\n", TuiRenderer::DIM, TuiRenderer::RESET);
                } else {
                    println!("\n{}[BACKGROUND DAEMON REGISTRY]{}", TuiRenderer::BOLD, TuiRenderer::RESET);
                    for d in list {
                        let status_badge = if d.is_alive {
                            format!("{}[RUNNING]{}", TuiRenderer::GREEN, TuiRenderer::RESET)
                        } else {
                            format!("{}[EXIT: {}]{}", TuiRenderer::DIM, d.exit_code.unwrap_or(0), TuiRenderer::RESET)
                        };
                        println!("  - ID: {}{}{} | PID: {} | Uptime: {}s | Status: {}",
                            TuiRenderer::CYAN, d.id, TuiRenderer::RESET, d.pid, d.uptime_secs, status_badge
                        );
                        println!("    Command: {}", d.command);
                        if !d.recent_logs.is_empty() {
                            let last = d.recent_logs.last().unwrap();
                            println!("    Last output: {}{}{}", TuiRenderer::DIM, last, TuiRenderer::RESET);
                        }
                    }
                    println!("\nCommands: /daemon logs <id> | /daemon kill <id> | /daemon start <cmd>\n");
                }
                true
            }
            "/daemon" => {
                let parts: Vec<&str> = arg.split_whitespace().collect();
                if parts.len() < 2 {
                    println!("{}Usage: /daemon [logs <id> | kill <id> | start <command>]{}\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                    return true;
                }
                let sub = parts[0].to_lowercase();
                let target = parts[1];

                match sub.as_str() {
                    "logs" => {
                        match orchestrator.tools.daemons.get_daemon_logs(target, 30) {
                            Some(logs) => {
                                println!("\n{}[DAEMON LOGS: {}]{}", TuiRenderer::BOLD, target, TuiRenderer::RESET);
                                if logs.is_empty() {
                                    println!("  (no output captured yet)");
                                } else {
                                    for line in logs {
                                        println!("  {}", line);
                                    }
                                }
                                println!();
                            }
                            None => println!("{}[FAIL] Daemon '{}' not found.{}\n", TuiRenderer::RED, target, TuiRenderer::RESET),
                        }
                    }
                    "kill" => {
                        match orchestrator.tools.daemons.kill_daemon(target) {
                            Ok(_) => println!("{}[OK] Daemon '{}' terminated.{}\n", TuiRenderer::GREEN, target, TuiRenderer::RESET),
                            Err(e) => println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                        }
                    }
                    "start" => {
                        let cmd = parts[1..].join(" ");
                        match orchestrator.tools.daemons.start_daemon(&cmd, &orchestrator.workspace_root) {
                            Ok(info) => println!("{}[OK] Daemon '{}' spawned with PID {}: {}{}\n", TuiRenderer::GREEN, info.id, info.pid, cmd, TuiRenderer::RESET),
                            Err(e) => println!("{}[FAIL] Failed to start daemon: {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                        }
                    }
                    _ => {
                        println!("{}Usage: /daemon [logs <id> | kill <id> | start <command>]{}\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                    }
                }
                true
            }
            "/deps" => {
                let parts: Vec<&str> = arg.split_whitespace().collect();
                if parts.is_empty() {
                    match orchestrator.tools.deps.list_dependencies(&orchestrator.workspace_root) {
                        Ok(list) => {
                            let eco = orchestrator.tools.deps.detect_ecosystem(&orchestrator.workspace_root)
                                .map(|e| e.as_str())
                                .unwrap_or("unknown");
                            println!("\n{}[PROJECT DEPENDENCIES ({})]{} (Total: {})", TuiRenderer::BOLD, eco, TuiRenderer::RESET, list.len());
                            if list.is_empty() {
                                println!("  No direct dependencies found in manifest.");
                            } else {
                                for d in list {
                                    let dev_str = if d.is_dev { format!("{}[dev]{}", TuiRenderer::DIM, TuiRenderer::RESET) } else { String::new() };
                                    println!("  - {}{}{} @ {} {}", TuiRenderer::CYAN, d.name, TuiRenderer::RESET, d.version, dev_str);
                                }
                            }
                            println!("\nCommands: /deps add <pkg> | /deps rm <pkg> | /deps audit\n");
                        }
                        Err(e) => println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                    }
                } else {
                    match parts[0].to_lowercase().as_str() {
                        "add" => {
                            if parts.len() < 2 {
                                println!("{}Usage: /deps add <package_name> [--dev]{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                            } else {
                                let pkg = parts[1];
                                let is_dev = parts.contains(&"--dev") || parts.contains(&"-D");
                                println!("{}[ACODE] Installing dependency '{}'...{}", TuiRenderer::DIM, pkg, TuiRenderer::RESET);
                                match orchestrator.tools.deps.add_dependency(&orchestrator.workspace_root, pkg, None, is_dev) {
                                    Ok(out) => println!("{}[OK] Installed '{}':\n{}{}\n", TuiRenderer::GREEN, pkg, out, TuiRenderer::RESET),
                                    Err(e) => println!("{}[FAIL] Installation failed: {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                                }
                            }
                        }
                        "remove" | "rm" => {
                            if parts.len() < 2 {
                                println!("{}Usage: /deps rm <package_name>{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                            } else {
                                let pkg = parts[1];
                                println!("{}[ACODE] Removing dependency '{}'...{}", TuiRenderer::DIM, pkg, TuiRenderer::RESET);
                                match orchestrator.tools.deps.remove_dependency(&orchestrator.workspace_root, pkg) {
                                    Ok(out) => println!("{}[OK] Removed '{}':\n{}{}\n", TuiRenderer::GREEN, pkg, out, TuiRenderer::RESET),
                                    Err(e) => println!("{}[FAIL] Removal failed: {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                                }
                            }
                        }
                        "audit" => {
                            println!("{}[ACODE] Running ecosystem security audit...{}", TuiRenderer::DIM, TuiRenderer::RESET);
                            match orchestrator.tools.deps.audit_dependencies(&orchestrator.workspace_root) {
                                Ok(out) => println!("\n{}[AUDIT RESULTS]{}\n{}\n", TuiRenderer::BOLD, TuiRenderer::RESET, out),
                                Err(e) => println!("{}[FAIL] Audit error: {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                            }
                        }
                        _ => {
                            println!("{}Usage: /deps [add <pkg> | rm <pkg> | audit]{}\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                        }
                    }
                }
                true
            }
            "/docker" | "/container" => {
                let parts: Vec<&str> = arg.split_whitespace().collect();
                if parts.is_empty() || parts[0].eq_ignore_ascii_case("ps") || parts[0].eq_ignore_ascii_case("list") {
                    match orchestrator.tools.containers.list_containers(true) {
                        Ok(list) => {
                            println!("\n{}[CONTAINER FLEET STATUS]{} (Total: {})", TuiRenderer::BOLD, TuiRenderer::RESET, list.len());
                            if list.is_empty() {
                                println!("  No containers found.");
                            } else {
                                for c in list {
                                    let state_badge = if c.state == "running" {
                                        format!("{}[RUNNING]{}", TuiRenderer::GREEN, TuiRenderer::RESET)
                                    } else {
                                        format!("{}[{}]{}", TuiRenderer::DIM, c.state, TuiRenderer::RESET)
                                    };
                                    println!("  - {}{}{} ({}) | Image: {} | {}", TuiRenderer::CYAN, c.id, TuiRenderer::RESET, c.name, c.image, state_badge);
                                    if !c.ports.is_empty() {
                                        println!("    Ports: {}", c.ports);
                                    }
                                }
                            }
                            println!("\nCommands: /docker logs <id> | /docker up | /docker down | /docker inspect <id>\n");
                        }
                        Err(e) => println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                    }
                } else {
                    match parts[0].to_lowercase().as_str() {
                        "status" => {
                            match orchestrator.tools.containers.check_status() {
                                Ok(s) => println!("\n{}[OK]{} {}\n", TuiRenderer::GREEN, TuiRenderer::RESET, s),
                                Err(e) => println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                            }
                        }
                        "logs" => {
                            if parts.len() < 2 {
                                println!("{}Usage: /docker logs <id_or_name>{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                            } else {
                                let target = parts[1];
                                match orchestrator.tools.containers.get_logs(target, 40) {
                                    Ok(logs) => println!("\n{}[LOGS: {}]{}\n{}\n", TuiRenderer::BOLD, target, TuiRenderer::RESET, logs),
                                    Err(e) => println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                                }
                            }
                        }
                        "inspect" => {
                            if parts.len() < 2 {
                                println!("{}Usage: /docker inspect <id_or_name>{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                            } else {
                                let target = parts[1];
                                match orchestrator.tools.containers.inspect_container(target) {
                                    Ok(info) => println!("\n{}[INSPECT: {}]{}\n{}\n", TuiRenderer::BOLD, target, TuiRenderer::RESET, info),
                                    Err(e) => println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                                }
                            }
                        }
                        "up" => {
                            println!("{}[ACODE] Launching compose cluster in background...{}", TuiRenderer::DIM, TuiRenderer::RESET);
                            match orchestrator.tools.containers.compose_up(&orchestrator.workspace_root) {
                                Ok(out) => println!("{}[OK] Compose up:\n{}{}\n", TuiRenderer::GREEN, out, TuiRenderer::RESET),
                                Err(e) => println!("{}[FAIL] Compose up failed: {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                            }
                        }
                        "down" => {
                            println!("{}[ACODE] Stopping compose cluster...{}", TuiRenderer::DIM, TuiRenderer::RESET);
                            match orchestrator.tools.containers.compose_down(&orchestrator.workspace_root) {
                                Ok(out) => println!("{}[OK] Compose down:\n{}{}\n", TuiRenderer::GREEN, out, TuiRenderer::RESET),
                                Err(e) => println!("{}[FAIL] Compose down failed: {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                            }
                        }
                        _ => {
                            println!("{}Usage: /docker [ps | logs <id> | inspect <id> | up | down | status]{}\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                        }
                    }
                }
                true
            }
            "/symbols" => {
                let parts: Vec<&str> = arg.split_whitespace().collect();
                if parts.is_empty() {
                    println!("{}Usage: /symbols <file_path> | /symbols find <name>{}\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                } else if parts[0].eq_ignore_ascii_case("find") {
                    if parts.len() < 2 {
                        println!("{}Usage: /symbols find <identifier_name>{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                    } else {
                        let name = parts[1];
                        let matches = orchestrator.tools.symbols.find_symbol_definitions(name);
                        println!("\n{}[SYMBOL DEFINITIONS: '{}']{} (Matches: {})", TuiRenderer::BOLD, name, TuiRenderer::RESET, matches.len());
                        if matches.is_empty() {
                            println!("  No definitions found.");
                        } else {
                            for m in matches {
                                let pub_badge = if m.is_public { format!("{}[pub]{}", TuiRenderer::GREEN, TuiRenderer::RESET) } else { String::new() };
                                println!("  - {}:{}: {}{} {}", m.file_path, m.line_number, TuiRenderer::CYAN, m.signature, pub_badge);
                            }
                        }
                        println!();
                    }
                } else {
                    let file_path = orchestrator.workspace_root.join(parts[0]);
                    match orchestrator.tools.symbols.extract_symbols_from_file(&file_path) {
                        Ok(syms) => {
                            println!("\n{}[SYMBOLS: {}]{} (Total: {})", TuiRenderer::BOLD, parts[0], TuiRenderer::RESET, syms.len());
                            if syms.is_empty() {
                                println!("  No symbol declarations found.");
                            } else {
                                for s in syms {
                                    let pub_badge = if s.is_public { format!("{}[pub]{}", TuiRenderer::GREEN, TuiRenderer::RESET) } else { String::new() };
                                    println!("  L{:4} | {:9} | {}{}{} {}", s.line_number, s.kind, TuiRenderer::CYAN, s.name, TuiRenderer::RESET, pub_badge);
                                }
                            }
                            println!();
                        }
                        Err(e) => println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                    }
                }
                true
            }
            "/rename" => {
                let parts: Vec<&str> = arg.split_whitespace().collect();
                if parts.len() < 2 {
                    println!("{}Usage: /rename <old_identifier> <new_identifier> [path_filter]{}\n  Example: /rename EngineContext ExecutionContext src/\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                    return true;
                }
                let old_name = parts[0];
                let new_name = parts[1];
                let filter = if parts.len() > 2 { Some(parts[2]) } else { None };

                match orchestrator.tools.symbols.rename_symbol(old_name, new_name, filter) {
                    Ok(modified) => {
                        if modified.is_empty() {
                            println!("{}[INFO] Identifier '{}' not found in workspace.{}\n", TuiRenderer::YELLOW, old_name, TuiRenderer::RESET);
                        } else {
                            let total_occ: usize = modified.iter().map(|(_, c)| *c).sum();
                            println!("\n{}[RENAME COMPLETE]{} '{}' -> '{}' ({} replacements in {} files)", TuiRenderer::BOLD, TuiRenderer::RESET, old_name, new_name, total_occ, modified.len());
                            for (file, count) in modified {
                                println!("  {}[MODIFIED]{} {} ({} tokens)", TuiRenderer::GREEN, TuiRenderer::RESET, file, count);
                            }
                            println!();
                        }
                    }
                    Err(e) => println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                }
                true
            }
            "/tasks" => {
                match &orchestrator.task_graph {
                    Some(graph) => {
                        println!("\n{}[ACTIVE GOAL DECOMPOSITION GRAPH]{}\n{}\n", TuiRenderer::BOLD, TuiRenderer::RESET, graph.render_progress());
                    }
                    None => {
                        println!("{}No active task graph. Create one using: /task new <goal>{}\n", TuiRenderer::DIM, TuiRenderer::RESET);
                    }
                }
                true
            }
            "/task" => {
                let parts: Vec<&str> = arg.split_whitespace().collect();
                if parts.is_empty() {
                    println!("{}Usage: /task [new <goal> | add <step_title> | done | fail <reason> | clear]{}\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                    return true;
                }
                let sub = parts[0].to_lowercase();
                match sub.as_str() {
                    "new" => {
                        let goal = parts[1..].join(" ");
                        if goal.is_empty() {
                            println!("{}Usage: /task new <goal summary>{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                        } else {
                            orchestrator.task_graph = Some(crate::agent::planner::TaskGraph::new(&goal));
                            println!("{}[OK] Initialized task graph for goal: '{}'{}\nAdd steps via: /task add <step_title>\n", TuiRenderer::GREEN, goal, TuiRenderer::RESET);
                        }
                    }
                    "add" => {
                        let step = parts[1..].join(" ");
                        if step.is_empty() {
                            println!("{}Usage: /task add <step title>{}\n", TuiRenderer::RED, TuiRenderer::RESET);
                        } else {
                            if orchestrator.task_graph.is_none() {
                                orchestrator.task_graph = Some(crate::agent::planner::TaskGraph::new("Active Task Sequence"));
                            }
                            if let Some(graph) = orchestrator.task_graph.as_mut() {
                                let id = graph.add_step(&step, "");
                                println!("{}[OK] Added step {}: '{}'{}\n", TuiRenderer::GREEN, id, step, TuiRenderer::RESET);
                            }
                        }
                    }
                    "done" | "next" => {
                        if let Some(graph) = orchestrator.task_graph.as_mut() {
                            graph.complete_current();
                            println!("\n{}[STEP ADVANCED]{}\n{}\n", TuiRenderer::BOLD, TuiRenderer::RESET, graph.render_progress());
                        } else {
                            println!("{}No active task graph. Create one via: /task new <goal>{}\n", TuiRenderer::YELLOW, TuiRenderer::RESET);
                        }
                    }
                    "fail" => {
                        let reason = parts[1..].join(" ");
                        if let Some(graph) = orchestrator.task_graph.as_mut() {
                            graph.fail_current(if reason.is_empty() { "Manual failure mark" } else { &reason });
                            println!("{}[WARN] Current step marked failed. Re-planning or repair suggested.{}\n", TuiRenderer::YELLOW, TuiRenderer::RESET);
                        }
                    }
                    "clear" | "reset" => {
                        orchestrator.task_graph = None;
                        println!("{}[OK] Cleared active task graph.{}\n", TuiRenderer::GREEN, TuiRenderer::RESET);
                    }
                    _ => {
                        println!("{}Usage: /task [new <goal> | add <step> | done | fail <reason> | clear]{}\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                    }
                }
                true
            }
            "/sandbox" => {
                let parts: Vec<&str> = arg.split_whitespace().collect();
                if parts.is_empty() || parts[0].eq_ignore_ascii_case("status") || parts[0].eq_ignore_ascii_case("list") {
                    println!("\n{}[SHADOW WORKSPACE SANDBOX]{}", TuiRenderer::BOLD, TuiRenderer::RESET);
                    if let Some(session) = &orchestrator.active_sandbox {
                        println!("  Active Session:  {}{}{}", TuiRenderer::GREEN, session.session_id, TuiRenderer::RESET);
                        println!("  Branch:          {}", session.branch_name);
                        println!("  Worktree Path:   {}", session.worktree_path.display());
                        println!("  Original Root:   {}", session.original_root.display());
                        println!("  Status:          Attached & Active\n");
                    } else {
                        println!("  Active Session:  None (Running on primary workspace)\n");
                    }

                    match orchestrator.tools.sandbox.list_sandboxes() {
                        Ok(list) => {
                            if list.is_empty() {
                                println!("  Zero shadow worktrees currently in .daedalus/shadow/.\n");
                            } else {
                                println!("  {}[ACTIVE SHADOW WORKTREES]{}", TuiRenderer::BOLD, TuiRenderer::RESET);
                                for s in list {
                                    let active_mark = if orchestrator.active_sandbox.as_ref().map(|a| a.session_id == s.session_id).unwrap_or(false) {
                                        format!(" {}[ATTACHED]{}", TuiRenderer::GREEN, TuiRenderer::RESET)
                                    } else {
                                        String::new()
                                    };
                                    println!("  - {}{}{} ({}){}", TuiRenderer::CYAN, s.session_id, TuiRenderer::RESET, s.branch, active_mark);
                                    println!("    Path: {}", s.path);
                                }
                                println!();
                            }
                        }
                        Err(e) => println!("{}[FAIL] Error listing sandboxes: {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                    }
                    println!("Commands:");
                    println!("  /sandbox start [name] - Create and attach isolated shadow git worktree");
                    println!("  /sandbox merge        - Apply shadow modifications to primary workspace & teardown");
                    println!("  /sandbox abort        - Discard shadow worktree & branch without touching primary code\n");
                    return true;
                }

                match parts[0].to_lowercase().as_str() {
                    "start" | "create" | "new" => {
                        let custom_name = if parts.len() > 1 { Some(parts[1]) } else { None };
                        println!("{}[ACODE] Spawning isolated git shadow worktree...{}", TuiRenderer::DIM, TuiRenderer::RESET);
                        match orchestrator.tools.sandbox.create_sandbox(custom_name) {
                            Ok(session) => {
                                let wt_path = session.worktree_path.clone();
                                let session_id = session.session_id.clone();
                                let branch = session.branch_name.clone();
                                orchestrator.active_sandbox = Some(session);
                                orchestrator.set_workspace(wt_path.clone());
                                println!("\n{}[OK] Shadow Sandbox '{}' active!{}", TuiRenderer::GREEN, session_id, TuiRenderer::RESET);
                                println!("  Branch:        {}{}{}", TuiRenderer::CYAN, branch, TuiRenderer::RESET);
                                println!("  Worktree Path: {}", wt_path.display());
                                println!("  {}[INFO] Workspace root switched to shadow tree. All autonomous edits & builds are 100% isolated.{}\n", TuiRenderer::DIM, TuiRenderer::RESET);
                            }
                            Err(e) => println!("{}[FAIL] Failed to create sandbox: {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                        }
                    }
                    "merge" | "apply" => {
                        if let Some(session) = orchestrator.active_sandbox.take() {
                            println!("{}[ACODE] Computing diff and merging shadow sandbox '{}' back into primary workspace...{}", TuiRenderer::DIM, session.session_id, TuiRenderer::RESET);
                            let orig_root = session.original_root.clone();
                            match orchestrator.tools.sandbox.merge_sandbox(&session) {
                                Ok(msg) => {
                                    orchestrator.set_workspace(orig_root.clone());
                                    println!("\n{}[OK] Sandbox merged successfully!{}", TuiRenderer::GREEN, TuiRenderer::RESET);
                                    println!("  {}\n  Workspace restored to: {}\n", msg, orig_root.display());
                                }
                                Err(e) => {
                                    orchestrator.active_sandbox = Some(session);
                                    println!("{}[FAIL] Merge failed: {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET);
                                }
                            }
                        } else {
                            println!("{}No active shadow sandbox attached. Use /sandbox start [name]{}\n", TuiRenderer::YELLOW, TuiRenderer::RESET);
                        }
                    }
                    "abort" | "discard" | "drop" => {
                        if let Some(session) = orchestrator.active_sandbox.take() {
                            println!("{}[ACODE] Discarding shadow sandbox '{}'...{}", TuiRenderer::DIM, session.session_id, TuiRenderer::RESET);
                            let orig_root = session.original_root.clone();
                            match orchestrator.tools.sandbox.discard_sandbox(&session) {
                                Ok(msg) => {
                                    orchestrator.set_workspace(orig_root.clone());
                                    println!("\n{}[OK] Sandbox discarded cleanly. Zero modifications applied to primary workspace.{}", TuiRenderer::GREEN, TuiRenderer::RESET);
                                    println!("  {}\n  Workspace restored to: {}\n", msg, orig_root.display());
                                }
                                Err(e) => {
                                    orchestrator.set_workspace(orig_root);
                                    println!("{}[FAIL] Discard error: {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET);
                                }
                            }
                        } else {
                            println!("{}No active shadow sandbox attached to abort.{}\n", TuiRenderer::YELLOW, TuiRenderer::RESET);
                        }
                    }
                    _ => {
                        println!("{}Usage: /sandbox [start [name] | merge | abort | list | status]{}\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                    }
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

    fn load_bookmarks() -> std::collections::HashMap<String, String> {
        if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
            let p = std::path::PathBuf::from(home).join(".daedalus").join("bookmarks.json");
            if let Ok(content) = std::fs::read_to_string(p) {
                if let Ok(map) = serde_json::from_str(&content) {
                    return map;
                }
            }
        }
        std::collections::HashMap::new()
    }

    fn save_bookmarks(bookmarks: &std::collections::HashMap<String, String>) {
        if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
            let dir = std::path::PathBuf::from(home).join(".daedalus");
            let _ = std::fs::create_dir_all(&dir);
            let p = dir.join("bookmarks.json");
            if let Ok(content) = serde_json::to_string_pretty(bookmarks) {
                let _ = std::fs::write(p, content);
            }
        }
    }

    fn print_help() {
        println!("\n{}[ATLAS DAEDALUS COMMANDS]{}", TuiRenderer::BOLD, TuiRenderer::RESET);
        println!("  {}/help{}            - Show this command reference", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/security{}        - Manage agent security mode (/security mode full|sandboxed|strict)", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/sandbox [action]{} - Shadow git worktrees (/sandbox start [name], merge, abort, list)", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/memory{}          - Inspect or update persistent USER.md and MEMORY.md", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/blackbox{}        - View BlackBox flight recorder telemetry & audit stream", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/daemons{}         - List and inspect background process daemons", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/daemon <action>{} - Manage daemon (logs <id>, kill <id>, start <cmd>)", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/deps [action]{}   - Cross-ecosystem package manager (list, add, rm, audit)", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/docker [action]{} - Container fleet manager (ps, logs, inspect, up, down)", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/symbols <path>{}  - Extract all symbols or find symbol definition (/symbols find <name>)", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/rename <o> <n>{}  - Semantic identifier renaming across workspace", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/tasks{}           - View active goal decomposition task graph and checklist", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/task <action>{}   - Manage task graph (new <goal>, add <step>, done, clear)", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/plan <goal>{}     - Architect Mode: generate implementation plan before coding", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/info{}            - Display active workspace root, git branch, and context", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/cd <path|@alias>{} - Switch active workspace root directory", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/web <query>{}     - Direct terminal web search (DuckDuckGo + HackerNews)", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/fetch <url>{}     - Extract readable Markdown from any URL, paper, or transcript", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/test [cmd]{}      - Execute project test suite or custom test command", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/repos{}           - Discover sibling workspaces in parent directory", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/history{}         - Display recent command history", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/undo{}            - Revert all uncommitted working tree changes", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/diff{}            - Inspect working tree git diff", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/status{}          - Display git repository status", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/commit <msg>{}    - Commit all changes with standard author", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/cost{}            - Display Chronicle token usage and USD costs", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/audit{}           - Run Argus AST code review and secret scan", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/tunnel <port>{}  - Spawn a public HTTPS Beacon tunnel", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/swarm <goal>{}    - Autonomous multi-agent swarm delegation (Explorer -> Coder -> Verifier)", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/mcp [action]{}    - Model Context Protocol server suite (list, refresh, add, remove)", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/host{}            - Live host metrics (CPU cores, free RAM, process RSS, uptime)", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/pulse{}           - View live OS host memory and CPU metrics", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/model <name>{}   - Switch active LLM model", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/key <p> <key>{}   - Configure API key (gemini, anthropic, openai, powerbox)", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/clear{}           - Clear console and reset conversational context", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}/exit{}            - Terminate REPL session", TuiRenderer::CYAN, TuiRenderer::RESET);
        println!("  {}!<cmd>{}            - Direct synchronous shell execution bypass", TuiRenderer::YELLOW, TuiRenderer::RESET);
        println!("  {}!&<cmd>{}           - Direct non-blocking background daemon spawn\n", TuiRenderer::YELLOW, TuiRenderer::RESET);
    }
}
