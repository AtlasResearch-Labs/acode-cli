//! Atlas Daedalus — Semantic Symbol & Refactoring Engine (Hephaestus Architecture)
//! Parses symbols (functions, structs, traits, classes, interfaces, enums) across Rust,
//! TypeScript/JavaScript, Python, and Go for structural navigation and precise cross-file renaming.

use std::fs;
use std::path::{Path, PathBuf};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Struct,
    Class,
    Trait,
    Interface,
    Enum,
    TypeAlias,
    Constant,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Struct => "struct",
            SymbolKind::Class => "class",
            SymbolKind::Trait => "trait",
            SymbolKind::Interface => "interface",
            SymbolKind::Enum => "enum",
            SymbolKind::TypeAlias => "type",
            SymbolKind::Constant => "constant",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolRecord {
    pub name: String,
    pub kind: String,
    pub line_number: usize,
    pub signature: String,
    pub is_public: bool,
    pub file_path: String,
}

pub struct SymbolEngine {
    pub workspace_root: PathBuf,
}

impl SymbolEngine {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
    }

    /// Extracts all symbol definitions from a specific source file
    pub fn extract_symbols_from_file(&self, file_path: &Path) -> Result<Vec<SymbolRecord>, String> {
        if !file_path.exists() {
            return Err(format!("File '{}' does not exist.", file_path.display()));
        }

        let content = fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read file '{}': {}", file_path.display(), e))?;

        let ext = file_path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let rel_path = file_path.strip_prefix(&self.workspace_root)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        match ext {
            "rs" => Self::parse_rust_symbols(&content, &rel_path),
            "ts" | "tsx" | "js" | "jsx" => Self::parse_js_ts_symbols(&content, &rel_path),
            "py" => Self::parse_python_symbols(&content, &rel_path),
            "go" => Self::parse_go_symbols(&content, &rel_path),
            _ => Ok(Vec::new()),
        }
    }

    /// Searches the workspace for definitions matching `symbol_name`
    pub fn find_symbol_definitions(&self, symbol_name: &str) -> Vec<SymbolRecord> {
        let mut matches = Vec::new();
        let src_dirs = ["src", "lib", "app", "pkg", "."];

        for dir in &src_dirs {
            let target = self.workspace_root.join(dir);
            if target.exists() {
                self.walk_and_collect_symbols(&target, symbol_name, &mut matches);
            }
        }

        matches.sort_by(|a, b| a.file_path.cmp(&b.file_path).then(a.line_number.cmp(&b.line_number)));
        matches.dedup_by(|a, b| a.file_path == b.file_path && a.line_number == b.line_number);
        matches
    }

    /// Safely renames an identifier across matching workspace files using word-boundary preservation
    pub fn rename_symbol(
        &self,
        old_name: &str,
        new_name: &str,
        path_filter: Option<&str>,
    ) -> Result<Vec<(String, usize)>, String> {
        if old_name.is_empty() || new_name.is_empty() {
            return Err("Old name and new name must not be empty.".to_string());
        }

        // Identifier regex with word boundary
        let pattern = format!(r"\b{}\b", regex::escape(old_name));
        let re = Regex::new(&pattern).map_err(|e| format!("Regex compilation failed: {}", e))?;

        let mut modified = Vec::new();
        let files = self.collect_source_files(path_filter);

        for file in files {
            if let Ok(content) = fs::read_to_string(&file) {
                if re.is_match(&content) {
                    let mut count = 0;
                    for _ in re.find_iter(&content) {
                        count += 1;
                    }

                    let replaced = re.replace_all(&content, new_name).to_string();
                    if let Ok(_) = fs::write(&file, replaced) {
                        let rel = file.strip_prefix(&self.workspace_root)
                            .unwrap_or(&file)
                            .to_string_lossy()
                            .to_string();
                        modified.push((rel, count));
                    }
                }
            }
        }

        Ok(modified)
    }

    fn walk_and_collect_symbols(&self, dir: &Path, symbol_name: &str, results: &mut Vec<SymbolRecord>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Ok(ft) = entry.file_type() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') || name == "target" || name == "node_modules" || name == "vendor" || name == "dist" {
                        continue;
                    }
                    if ft.is_dir() {
                        self.walk_and_collect_symbols(&p, symbol_name, results);
                    } else if ft.is_file() {
                        if let Ok(syms) = self.extract_symbols_from_file(&p) {
                            for s in syms {
                                if s.name.eq_ignore_ascii_case(symbol_name) {
                                    results.push(s);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn collect_source_files(&self, filter: Option<&str>) -> Vec<PathBuf> {
        let mut list = Vec::new();
        self.walk_source_files(&self.workspace_root, filter, &mut list);
        list
    }

    fn walk_source_files(&self, dir: &Path, filter: Option<&str>, list: &mut Vec<PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name == "target" || name == "node_modules" || name == "vendor" || name == "dist" {
                    continue;
                }
                if let Ok(ft) = entry.file_type() {
                    if ft.is_dir() {
                        self.walk_source_files(&p, filter, list);
                    } else if ft.is_file() {
                        let is_src = match p.extension().and_then(|s| s.to_str()) {
                            Some("rs") | Some("ts") | Some("tsx") | Some("js") | Some("jsx") | Some("py") | Some("go") => true,
                            _ => false,
                        };
                        if is_src {
                            if let Some(f) = filter {
                                if p.to_string_lossy().contains(f) {
                                    list.push(p);
                                }
                            } else {
                                list.push(p);
                            }
                        }
                    }
                }
            }
        }
    }

    fn parse_rust_symbols(content: &str, rel_path: &str) -> Result<Vec<SymbolRecord>, String> {
        let mut symbols = Vec::new();

        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let is_pub = trimmed.starts_with("pub ");
            let line_number = idx + 1;

            if let Some(pos) = trimmed.find("fn ") {
                if pos == 0 || (is_pub && pos == 4) || trimmed.contains("async fn ") {
                    let sub = &trimmed[pos + 3..];
                    if let Some(paren) = sub.find('(') {
                        let name = sub[..paren].trim();
                        if !name.is_empty() && is_valid_ident(name) {
                            symbols.push(SymbolRecord {
                                name: name.to_string(),
                                kind: SymbolKind::Function.as_str().to_string(),
                                line_number,
                                signature: trimmed.to_string(),
                                is_public: is_pub,
                                file_path: rel_path.to_string(),
                            });
                        }
                    }
                }
            } else if let Some(pos) = trimmed.find("struct ") {
                if pos == 0 || (is_pub && pos == 4) {
                    let sub = &trimmed[pos + 7..];
                    let name = sub.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").trim();
                    if !name.is_empty() && is_valid_ident(name) {
                        symbols.push(SymbolRecord {
                            name: name.to_string(),
                            kind: SymbolKind::Struct.as_str().to_string(),
                            line_number,
                            signature: trimmed.to_string(),
                            is_public: is_pub,
                            file_path: rel_path.to_string(),
                        });
                    }
                }
            } else if let Some(pos) = trimmed.find("enum ") {
                if pos == 0 || (is_pub && pos == 4) {
                    let sub = &trimmed[pos + 5..];
                    let name = sub.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").trim();
                    if !name.is_empty() && is_valid_ident(name) {
                        symbols.push(SymbolRecord {
                            name: name.to_string(),
                            kind: SymbolKind::Enum.as_str().to_string(),
                            line_number,
                            signature: trimmed.to_string(),
                            is_public: is_pub,
                            file_path: rel_path.to_string(),
                        });
                    }
                }
            } else if let Some(pos) = trimmed.find("trait ") {
                if pos == 0 || (is_pub && pos == 4) {
                    let sub = &trimmed[pos + 6..];
                    let name = sub.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").trim();
                    if !name.is_empty() && is_valid_ident(name) {
                        symbols.push(SymbolRecord {
                            name: name.to_string(),
                            kind: SymbolKind::Trait.as_str().to_string(),
                            line_number,
                            signature: trimmed.to_string(),
                            is_public: is_pub,
                            file_path: rel_path.to_string(),
                        });
                    }
                }
            }
        }

        Ok(symbols)
    }

    fn parse_js_ts_symbols(content: &str, rel_path: &str) -> Result<Vec<SymbolRecord>, String> {
        let mut symbols = Vec::new();

        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let is_pub = trimmed.starts_with("export ");
            let line_number = idx + 1;

            if let Some(pos) = trimmed.find("function ") {
                let sub = &trimmed[pos + 9..];
                if let Some(paren) = sub.find('(') {
                    let name = sub[..paren].trim();
                    if !name.is_empty() && is_valid_ident(name) {
                        symbols.push(SymbolRecord {
                            name: name.to_string(),
                            kind: SymbolKind::Function.as_str().to_string(),
                            line_number,
                            signature: trimmed.to_string(),
                            is_public: is_pub,
                            file_path: rel_path.to_string(),
                        });
                    }
                }
            } else if let Some(pos) = trimmed.find("class ") {
                let sub = &trimmed[pos + 6..];
                let name = sub.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").trim();
                if !name.is_empty() && is_valid_ident(name) {
                    symbols.push(SymbolRecord {
                        name: name.to_string(),
                        kind: SymbolKind::Class.as_str().to_string(),
                        line_number,
                        signature: trimmed.to_string(),
                        is_public: is_pub,
                        file_path: rel_path.to_string(),
                    });
                }
            } else if let Some(pos) = trimmed.find("interface ") {
                let sub = &trimmed[pos + 10..];
                let name = sub.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").trim();
                if !name.is_empty() && is_valid_ident(name) {
                    symbols.push(SymbolRecord {
                        name: name.to_string(),
                        kind: SymbolKind::Interface.as_str().to_string(),
                        line_number,
                        signature: trimmed.to_string(),
                        is_public: is_pub,
                        file_path: rel_path.to_string(),
                    });
                }
            } else if let Some(pos) = trimmed.find("type ") {
                if pos == 0 || (is_pub && pos == 7) {
                    let sub = &trimmed[pos + 5..];
                    if let Some(eq) = sub.find('=') {
                        let name = sub[..eq].trim();
                        if !name.is_empty() && is_valid_ident(name) {
                            symbols.push(SymbolRecord {
                                name: name.to_string(),
                                kind: SymbolKind::TypeAlias.as_str().to_string(),
                                line_number,
                                signature: trimmed.to_string(),
                                is_public: is_pub,
                                file_path: rel_path.to_string(),
                            });
                        }
                    }
                }
            }
        }

        Ok(symbols)
    }

    fn parse_python_symbols(content: &str, rel_path: &str) -> Result<Vec<SymbolRecord>, String> {
        let mut symbols = Vec::new();

        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let line_number = idx + 1;

            if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                let sub = if trimmed.starts_with("async def ") { &trimmed[10..] } else { &trimmed[4..] };
                if let Some(paren) = sub.find('(') {
                    let name = sub[..paren].trim();
                    if !name.is_empty() && is_valid_ident(name) {
                        let is_pub = !name.starts_with('_');
                        symbols.push(SymbolRecord {
                            name: name.to_string(),
                            kind: SymbolKind::Function.as_str().to_string(),
                            line_number,
                            signature: trimmed.to_string(),
                            is_public: is_pub,
                            file_path: rel_path.to_string(),
                        });
                    }
                }
            } else if trimmed.starts_with("class ") {
                let sub = &trimmed[6..];
                let name = sub.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").trim();
                if !name.is_empty() && is_valid_ident(name) {
                    let is_pub = !name.starts_with('_');
                    symbols.push(SymbolRecord {
                        name: name.to_string(),
                        kind: SymbolKind::Class.as_str().to_string(),
                        line_number,
                        signature: trimmed.to_string(),
                        is_public: is_pub,
                        file_path: rel_path.to_string(),
                    });
                }
            }
        }

        Ok(symbols)
    }

    fn parse_go_symbols(content: &str, rel_path: &str) -> Result<Vec<SymbolRecord>, String> {
        let mut symbols = Vec::new();

        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let line_number = idx + 1;

            if trimmed.starts_with("func ") {
                let sub = &trimmed[5..];
                let after_receiver = if sub.starts_with('(') {
                    if let Some(close) = sub.find(')') {
                        sub[close + 1..].trim()
                    } else {
                        sub
                    }
                } else {
                    sub
                };

                if let Some(paren) = after_receiver.find('(') {
                    let name = after_receiver[..paren].trim();
                    if !name.is_empty() && is_valid_ident(name) {
                        let is_pub = name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
                        symbols.push(SymbolRecord {
                            name: name.to_string(),
                            kind: SymbolKind::Function.as_str().to_string(),
                            line_number,
                            signature: trimmed.to_string(),
                            is_public: is_pub,
                            file_path: rel_path.to_string(),
                        });
                    }
                }
            } else if trimmed.starts_with("type ") && trimmed.contains("struct") {
                let sub = &trimmed[5..];
                let name = sub.split_whitespace().next().unwrap_or("").trim();
                if !name.is_empty() && is_valid_ident(name) {
                    let is_pub = name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
                    symbols.push(SymbolRecord {
                        name: name.to_string(),
                        kind: SymbolKind::Struct.as_str().to_string(),
                        line_number,
                        signature: trimmed.to_string(),
                        is_public: is_pub,
                        file_path: rel_path.to_string(),
                    });
                }
            }
        }

        Ok(symbols)
    }
}

