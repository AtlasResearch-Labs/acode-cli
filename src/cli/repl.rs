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

    pub async fn start(&mut self) {
        let root = self.orchestrator.workspace_root.to_string_lossy().to_string();
        let model = self.orchestrator.provider.config.model.clone();
        let provider = self.orchestrator.provider.config.provider.clone();

        TuiRenderer::print_header(&root, &model, &provider);

        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();

        loop {
            print!("{}{}daedalus>{} ", TuiRenderer::BOLD, TuiRenderer::CYAN, TuiRenderer::RESET);
            let _ = io::stdout().flush();

            match reader.next_line().await {
                Ok(Some(line)) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    if trimmed.starts_with('/') {
                        if SlashCommandHandler::handle(trimmed, &mut self.orchestrator).await {
                            continue;
                        }
                    }

                    println!();
                    let res = self.orchestrator.run_task(trimmed, |event| match event {
                        AgentEvent::Thinking(s) => TuiRenderer::print_thinking(s),
                        AgentEvent::ToolStart { name, args } => TuiRenderer::print_tool_start(name, args),
                        AgentEvent::ToolEnd { name, success, output } => TuiRenderer::print_tool_end(name, success, output),
                        AgentEvent::TestResult { command, passed, output } => TuiRenderer::print_test_result(command, passed, output),
                        AgentEvent::Done { files_modified, .. } => {
                            if files_modified.is_empty() {
                                TuiRenderer::print_success("Task executed successfully.");
                            } else {
                                TuiRenderer::print_success(&format!("Modified: {}", files_modified.join(", ")));
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
