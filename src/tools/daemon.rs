//! Atlas Daedalus — Background Process Daemon Registry (Kiri Architecture)
//! Manages long-running background tasks (dev servers, test watchers, microservices)
//! with circular output buffers, log streaming, and lifecycle control without blocking the agent.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

pub const MAX_BUFFER_LINES: usize = 1_000;

#[derive(Clone)]
pub struct DaemonJobInfo {
    pub id: String,
    pub command: String,
    pub pid: u32,
    pub uptime_secs: u64,
    pub is_alive: bool,
    pub exit_code: Option<i32>,
    pub recent_logs: Vec<String>,
}

struct InternalJob {
    id: String,
    command: String,
    pid: u32,
    start_time: Instant,
    is_alive: Arc<Mutex<bool>>,
    exit_code: Arc<Mutex<Option<i32>>>,
    logs: Arc<Mutex<Vec<String>>>,
    child: Arc<Mutex<Option<Child>>>,
}

#[derive(Clone)]
pub struct DaemonRegistry {
    jobs: Arc<Mutex<HashMap<String, InternalJob>>>,
    counter: Arc<Mutex<usize>>,
}

impl DaemonRegistry {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            counter: Arc::new(Mutex::new(1)),
        }
    }

    pub fn start_daemon<P: AsRef<Path>>(
        &self,
        command: &str,
        cwd: P,
    ) -> Result<DaemonJobInfo, String> {
        let is_win = cfg!(target_os = "windows");
        let mut cmd = if is_win {
            let mut c = Command::new("powershell.exe");
            c.arg("-NoProfile").arg("-Command").arg(command);
            c
        } else {
            let mut c = Command::new("/bin/sh");
            c.arg("-c").arg(command);
            c
        };

        cmd.current_dir(cwd.as_ref())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn daemon: {}", e))?;
        let pid = child.id();

        let id = {
            let mut c = self.counter.lock().unwrap();
            let job_id = format!("d_{:02}", *c);
            *c += 1;
            job_id
        };

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let is_alive = Arc::new(Mutex::new(true));
        let exit_code = Arc::new(Mutex::new(None));
        let logs = Arc::new(Mutex::new(Vec::new()));
        let child_cell = Arc::new(Mutex::new(Some(child)));

        // Background reader for stdout
        if let Some(out) = stdout {
            let logs_clone = Arc::clone(&logs);
            thread::spawn(move || {
                let reader = BufReader::new(out);
                for line in reader.lines().flatten() {
                    let mut buffer = logs_clone.lock().unwrap();
                    buffer.push(format!("[stdout] {}", line));
                    if buffer.len() > MAX_BUFFER_LINES {
                        buffer.remove(0);
                    }
                }
            });
        }

        // Background reader for stderr
        if let Some(err) = stderr {
            let logs_clone = Arc::clone(&logs);
            thread::spawn(move || {
                let reader = BufReader::new(err);
                for line in reader.lines().flatten() {
                    let mut buffer = logs_clone.lock().unwrap();
                    buffer.push(format!("[stderr] {}", line));
                    if buffer.len() > MAX_BUFFER_LINES {
                        buffer.remove(0);
                    }
                }
            });
        }

        // Monitor thread for process exit
        {
            let child_cell_clone = Arc::clone(&child_cell);
            let is_alive_clone = Arc::clone(&is_alive);
            let exit_code_clone = Arc::clone(&exit_code);

            thread::spawn(move || {
                loop {
                    thread::sleep(std::time::Duration::from_millis(500));
                    let mut maybe_child = child_cell_clone.lock().unwrap();
                    if let Some(child_ref) = maybe_child.as_mut() {
                        match child_ref.try_wait() {
                            Ok(Some(status)) => {
                                *is_alive_clone.lock().unwrap() = false;
                                *exit_code_clone.lock().unwrap() = status.code();
                                break;
                            }
                            Ok(None) => {}
                            Err(_) => {
                                *is_alive_clone.lock().unwrap() = false;
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }
            });
        }

        let job = InternalJob {
            id: id.clone(),
            command: command.to_string(),
            pid,
            start_time: Instant::now(),
            is_alive: Arc::clone(&is_alive),
            exit_code: Arc::clone(&exit_code),
            logs: Arc::clone(&logs),
            child: child_cell,
        };

        let info = DaemonJobInfo {
            id: id.clone(),
            command: command.to_string(),
            pid,
            uptime_secs: 0,
            is_alive: true,
            exit_code: None,
            recent_logs: Vec::new(),
        };

        self.jobs.lock().unwrap().insert(id, job);
        Ok(info)
    }

    pub fn list_daemons(&self) -> Vec<DaemonJobInfo> {
        let jobs = self.jobs.lock().unwrap();
        let mut list = Vec::new();

        for (id, job) in jobs.iter() {
            let is_alive = *job.is_alive.lock().unwrap();
            let exit_code = *job.exit_code.lock().unwrap();
            let logs = job.logs.lock().unwrap();
            let tail = logs.iter().rev().take(15).cloned().collect::<Vec<_>>();
            let mut recent = tail;
            recent.reverse();

            list.push(DaemonJobInfo {
                id: id.clone(),
                command: job.command.clone(),
                pid: job.pid,
                uptime_secs: job.start_time.elapsed().as_secs(),
                is_alive,
                exit_code,
                recent_logs: recent,
            });
        }

        list.sort_by(|a, b| a.id.cmp(&b.id));
        list
    }

    pub fn get_daemon_logs(&self, id: &str, line_count: usize) -> Option<Vec<String>> {
        let jobs = self.jobs.lock().unwrap();
        jobs.get(id).map(|job| {
            let logs = job.logs.lock().unwrap();
            let count = if line_count == 0 { 25 } else { line_count };
            let tail = logs.iter().rev().take(count).cloned().collect::<Vec<_>>();
            let mut recent = tail;
            recent.reverse();
            recent
        })
    }

    pub fn kill_daemon(&self, id: &str) -> Result<bool, String> {
        let jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.get(id) {
            let mut maybe_child = job.child.lock().unwrap();
            if let Some(child_ref) = maybe_child.as_mut() {
                let _ = child_ref.kill();
                *job.is_alive.lock().unwrap() = false;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Err(format!("Daemon '{}' not found", id))
        }
    }
}

impl Default for DaemonRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_lifecycle() {
        let reg = DaemonRegistry::new();
        let cmd = if cfg!(target_os = "windows") { "Start-Sleep -Seconds 1; echo done" } else { "sleep 1 && echo done" };
        let job = reg.start_daemon(cmd, Path::new("."));
        assert!(job.is_ok());

        let list = reg.list_daemons();
        assert_eq!(list.len(), 1);
        assert!(list[0].is_alive);

        let killed = reg.kill_daemon(&list[0].id);
        assert!(killed.is_ok());
    }
}
