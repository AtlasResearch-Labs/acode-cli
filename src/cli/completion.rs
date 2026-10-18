//! Atlas Daedalus — Three-Tier Autocomplete & Context Engine
//! (C) 2026 Atlas Research Labs — MIT License

use std::collections::HashSet;
use std::path::Path;

/// Category of completion candidate
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    /// Slash command (e.g. /help, /reset)
    SlashCommand,
    /// Workspace file mention (@path)
    FileMention,
    /// Merkle AST code symbol mention (#symbol)
    SymbolMention,
    /// Session prompt history
    History,
}

/// A candidate match with ranking score
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub display: String,
    pub insert_text: String,
    pub kind: CompletionKind,
    pub description: Option<String>,
    pub score: i32,
}

/// Autocomplete and context engine
pub struct CompletionEngine {
    pub workspace_files: Vec<String>,
    pub history: Vec<String>,
    pub slash_commands: Vec<(String, String)>, // (name, description)
}

impl CompletionEngine {
    /// Create a new completion engine
    pub fn new(history: Vec<String>) -> Self {
        let default_commands = vec![
            ("/help".to_string(), "Display interactive command cheat sheet and cockpit guide".to_string()),
            ("/reset".to_string(), "Wipe conversation working context while preserving Merkle AST".to_string()),
            ("/stats".to_string(), "Display session telemetry, token burn rate, and cache hit metrics".to_string()),
            ("/diff".to_string(), "Review pending workspace file modifications as unified diffs".to_string()),
            ("/compact".to_string(), "Summarize and compact conversation memory to minimize context".to_string()),
            ("/model".to_string(), "Inspect or switch active LLM provider and inference model".to_string()),
            ("/swarm".to_string(), "Show status of autonomous background agent tasks and locks".to_string()),
            ("/cost".to_string(), "Detailed token expenditure analysis across all tool calls".to_string()),
            ("/review".to_string(), "Trigger comprehensive code review across staged git changes".to_string()),
            ("/doctor".to_string(), "Run diagnostic environment and security shield health check".to_string()),
            ("/exit".to_string(), "Gracefully terminate session and persist command history".to_string()),
            ("/clear".to_string(), "Clear the terminal screen buffer and re-anchor cockpit header".to_string()),
        ];

        Self {
            workspace_files: Vec::new(),
            history,
            slash_commands: default_commands,
        }
    }

    /// Refresh workspace file paths for `@` context completion
    pub fn refresh_workspace_files<P: AsRef<Path>>(&mut self, root: P) {
        let mut files = Vec::new();
        let walker = ignore::WalkBuilder::new(root)
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        for result in walker.flatten() {
            let path = result.path();
            if path.is_file() {
                if let Ok(rel) = path.strip_prefix(std::env::current_dir().unwrap_or_default()) {
                    let s = rel.to_string_lossy().replace('\\', "/");
                    files.push(s);
                } else {
                    let s = path.to_string_lossy().replace('\\', "/");
                    files.push(s);
                }
            }
            if files.len() >= 2000 {
                break; // Cap to keep autocomplete instantaneous
            }
        }
        self.workspace_files = files;
    }

    /// Set file list directly (useful for tests and dynamic indexing)
    pub fn set_workspace_files(&mut self, files: Vec<String>) {
        self.workspace_files = files;
    }

    /// Derive inline ghost-text completion suffix based on current buffer content
    pub fn get_ghost_suggestion(&self, buffer: &str, ast_symbols: &[String]) -> Option<String> {
        if buffer.trim().is_empty() {
            return None;
        }

        // 1. Slash commands: buffer starts with '/' and is a single word
        if buffer.starts_with('/') && !buffer.contains(' ') {
            for (cmd, _) in &self.slash_commands {
                if cmd.starts_with(buffer) && cmd.len() > buffer.len() {
                    return Some(cmd[buffer.len()..].to_string());
                }
            }
            return None;
        }

        // Check if cursor is at the end of an active mention token (@file or #symbol)
        if let Some(token) = buffer.split_whitespace().last() {
            if token.starts_with('@') && token.len() > 1 {
                let query = &token[1..];
                for file in &self.workspace_files {
                    if file.starts_with(query) && file.len() > query.len() {
                        return Some(file[query.len()..].to_string());
                    }
                }
            } else if token.starts_with('#') && token.len() > 1 {
                let query = &token[1..];
                for sym in ast_symbols {
                    if sym.starts_with(query) && sym.len() > query.len() {
                        return Some(sym[query.len()..].to_string());
                    }
                }
            }
        }

        // 2. Fall back to Prompt History (most recent prompt starting with buffer)
        for past_prompt in self.history.iter().rev() {
            let trimmed = past_prompt.trim();
            if trimmed.starts_with(buffer) && trimmed.len() > buffer.len() {
                return Some(trimmed[buffer.len()..].to_string());
            }
        }

        None
    }

