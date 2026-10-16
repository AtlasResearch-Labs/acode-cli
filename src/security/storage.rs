//! Atlas Daedalus — Security Config Persistence
//! (C) 2026 Atlas Research Labs — MIT License

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::security::policy::SecurityMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub mode: SecurityMode,
    pub trusted_paths: Vec<PathBuf>,
    pub trusted_commands: Vec<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            mode: SecurityMode::Sandboxed,
            trusted_paths: Vec::new(),
            trusted_commands: Vec::new(),
        }
    }
}

pub struct SecurityStorage;

impl SecurityStorage {
    fn config_path() -> Option<PathBuf> {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .ok()?;
        Some(PathBuf::from(home).join(".daedalus").join("security.json"))
    }

    pub fn load() -> SecurityConfig {
        if let Some(path) = Self::config_path() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<SecurityConfig>(&content) {
                    return cfg;
                }
            }
        }
        SecurityConfig::default()
    }

    pub fn save(cfg: &SecurityConfig) {
        if let Some(path) = Self::config_path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(cfg) {
                let _ = fs::write(&path, json);
            }
        }
    }
}
