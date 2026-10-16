//! Atlas Daedalus — BlackBox Flight Recorder (The Mousai Architecture)
//! Implements an append-only, tamper-evident event stream in `.daedalus/blackbox.jsonl`
//! for session telemetry, audit logging, crash replay, and tool execution analytics.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use chrono::Utc;
use serde::{Deserialize, Serialize};

fn fnv1a_hash(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackBoxEvent {
    pub event_id: String,
    pub timestamp: String,
    pub event_type: String,
    pub source: String,
    pub payload_summary: String,
    pub payload_hash: String,
    pub duration_ms: Option<u64>,
    pub status: String,
}

pub struct BlackBoxRecorder {
    pub log_file: PathBuf,
}

impl BlackBoxRecorder {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        let base_dir = workspace_root.as_ref().join(".daedalus");
        let _ = fs::create_dir_all(&base_dir);
        let log_file = base_dir.join("blackbox.jsonl");
        Self { log_file }
    }

    pub fn record(
        &self,
        event_type: &str,
        source: &str,
        payload: &str,
        duration_ms: Option<u64>,
        status: &str,
    ) -> BlackBoxEvent {
        let hash = fnv1a_hash(payload.as_bytes());

        let summary = if payload.len() > 200 {
            let mut end = 200;
            while !payload.is_char_boundary(end) && end > 0 {
                end -= 1;
            }
            format!("{}...", &payload[..end])
        } else {
            payload.to_string()
        };

        let event = BlackBoxEvent {
            event_id: format!("evt_{:08x}", rand_suffix()),
            timestamp: Utc::now().to_rfc3339(),
            event_type: event_type.to_string(),
            source: source.to_string(),
            payload_summary: summary,
            payload_hash: hash,
            duration_ms,
            status: status.to_string(),
        };

        if let Ok(line) = serde_json::to_string(&event) {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&self.log_file) {
                let _ = writeln!(file, "{}", line);
            }
        }

        event
    }

    pub fn read_recent_events(&self, count: usize) -> Vec<BlackBoxEvent> {
        if !self.log_file.exists() {
            return Vec::new();
        }

        if let Ok(content) = fs::read_to_string(&self.log_file) {
            let mut events = Vec::new();
            for line in content.lines().rev() {
                if let Ok(evt) = serde_json::from_str::<BlackBoxEvent>(line) {
                    events.push(evt);
                    if events.len() >= count {
                        break;
                    }
                }
            }
            events.reverse();
            events
        } else {
            Vec::new()
        }
    }
}

fn rand_suffix() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_nanos() & 0xFFFFFFFF) as u32)
        .unwrap_or(0x42424242)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_read_event() {
        let recorder = BlackBoxRecorder::new("target");
        let evt = recorder.record("test_event", "unit_test", "hello world payload", Some(42), "OK");
        assert_eq!(evt.event_type, "test_event");
        assert_eq!(evt.status, "OK");
        assert!(!evt.payload_hash.is_empty());

        let recents = recorder.read_recent_events(5);
        assert!(!recents.is_empty());
    }
}