    /// Perform fuzzy scoring of a candidate against a query
    /// Uses word boundary bonuses, camelCase transitions, separator bonuses, and length penalties.
    pub fn fuzzy_score(query: &str, candidate: &str) -> Option<i32> {
        if query.is_empty() {
            return Some(0);
        }

        let q_lower = query.to_lowercase();
        let c_lower = candidate.to_lowercase();

        // Exact match
        if q_lower == c_lower {
            return Some(2000 - candidate.len() as i32);
        }

        // Exact prefix match
        if c_lower.starts_with(&q_lower) {
            return Some(1000 - (candidate.len() - query.len()) as i32);
        }

        // Subsequence matching
        let mut q_chars = q_lower.chars().peekable();
        let mut score = 0;
        let mut prev_matched_idx: Option<usize> = None;

        let cand_chars: Vec<char> = candidate.chars().collect();
        let mut cand_idx = 0;

        while let Some(&qc) = q_chars.peek() {
            let mut found = false;
            while cand_idx < cand_chars.len() {
                let cc = cand_chars[cand_idx].to_lowercase().next().unwrap_or(cand_chars[cand_idx]);
                if cc == qc {
                    found = true;
                    q_chars.next();

                    // Word boundary bonus: preceded by '/', '_', '-', or '.', or start of string
                    if cand_idx == 0 || ['/', '_', '-', '.'].contains(&cand_chars[cand_idx - 1]) {
                        score += 80;
                    }

                    // CamelCase uppercase bonus
                    if cand_idx > 0 && cand_chars[cand_idx].is_uppercase() && cand_chars[cand_idx - 1].is_lowercase() {
                        score += 60;
                    }

                    // Consecutive character bonus
                    if let Some(prev) = prev_matched_idx {
                        if cand_idx == prev + 1 {
                            score += 50;
                        }
                    }

                    prev_matched_idx = Some(cand_idx);
                    cand_idx += 1;
                    break;
                }
                cand_idx += 1;
            }

            if !found {
                return None; // Characters didn't match sequentially
            }
        }

        // Length penalty to favor shorter, tighter matches
        score -= (candidate.len() as i32) * 2;
        Some(score)
    }

    /// Query completions for the active input token
    pub fn query_completions(&self, buffer: &str, ast_symbols: &[String]) -> Vec<CompletionItem> {
        let mut results = Vec::new();

        // 1. Slash command mode
        if buffer.starts_with('/') && !buffer.contains(' ') {
            let query = &buffer[1..];
            for (cmd, desc) in &self.slash_commands {
                let cmd_name = &cmd[1..];
                if let Some(score) = Self::fuzzy_score(query, cmd_name) {
                    results.push(CompletionItem {
                        display: cmd.clone(),
                        insert_text: cmd.clone(),
                        kind: CompletionKind::SlashCommand,
                        description: Some(desc.clone()),
                        score,
                    });
                }
            }
            results.sort_by(|a, b| b.score.cmp(&a.score));
            return results;
        }

        // Active token at cursor
        let token = buffer.split_whitespace().last().unwrap_or("");

        // 2. File mention mode (@)
        if token.starts_with('@') {
            let query = &token[1..];
            for file in &self.workspace_files {
                if let Some(score) = Self::fuzzy_score(query, file) {
                    results.push(CompletionItem {
                        display: format!("@{}", file),
                        insert_text: format!("@{}", file),
                        kind: CompletionKind::FileMention,
                        description: Some("Workspace file".to_string()),
                        score,
                    });
                }
            }
            results.sort_by(|a, b| b.score.cmp(&a.score));
            results.truncate(15);
            return results;
        }

        // 3. Symbol mention mode (#)
        if token.starts_with('#') {
            let query = &token[1..];
            for sym in ast_symbols {
                if let Some(score) = Self::fuzzy_score(query, sym) {
                    results.push(CompletionItem {
                        display: format!("#{}", sym),
                        insert_text: format!("#{}", sym),
                        kind: CompletionKind::SymbolMention,
                        description: Some("Merkle AST symbol".to_string()),
                        score,
                    });
                }
            }
            results.sort_by(|a, b| b.score.cmp(&a.score));
            results.truncate(15);
            return results;
        }

        results
    }