fn is_valid_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => chars.all(|c| c.is_alphanumeric() || c == '_'),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rust_symbols() {
        let code = "pub struct EngineContext {\n    pub id: String,\n}\n\npub fn initialize_agent(name: &str) -> bool {\n    true\n}\n\nenum PriorityState {\n    High,\n}\n";
        let syms = SymbolEngine::parse_rust_symbols(code, "src/agent.rs").unwrap();
        assert_eq!(syms.len(), 3);
        assert_eq!(syms[0].name, "EngineContext");
        assert_eq!(syms[0].kind, "struct");
        assert!(syms[0].is_public);

        assert_eq!(syms[1].name, "initialize_agent");
        assert_eq!(syms[1].kind, "function");
        assert!(syms[1].is_public);

        assert_eq!(syms[2].name, "PriorityState");
        assert_eq!(syms[2].kind, "enum");
        assert!(!syms[2].is_public);
    }

    #[test]
    fn test_parse_ts_symbols() {
        let code = "export interface UserPayload {\n  id: string;\n}\n\nexport function fetchUserData(id: string) {\n  return id;\n}\n\nclass LocalBuffer {}\n";
        let syms = SymbolEngine::parse_js_ts_symbols(code, "src/api.ts").unwrap();
        assert_eq!(syms.len(), 3);
        assert_eq!(syms[0].name, "UserPayload");
        assert_eq!(syms[0].kind, "interface");
        assert!(syms[0].is_public);

        assert_eq!(syms[1].name, "fetchUserData");
        assert_eq!(syms[1].kind, "function");
        assert!(syms[1].is_public);

        assert_eq!(syms[2].name, "LocalBuffer");
        assert_eq!(syms[2].kind, "class");
        assert!(!syms[2].is_public);
    }
}
