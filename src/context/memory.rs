//! Atlas Daedalus — Dual-Profile Persistent Memory Engine (Mneme Architecture)
//! Manages global user profile (~/.daedalus/USER.md) and workspace architectural memory (<workspace>/.daedalus/MEMORY.md).

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

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
    pub workspace_root: PathBuf,
    pub base_dir: PathBuf,
    pub helix_dir: PathBuf,
    pub memory_json_file: PathBuf,
    pub project_memory_md: PathBuf,
    pub user_profile_md: PathBuf,
    pub memory: ProjectMemory,
}

impl MemoryEngine {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        let ws = workspace_root.as_ref().to_path_buf();
        let base_dir = ws.join(".daedalus");
        let helix_dir = ws.join(".helix").join("vectors");
        let memory_json_file = base_dir.join("memory.json");
        let project_memory_md = base_dir.join("MEMORY.md");

        let home_dir = env::var("USERPROFILE")
            .or_else(|_| env::var("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| ws.clone());
        let user_profile_md = home_dir.join(".daedalus").join("USER.md");

        let mut engine = Self {
            workspace_root: ws,
            base_dir,
            helix_dir,
            memory_json_file: memory_json_file.clone(),
            project_memory_md,
            user_profile_md,
            memory: Self::load_json_memory(&memory_json_file),
        };

        engine.ensure_files();
        engine
    }

    fn ensure_files(&mut self) {
        let _ = fs::create_dir_all(&self.base_dir);

        // Ensure global ~/.daedalus directory exists
        if let Some(parent) = self.user_profile_md.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Initialize USER.md if missing
        if !self.user_profile_md.exists() {
            let user_name = std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_else(|_| "Developer".to_string());
            let default_user = format!(
                "# User Profile & Preferences\n\
                - User: {}\n\
                - Primary Workspace: {}\n\
                - Coding Style: Clean, modular, robust error handling, zero emojis\n\
                - Communication Tone: Concise, high-density, technical\n",
                user_name,
                self.workspace_root.display()
            );
            let _ = fs::write(&self.user_profile_md, default_user);
        }

        // Initialize workspace MEMORY.md if missing
        if !self.project_memory_md.exists() {
            let ws_name = self
                .workspace_root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("workspace");
            let default_project = format!(
                "# Project Architectural Memory: {}\n\n\
                ## Core Tech Stack\n\
                - Workspace: {}\n\n\
                ## Architectural Guidelines & Conventions\n\
                - Maintained by Atlas Daedalus autonomous engine.\n\
                - Prefer surgical patching over rewriting full files.\n\
                - Verify all changes with project test suites before concluding.\n\n\
                ## Key Decisions & Learnings\n\
                - [{}] Initialized workspace architectural ledger.\n",
                ws_name,
                self.workspace_root.display(),
                Utc::now().format("%Y-%m-%d")
            );
            let _ = fs::write(&self.project_memory_md, default_project);
        }

        // Auto-ignore .daedalus/ in user repos if in a git workspace
        let git_dir = self.workspace_root.join(".git");
        if git_dir.exists() {
            let gitignore_path = self.workspace_root.join(".gitignore");
            let mut content = if gitignore_path.exists() {
                fs::read_to_string(&gitignore_path).unwrap_or_default()
            } else {
                String::new()
            };
            let has_daedalus = content.lines().any(|l| {
                let trimmed = l.trim();
                trimmed == ".daedalus"
                    || trimmed == ".daedalus/"
                    || trimmed == "/.daedalus/"
                    || trimmed == "/.daedalus"
            });
            if !has_daedalus {
                if !content.is_empty() && !content.ends_with('\n') {
                    content.push('\n');
                }
                content.push_str("\n# Atlas Daedalus local cache & memory\n.daedalus/\n");
                let _ = fs::write(&gitignore_path, content);
            }
        }
    }

    fn load_json_memory(path: &Path) -> ProjectMemory {
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
            let _ = fs::write(&self.memory_json_file, content);
        }
    }

    pub fn get_user_profile(&self) -> String {
        if self.user_profile_md.exists() {
            fs::read_to_string(&self.user_profile_md).unwrap_or_default()
        } else {
            String::new()
        }
    }

    pub fn get_project_memory(&self) -> String {
        if self.project_memory_md.exists() {
            fs::read_to_string(&self.project_memory_md).unwrap_or_default()
        } else {
            String::new()
        }
    }

    pub fn update_user_memory(&mut self, text: &str) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.user_profile_md)
            .map_err(|e| format!("Failed to open USER.md: {}", e))?;
        writeln!(file, "\n{}", text).map_err(|e| format!("Failed to write to USER.md: {}", e))
    }

    pub fn update_project_memory(&mut self, text: &str) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.project_memory_md)
            .map_err(|e| format!("Failed to open MEMORY.md: {}", e))?;
        writeln!(file, "\n{}", text).map_err(|e| format!("Failed to write to MEMORY.md: {}", e))
    }

    pub fn add_convention(&mut self, rule: String) {
        if !self.memory.conventions.contains(&rule) {
            self.memory.conventions.push(rule.clone());
            self.save_memory();
            let _ = self.update_project_memory(&format!("- Rule: {}", rule));
        }
    }

    pub fn add_decision(&mut self, summary: String) {
        let now_str = Utc::now().format("%Y-%m-%d").to_string();
        self.memory.key_decisions.push(DecisionRecord {
            timestamp: Utc::now().to_rfc3339(),
            summary: summary.clone(),
        });
        self.save_memory();
        let _ = self.update_project_memory(&format!("- [{}] Decision: {}", now_str, summary));
    }

    /// Assembles compact dual-profile memory context for the agent system prompt
    pub fn get_memory_context(&self) -> String {
        let mut sections = Vec::new();

        let user_prof = self.get_user_profile().trim().to_string();
        if !user_prof.is_empty() {
            sections.push(format!(
                "[USER PROFILE & PREFERENCES (from ~/.daedalus/USER.md)]\n{}",
                user_prof
            ));
        }

        let proj_mem = self.get_project_memory().trim().to_string();
        if !proj_mem.is_empty() {
            sections.push(format!(
                "[PROJECT ARCHITECTURAL MEMORY (from .daedalus/MEMORY.md)]\n{}",
                proj_mem
            ));
        }

        sections.join("\n\n")
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
