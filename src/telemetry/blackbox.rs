//! Atlas Daedalus — BlackBox Flight Recorder (The Mousai Architecture)
//! Implements an append-only, tamper-evident event stream in `.daedalus/blackbox.jsonl`
//! for session telemetry, audit logging, crash replay, and tool execution analytics.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::telemetry::host::{HostInspector, HostMetrics};

fn sha256_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackBoxEvent {
    pub event_id: String,
    pub timestamp: String,
    pub event_type: String,
    pub source: String,
    pub payload_summary: String,
    pub payload_hash: String,
    #[serde(default)]
    pub prev_hash: Option<String>,
    #[serde(default)]
    pub chain_hash: Option<String>,
    #[serde(default)]
    pub host_metrics: Option<HostMetrics>,
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

    pub fn get_last_chain_hash(&self) -> String {
        let genesis = "0".repeat(64);
        if !self.log_file.exists() {
            return genesis;
        }

        if let Ok(content) = fs::read_to_string(&self.log_file) {
            for line in content.lines().rev() {
                if let Ok(evt) = serde_json::from_str::<BlackBoxEvent>(line) {
                    if let Some(ch) = evt.chain_hash {
                        return ch;
                    }
                    return evt.payload_hash;
                }
            }
        }
        genesis
    }

    pub fn record(
        &self,
        event_type: &str,
        source: &str,
        payload: &str,
        duration_ms: Option<u64>,
        status: &str,
    ) -> BlackBoxEvent {
        let prev_hash = self.get_last_chain_hash();
        let payload_hash = sha256_hash(payload.as_bytes());
        let host_metrics = HostInspector::collect();
        let chain_material = format!("{}:{}:{}:{}:{}", prev_hash, event_type, source, payload_hash, status);
        let chain_hash = sha256_hash(chain_material.as_bytes());

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
            payload_hash,
            prev_hash: Some(prev_hash),
            chain_hash: Some(chain_hash),
            host_metrics: Some(host_metrics),
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

    pub fn verify_chain(&self) -> (bool, usize) {
        if !self.log_file.exists() {
            return (true, 0);
        }

        let genesis = "0".repeat(64);
        if let Ok(content) = fs::read_to_string(&self.log_file) {
            let mut expected_prev = genesis;
            let mut verified_count = 0;
            for line in content.lines() {
                if let Ok(evt) = serde_json::from_str::<BlackBoxEvent>(line) {
                    if let (Some(prev), Some(chain)) = (&evt.prev_hash, &evt.chain_hash) {
                        if prev != &expected_prev {
                            return (false, verified_count);
                        }
                        let chain_material = format!("{}:{}:{}:{}:{}", prev, evt.event_type, evt.source, evt.payload_hash, evt.status);
                        let computed = sha256_hash(chain_material.as_bytes());
                        if &computed != chain {
                            return (false, verified_count);
                        }
                        expected_prev = chain.clone();
                        verified_count += 1;
                    }
                }
            }
            (true, verified_count)
        } else {
            (true, 0)
        }
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
        let temp_dir = std::env::temp_dir().join(format!("daedalus_test_{}", rand_suffix()));
        let _ = fs::create_dir_all(&temp_dir);
        let recorder = BlackBoxRecorder::new(&temp_dir);
        let evt = recorder.record("test_event", "unit_test", "hello world payload", Some(42), "OK");
        assert_eq!(evt.event_type, "test_event");
        assert_eq!(evt.status, "OK");
        assert_eq!(evt.payload_hash.len(), 64);
        assert!(evt.chain_hash.is_some());
        assert!(evt.host_metrics.is_some());

        let recents = recorder.read_recent_events(5);
        assert!(!recents.is_empty());

        let (valid, count) = recorder.verify_chain();
        assert!(valid);
        assert!(count >= 1);
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
