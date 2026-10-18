//! Atlas Daedalus — Terminal Interactive REPL

use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::agent::orchestrator::{AgentEvent, DaedalusOrchestrator};
use crate::cli::commands::SlashCommandHandler;
use crate::cli::tui::TuiRenderer;

pub struct DaedalusREPL {
    orchestrator: DaedalusOrchestrator,
}

impl DaedalusREPL {
    pub fn new(orchestrator: DaedalusOrchestrator) -> Self {
        Self { orchestrator }
    }

    async fn render_prompt(&self) -> String {
        let metrics = crate::telemetry::host::HostInspector::collect();
        let folder_name = self
            .orchestrator
            .workspace_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_else(|| self.orchestrator.workspace_root.to_str().unwrap_or("root"));

        let status = self.orchestrator.tools.git.get_status().await;
        let git_info = if !status.branch.is_empty()
            && status.branch != "detached"
            && status.branch != "unknown"
        {
            let dirty = if status.clean { "" } else { "*" };
            format!(" ({}{})", status.branch, dirty)
        } else {
            String::new()
        };

        let model = &self.orchestrator.provider.config.model;
        let sec_mode = self.orchestrator.security.config.mode.to_string();

        let sec_color = match sec_mode.as_str() {
            "full" => TuiRenderer::EMERALD,
            "sandboxed" => TuiRenderer::AMBER,
            "strict" => TuiRenderer::ROSE,
            _ => TuiRenderer::CYAN_ACCENT,
        };

        let vp = crate::cli::viewport::Viewport::current();

        if vp.is_compact() {
            let indigo = TuiRenderer::INDIGO;
            let box_tl = TuiRenderer::BOX_TL;
            let box_bl = TuiRenderer::BOX_BL;
            let bold = TuiRenderer::BOLD;
            let white = TuiRenderer::WHITE;
            let slate = TuiRenderer::SLATE;
            let emerald = TuiRenderer::EMERALD;
            let reset = TuiRenderer::RESET;

            format!(
                "\n{indigo}{box_tl}─ {bold}{white}[acode]{reset} {slate}{folder_name}{reset}{emerald}{git_info}{reset}\n{indigo}{box_bl}❯{reset} "
            )
        } else {
            let indigo = TuiRenderer::INDIGO;
            let box_tl = TuiRenderer::BOX_TL;
            let box_bl = TuiRenderer::BOX_BL;
            let bold = TuiRenderer::BOLD;
            let white = TuiRenderer::WHITE;
            let slate = TuiRenderer::SLATE;
            let emerald = TuiRenderer::EMERALD;
            let cyan = TuiRenderer::CYAN_ACCENT;
            let charcoal = TuiRenderer::CHARCOAL;
            let reset = TuiRenderer::RESET;
            let rss = metrics.process_rss_mb;

            format!(
                "\n{indigo}{box_tl}─ {bold}{white}[acode]{reset} {slate}{folder_name}{reset}{emerald}{git_info}{reset} {cyan}{model}{reset} {sec_color}{sec_mode}{reset} {charcoal}{rss}MB{reset}\n{indigo}{box_bl}❯{reset} "
            )
        }
    }

