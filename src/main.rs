//! Atlas Daedalus (`acode`)
//! Industrial Autonomous Terminal AI Coder
//! (C) 2026 Atlas Research Labs — MIT License

#![allow(dead_code)]

mod agent;
mod cli;
mod context;
mod providers;
mod security;
mod telemetry;
mod tools;

use clap::Parser;
use std::env;

use crate::agent::orchestrator::{AgentEvent, DaedalusOrchestrator};
use crate::cli::commands::SlashCommandHandler;
use crate::cli::repl::DaedalusREPL;
use crate::cli::tui::TuiRenderer;

#[derive(Parser, Debug)]
#[command(
    name = "acode",
    author = "Eren Yeager <victor@atlasresearchlabs.online>",
    version = env!("CARGO_PKG_VERSION"),
    about = "Atlas Daedalus — Industrial Autonomous Terminal AI Coder (Atlas Research Labs)"
)]
struct CliArgs {
    /// Autonomous task prompt to run non-interactively
    #[arg(trailing_var_arg = true)]
    prompt: Vec<String>,

    /// Override active LLM model (e.g. gemini-2.0-flash, claude-3-5-sonnet, gpt-4o)
    #[arg(short, long)]
    model: Option<String>,

    /// Override active AI provider (gemini, anthropic, openai, powerbox, ollama)
    #[arg(short, long)]
    provider: Option<String>,

    /// Override agent security mode (full, sandboxed, strict)
    #[arg(short = 's', long)]
    security: Option<String>,
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    // Global fallback from ~/.daedalus/.env
    if let Ok(home) = env::var("USERPROFILE").or_else(|_| env::var("HOME")) {
        let global_env = std::path::PathBuf::from(home)
            .join(".daedalus")
            .join(".env");
        if global_env.exists() {
            let _ = dotenvy::from_path(&global_env);
        }
    }

    // Support standard -v lowercase alias for --version
    if env::args().any(|a| a == "-v") {
        println!("acode {}-industrial", env!("CARGO_PKG_VERSION"));
        return;
    }

    let args = CliArgs::parse();
    let workspace_root = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let security_mode = args.security.as_deref().and_then(|s| s.parse().ok());

    let mut orchestrator = DaedalusOrchestrator::new(
        &workspace_root,
        args.model.clone(),
        args.provider.clone(),
        security_mode,
    );

    let full_prompt = args.prompt.join(" ").trim().to_string();

    if !full_prompt.is_empty() {
        if full_prompt.starts_with('/') {
            let _ = SlashCommandHandler::handle(&full_prompt, &mut orchestrator).await;
            return;
        }

        // Non-interactive task execution
        let root_str = workspace_root.to_string_lossy().to_string();
        let sec_str = orchestrator.security.config.mode.to_string();
        TuiRenderer::print_header(
            &root_str,
            &orchestrator.provider.config.model,
            &orchestrator.provider.config.provider,
            &sec_str,
        );

        println!(
            "{}[TASK]{} {}\n",
            TuiRenderer::BOLD,
            TuiRenderer::RESET,
            full_prompt
        );

        let task_start = std::time::Instant::now();
        let mut streamed_any = false;
        let mut has_streamed_tokens = false;
        let mut token_buffer = String::new();
        let mut tick_counter = 0usize;
        let mut last_tool_start = std::time::Instant::now();
        let cost_tracker = orchestrator.cost_tracker.clone();

        let res = orchestrator
            .run_task(&full_prompt, |event| match event {
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
                    has_streamed_tokens = true;
                }
                AgentEvent::ToolStart { name, args } => {
                    if streamed_any {
                        if !token_buffer.is_empty() {
                            println!("{}", TuiRenderer::render_markdown_line(&token_buffer));
                            token_buffer.clear();
                        }
                        println!();
                        streamed_any = false;
                    }
                    TuiRenderer::clear_line();
                    last_tool_start = std::time::Instant::now();
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
                            println!("{}", TuiRenderer::render_markdown_line(&token_buffer));
                            token_buffer.clear();
                        }
                        println!();
                        streamed_any = false;
                    }
                    TuiRenderer::clear_line();
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
                            println!("{}", TuiRenderer::render_markdown_line(&token_buffer));
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

        match res {
            Ok(summary) => {
                if !summary.is_empty() && !has_streamed_tokens {
                    println!("{}\n", summary);
                }
            }
            Err(e) => {
                TuiRenderer::print_error(&e);
                std::process::exit(1);
            }
        }
    } else {
        // Interactive REPL loop
        let mut repl = DaedalusREPL::new(orchestrator);
        repl.start().await;
    }
}
