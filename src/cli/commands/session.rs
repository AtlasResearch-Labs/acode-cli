//! Atlas Daedalus — Session, Navigation, Observability & Config Commands
//! (/help, /clear, /web, /fetch, /test, /cost, /audit, /pulse, /host, /swarm, /model, /key, /plan, /info, /cd, /bookmark, /history, /security, /memory, /blackbox, /deps, /symbols, /rename, /tasks, /task, /sandbox, /exit)

use crate::agent::orchestrator::{AgentEvent, DaedalusOrchestrator};
use crate::cli::tui::TuiRenderer;
use crate::cli::viewport::{pad_box_line, Viewport};

pub async fn handle(
    cmd: &str,
    arg: &str,
    _raw_input: &str,
    orchestrator: &mut DaedalusOrchestrator,
) -> bool {
    match cmd {
        "/help" => {
            print_help();
            true
        }
        "/clear" => {
            print!("\x1B[2J\x1B[1;1H");
            orchestrator.clear_history();
            println!(
                "{}[ACODE] Terminal cleared and session context reset.{}\n",
                TuiRenderer::DIM,
                TuiRenderer::RESET
            );
            true
        }
        "/web" => {
            if arg.is_empty() {
                println!(
                    "{}Usage: /web <query>{}\n  Example: /web rust reqwest timeout\n",
                    TuiRenderer::RED,
                    TuiRenderer::RESET
                );
                return true;
            }
            println!(
                "{}[ACODE] Searching web across DuckDuckGo and HackerNews...{}\n",
                TuiRenderer::DIM,
                TuiRenderer::RESET
            );
            match orchestrator.tools.web.web_search(arg, 5).await {
                Ok(res) => println!("{}\n", res),
                Err(e) => println!(
                    "{}[FAIL] Web search error: {}{}\n",
                    TuiRenderer::RED,
                    e,
                    TuiRenderer::RESET
                ),
            }
            true
        }
        "/fetch" | "/read" => {
            if arg.is_empty() {
                println!(
                    "{}Usage: /fetch <url>{}\n  Example: /fetch https://arxiv.org/abs/2312.11805\n",
                    TuiRenderer::RED,
                    TuiRenderer::RESET
                );
                return true;
            }
            let gate_args = serde_json::json!({ "url": arg });
            if !orchestrator.check_gate("web_fetch", &gate_args) {
                return true;
            }
            println!(
                "{}[ACODE] Fetching and parsing web content...{}\n",
                TuiRenderer::DIM,
                TuiRenderer::RESET
            );
            match orchestrator.tools.web.web_fetch(arg, 8000).await {
                Ok(content) => println!("{}\n", content),
                Err(e) => println!(
                    "{}[FAIL] Fetch error: {}{}\n",
                    TuiRenderer::RED,
                    e,
                    TuiRenderer::RESET
                ),
            }
            true
        }
        "/test" => {
            let test_cmd = if arg.is_empty() { "cargo test" } else { arg };
            let gate_args = serde_json::json!({ "command": test_cmd });
            if !orchestrator.check_gate("run_tests", &gate_args) {
                return true;
            }
            println!(
                "{}[ACODE] Running test suite...{}",
                TuiRenderer::DIM,
                TuiRenderer::RESET
            );
            let res = if arg.is_empty() {
                orchestrator.verifier.run_verification().await
            } else {
                orchestrator.verifier.run_custom_test(arg, None).await
            };

            let cmd_name = res
                .command
                .as_deref()
                .unwrap_or("no test framework detected");
            println!(
                "\n{}[TEST EXECUTION]{} Command: {}{}{} ({}ms)",
                TuiRenderer::BOLD,
                TuiRenderer::RESET,
                TuiRenderer::CYAN,
                cmd_name,
                TuiRenderer::RESET,
                res.duration_ms
            );
            if res.passed {
                println!(
                    "{}[OK] All tests passed successfully.{}\n",
                    TuiRenderer::GREEN,
                    TuiRenderer::RESET
                );
            } else {
                println!("{}[FAIL] Tests failed.\n{}\n", TuiRenderer::RED, res.output);
            }
            true
        }
        "/cost" => {
            let traces = orchestrator.chronicle.get_recent_traces(10);
            let metrics = orchestrator.cost_tracker.get_metrics();
            let total_cost = metrics.total_cost_usd;
            let total_tokens = metrics.prompt_tokens + metrics.completion_tokens;

            let vp = Viewport::current();
            let width = vp.card_width(64);
            let title = if width < 42 {
                "╭─ [COST] "
            } else {
                "╭─ [SESSION COST & TOKEN TELEMETRY] "
            };
            let rem_top = width.saturating_sub(title.chars().count() + 1);
            let top_border = format!(
                "{}{title}{}╮{}",
                TuiRenderer::INDIGO,
                "─".repeat(rem_top),
                TuiRenderer::RESET
            );
            let bot_border = format!(
                "{INDIGO}╰{}╯{RESET}",
                "─".repeat(width.saturating_sub(2)),
                INDIGO = TuiRenderer::INDIGO,
                RESET = TuiRenderer::RESET
            );

            println!("\n{}", top_border);
            println!(
                "{}",
                pad_box_line(
                    &format!(
                        "  Model:               {}{}",
                        TuiRenderer::CYAN_ACCENT,
                        orchestrator.provider.config.model
                    ),
                    width,
                    TuiRenderer::INDIGO,
                    TuiRenderer::BOX_V,
                    TuiRenderer::RESET
                )
            );
            println!(
                "{}",
                pad_box_line(
                    &format!(
                        "  Total Turns:         {}{}",
                        TuiRenderer::WHITE,
                        metrics.turns_count
                    ),
                    width,
                    TuiRenderer::INDIGO,
                    TuiRenderer::BOX_V,
                    TuiRenderer::RESET
                )
            );
            println!(
                "{}",
                pad_box_line(
                    &format!(
                        "  Prompt Tokens:       {}{} tokens",
                        TuiRenderer::WHITE,
                        metrics.prompt_tokens
                    ),
                    width,
                    TuiRenderer::INDIGO,
                    TuiRenderer::BOX_V,
                    TuiRenderer::RESET
                )
            );
            println!(
                "{}",
                pad_box_line(
                    &format!(
                        "  Completion Tokens:   {}{} tokens",
                        TuiRenderer::WHITE,
                        metrics.completion_tokens
                    ),
                    width,
                    TuiRenderer::INDIGO,
                    TuiRenderer::BOX_V,
                    TuiRenderer::RESET
                )
            );
            println!(
                "{}",
                pad_box_line(
                    &format!(
                        "  Total Tokens:        {}{} tokens",
                        TuiRenderer::BOLD,
                        total_tokens
                    ),
                    width,
                    TuiRenderer::INDIGO,
                    TuiRenderer::BOX_V,
                    TuiRenderer::RESET
                )
            );
            println!(
                "{}",
                pad_box_line(
                    &format!(
                        "  Estimated USD Cost:  {}${:.5}",
                        TuiRenderer::EMERALD,
                        total_cost
                    ),
                    width,
                    TuiRenderer::INDIGO,
                    TuiRenderer::BOX_V,
                    TuiRenderer::RESET
                )
            );
            println!("{}\n", bot_border);

            if !traces.is_empty() {
                println!("Recent Model Spans:");
                for t in &traces {
                    let time = if t.timestamp.len() >= 19 {
                        &t.timestamp[11..19]
                    } else {
                        &t.timestamp
                    };
                    println!(
                        "  [{}] {} - {} tokens (${:.5})",
                        time, t.model, t.tokens.total, t.cost_usd
                    );
                }
                println!();
            }
            true
        }
        "/timer" | "/countdown" => {
            if arg.is_empty() {
                println!(
                    "{}Usage: /timer <duration> [label]{}\n  Examples: /timer 5s, /timer 10, /timer 2m \"build check\"\n",
                    TuiRenderer::ROSE,
                    TuiRenderer::RESET
                );
                return true;
            }
            let parts: Vec<&str> = arg.split_whitespace().collect();
            let raw_dur = parts[0];
            let label = if parts.len() > 1 {
                parts[1..].join(" ")
            } else {
                format!("{} timer", raw_dur)
            };

            let seconds = if let Some(stripped) = raw_dur.strip_suffix('m') {
                stripped.parse::<u64>().unwrap_or(1) * 60
            } else if let Some(stripped) = raw_dur.strip_suffix('s') {
                stripped.parse::<u64>().unwrap_or(1)
            } else {
                raw_dur.parse::<u64>().unwrap_or(5)
            };

            println!(
                "\n{}╭─ [TIMER STARTED] ─── {}{} ({}s)",
                TuiRenderer::INDIGO,
                TuiRenderer::WHITE,
                label,
                seconds
            );

            let start = std::time::Instant::now();
            let total_dur = std::time::Duration::from_secs(seconds);
            let mut tick = 0;

            while start.elapsed() < total_dur {
                let remaining = total_dur.saturating_sub(start.elapsed());
                let rem_str = TuiRenderer::format_duration(remaining);
                let spinner = TuiRenderer::spinner_frame(tick);
                print!(
                    "\r\x1b[2K{}│{} {}{} {}remaining: {}{}{}",
                    TuiRenderer::INDIGO,
                    TuiRenderer::RESET,
                    TuiRenderer::CYAN_ACCENT,
                    spinner,
                    TuiRenderer::SLATE,
                    TuiRenderer::AMBER,
                    rem_str,
                    TuiRenderer::RESET
                );
                let _ = std::io::Write::flush(&mut std::io::stdout());
                tokio::time::sleep(std::time::Duration::from_millis(80)).await;
                tick += 1;
            }

            print!("\r\x1b[2K");
            println!(
                "{}╰─ [OK] Timer '{}' completed! {}\x07\n",
                TuiRenderer::INDIGO,
                label,
                TuiRenderer::RESET
            );
            true
        }
        "/outline" | "/tree" => {
            if arg.is_empty() {
                println!(
                    "{}Usage: /outline <file_path>{}\n  Example: /outline src/main.rs\n",
                    TuiRenderer::ROSE,
                    TuiRenderer::RESET
                );
                return true;
            }
            match orchestrator.tools.outline.file_outline(arg) {
                Ok(outline) => println!("\n{}\n", outline),
                Err(e) => println!("{}[FAIL] {}\n", TuiRenderer::ROSE, e),
            }
            true
        }
        "/audit" => {
            println!(
                "{}[ACODE] Launching Argus AST security audit...{}",
                TuiRenderer::DIM,
                TuiRenderer::RESET
            );
            let target = if arg.is_empty() { "." } else { arg };
            let (clean, summary) = orchestrator.tools.pillars.run_argus_audit(target).await;
            println!(
                "\n{}[ARGUS AUDIT]{} {}",
                TuiRenderer::BOLD,
                TuiRenderer::RESET,
                summary
            );
            if clean {
                println!(
                    "{}[OK] All files verified clean. Zero vulnerabilities detected.{}\n",
                    TuiRenderer::GREEN,
                    TuiRenderer::RESET
                );
            }
            true
        }
        "/pulse" => {
            let metrics = orchestrator.tools.pillars.get_pulse_metrics();
            println!(
                "\n{}[PULSE OBSERVABILITY]{}",
                TuiRenderer::BOLD,
                TuiRenderer::RESET
            );
            println!(
                "CPU Cores: {} · System Uptime: {}s\n",
                metrics.cpu_count, metrics.uptime_secs
            );
            true
        }
        "/host" | "/metrics" => {
            let m = crate::telemetry::host::HostInspector::collect();
            println!(
                "\n{}[HOST TELEMETRY METRICS]{}",
                TuiRenderer::BOLD,
                TuiRenderer::RESET
            );
            println!(
                "  Operating System: {}{}{}",
                TuiRenderer::CYAN,
                m.os_name,
                TuiRenderer::RESET
            );
            println!(
                "  CPU Cores:        {}{}{}",
                TuiRenderer::CYAN,
                m.cpu_cores,
                TuiRenderer::RESET
            );
            println!(
                "  Free Memory:      {}{}{} MB (Total: {} MB)",
                TuiRenderer::GREEN,
                m.available_memory_mb,
                TuiRenderer::RESET,
                m.total_memory_mb
            );
            println!(
                "  Process RSS:      {}{}{} MB",
                TuiRenderer::YELLOW,
                m.process_rss_mb,
                TuiRenderer::RESET
            );
            let hours = m.uptime_secs / 3600;
            let mins = (m.uptime_secs % 3600) / 60;
            let secs = m.uptime_secs % 60;
            println!(
                "  System Uptime:    {}h {}m {}s ({}s)\n",
                hours, mins, secs, m.uptime_secs
            );
            true
        }
        "/swarm" => {
            if arg.is_empty() {
                println!("{}Usage: /swarm <complex objective to delegate>{}\n  Example: /swarm add comprehensive tests for host metrics and verify build\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                return true;
            }
            println!("\n{}[MULTI-AGENT SWARM ACTIVE]{} Delegating objective to Explorer, Coder, and Verifier...", TuiRenderer::BOLD, TuiRenderer::RESET);
            println!("Goal: {}{}{}\n", TuiRenderer::CYAN, arg, TuiRenderer::RESET);

            let res = orchestrator
                .run_swarm(arg, |event| match event {
                    AgentEvent::Thinking(s) => TuiRenderer::print_thinking(s),
                    AgentEvent::Token(tok) => {
                        print!("{}", tok);
                        let _ = std::io::Write::flush(&mut std::io::stdout());
                    }
                    AgentEvent::ToolStart { name, args } => {
                        TuiRenderer::print_tool_start(name, args)
                    }
                    AgentEvent::ToolEnd {
                        name,
                        success,
                        output,
                    } => TuiRenderer::print_tool_end(name, success, output),
                    AgentEvent::TestResult {
                        command,
                        passed,
                        output,
                    } => TuiRenderer::print_test_result(command, passed, output),
                    AgentEvent::Error(e) => TuiRenderer::print_error(e),
                    _ => {}
                })
                .await;

            match res {
                Ok(outcome) => {
                    println!(
                        "\n{}[SWARM EXECUTION COMPLETE]{} in {}ms",
                        TuiRenderer::BOLD,
                        TuiRenderer::RESET,
                        outcome.duration_ms
                    );
                    if !outcome.files_modified.is_empty() {
                        println!(
                            "  Files Modified: {}{}{}",
                            TuiRenderer::GREEN,
                            outcome.files_modified.join(", "),
                            TuiRenderer::RESET
                        );
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
        "/model" => {
            if arg.is_empty() {
                println!(
                    "{}Current model: {} ({}){}\n",
                    TuiRenderer::CYAN,
                    orchestrator.provider.config.model,
                    orchestrator.provider.config.provider,
                    TuiRenderer::RESET
                );
            } else {
                orchestrator.provider.config.model = arg.to_string();
                if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
                    let daedalus_dir = std::path::PathBuf::from(home).join(".daedalus");
                    let _ = std::fs::create_dir_all(&daedalus_dir);
                    let env_file = daedalus_dir.join(".env");
                    let var_name = "DAEDALUS_MODEL";
                    let mut existing = std::fs::read_to_string(&env_file).unwrap_or_default();
                    if existing.contains(var_name) {
                        let lines: Vec<String> = existing
                            .lines()
                            .map(|l| {
                                if l.starts_with(var_name) {
                                    format!("{}={}", var_name, arg)
                                } else {
                                    l.to_string()
                                }
                            })
                            .collect();
                        existing = lines.join("\n") + "\n";
                    } else {
                        existing.push_str(&format!("{}={}\n", var_name, arg));
                    }
                    let _ = std::fs::write(&env_file, existing);
                }
                println!(
                    "{}[OK] Switched active model to: {}{}\n",
                    TuiRenderer::GREEN,
                    arg,
                    TuiRenderer::RESET
                );
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
                orchestrator.provider.config.base_url =
                    Some("https://powerbox.atlasresearchlabs.online/v1".to_string());
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
                    let lines: Vec<String> = existing
                        .lines()
                        .map(|l| {
                            if l.starts_with(var_name) {
                                format!("{}={}", var_name, key)
                            } else {
                                l.to_string()
                            }
                        })
                        .collect();
                    existing = lines.join("\n") + "\n";
                } else {
                    existing.push_str(&format!("{}={}\n", var_name, key));
                }

                if provider == "powerbox" {
                    if !existing.contains("POWERBOX_BASE_URL") {
                        existing.push_str(
                            "POWERBOX_BASE_URL=https://powerbox.atlasresearchlabs.online/v1\n",
                        );
                    }
                    if !existing.contains("POWERBOX_MODEL") {
                        existing.push_str("POWERBOX_MODEL=auto\n");
                    }
                }

                let _ = std::fs::write(&env_file, existing);
                println!(
                    "{}[OK] Saved '{}' key to ~/.daedalus/.env{}\n",
                    TuiRenderer::GREEN,
                    provider,
                    TuiRenderer::RESET
                );
            } else {
                println!(
                    "{}[OK] Updated '{}' key for current session.{}\n",
                    TuiRenderer::GREEN,
                    provider,
                    TuiRenderer::RESET
                );
            }
            true
        }
        "/plan" => {
            if arg.is_empty() {
                println!("{}Usage: /plan <goal or feature to architect>{}\n  Example: /plan implement JWT authentication in src/auth.rs\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                return true;
            }
            println!(
                "\n{}[ARCHITECT MODE]{} Generating implementation plan for: {}{}{}\n",
                TuiRenderer::BOLD,
                TuiRenderer::RESET,
                TuiRenderer::CYAN,
                arg,
                TuiRenderer::RESET
            );
            let _ = orchestrator
                .run_plan(arg, |event| match event {
                    AgentEvent::Thinking(s) => TuiRenderer::print_thinking(s),
                    AgentEvent::ToolStart { name, args } => {
                        TuiRenderer::print_tool_start(name, args)
                    }
                    AgentEvent::ToolEnd {
                        name,
                        success,
                        output,
                    } => TuiRenderer::print_tool_end(name, success, output),
                    AgentEvent::Error(err) => TuiRenderer::print_error(err),
                    _ => {}
                })
                .await;
            println!(
                "{}[OK] Plan generated. To execute this plan, prompt Daedalus to proceed.{}\n",
                TuiRenderer::GREEN,
                TuiRenderer::RESET
            );
            true
        }
        "/info" => {
            let status = orchestrator.tools.git.get_status().await;
            let traces = orchestrator.chronicle.get_recent_traces(1);
            let last_tokens = traces.first().map(|t| t.tokens.total).unwrap_or(0);
            let last_cost = traces.first().map(|t| t.cost_usd).unwrap_or(0.0);

            println!(
                "\n{}[WORKSPACE SITUATIONAL CONTEXT]{}",
                TuiRenderer::BOLD,
                TuiRenderer::RESET
            );
            println!(
                "  Workspace Root: {}{}{}",
                TuiRenderer::CYAN,
                orchestrator.workspace_root.display(),
                TuiRenderer::RESET
            );
            println!(
                "  Git Branch:     {}{}{}",
                TuiRenderer::CYAN,
                status.branch,
                TuiRenderer::RESET
            );
            println!(
                "  Working Tree:   {}",
                if status.clean {
                    format!("{}[CLEAN]{}", TuiRenderer::GREEN, TuiRenderer::RESET)
                } else {
                    format!(
                        "{}[DIRTY: {} modified]{}",
                        TuiRenderer::YELLOW,
                        status.modified.len(),
                        TuiRenderer::RESET
                    )
                }
            );
            println!(
                "  Provider/Model: {} ({})",
                orchestrator.provider.config.provider, orchestrator.provider.config.model
            );
            println!(
                "  Last Turn Cost: {} tokens (${:.5})\n",
                last_tokens, last_cost
            );
            true
        }
        "/cd" => {
            if arg.is_empty() {
                println!("{}Usage: /cd <directory_path | @alias>{}\n  Example: /cd F:\\projects\\Atlas-Research-Labs\n  Example: /cd @labs\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                return true;
            }

            let target_path_str = if let Some(alias) = arg.strip_prefix('@') {
                let bookmarks = load_bookmarks();
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
                println!(
                    "{}[FAIL] Directory does not exist: {}{}\n",
                    TuiRenderer::RED,
                    new_path.display(),
                    TuiRenderer::RESET
                );
                return true;
            }

            if !new_path.is_dir() {
                println!(
                    "{}[FAIL] Path is not a directory: {}{}\n",
                    TuiRenderer::RED,
                    new_path.display(),
                    TuiRenderer::RESET
                );
                return true;
            }

            let canonical = new_path.canonicalize().unwrap_or(new_path);
            orchestrator.set_workspace(canonical.clone());
            println!(
                "{}[OK] Switched active workspace to: {}{}\n",
                TuiRenderer::GREEN,
                canonical.display(),
                TuiRenderer::RESET
            );
            true
        }
        "/bookmark" => {
            let parts: Vec<&str> = arg.split_whitespace().collect();
            let mut bookmarks = load_bookmarks();

            if parts.is_empty() {
                if bookmarks.is_empty() {
                    println!(
                        "{}No bookmarks saved yet. Use: /bookmark add <alias> [path]{}\n",
                        TuiRenderer::DIM,
                        TuiRenderer::RESET
                    );
                } else {
                    println!(
                        "\n{}[SAVED WORKSPACE BOOKMARKS]{}",
                        TuiRenderer::BOLD,
                        TuiRenderer::RESET
                    );
                    for (alias, path) in &bookmarks {
                        println!(
                            "  {}{}@{}{} -> {}",
                            TuiRenderer::CYAN,
                            TuiRenderer::BOLD,
                            alias,
                            TuiRenderer::RESET,
                            path
                        );
                    }
                    println!("\nUsage: /cd @<alias>\n");
                }
                return true;
            }

            match parts[0].to_lowercase().as_str() {
                "add" => {
                    if parts.len() < 2 {
                        println!(
                            "{}Usage: /bookmark add <alias> [path]{}\n",
                            TuiRenderer::RED,
                            TuiRenderer::RESET
                        );
                        return true;
                    }
                    let alias = parts[1].trim_start_matches('@').to_string();
                    let path = if parts.len() > 2 {
                        parts[2..].join(" ")
                    } else {
                        orchestrator.workspace_root.to_string_lossy().to_string()
                    };
                    bookmarks.insert(alias.clone(), path.clone());
                    save_bookmarks(&bookmarks);
                    println!(
                        "{}[OK] Bookmarked @{} -> {}{}\n",
                        TuiRenderer::GREEN,
                        alias,
                        path,
                        TuiRenderer::RESET
                    );
                }
                "remove" | "rm" => {
                    if parts.len() < 2 {
                        println!(
                            "{}Usage: /bookmark remove <alias>{}\n",
                            TuiRenderer::RED,
                            TuiRenderer::RESET
                        );
                        return true;
                    }
                    let alias = parts[1].trim_start_matches('@');
                    if bookmarks.remove(alias).is_some() {
                        save_bookmarks(&bookmarks);
                        println!(
                            "{}[OK] Removed bookmark @{}{}\n",
                            TuiRenderer::GREEN,
                            alias,
                            TuiRenderer::RESET
                        );
                    } else {
                        println!(
                            "{}[FAIL] Bookmark @{} not found.{}\n",
                            TuiRenderer::RED,
                            alias,
                            TuiRenderer::RESET
                        );
                    }
                }
                _ => {
                    println!(
                        "{}Usage: /bookmark [add <alias> [path] | remove <alias>]{}\n",
                        TuiRenderer::CYAN,
                        TuiRenderer::RESET
                    );
                }
            }
            true
        }
        "/history" => {
            if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
                let history_path = std::path::PathBuf::from(home)
                    .join(".daedalus")
                    .join("history.txt");
                if let Ok(content) = std::fs::read_to_string(history_path) {
                    let lines: Vec<&str> = content.lines().collect();
                    let start_idx = if lines.len() > 15 {
                        lines.len() - 15
                    } else {
                        0
                    };
                    println!(
                        "\n{}[RECENT COMMAND HISTORY]{}",
                        TuiRenderer::BOLD,
                        TuiRenderer::RESET
                    );
                    for (idx, line) in lines[start_idx..].iter().enumerate() {
                        println!("  {:2}. {}", start_idx + idx + 1, line);
                    }
                    println!();
                } else {
                    println!(
                        "{}No command history recorded yet.{}\n",
                        TuiRenderer::DIM,
                        TuiRenderer::RESET
                    );
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
                println!(
                    "\n{}[AGENT SECURITY GATE STATUS]{}",
                    TuiRenderer::BOLD,
                    TuiRenderer::RESET
                );
                println!(
                    "  Security Mode:   {}{}{}",
                    mode_color,
                    cfg.mode,
                    TuiRenderer::RESET
                );
                println!("  Trusted Paths:   {}", cfg.trusted_paths.len());
                for tp in &cfg.trusted_paths {
                    println!("    + {}", tp.display());
                }
                println!(
                    "  Session Grants:  {} paths, {} commands",
                    orchestrator.security.session_allowed_paths.len(),
                    orchestrator.security.session_allowed_commands.len()
                );
                println!("\nCommands:");
                println!(
                    "  /security <full|sandboxed|strict>      - Set security profile directly"
                );
                println!("  /security mode <full|sandboxed|strict> - Toggle security profile");
                println!("  /security trust <path>                 - Add persistent trusted path");
                println!("  /security untrust <path>               - Remove trusted path");
                println!(
                    "  /security reset                        - Clear active session grants\n"
                );
                return true;
            }

            match parts[0].to_lowercase().as_str() {
                "full" | "sandboxed" | "strict" => {
                    match parts[0].parse::<crate::security::SecurityMode>() {
                        Ok(new_mode) => {
                            orchestrator.security.set_mode(new_mode);
                            println!(
                                "{}[OK] Switched agent security mode to: {}{}\n",
                                TuiRenderer::GREEN,
                                new_mode,
                                TuiRenderer::RESET
                            );
                        }
                        Err(e) => {
                            println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET);
                        }
                    }
                }
                "mode" => {
                    if parts.len() < 2 {
                        println!(
                            "{}Usage: /security mode <full | sandboxed | strict>{}\n",
                            TuiRenderer::RED,
                            TuiRenderer::RESET
                        );
                        return true;
                    }
                    match parts[1].parse::<crate::security::SecurityMode>() {
                        Ok(new_mode) => {
                            orchestrator.security.set_mode(new_mode);
                            println!(
                                "{}[OK] Switched agent security mode to: {}{}\n",
                                TuiRenderer::GREEN,
                                new_mode,
                                TuiRenderer::RESET
                            );
                        }
                        Err(e) => {
                            println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET);
                        }
                    }
                }
                "trust" => {
                    if parts.len() < 2 {
                        println!(
                            "{}Usage: /security trust <directory_path>{}\n",
                            TuiRenderer::RED,
                            TuiRenderer::RESET
                        );
                        return true;
                    }
                    let p = std::path::PathBuf::from(parts[1..].join(" "));
                    orchestrator.security.trust_path(p.clone());
                    println!(
                        "{}[OK] Added trusted path: {}{}\n",
                        TuiRenderer::GREEN,
                        p.display(),
                        TuiRenderer::RESET
                    );
                }
                "untrust" => {
                    if parts.len() < 2 {
                        println!(
                            "{}Usage: /security untrust <directory_path>{}\n",
                            TuiRenderer::RED,
                            TuiRenderer::RESET
                        );
                        return true;
                    }
                    let p = std::path::Path::new(parts[1]);
                    if orchestrator.security.untrust_path(p) {
                        println!(
                            "{}[OK] Removed trusted path: {}{}\n",
                            TuiRenderer::GREEN,
                            p.display(),
                            TuiRenderer::RESET
                        );
                    } else {
                        println!(
                            "{}[FAIL] Path was not in trusted list: {}{}\n",
                            TuiRenderer::RED,
                            p.display(),
                            TuiRenderer::RESET
                        );
                    }
                }
                "reset" => {
                    orchestrator.security.clear_session_grants();
                    println!(
                        "{}[OK] Cleared all active session permission grants.{}\n",
                        TuiRenderer::GREEN,
                        TuiRenderer::RESET
                    );
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
                println!(
                    "\n{}[PERSISTENT MEMORY LEDGER]{}",
                    TuiRenderer::BOLD,
                    TuiRenderer::RESET
                );
                println!(
                    "{}User Profile (~/.daedalus/USER.md):{}",
                    TuiRenderer::CYAN,
                    TuiRenderer::RESET
                );
                let user_prof = orchestrator.memory.get_user_profile();
                if user_prof.trim().is_empty() {
                    println!("  (empty)");
                } else {
                    for line in user_prof.lines() {
                        println!("  {}", line);
                    }
                }
                println!(
                    "\n{}Workspace Architectural Memory (.daedalus/MEMORY.md):{}",
                    TuiRenderer::CYAN,
                    TuiRenderer::RESET
                );
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
                    println!(
                        "{}Usage: /memory [user|project] <note to record>{}\n",
                        TuiRenderer::RED,
                        TuiRenderer::RESET
                    );
                } else if target == "user" {
                    match orchestrator.memory.update_user_memory(&note) {
                        Ok(_) => println!(
                            "{}[OK] Recorded note in ~/.daedalus/USER.md{}\n",
                            TuiRenderer::GREEN,
                            TuiRenderer::RESET
                        ),
                        Err(e) => {
                            println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET)
                        }
                    }
                } else if target == "project" {
                    match orchestrator.memory.update_project_memory(&note) {
                        Ok(_) => println!(
                            "{}[OK] Recorded note in .daedalus/MEMORY.md{}\n",
                            TuiRenderer::GREEN,
                            TuiRenderer::RESET
                        ),
                        Err(e) => {
                            println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET)
                        }
                    }
                } else {
                    println!(
                        "{}Unknown memory target '{}'. Use 'user' or 'project'.{}\n",
                        TuiRenderer::RED,
                        target,
                        TuiRenderer::RESET
                    );
                }
            }
            true
        }
        "/blackbox" | "/flight" => {
            let events = orchestrator.blackbox.read_recent_events(15);
            if events.is_empty() {
                println!(
                    "{}No telemetry events recorded in .daedalus/blackbox.jsonl yet.{}\n",
                    TuiRenderer::DIM,
                    TuiRenderer::RESET
                );
            } else {
                println!(
                    "\n{}[BLACKBOX FLIGHT RECORDER (RECENT 15 EVENTS)]{}",
                    TuiRenderer::BOLD,
                    TuiRenderer::RESET
                );
                println!(
                    "Log path: {}{}{}\n",
                    TuiRenderer::CYAN,
                    orchestrator.blackbox.log_file.display(),
                    TuiRenderer::RESET
                );
                for evt in events {
                    let time = if evt.timestamp.len() >= 19 {
                        &evt.timestamp[11..19]
                    } else {
                        &evt.timestamp
                    };
                    let status_color = if evt.status == "ok" {
                        TuiRenderer::GREEN
                    } else {
                        TuiRenderer::YELLOW
                    };
                    let duration_str = evt
                        .duration_ms
                        .map(|d| format!(" ({}ms)", d))
                        .unwrap_or_default();
                    let short_hash = if evt.payload_hash.len() >= 8 {
                        &evt.payload_hash[..8]
                    } else {
                        &evt.payload_hash
                    };
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
        "/deps" => {
            let parts: Vec<&str> = arg.split_whitespace().collect();
            if parts.is_empty() {
                match orchestrator
                    .tools
                    .deps
                    .list_dependencies(&orchestrator.workspace_root)
                {
                    Ok(list) => {
                        let eco = orchestrator
                            .tools
                            .deps
                            .detect_ecosystem(&orchestrator.workspace_root)
                            .map(|e| e.as_str())
                            .unwrap_or("unknown");
                        println!(
                            "\n{}[PROJECT DEPENDENCIES ({})]{} (Total: {})",
                            TuiRenderer::BOLD,
                            eco,
                            TuiRenderer::RESET,
                            list.len()
                        );
                        if list.is_empty() {
                            println!("  No direct dependencies found in manifest.");
                        } else {
                            for d in list {
                                let dev_str = if d.is_dev {
                                    format!("{}[dev]{}", TuiRenderer::DIM, TuiRenderer::RESET)
                                } else {
                                    String::new()
                                };
                                println!(
                                    "  - {}{}{} @ {} {}",
                                    TuiRenderer::CYAN,
                                    d.name,
                                    TuiRenderer::RESET,
                                    d.version,
                                    dev_str
                                );
                            }
                        }
                        println!("\nCommands: /deps add <pkg> | /deps rm <pkg> | /deps audit\n");
                    }
                    Err(e) => {
                        println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET)
                    }
                }
            } else {
                match parts[0].to_lowercase().as_str() {
                    "add" => {
                        if parts.len() < 2 {
                            println!(
                                "{}Usage: /deps add <package_name> [--dev]{}\n",
                                TuiRenderer::RED,
                                TuiRenderer::RESET
                            );
                        } else {
                            let pkg = parts[1];
                            let is_dev = parts.contains(&"--dev") || parts.contains(&"-D");
                            println!(
                                "{}[ACODE] Installing dependency '{}'...{}",
                                TuiRenderer::DIM,
                                pkg,
                                TuiRenderer::RESET
                            );
                            match orchestrator.tools.deps.add_dependency(
                                &orchestrator.workspace_root,
                                pkg,
                                None,
                                is_dev,
                            ) {
                                Ok(out) => println!(
                                    "{}[OK] Installed '{}':\n{}{}\n",
                                    TuiRenderer::GREEN,
                                    pkg,
                                    out,
                                    TuiRenderer::RESET
                                ),
                                Err(e) => println!(
                                    "{}[FAIL] Installation failed: {}{}\n",
                                    TuiRenderer::RED,
                                    e,
                                    TuiRenderer::RESET
                                ),
                            }
                        }
                    }
                    "remove" | "rm" => {
                        if parts.len() < 2 {
                            println!(
                                "{}Usage: /deps rm <package_name>{}\n",
                                TuiRenderer::RED,
                                TuiRenderer::RESET
                            );
                        } else {
                            let pkg = parts[1];
                            println!(
                                "{}[ACODE] Removing dependency '{}'...{}",
                                TuiRenderer::DIM,
                                pkg,
                                TuiRenderer::RESET
                            );
                            match orchestrator
                                .tools
                                .deps
                                .remove_dependency(&orchestrator.workspace_root, pkg)
                            {
                                Ok(out) => println!(
                                    "{}[OK] Removed '{}':\n{}{}\n",
                                    TuiRenderer::GREEN,
                                    pkg,
                                    out,
                                    TuiRenderer::RESET
                                ),
                                Err(e) => println!(
                                    "{}[FAIL] Removal failed: {}{}\n",
                                    TuiRenderer::RED,
                                    e,
                                    TuiRenderer::RESET
                                ),
                            }
                        }
                    }
                    "audit" => {
                        println!(
                            "{}[ACODE] Running ecosystem security audit...{}",
                            TuiRenderer::DIM,
                            TuiRenderer::RESET
                        );
                        match orchestrator
                            .tools
                            .deps
                            .audit_dependencies(&orchestrator.workspace_root)
                        {
                            Ok(out) => println!(
                                "\n{}[AUDIT RESULTS]{}\n{}\n",
                                TuiRenderer::BOLD,
                                TuiRenderer::RESET,
                                out
                            ),
                            Err(e) => println!(
                                "{}[FAIL] Audit error: {}{}\n",
                                TuiRenderer::RED,
                                e,
                                TuiRenderer::RESET
                            ),
                        }
                    }
                    _ => {
                        println!(
                            "{}Usage: /deps [add <pkg> | rm <pkg> | audit]{}\n",
                            TuiRenderer::CYAN,
                            TuiRenderer::RESET
                        );
                    }
                }
            }
            true
        }
        "/symbols" => {
            let parts: Vec<&str> = arg.split_whitespace().collect();
            if parts.is_empty() {
                println!(
                    "{}Usage: /symbols <file_path> | /symbols find <name> | /symbols map [path_filter]{}\n",
                    TuiRenderer::CYAN,
                    TuiRenderer::RESET
                );
            } else if parts[0].eq_ignore_ascii_case("find") {
                if parts.len() < 2 {
                    println!(
                        "{}Usage: /symbols find <identifier_name>{}\n",
                        TuiRenderer::RED,
                        TuiRenderer::RESET
                    );
                } else {
                    let name = parts[1];
                    let matches = orchestrator.tools.symbols.find_symbol_definitions(name);
                    println!(
                        "\n{}[SYMBOL DEFINITIONS: '{}']{} (Matches: {})",
                        TuiRenderer::BOLD,
                        name,
                        TuiRenderer::RESET,
                        matches.len()
                    );
                    if matches.is_empty() {
                        println!("  No definitions found.");
                    } else {
                        for m in matches {
                            let pub_badge = if m.is_public {
                                format!("{}[pub]{}", TuiRenderer::GREEN, TuiRenderer::RESET)
                            } else {
                                String::new()
                            };
                            println!(
                                "  - {}:{}: {}{} {}",
                                m.file_path,
                                m.line_number,
                                TuiRenderer::CYAN,
                                m.signature,
                                pub_badge
                            );
                        }
                    }
                    println!();
                }
            } else if parts[0].eq_ignore_ascii_case("map") {
                let filter = if parts.len() > 1 {
                    Some(parts[1])
                } else {
                    None
                };
                match orchestrator.tools.symbols.generate_repo_map(filter) {
                    Ok(map) => println!("\n{}\n", map),
                    Err(e) => println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET),
                }
            } else {
                let file_path = orchestrator.workspace_root.join(parts[0]);
                match orchestrator
                    .tools
                    .symbols
                    .extract_symbols_from_file(&file_path)
                {
                    Ok(syms) => {
                        println!(
                            "\n{}[SYMBOLS: {}]{} (Total: {})",
                            TuiRenderer::BOLD,
                            parts[0],
                            TuiRenderer::RESET,
                            syms.len()
                        );
                        if syms.is_empty() {
                            println!("  No symbol declarations found.");
                        } else {
                            for s in syms {
                                let pub_badge = if s.is_public {
                                    format!("{}[pub]{}", TuiRenderer::GREEN, TuiRenderer::RESET)
                                } else {
                                    String::new()
                                };
                                println!(
                                    "  L{:4} | {:9} | {}{}{} {}",
                                    s.line_number,
                                    s.kind,
                                    TuiRenderer::CYAN,
                                    s.name,
                                    TuiRenderer::RESET,
                                    pub_badge
                                );
                            }
                        }
                        println!();
                    }
                    Err(e) => {
                        println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET)
                    }
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
            let filter = if parts.len() > 2 {
                Some(parts[2])
            } else {
                None
            };

            match orchestrator
                .tools
                .symbols
                .rename_symbol(old_name, new_name, filter)
            {
                Ok(modified) => {
                    if modified.is_empty() {
                        println!(
                            "{}[INFO] Identifier '{}' not found in workspace.{}\n",
                            TuiRenderer::YELLOW,
                            old_name,
                            TuiRenderer::RESET
                        );
                    } else {
                        let total_occ: usize = modified.iter().map(|(_, c)| *c).sum();
                        println!(
                            "\n{}[RENAME COMPLETE]{} '{}' -> '{}' ({} replacements in {} files)",
                            TuiRenderer::BOLD,
                            TuiRenderer::RESET,
                            old_name,
                            new_name,
                            total_occ,
                            modified.len()
                        );
                        for (file, count) in modified {
                            println!(
                                "  {}[MODIFIED]{} {} ({} tokens)",
                                TuiRenderer::GREEN,
                                TuiRenderer::RESET,
                                file,
                                count
                            );
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
                    println!(
                        "\n{}[ACTIVE GOAL DECOMPOSITION GRAPH]{}\n{}\n",
                        TuiRenderer::BOLD,
                        TuiRenderer::RESET,
                        graph.render_progress()
                    );
                }
                None => {
                    println!(
                        "{}No active task graph. Create one using: /task new <goal>{}\n",
                        TuiRenderer::DIM,
                        TuiRenderer::RESET
                    );
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
                        println!(
                            "{}Usage: /task new <goal summary>{}\n",
                            TuiRenderer::RED,
                            TuiRenderer::RESET
                        );
                    } else {
                        orchestrator.task_graph =
                            Some(crate::agent::planner::TaskGraph::new(&goal));
                        println!("{}[OK] Initialized task graph for goal: '{}'{}\nAdd steps via: /task add <step_title>\n", TuiRenderer::GREEN, goal, TuiRenderer::RESET);
                    }
                }
                "add" => {
                    let step = parts[1..].join(" ");
                    if step.is_empty() {
                        println!(
                            "{}Usage: /task add <step title>{}\n",
                            TuiRenderer::RED,
                            TuiRenderer::RESET
                        );
                    } else {
                        if orchestrator.task_graph.is_none() {
                            orchestrator.task_graph = Some(crate::agent::planner::TaskGraph::new(
                                "Active Task Sequence",
                            ));
                        }
                        if let Some(graph) = orchestrator.task_graph.as_mut() {
                            let id = graph.add_step(&step, "");
                            println!(
                                "{}[OK] Added step {}: '{}'{}\n",
                                TuiRenderer::GREEN,
                                id,
                                step,
                                TuiRenderer::RESET
                            );
                        }
                    }
                }
                "done" | "next" => {
                    if let Some(graph) = orchestrator.task_graph.as_mut() {
                        graph.complete_current();
                        println!(
                            "\n{}[STEP ADVANCED]{}\n{}\n",
                            TuiRenderer::BOLD,
                            TuiRenderer::RESET,
                            graph.render_progress()
                        );
                    } else {
                        println!(
                            "{}No active task graph. Create one via: /task new <goal>{}\n",
                            TuiRenderer::YELLOW,
                            TuiRenderer::RESET
                        );
                    }
                }
                "fail" => {
                    let reason = parts[1..].join(" ");
                    if let Some(graph) = orchestrator.task_graph.as_mut() {
                        graph.fail_current(if reason.is_empty() {
                            "Manual failure mark"
                        } else {
                            &reason
                        });
                        println!("{}[WARN] Current step marked failed. Re-planning or repair suggested.{}\n", TuiRenderer::YELLOW, TuiRenderer::RESET);
                    }
                }
                "clear" | "reset" => {
                    orchestrator.task_graph = None;
                    println!(
                        "{}[OK] Cleared active task graph.{}\n",
                        TuiRenderer::GREEN,
                        TuiRenderer::RESET
                    );
                }
                _ => {
                    println!("{}Usage: /task [new <goal> | add <step> | done | fail <reason> | clear]{}\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                }
            }
            true
        }
        "/sandbox" => {
            let parts: Vec<&str> = arg.split_whitespace().collect();
            if parts.is_empty()
                || parts[0].eq_ignore_ascii_case("status")
                || parts[0].eq_ignore_ascii_case("list")
            {
                println!(
                    "\n{}[SHADOW WORKSPACE SANDBOX]{}",
                    TuiRenderer::BOLD,
                    TuiRenderer::RESET
                );
                if let Some(session) = &orchestrator.active_sandbox {
                    println!(
                        "  Active Session:  {}{}{}",
                        TuiRenderer::GREEN,
                        session.session_id,
                        TuiRenderer::RESET
                    );
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
                            println!(
                                "  {}[ACTIVE SHADOW WORKTREES]{}",
                                TuiRenderer::BOLD,
                                TuiRenderer::RESET
                            );
                            for s in list {
                                let active_mark = if orchestrator
                                    .active_sandbox
                                    .as_ref()
                                    .map(|a| a.session_id == s.session_id)
                                    .unwrap_or(false)
                                {
                                    format!(
                                        " {}[ATTACHED]{}",
                                        TuiRenderer::GREEN,
                                        TuiRenderer::RESET
                                    )
                                } else {
                                    String::new()
                                };
                                println!(
                                    "  - {}{}{} ({}){}",
                                    TuiRenderer::CYAN,
                                    s.session_id,
                                    TuiRenderer::RESET,
                                    s.branch,
                                    active_mark
                                );
                                println!("    Path: {}", s.path);
                            }
                            println!();
                        }
                    }
                    Err(e) => println!(
                        "{}[FAIL] Error listing sandboxes: {}{}\n",
                        TuiRenderer::RED,
                        e,
                        TuiRenderer::RESET
                    ),
                }
                println!("Commands:");
                println!(
                    "  /sandbox start [name] - Create and attach isolated shadow git worktree"
                );
                println!("  /sandbox merge        - Apply shadow modifications to primary workspace & teardown");
                println!("  /sandbox abort        - Discard shadow worktree & branch without touching primary code\n");
                return true;
            }

            match parts[0].to_lowercase().as_str() {
                "start" | "create" | "new" => {
                    let custom_name = if parts.len() > 1 {
                        Some(parts[1])
                    } else {
                        None
                    };
                    println!(
                        "{}[ACODE] Spawning isolated git shadow worktree...{}",
                        TuiRenderer::DIM,
                        TuiRenderer::RESET
                    );
                    match orchestrator.tools.sandbox.create_sandbox(custom_name) {
                        Ok(session) => {
                            let wt_path = session.worktree_path.clone();
                            let session_id = session.session_id.clone();
                            let branch = session.branch_name.clone();
                            orchestrator.active_sandbox = Some(session);
                            orchestrator.set_workspace(wt_path.clone());
                            println!(
                                "\n{}[OK] Shadow Sandbox '{}' active!{}",
                                TuiRenderer::GREEN,
                                session_id,
                                TuiRenderer::RESET
                            );
                            println!(
                                "  Branch:        {}{}{}",
                                TuiRenderer::CYAN,
                                branch,
                                TuiRenderer::RESET
                            );
                            println!("  Worktree Path: {}", wt_path.display());
                            println!("  {}[INFO] Workspace root switched to shadow tree. All autonomous edits & builds are 100% isolated.{}\n", TuiRenderer::DIM, TuiRenderer::RESET);
                        }
                        Err(e) => println!(
                            "{}[FAIL] Failed to create sandbox: {}{}\n",
                            TuiRenderer::RED,
                            e,
                            TuiRenderer::RESET
                        ),
                    }
                }
                "merge" | "apply" => {
                    if let Some(session) = orchestrator.active_sandbox.take() {
                        println!("{}[ACODE] Computing diff and merging shadow sandbox '{}' back into primary workspace...{}", TuiRenderer::DIM, session.session_id, TuiRenderer::RESET);
                        let orig_root = session.original_root.clone();
                        match orchestrator.tools.sandbox.merge_sandbox(&session) {
                            Ok(msg) => {
                                orchestrator.set_workspace(orig_root.clone());
                                println!(
                                    "\n{}[OK] Sandbox merged successfully!{}",
                                    TuiRenderer::GREEN,
                                    TuiRenderer::RESET
                                );
                                println!(
                                    "  {}\n  Workspace restored to: {}\n",
                                    msg,
                                    orig_root.display()
                                );
                            }
                            Err(e) => {
                                orchestrator.active_sandbox = Some(session);
                                println!(
                                    "{}[FAIL] Merge failed: {}{}\n",
                                    TuiRenderer::RED,
                                    e,
                                    TuiRenderer::RESET
                                );
                            }
                        }
                    } else {
                        println!(
                            "{}No active shadow sandbox attached. Use /sandbox start [name]{}\n",
                            TuiRenderer::YELLOW,
                            TuiRenderer::RESET
                        );
                    }
                }
                "abort" | "discard" | "drop" => {
                    if let Some(session) = orchestrator.active_sandbox.take() {
                        println!(
                            "{}[ACODE] Discarding shadow sandbox '{}'...{}",
                            TuiRenderer::DIM,
                            session.session_id,
                            TuiRenderer::RESET
                        );
                        let orig_root = session.original_root.clone();
                        match orchestrator.tools.sandbox.discard_sandbox(&session) {
                            Ok(msg) => {
                                orchestrator.set_workspace(orig_root.clone());
                                println!("\n{}[OK] Sandbox discarded cleanly. Zero modifications applied to primary workspace.{}", TuiRenderer::GREEN, TuiRenderer::RESET);
                                println!(
                                    "  {}\n  Workspace restored to: {}\n",
                                    msg,
                                    orig_root.display()
                                );
                            }
                            Err(e) => {
                                orchestrator.set_workspace(orig_root);
                                println!(
                                    "{}[FAIL] Discard error: {}{}\n",
                                    TuiRenderer::RED,
                                    e,
                                    TuiRenderer::RESET
                                );
                            }
                        }
                    } else {
                        println!(
                            "{}No active shadow sandbox attached to abort.{}\n",
                            TuiRenderer::YELLOW,
                            TuiRenderer::RESET
                        );
                    }
                }
                _ => {
                    println!(
                        "{}Usage: /sandbox [start [name] | merge | abort | list | status]{}\n",
                        TuiRenderer::CYAN,
                        TuiRenderer::RESET
                    );
                }
            }
            true
        }
        "/exit" | "/quit" => {
            println!(
                "\n{}[ACODE] Exiting Atlas Daedalus. Sovereign Rust runtime stopped.{}\n",
                TuiRenderer::DIM,
                TuiRenderer::RESET
            );
            std::process::exit(0);
        }
        _ => false,
    }
}

pub fn load_bookmarks() -> std::collections::HashMap<String, String> {
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let p = std::path::PathBuf::from(home)
            .join(".daedalus")
            .join("bookmarks.json");
        if let Ok(content) = std::fs::read_to_string(p) {
            if let Ok(map) = serde_json::from_str(&content) {
                return map;
            }
        }
    }
    std::collections::HashMap::new()
}

pub fn save_bookmarks(bookmarks: &std::collections::HashMap<String, String>) {
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let dir = std::path::PathBuf::from(home).join(".daedalus");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("bookmarks.json");
        if let Ok(content) = serde_json::to_string_pretty(bookmarks) {
            let _ = std::fs::write(p, content);
        }
    }
}

pub fn print_help() {
    println!(
        "\n{}[ATLAS DAEDALUS COMMANDS]{}",
        TuiRenderer::BOLD,
        TuiRenderer::RESET
    );

    println!(
        "\n{}── Core Session ──────────────────────────{}",
        TuiRenderer::INDIGO,
        TuiRenderer::RESET
    );
    println!(
        "  {}/help{}             Show command reference",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );
    println!(
        "  {}/info{}             Show active workspace & context",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );
    println!(
        "  {}/clear{}            Clear console & reset context",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );
    println!(
        "  {}/cost{}             Token usage & USD cost telemetry",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );
    println!(
        "  {}/model <name>{}     Switch active model",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );
    println!(
        "  {}/security <mode>{}  Set mode (full | sandboxed | strict)",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );
    println!(
        "  {}/exit{}             Exit REPL session",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );

    println!(
        "\n{}── Coding, AST & Tasks ───────────────────{}",
        TuiRenderer::INDIGO,
        TuiRenderer::RESET
    );
    println!(
        "  {}/outline <path>{}   Tree-sitter AST structural outline",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );
    println!(
        "  {}/symbols <path>{}   Extract declarations or find callsite",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );
    println!(
        "  {}/rename <o> <n>{}   Semantic AST identifier renaming",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );
    println!(
        "  {}/plan <goal>{}      Architect Mode: generate plan",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );
    println!(
        "  {}/tasks{}            Active goal decomposition graph",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );
    println!(
        "  {}/timer <sec>{}      Countdown timer or stopwatch",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );

    println!(
        "\n{}── Git, Sandbox & Execution ──────────────{}",
        TuiRenderer::INDIGO,
        TuiRenderer::RESET
    );
    println!(
        "  {}/diff{}             Inspect uncommitted git diff",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );
    println!(
        "  {}/undo{}             Revert uncommitted & untracked files",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );
    println!(
        "  {}/test [cmd]{}       Execute verification suite",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );
    println!(
        "  {}/sandbox [act]{}    Shadow worktrees (start, merge, abort)",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );
    println!(
        "  {}/daemons{}          List background process daemons",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );
    println!(
        "  {}/mcp [act]{}        MCP server suite management",
        TuiRenderer::CYAN,
        TuiRenderer::RESET
    );

    println!(
        "\n{}── Shortcuts ─────────────────────────────{}",
        TuiRenderer::INDIGO,
        TuiRenderer::RESET
    );
    println!(
        "  {}!<cmd>{}              Execute direct host shell command",
        TuiRenderer::AMBER,
        TuiRenderer::RESET
    );
    println!(
        "  {}!&<cmd>{}             Spawn non-blocking background daemon",
        TuiRenderer::VIOLET,
        TuiRenderer::RESET
    );
    println!(
        "  {}\\ (at line end){}     Continue typing on next line\n",
        TuiRenderer::SLATE,
        TuiRenderer::RESET
    );
}