    fn append_history(line: &str) {
        if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
            let daedalus_dir = std::path::PathBuf::from(home).join(".daedalus");
            let _ = std::fs::create_dir_all(&daedalus_dir);
            let history_path = daedalus_dir.join("history.txt");
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&history_path)
            {
                let _ = writeln!(file, "{}", line);
            }
        }
    }

    pub async fn start(&mut self) {
        let root = self
            .orchestrator
            .workspace_root
            .to_string_lossy()
            .to_string();
        let model = self.orchestrator.provider.config.model.clone();
        let provider = self.orchestrator.provider.config.provider.clone();

        let security = self.orchestrator.security.config.mode.to_string();
        TuiRenderer::print_header(&root, &model, &provider, &security);
        TuiRenderer::print_status_bar(&model, &provider, &security);

        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();
        let mut multi_line_buffer = String::new();

        loop {
            if multi_line_buffer.is_empty() {
                let prompt_str = self.render_prompt().await;
                print!("{}", prompt_str);
            } else {
                print!("{}... ❯{} ", TuiRenderer::SLATE, TuiRenderer::RESET);
            }
            let _ = io::stdout().flush();

            match reader.next_line().await {
                Ok(Some(line)) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() && multi_line_buffer.is_empty() {
                        continue;
                    }

                    if let Some(continuation) = trimmed.strip_suffix('\\') {
                        if !multi_line_buffer.is_empty() {
                            multi_line_buffer.push('\n');
                        }
                        multi_line_buffer.push_str(continuation.trim());
                        continue;
                    }

                    let full_line = if !multi_line_buffer.is_empty() {
                        multi_line_buffer.push('\n');
                        multi_line_buffer.push_str(trimmed);
                        let res = multi_line_buffer.clone();
                        multi_line_buffer.clear();
                        res
                    } else {
                        trimmed.to_string()
                    };

                    let trimmed = full_line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    Self::append_history(trimmed);

                    if trimmed.starts_with('/')
                        && SlashCommandHandler::handle(trimmed, &mut self.orchestrator).await
                    {
                        continue;
                    }

                    // Direct non-blocking background daemon spawn with '!&' prefix
                    if let Some(stripped) = trimmed.strip_prefix("!&") {
                        let shell_cmd = stripped.trim();
                        if !shell_cmd.is_empty() {
                            let gate_args =
                                serde_json::json!({ "command": shell_cmd, "cmd": shell_cmd });
                            if !self.orchestrator.check_gate("start_daemon", &gate_args) {
                                continue;
                            }
                            println!();
                            match self
                                .orchestrator
                                .tools
                                .daemons
                                .start_daemon(shell_cmd, &self.orchestrator.workspace_root)
                            {
                                Ok(job) => {
                                    println!(
                                        "{}[OK] Started background daemon [{}]: {}{}\n",
                                        TuiRenderer::GREEN,
                                        job.id,
                                        shell_cmd,
                                        TuiRenderer::RESET
                                    );
                                }
                                Err(e) => {
                                    println!(
                                        "{}[FAIL] Failed to spawn background daemon: {}{}\n",
                                        TuiRenderer::RED,
                                        e,
                                        TuiRenderer::RESET
                                    );
                                }
                            }
                            continue;
                        }
                    }

                    // Direct shell command with '!' prefix
                    if let Some(stripped) = trimmed.strip_prefix('!') {
                        let shell_cmd = stripped.trim();
                        if !shell_cmd.is_empty() {
                            let gate_args =
                                serde_json::json!({ "command": shell_cmd, "cmd": shell_cmd });
                            if !self.orchestrator.check_gate("run_command", &gate_args) {
                                continue;
                            }
                            println!();
                            let res = self
                                .orchestrator
                                .tools
                                .shell
                                .run_command(shell_cmd, None)
                                .await;
                            if !res.stdout.is_empty() {
                                print!("{}", res.stdout);
                            }
                            if !res.stderr.is_empty() {
                                eprint!("{}", res.stderr);
                            }
                            println!();
                            continue;
                        }
                    }

                    // Auto-unwrap accidental CLI invocation syntax inside interactive REPL
                    let effective_prompt = if let Some(stripped) = trimmed.strip_prefix("acode ") {
                        stripped.trim_matches('"').trim_matches('\'').trim()
                    } else if let Some(stripped) = trimmed.strip_prefix("acode.exe ") {
                        stripped.trim_matches('"').trim_matches('\'').trim()
                    } else {
                        trimmed
                    };

                    let folder_name = self
                        .orchestrator
                        .workspace_root
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("project")
                        .to_string();

                    TuiRenderer::set_terminal_title(&format!(
                        "acode: [Working] {} - {}",
                        self.orchestrator.provider.config.model, folder_name
                    ));

                    println!(
                        "\n{}╭─ [ACODE DISPATCH] ─── {}{} ({}){}",
                        TuiRenderer::INDIGO,
                        TuiRenderer::CYAN_ACCENT,
                        self.orchestrator.provider.config.model,
                        self.orchestrator.provider.config.provider,
                        TuiRenderer::RESET
                    );

                    let task_start = std::time::Instant::now();
                    let mut streamed_any = false;
                    let mut token_buffer = String::new();
                    let mut tick_counter = 0usize;
                    let mut last_tool_start = std::time::Instant::now();
                    let mut files_modified_tracker = Vec::new();
                    let mut tools_called_count = 0usize;
                    let cost_tracker = self.orchestrator.cost_tracker.clone();

                    let res = self
                        .orchestrator
                        .run_task(effective_prompt, |event| match event {
                            AgentEvent::Thinking(s) => {
                                if streamed_any {
                                    println!();
                                    streamed_any = false;
                                }
                                TuiRenderer::print_thinking_animated(s, tick_counter, task_start);
                                tick_counter += 1;
                            }
                            AgentEvent::Token(s) => {
                                if !streamed_any {
                                    TuiRenderer::clear_line();
                                }
                                token_buffer.push_str(s);
                                while let Some(pos) = token_buffer.find('\n') {
                                    let line: String = token_buffer.drain(..=pos).collect();
                                    let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
                                    println!("{}", TuiRenderer::render_markdown_line(trimmed));
                                }
                                streamed_any = true;
                            }
                            AgentEvent::ToolStart { name, args } => {
                                if streamed_any {
                                    if !token_buffer.is_empty() {
                                        println!(
                                            "{}",
                                            TuiRenderer::render_markdown_line(&token_buffer)
                                        );
                                        token_buffer.clear();
                                    }
                                    println!();
                                    streamed_any = false;
                                }
                                TuiRenderer::clear_line();
                                last_tool_start = std::time::Instant::now();
                                tools_called_count += 1;
                                TuiRenderer::print_tool_start(name, args);
                            }
                            AgentEvent::ToolEnd {
                                name,
                                success,
                                output,
                            } => {
                                let dur = last_tool_start.elapsed();
                                TuiRenderer::print_tool_end_timed(name, success, output, Some(dur));
                            }
                            AgentEvent::TestResult {
                                command,
                                passed,
                                output,
                            } => TuiRenderer::print_test_result(command, passed, output),
                            AgentEvent::Done {
                                files_modified,
                                tools_called,
                                ..
                            } => {
                                if streamed_any {
                                    if !token_buffer.is_empty() {
                                        println!(
                                            "{}",
                                            TuiRenderer::render_markdown_line(&token_buffer)
                                        );
                                        token_buffer.clear();
                                    }
                                    println!();
                                    streamed_any = false;
                                }
                                TuiRenderer::clear_line();
                                files_modified_tracker = files_modified.to_vec();
                                if !files_modified.is_empty() {
                                    TuiRenderer::print_success(&format!(
                                        "Modified: {}",
                                        files_modified.join(", ")
                                    ));
                                } else if tools_called > 0 {
                                    TuiRenderer::print_success("Task executed successfully.");
                                }

                                let m = cost_tracker.get_metrics();
                                TuiRenderer::print_turn_summary(
                                    task_start.elapsed(),
                                    tools_called,
                                    files_modified,
                                    Some(m.prompt_tokens + m.completion_tokens),
                                    Some(m.total_cost_usd),
                                );
                            }
                            AgentEvent::Error(err) => {
                                if streamed_any {
                                    if !token_buffer.is_empty() {
                                        println!(
                                            "{}",
                                            TuiRenderer::render_markdown_line(&token_buffer)
                                        );
                                        token_buffer.clear();
                                    }
                                    println!();
                                    streamed_any = false;
                                }
                                TuiRenderer::clear_line();
                                TuiRenderer::print_error(err);
                            }
                        })
                        .await;

                    TuiRenderer::set_terminal_title(&format!("acode: [Ready] - {}", folder_name));

                    if task_start.elapsed() >= std::time::Duration::from_secs(5) {
                        TuiRenderer::bell();
                    }

                    match res {
                        Ok(summary) => {
                            if !summary.is_empty() && !streamed_any {
                                println!("{}\n", summary);
                            }
                        }
                        Err(e) => {
                            TuiRenderer::print_error(&e);
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    TuiRenderer::print_error(&format!("IO error: {}", e));
                    break;
                }
            }
        }
    }
}
