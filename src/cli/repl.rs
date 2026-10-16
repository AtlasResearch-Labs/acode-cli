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
        let folder_name = self.orchestrator.workspace_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_else(|| {
                self.orchestrator.workspace_root.to_str().unwrap_or("root")
            });

        let status = self.orchestrator.tools.git.get_status().await;
        let git_info = if !status.branch.is_empty() && status.branch != "detached" && status.branch != "unknown" {
            let dirty = if status.clean { "" } else { "*" };
            format!(" ({}{})", status.branch, dirty)
        } else {
            String::new()
        };

        format!(
            "{}{}daedalus {}{}[{}{}{}]{}>{} ",
            TuiRenderer::BOLD,
            TuiRenderer::CYAN,
            TuiRenderer::RESET,
            TuiRenderer::DIM,
            folder_name,
            TuiRenderer::GREEN,
            git_info,
            TuiRenderer::CYAN,
            TuiRenderer::RESET
        )
    }

    fn append_history(line: &str) {
        if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
            let daedalus_dir = std::path::PathBuf::from(home).join(".daedalus");
            let _ = std::fs::create_dir_all(&daedalus_dir);
            let history_path = daedalus_dir.join("history.txt");
            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&history_path) {
                let _ = writeln!(file, "{}", line);
            }
        }
    }

    pub async fn start(&mut self) {
        let root = self.orchestrator.workspace_root.to_string_lossy().to_string();
        let model = self.orchestrator.provider.config.model.clone();
        let provider = self.orchestrator.provider.config.provider.clone();

        let security = self.orchestrator.security.config.mode.to_string();
        TuiRenderer::print_header(&root, &model, &provider, &security);

        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();

        loop {
            let prompt_str = self.render_prompt().await;
            print!("{}", prompt_str);
            let _ = io::stdout().flush();

            match reader.next_line().await {
                Ok(Some(line)) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    Self::append_history(trimmed);

                    if trimmed.starts_with('/') {
                        if SlashCommandHandler::handle(trimmed, &mut self.orchestrator).await {
                            continue;
                        }
                    }

                    // Direct non-blocking background daemon spawn with '!&' prefix
                    if trimmed.starts_with("!&") {
                        let shell_cmd = trimmed[2..].trim();
                        if !shell_cmd.is_empty() {
                            println!();
                            match self.orchestrator.tools.daemons.start_daemon(shell_cmd, &self.orchestrator.workspace_root) {
                                Ok(job) => {
                                    println!("{}[OK] Started background daemon [{}]: {}{}\n", TuiRenderer::GREEN, job.id, shell_cmd, TuiRenderer::RESET);
                                }
                                Err(e) => {
                                    println!("{}[FAIL] Failed to spawn background daemon: {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET);
                                }
                            }
                            continue;
                        }
                    }

                    // Direct shell command bypass with '!' prefix
                    if trimmed.starts_with('!') {
                        let shell_cmd = trimmed[1..].trim();
                        if !shell_cmd.is_empty() {
                            println!();
                            let res = self.orchestrator.tools.shell.run_command(shell_cmd, None).await;
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

                    println!("{}[ACODE] Dispatching to {} ({})...{}\n", TuiRenderer::DIM, self.orchestrator.provider.config.model, self.orchestrator.provider.config.provider, TuiRenderer::RESET);
                    let res = self.orchestrator.run_task(effective_prompt, |event| match event {
                        AgentEvent::Thinking(s) => TuiRenderer::print_thinking(s),
                        AgentEvent::ToolStart { name, args } => TuiRenderer::print_tool_start(name, args),
                        AgentEvent::ToolEnd { name, success, output } => TuiRenderer::print_tool_end(name, success, output),
                        AgentEvent::TestResult { command, passed, output } => TuiRenderer::print_test_result(command, passed, output),
                        AgentEvent::Done { files_modified, tools_called, .. } => {
                            if !files_modified.is_empty() {
                                TuiRenderer::print_success(&format!("Modified: {}", files_modified.join(", ")));
                            } else if tools_called > 0 {
                                TuiRenderer::print_success("Task executed successfully.");
                            }
                        }
                        AgentEvent::Error(err) => TuiRenderer::print_error(err),
                    }).await;

                    match res {
                        Ok(summary) => {
                            if !summary.is_empty() {
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
