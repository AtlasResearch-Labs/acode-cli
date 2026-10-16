//! Atlas Daedalus — Git Slash Commands (/diff, /status, /undo, /commit, /repos)

use crate::agent::orchestrator::DaedalusOrchestrator;
use crate::cli::tui::TuiRenderer;

pub async fn handle(cmd: &str, arg: &str, orchestrator: &mut DaedalusOrchestrator) -> bool {
    match cmd {
        "/diff" => {
            let git_diff = orchestrator.tools.git.get_diff(false).await;
            if !git_diff.is_empty() {
                TuiRenderer::print_diff(&git_diff);
            } else if !orchestrator.session_diffs.is_empty() {
                println!(
                    "\n{}[SESSION MODIFICATIONS]{}",
                    TuiRenderer::BOLD,
                    TuiRenderer::RESET
                );
                for (file, diff) in &orchestrator.session_diffs {
                    println!("{}File: {}{}", TuiRenderer::CYAN, file, TuiRenderer::RESET);
                    TuiRenderer::print_diff(diff);
                }
                println!();
            } else {
                println!(
                    "{}No working tree or session modifications recorded.{}\n",
                    TuiRenderer::DIM,
                    TuiRenderer::RESET
                );
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
                    TuiRenderer::BOLD,
                    TuiRenderer::RESET,
                    TuiRenderer::CYAN,
                    status.branch,
                    TuiRenderer::RESET
                );
                if status.clean {
                    println!(
                        "{}[OK] Working tree clean.{}\n",
                        TuiRenderer::GREEN,
                        TuiRenderer::RESET
                    );
                } else {
                    if !status.modified.is_empty() {
                        println!(
                            "{}Modified files:{}",
                            TuiRenderer::YELLOW,
                            TuiRenderer::RESET
                        );
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
        "/undo" => {
            let gate_args = serde_json::json!({ "command": "git checkout -- ." });
            if !orchestrator.check_gate("run_command", &gate_args) {
                return true;
            }
            println!(
                "{}[ACODE] Reverting uncommitted working tree changes...{}",
                TuiRenderer::YELLOW,
                TuiRenderer::RESET
            );
            let res = orchestrator
                .tools
                .shell
                .run_command("git checkout -- .", None)
                .await;
            if res.success {
                println!(
                    "{}[OK] Working tree restored to previous clean git commit.{}\n",
                    TuiRenderer::GREEN,
                    TuiRenderer::RESET
                );
            } else {
                println!(
                    "{}[FAIL] Revert failed: {}{}\n",
                    TuiRenderer::RED,
                    res.stderr,
                    TuiRenderer::RESET
                );
            }
            true
        }
        "/commit" => {
            if arg.is_empty() {
                println!(
                    "{}Usage: /commit <commit message>{}\n",
                    TuiRenderer::RED,
                    TuiRenderer::RESET
                );
                return true;
            }
            match orchestrator
                .tools
                .git
                .commit(arg, "Eren Yeager <victor@atlasresearchlabs.online>")
                .await
            {
                Ok(hash) => {
                    println!(
                        "{}[OK] Committed as [{}]: {}{}\n",
                        TuiRenderer::GREEN,
                        hash,
                        arg,
                        TuiRenderer::RESET
                    );
                }
                Err(e) => {
                    println!(
                        "{}[FAIL] Git commit failed: {}{}\n",
                        TuiRenderer::RED,
                        e,
                        TuiRenderer::RESET
                    );
                }
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
                    println!(
                        "\n{}[SIBLING WORKSPACES]{} in {}",
                        TuiRenderer::BOLD,
                        TuiRenderer::RESET,
                        parent.display()
                    );
                    for (r, is_git) in &repos {
                        let git_badge = if *is_git {
                            format!("{}[git]{}", TuiRenderer::GREEN, TuiRenderer::RESET)
                        } else {
                            String::new()
                        };
                        println!(
                            "  - {}{}{} {}",
                            TuiRenderer::CYAN,
                            r,
                            TuiRenderer::RESET,
                            git_badge
                        );
                    }
                    println!("\nSwitch active workspace using: /cd <folder_name>\n");
                } else {
                    println!(
                        "{}[FAIL] Cannot read parent folder: {}{}\n",
                        TuiRenderer::RED,
                        parent.display(),
                        TuiRenderer::RESET
                    );
                }
            } else {
                println!(
                    "{}Active workspace has no parent folder.{}\n",
                    TuiRenderer::DIM,
                    TuiRenderer::RESET
                );
            }
            true
        }
        _ => false,
    }
}
