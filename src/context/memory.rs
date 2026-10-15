//! Atlas Daedalus — Long-Term Memory & Helix Vector Engine

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub timestamp: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMemory {
    pub conventions: Vec<String>,
    pub tech_stack: Vec<String>,
    pub last_session_time: String,
    pub key_decisions: Vec<DecisionRecord>,
}

pub struct MemoryEngine {
    base_dir: PathBuf,
    helix_dir: PathBuf,
    memory_file: PathBuf,
    pub memory: ProjectMemory,
}

impl MemoryEngine {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        let base_dir = workspace_root.as_ref().join(".daedalus");
        let helix_dir = workspace_root.as_ref().join(".helix").join("vectors");
        let memory_file = base_dir.join("memory.json");

        let memory = Self::load_memory(&memory_file);

        Self {
            base_dir,
            helix_dir,
            memory_file,
            memory,
        }
    }

    fn load_memory(path: &Path) -> ProjectMemory {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(mem) = serde_json::from_str::<ProjectMemory>(&content) {
                    return mem;
                }
            }
        }

        ProjectMemory {
            conventions: Vec::new(),
            tech_stack: Vec::new(),
            last_session_time: Utc::now().to_rfc3339(),
            key_decisions: Vec::new(),
        }
    }

    pub fn save_memory(&mut self) {
        let _ = fs::create_dir_all(&self.base_dir);
        self.memory.last_session_time = Utc::now().to_rfc3339();
        if let Ok(content) = serde_json::to_string_pretty(&self.memory) {
            let _ = fs::write(&self.memory_file, content);
        }
    }

    pub fn add_convention(&mut self, rule: String) {
        if !self.memory.conventions.contains(&rule) {
            self.memory.conventions.push(rule);
            self.save_memory();
        }
    }

    pub fn add_decision(&mut self, summary: String) {
        self.memory.key_decisions.push(DecisionRecord {
            timestamp: Utc::now().to_rfc3339(),
            summary,
        });
        self.save_memory();
    }

    pub fn get_memory_context(&self) -> String {
        let mut parts = Vec::new();
        if !self.memory.tech_stack.is_empty() {
            parts.push(format!("Tech Stack: {}", self.memory.tech_stack.join(", ")));
        }
        if !self.memory.conventions.is_empty() {
            let convs = self.memory.conventions.iter().map(|c| format!("- {}", c)).collect::<Vec<_>>().join("\n");
            parts.push(format!("Project Rules & Conventions:\n{}", convs));
        }
        if !self.memory.key_decisions.is_empty() {
            let decs = self.memory.key_decisions.iter().rev().take(5).map(|d| {
                let date = if d.timestamp.len() >= 10 { &d.timestamp[0..10] } else { &d.timestamp };
                format!("- [{}] {}", date, d.summary)
            }).collect::<Vec<_>>().join("\n");
            parts.push(format!("Recent Key Decisions:\n{}", decs));
        }
        parts.join("\n\n")
    }

    pub fn sync_helix_vector(&self, collection: &str, id: &str, metadata: serde_json::Value) {
        let _ = fs::create_dir_all(&self.helix_dir);
        let file_path = self.helix_dir.join(format!("{}.jsonl", collection));

        #[derive(Serialize)]
        struct VectorEntry<'a> {
            id: &'a str,
            metadata: serde_json::Value,
            timestamp: String,
        }

        let entry = VectorEntry {
            id,
            metadata,
            timestamp: Utc::now().to_rfc3339(),
        };

        if let Ok(line) = serde_json::to_string(&entry) {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(file_path) {
                let _ = writeln!(file, "{}", line);
            }
        }
    }
}
