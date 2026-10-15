//! Atlas Daedalus (`acode`)
//! Industrial Autonomous Terminal AI Coder
//! (C) 2026 Atlas Research Labs — MIT License

#![allow(dead_code)]

mod telemetry;
mod context;
mod tools;
mod providers;
mod agent;
mod cli;

use clap::Parser;
use std::env;

use crate::agent::orchestrator::{AgentEvent, DaedalusOrchestrator};
use crate::cli::repl::DaedalusREPL;
use crate::cli::tui::TuiRenderer;

#[derive(Parser, Debug)]
#[command(
    name = "acode",
    author = "Eren Yeager <victor@atlasresearchlabs.online>",
    version = "1.0.0-industrial",
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
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    // Global fallback from ~/.daedalus/.env
    if let Ok(home) = env::var("USERPROFILE").or_else(|_| env::var("HOME")) {
        let global_env = std::path::PathBuf::from(home).join(".daedalus").join(".env");
        if global_env.exists() {
            let _ = dotenvy::from_path(&global_env);
        }
    }

    let args = CliArgs::parse();
    let workspace_root = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    let mut orchestrator = DaedalusOrchestrator::new(
        &workspace_root,
        args.model.clone(),
        args.provider.clone(),
    );

    let full_prompt = args.prompt.join(" ").trim().to_string();

    if !full_prompt.is_empty() {
        // Non-interactive task execution
        let root_str = workspace_root.to_string_lossy().to_string();
        TuiRenderer::print_header(
            &root_str,
            &orchestrator.provider.config.model,
            &orchestrator.provider.config.provider,
        );

        println!("{}[TASK]{} {}\n", TuiRenderer::BOLD, TuiRenderer::RESET, full_prompt);

        let res = orchestrator.run_task(&full_prompt, |event| match event {
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
                std::process::exit(1);
            }
        }
    } else {
        // Interactive REPL loop
        let mut repl = DaedalusREPL::new(orchestrator);
        repl.start().await;
    }
}
