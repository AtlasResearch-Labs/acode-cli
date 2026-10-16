//! Atlas Daedalus — Daemon & Container Infrastructure Commands (/daemons, /daemon, /tunnel, /docker, /container)

use crate::agent::orchestrator::DaedalusOrchestrator;
use crate::cli::tui::TuiRenderer;

pub async fn handle(cmd: &str, arg: &str, orchestrator: &mut DaedalusOrchestrator) -> bool {
    match cmd {
        "/tunnel" => {
            let port: u16 = match arg.parse() {
                Ok(p) => p,
                Err(_) => {
                    println!(
                        "{}Usage: /tunnel <port>{}\n",
                        TuiRenderer::RED,
                        TuiRenderer::RESET
                    );
                    return true;
                }
            };
            println!(
                "{}[ACODE] Starting Beacon HTTPS tunnel on port {}...{}",
                TuiRenderer::DIM,
                port,
                TuiRenderer::RESET
            );
            let (success, url, inspector) = orchestrator
                .tools
                .pillars
                .start_beacon_tunnel(port, None)
                .await;
            if success {
                println!(
                    "\n{}[BEACON TUNNEL]{} Public URL: {}{}{}",
                    TuiRenderer::BOLD,
                    TuiRenderer::RESET,
                    TuiRenderer::GREEN,
                    url,
                    TuiRenderer::RESET
                );
                println!(
                    "Traffic Inspector: {}{}{}\n",
                    TuiRenderer::CYAN,
                    inspector,
                    TuiRenderer::RESET
                );
            }
            true
        }
        "/daemons" => {
            let list = orchestrator.tools.daemons.list_daemons();
            if list.is_empty() {
                println!("{}No background daemons currently managed.{}\nSpawn one via: !& <cmd> or start_daemon tool.\n", TuiRenderer::DIM, TuiRenderer::RESET);
            } else {
                println!(
                    "\n{}[BACKGROUND DAEMON REGISTRY]{}",
                    TuiRenderer::BOLD,
                    TuiRenderer::RESET
                );
                for d in list {
                    let status_badge = if d.is_alive {
                        format!("{}[RUNNING]{}", TuiRenderer::GREEN, TuiRenderer::RESET)
                    } else {
                        format!(
                            "{}[EXIT: {}]{}",
                            TuiRenderer::DIM,
                            d.exit_code.unwrap_or(0),
                            TuiRenderer::RESET
                        )
                    };
                    println!(
                        "  - ID: {}{}{} | PID: {} | Uptime: {}s | Status: {}",
                        TuiRenderer::CYAN,
                        d.id,
                        TuiRenderer::RESET,
                        d.pid,
                        d.uptime_secs,
                        status_badge
                    );
                    println!("    Command: {}", d.command);
                    if !d.recent_logs.is_empty() {
                        let last = d.recent_logs.last().unwrap();
                        println!(
                            "    Last output: {}{}{}",
                            TuiRenderer::DIM,
                            last,
                            TuiRenderer::RESET
                        );
                    }
                }
                println!(
                    "\nCommands: /daemon logs <id> | /daemon kill <id> | /daemon start <cmd>\n"
                );
            }
            true
        }
        "/daemon" => {
            let parts: Vec<&str> = arg.split_whitespace().collect();
            if parts.len() < 2 {
                println!(
                    "{}Usage: /daemon [logs <id> | kill <id> | start <command>]{}\n",
                    TuiRenderer::CYAN,
                    TuiRenderer::RESET
                );
                return true;
            }
            let sub = parts[0].to_lowercase();
            let target = parts[1];

            match sub.as_str() {
                "logs" => match orchestrator.tools.daemons.get_daemon_logs(target, 30) {
                    Some(logs) => {
                        println!(
                            "\n{}[DAEMON LOGS: {}]{}",
                            TuiRenderer::BOLD,
                            target,
                            TuiRenderer::RESET
                        );
                        if logs.is_empty() {
                            println!("  (no output captured yet)");
                        } else {
                            for line in logs {
                                println!("  {}", line);
                            }
                        }
                        println!();
                    }
                    None => println!(
                        "{}[FAIL] Daemon '{}' not found.{}\n",
                        TuiRenderer::RED,
                        target,
                        TuiRenderer::RESET
                    ),
                },
                "kill" => match orchestrator.tools.daemons.kill_daemon(target) {
                    Ok(_) => println!(
                        "{}[OK] Daemon '{}' terminated.{}\n",
                        TuiRenderer::GREEN,
                        target,
                        TuiRenderer::RESET
                    ),
                    Err(e) => {
                        println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET)
                    }
                },
                "start" => {
                    let cmd = parts[1..].join(" ");
                    match orchestrator
                        .tools
                        .daemons
                        .start_daemon(&cmd, &orchestrator.workspace_root)
                    {
                        Ok(info) => println!(
                            "{}[OK] Daemon '{}' spawned with PID {}: {}{}\n",
                            TuiRenderer::GREEN,
                            info.id,
                            info.pid,
                            cmd,
                            TuiRenderer::RESET
                        ),
                        Err(e) => println!(
                            "{}[FAIL] Failed to start daemon: {}{}\n",
                            TuiRenderer::RED,
                            e,
                            TuiRenderer::RESET
                        ),
                    }
                }
                _ => {
                    println!(
                        "{}Usage: /daemon [logs <id> | kill <id> | start <command>]{}\n",
                        TuiRenderer::CYAN,
                        TuiRenderer::RESET
                    );
                }
            }
            true
        }
        "/docker" | "/container" => {
            let parts: Vec<&str> = arg.split_whitespace().collect();
            if parts.is_empty()
                || parts[0].eq_ignore_ascii_case("ps")
                || parts[0].eq_ignore_ascii_case("list")
            {
                match orchestrator.tools.containers.list_containers(true) {
                    Ok(list) => {
                        println!(
                            "\n{}[CONTAINER FLEET STATUS]{} (Total: {})",
                            TuiRenderer::BOLD,
                            TuiRenderer::RESET,
                            list.len()
                        );
                        if list.is_empty() {
                            println!("  No containers found.");
                        } else {
                            for c in list {
                                let state_badge = if c.state == "running" {
                                    format!("{}[RUNNING]{}", TuiRenderer::GREEN, TuiRenderer::RESET)
                                } else {
                                    format!(
                                        "{}[{}]{}",
                                        TuiRenderer::DIM,
                                        c.state,
                                        TuiRenderer::RESET
                                    )
                                };
                                println!(
                                    "  - {}{}{} ({}) | Image: {} | {}",
                                    TuiRenderer::CYAN,
                                    c.id,
                                    TuiRenderer::RESET,
                                    c.name,
                                    c.image,
                                    state_badge
                                );
                                if !c.ports.is_empty() {
                                    println!("    Ports: {}", c.ports);
                                }
                            }
                        }
                        println!("\nCommands: /docker logs <id> | /docker up | /docker down | /docker inspect <id>\n");
                    }
                    Err(e) => {
                        println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET)
                    }
                }
            } else {
                match parts[0].to_lowercase().as_str() {
                    "status" => match orchestrator.tools.containers.check_status() {
                        Ok(s) => {
                            println!("\n{}[OK]{} {}\n", TuiRenderer::GREEN, TuiRenderer::RESET, s)
                        }
                        Err(e) => {
                            println!("{}[FAIL] {}{}\n", TuiRenderer::RED, e, TuiRenderer::RESET)
                        }
                    },
                    "logs" => {
                        if parts.len() < 2 {
                            println!(
                                "{}Usage: /docker logs <id_or_name>{}\n",
                                TuiRenderer::RED,
                                TuiRenderer::RESET
                            );
                        } else {
                            let target = parts[1];
                            match orchestrator.tools.containers.get_logs(target, 40) {
                                Ok(logs) => println!(
                                    "\n{}[LOGS: {}]{}\n{}\n",
                                    TuiRenderer::BOLD,
                                    target,
                                    TuiRenderer::RESET,
                                    logs
                                ),
                                Err(e) => println!(
                                    "{}[FAIL] {}{}\n",
                                    TuiRenderer::RED,
                                    e,
                                    TuiRenderer::RESET
                                ),
                            }
                        }
                    }
                    "inspect" => {
                        if parts.len() < 2 {
                            println!(
                                "{}Usage: /docker inspect <id_or_name>{}\n",
                                TuiRenderer::RED,
                                TuiRenderer::RESET
                            );
                        } else {
                            let target = parts[1];
                            match orchestrator.tools.containers.inspect_container(target) {
                                Ok(info) => println!(
                                    "\n{}[INSPECT: {}]{}\n{}\n",
                                    TuiRenderer::BOLD,
                                    target,
                                    TuiRenderer::RESET,
                                    info
                                ),
                                Err(e) => println!(
                                    "{}[FAIL] {}{}\n",
                                    TuiRenderer::RED,
                                    e,
                                    TuiRenderer::RESET
                                ),
                            }
                        }
                    }
                    "up" => {
                        println!(
                            "{}[ACODE] Launching compose cluster in background...{}",
                            TuiRenderer::DIM,
                            TuiRenderer::RESET
                        );
                        match orchestrator
                            .tools
                            .containers
                            .compose_up(&orchestrator.workspace_root)
                        {
                            Ok(out) => println!(
                                "{}[OK] Compose up:\n{}{}\n",
                                TuiRenderer::GREEN,
                                out,
                                TuiRenderer::RESET
                            ),
                            Err(e) => println!(
                                "{}[FAIL] Compose up failed: {}{}\n",
                                TuiRenderer::RED,
                                e,
                                TuiRenderer::RESET
                            ),
                        }
                    }
                    "down" => {
                        println!(
                            "{}[ACODE] Stopping compose cluster...{}",
                            TuiRenderer::DIM,
                            TuiRenderer::RESET
                        );
                        match orchestrator
                            .tools
                            .containers
                            .compose_down(&orchestrator.workspace_root)
                        {
                            Ok(out) => println!(
                                "{}[OK] Compose down:\n{}{}\n",
                                TuiRenderer::GREEN,
                                out,
                                TuiRenderer::RESET
                            ),
                            Err(e) => println!(
                                "{}[FAIL] Compose down failed: {}{}\n",
                                TuiRenderer::RED,
                                e,
                                TuiRenderer::RESET
                            ),
                        }
                    }
                    _ => {
                        println!("{}Usage: /docker [ps | logs <id> | inspect <id> | up | down | status]{}\n", TuiRenderer::CYAN, TuiRenderer::RESET);
                    }
                }
            }
            true
        }
        _ => false,
    }
}