    /// Extract all `@file` mention tokens from a prompt
    pub fn extract_file_mentions(prompt: &str) -> Vec<String> {
        let mut mentions = HashSet::new();
        for word in prompt.split_whitespace() {
            if let Some(path) = word.strip_prefix('@') {
                let clean_path = path
                    .trim_matches(|c: char| {
                        c == '"' || c == '\'' || c == '`' || c == '(' || c == ')' || c == '[' || c == ']' || c == '{' || c == '}'
                    })
                    .trim_end_matches(|c: char| {
                        c == '.' || c == ',' || c == '!' || c == '?' || c == ';' || c == ':'
                    });
                if !clean_path.is_empty() {
                    mentions.insert(clean_path.to_string());
                }
            }
        }
        mentions.into_iter().collect()
    }

    /// Extract all `#symbol` mention tokens from a prompt
    pub fn extract_symbol_mentions(prompt: &str) -> Vec<String> {
        let mut mentions = HashSet::new();
        for word in prompt.split_whitespace() {
            if let Some(sym) = word.strip_prefix('#') {
                let clean_sym = sym.trim_matches(|c: char| {
                    c.is_ascii_punctuation() && c != '_'
                });
                if !clean_sym.is_empty() {
                    mentions.insert(clean_sym.to_string());
                }
            }
        }
        mentions.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slash_ghost_text() {
        let engine = CompletionEngine::new(vec![]);
        let ghost = engine.get_ghost_suggestion("/he", &[]);
        assert_eq!(ghost, Some("lp".to_string()));

        let ghost_res = engine.get_ghost_suggestion("/res", &[]);
        assert_eq!(ghost_res, Some("et".to_string()));
    }

    #[test]
    fn test_history_ghost_text() {
        let engine = CompletionEngine::new(vec![
            "cargo test --bin acode".to_string(),
            "git status".to_string(),
        ]);
        let ghost = engine.get_ghost_suggestion("car", &[]);
        assert_eq!(ghost, Some("go test --bin acode".to_string()));
    }

    #[test]
    fn test_file_mention_ghost_text() {
        let mut engine = CompletionEngine::new(vec![]);
        engine.set_workspace_files(vec![
            "src/cli/tui.rs".to_string(),
            "src/cli/repl.rs".to_string(),
        ]);
        let ghost = engine.get_ghost_suggestion("read @src/cli/t", &[]);
        assert_eq!(ghost, Some("ui.rs".to_string()));
    }

    #[test]
    fn test_symbol_mention_ghost_text() {
        let engine = CompletionEngine::new(vec![]);
        let symbols = vec!["TerminalInputEngine".to_string(), "TuiRenderer".to_string()];
        let ghost = engine.get_ghost_suggestion("explain #Term", &symbols);
        assert_eq!(ghost, Some("inalInputEngine".to_string()));
    }

    #[test]
    fn test_fuzzy_score_ranking() {
        let _query = "tui";
        let score_exact = CompletionEngine::fuzzy_score("tui", "tui").unwrap();
        let score_prefix = CompletionEngine::fuzzy_score("tui", "tui_renderer").unwrap();
        let score_path = CompletionEngine::fuzzy_score("tui", "src/cli/tui.rs").unwrap();

        assert!(score_exact > score_prefix);
        assert!(score_prefix > 0);
        assert!(score_path > 0);
    }

    #[test]
    fn test_extract_mentions() {
        let prompt = "Please review @src/cli/tui.rs and check #TuiRenderer with @Cargo.toml.";
        let files = CompletionEngine::extract_file_mentions(prompt);
        let symbols = CompletionEngine::extract_symbol_mentions(prompt);

        assert!(files.contains(&"src/cli/tui.rs".to_string()));
        assert!(files.contains(&"Cargo.toml".to_string()));
        assert!(symbols.contains(&"TuiRenderer".to_string()));
    }
}
