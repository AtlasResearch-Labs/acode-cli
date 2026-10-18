//! Atlas Daedalus — Semantic AST Symbol & Repository Map Engine (Tree-sitter Architecture)
//! Parses symbols (functions, structs, traits, classes, interfaces, enums, callsites, imports)
//! across Rust, TypeScript, JavaScript, Python, and Go for structural navigation,
//! AST repository map generation, and AST-precise cross-file renaming.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AstLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
}

impl AstLanguage {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension().and_then(|s| s.to_str())?;
        match ext {
            "rs" => Some(Self::Rust),
            "py" => Some(Self::Python),
            "js" | "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            "ts" | "tsx" | "mts" | "cts" => Some(Self::TypeScript),
            "go" => Some(Self::Go),
            _ => None,
        }
    }

    pub fn language(&self) -> tree_sitter::Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Python => "Python",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Go => "Go",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Class,
    Trait,
    Interface,
    Enum,
    TypeAlias,
    Constant,
    Module,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Struct => "struct",
            SymbolKind::Class => "class",
            SymbolKind::Trait => "trait",
            SymbolKind::Interface => "interface",
            SymbolKind::Enum => "enum",
            SymbolKind::TypeAlias => "type",
            SymbolKind::Constant => "constant",
            SymbolKind::Module => "module",
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
    #[serde(default)]
    pub container: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallSiteRecord {
    pub caller: Option<String>,
    pub callee: String,
    pub line_number: usize,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRecord {
    pub source: String,
    pub imported_names: Vec<String>,
    pub line_number: usize,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstFileAnalysis {
    pub file_path: String,
    pub language: String,
    pub symbols: Vec<SymbolRecord>,
    pub callsites: Vec<CallSiteRecord>,
    pub imports: Vec<ImportRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstRepoMap {
    pub total_files: usize,
    pub total_symbols: usize,
    pub total_callsites: usize,
    pub total_imports: usize,
    pub file_analyses: Vec<AstFileAnalysis>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedAstEntry {
    pub file_path: PathBuf,
    pub rel_path: String,
    pub content_hash: String,
    pub file_size: u64,
    pub modified_time: SystemTime,
    pub analysis: AstFileAnalysis,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct IndexCacheStats {
    pub hits: usize,
    pub misses: usize,
    pub cached_files: usize,
    pub total_symbols: usize,
    pub merkle_root: String,
}

#[derive(Default)]
struct DirNode {
    files: HashMap<String, String>,
    subdirs: HashMap<String, DirNode>,
}

impl DirNode {
    fn insert(&mut self, path_parts: &[&str], hash: String) {
        if path_parts.is_empty() {
            return;
        }
        if path_parts.len() == 1 {
            self.files.insert(path_parts[0].to_string(), hash);
        } else {
            let subdir = self.subdirs.entry(path_parts[0].to_string()).or_default();
            subdir.insert(&path_parts[1..], hash);
        }
    }

    fn compute_hash(&self) -> String {
        let mut entries: Vec<(String, String)> = Vec::new();
        for (name, hash) in &self.files {
            entries.push((name.clone(), format!("blob:{}", hash)));
        }
        for (name, subdir) in &self.subdirs {
            let sub_hash = subdir.compute_hash();
            entries.push((name.clone(), format!("tree:{}", sub_hash)));
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        let mut hasher = Sha256::new();
        for (name, typed_hash) in entries {
            hasher.update(name.as_bytes());
            hasher.update(b":");
            hasher.update(typed_hash.as_bytes());
            hasher.update(b"\n");
        }
        format!("{:x}", hasher.finalize())
    }
}

#[derive(Clone)]
pub struct SymbolEngine {
    pub workspace_root: PathBuf,
    pub cache: Arc<RwLock<HashMap<PathBuf, CachedAstEntry>>>,
    pub stats: Arc<RwLock<(usize, usize)>>,
}

impl SymbolEngine {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new((0, 0))),
        }
    }

    /// Extracts all symbol definitions from a specific source file using Tree-sitter AST
    pub fn extract_symbols_from_file(&self, file_path: &Path) -> Result<Vec<SymbolRecord>, String> {
        let analysis = self.analyze_file(file_path)?;
        Ok(analysis.symbols)
    }

    /// Fully analyzes a source file extracting symbols, callsites, and imports via AST
    /// with two-tier incremental caching (metadata mtime/len fast-path + SHA-256 hash check).
    pub fn analyze_file(&self, file_path: &Path) -> Result<AstFileAnalysis, String> {
        if !file_path.exists() {
            return Err(format!("File '{}' does not exist.", file_path.display()));
        }

        let metadata = fs::metadata(file_path)
            .map_err(|e| format!("Failed to read metadata for '{}': {}", file_path.display(), e))?;
        let mtime = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let file_size = metadata.len();

        let rel_path = file_path
            .strip_prefix(&self.workspace_root)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        // 1. Fast Tier: In-memory cache lookup with mtime & file size check (< 0.001ms)
        if let Ok(cache) = self.cache.read() {
            if let Some(entry) = cache.get(file_path) {
                if entry.file_size == file_size && entry.modified_time == mtime {
                    if let Ok(mut stats) = self.stats.write() {
                        stats.0 += 1;
                    }
                    return Ok(entry.analysis.clone());
                }
            }
        }

        // 2. Read content and compute SHA-256
        let content = fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read file '{}': {}", file_path.display(), e))?;

        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash = format!("{:x}", hasher.finalize());

        // 3. Deep Tier: Hash check against cache
        let mut hit_by_hash = false;
        let mut cached_analysis = None;
        if let Ok(cache) = self.cache.read() {
            if let Some(entry) = cache.get(file_path) {
                if entry.content_hash == content_hash {
                    hit_by_hash = true;
                    cached_analysis = Some(entry.analysis.clone());
                }
            }
        }

        if hit_by_hash {
            if let Ok(mut stats) = self.stats.write() {
                stats.0 += 1;
            }
            if let Ok(mut wcache) = self.cache.write() {
                if let Some(wentry) = wcache.get_mut(file_path) {
                    wentry.modified_time = mtime;
                    wentry.file_size = file_size;
                }
            }
            if let Some(a) = cached_analysis {
                return Ok(a);
            }
        }

        // Cache miss: record miss
        if let Ok(mut stats) = self.stats.write() {
            stats.1 += 1;
        }

        let ast_lang = AstLanguage::from_path(file_path);
        let analysis = match ast_lang {
            Some(lang) => {
                let mut parser = tree_sitter::Parser::new();
                let ts_lang = lang.language();
                parser.set_language(&ts_lang).map_err(|e| {
                    format!("Failed to load language grammar for {}: {}", lang.name(), e)
                })?;

                let tree = parser
                    .parse(&content, None)
                    .ok_or_else(|| format!("Failed to parse AST for '{}'", file_path.display()))?;

                let root = tree.root_node();
                let mut symbols = Vec::new();
                let mut callsites = Vec::new();
                let mut imports = Vec::new();

                match lang {
                    AstLanguage::Rust => {
                        Self::walk_rust_ast(
                            root,
                            &content,
                            &rel_path,
                            None,
                            &mut symbols,
                            &mut callsites,
                            &mut imports,
                        );
                    }
                    AstLanguage::Python => {
                        Self::walk_python_ast(
                            root,
                            &content,
                            &rel_path,
                            None,
                            &mut symbols,
                            &mut callsites,
                            &mut imports,
                        );
                    }
                    AstLanguage::JavaScript | AstLanguage::TypeScript => {
                        Self::walk_js_ts_ast(
                            root,
                            &content,
                            &rel_path,
                            None,
                            &mut symbols,
                            &mut callsites,
                            &mut imports,
                        );
                    }
                    AstLanguage::Go => {
                        Self::walk_go_ast(
                            root,
                            &content,
                            &rel_path,
                            None,
                            &mut symbols,
                            &mut callsites,
                            &mut imports,
                        );
                    }
                }

                AstFileAnalysis {
                    file_path: rel_path.clone(),
                    language: lang.name().to_string(),
                    symbols,
                    callsites,
                    imports,
                }
            }
            None => AstFileAnalysis {
                file_path: rel_path.clone(),
                language: "Unsupported".to_string(),
                symbols: Vec::new(),
                callsites: Vec::new(),
                imports: Vec::new(),
            },
        };

        // Cache write
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(
                file_path.to_path_buf(),
                CachedAstEntry {
                    file_path: file_path.to_path_buf(),
                    rel_path,
                    content_hash,
                    file_size,
                    modified_time: mtime,
                    analysis: analysis.clone(),
                },
            );
        }

        Ok(analysis)
    }

    /// Generates a comprehensive, high-density AST repository map of the workspace
    pub fn generate_repo_map(&self, path_filter: Option<&str>) -> Result<String, String> {
        let files = self.collect_source_files(path_filter);
        let mut analyses = Vec::new();

        let mut total_symbols = 0;
        let mut total_callsites = 0;
        let mut total_imports = 0;

        for file in &files {
            if let Ok(analysis) = self.analyze_file(file) {
                if !analysis.symbols.is_empty() || !analysis.imports.is_empty() {
                    total_symbols += analysis.symbols.len();
                    total_callsites += analysis.callsites.len();
                    total_imports += analysis.imports.len();
                    analyses.push(analysis);
                }
            }
        }

        analyses.sort_by(|a, b| a.file_path.cmp(&b.file_path));

        let mut out = String::new();
        out.push_str(&format!(
            "[AST REPOSITORY MAP: {} files, {} symbol declarations, {} callsites, {} imports]\n\n",
            analyses.len(),
            total_symbols,
            total_callsites,
            total_imports
        ));

        for analysis in analyses {
            out.push_str(&format!(
                "FILE: {} ({}, {} symbols, {} imports)\n",
                analysis.file_path,
                analysis.language,
                analysis.symbols.len(),
                analysis.imports.len()
            ));

            if !analysis.imports.is_empty() {
                out.push_str("  IMPORTS:\n");
                for imp in analysis.imports.iter().take(8) {
                    if imp.imported_names.is_empty() {
                        out.push_str(&format!("    - from {}\n", imp.source));
                    } else {
                        out.push_str(&format!(
                            "    - {} from {}\n",
                            imp.imported_names.join(", "),
                            imp.source
                        ));
                    }
                }
                if analysis.imports.len() > 8 {
                    out.push_str(&format!(
                        "    ... ({} more imports)\n",
                        analysis.imports.len() - 8
                    ));
                }
            }

            if !analysis.symbols.is_empty() {
                out.push_str("  DECLARATIONS:\n");
                for sym in &analysis.symbols {
                    let pub_str = if sym.is_public { "pub " } else { "" };
                    let container_str = if let Some(ref c) = sym.container {
                        format!(" [in {}]", c)
                    } else {
                        String::new()
                    };
                    out.push_str(&format!(
                        "    L{:4} | {:9} | {}{}{}\n",
                        sym.line_number, sym.kind, pub_str, sym.signature, container_str
                    ));
                }
            }

            out.push('\n');
        }

        Ok(out)
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

        matches.sort_by(|a, b| {
            a.file_path
                .cmp(&b.file_path)
                .then(a.line_number.cmp(&b.line_number))
        });
        matches.dedup_by(|a, b| a.file_path == b.file_path && a.line_number == b.line_number);
        matches
    }

    /// Searches the workspace for callsites invoking `symbol_name`
    pub fn find_symbol_callsites(&self, symbol_name: &str) -> Vec<CallSiteRecord> {
        let mut matches = Vec::new();
        let files = self.collect_source_files(None);

        for file in files {
            if let Ok(analysis) = self.analyze_file(&file) {
                for call in analysis.callsites {
                    if call.callee.eq_ignore_ascii_case(symbol_name) {
                        matches.push(call);
                    }
                }
            }
        }

        matches
    }

    /// Safely renames an identifier across matching workspace files using Tree-sitter AST nodes
    /// (or word-boundary regex for non-AST source files) to eliminate string/comment false-positives.
    pub fn rename_symbol(
        &self,
        old_name: &str,
        new_name: &str,
        path_filter: Option<&str>,
    ) -> Result<Vec<(String, usize)>, String> {
        if old_name.is_empty() || new_name.is_empty() {
            return Err("Old name and new name must not be empty.".to_string());
        }

        let mut modified = Vec::new();
        let files = self.collect_source_files(path_filter);

        for file in files {
            if let Ok(content) = fs::read_to_string(&file) {
                if !content.contains(old_name) {
                    continue;
                }

                let rel = file
                    .strip_prefix(&self.workspace_root)
                    .unwrap_or(&file)
                    .to_string_lossy()
                    .to_string();

                if let Some(lang) = AstLanguage::from_path(&file) {
                    let mut parser = tree_sitter::Parser::new();
                    let ts_lang = lang.language();
                    if parser.set_language(&ts_lang).is_ok() {
                        if let Some(tree) = parser.parse(&content, None) {
                            let mut ranges = Vec::new();
                            Self::collect_identifier_ranges(
                                tree.root_node(),
                                &content,
                                old_name,
                                &mut ranges,
                            );

                            if !ranges.is_empty() {
                                // Sort reverse by start byte so replacement offsets remain valid
                                ranges.sort_by_key(|a| std::cmp::Reverse(a.0));
                                ranges.dedup();

                                let mut updated = content.clone();
                                let count = ranges.len();
                                for (start, end) in ranges {
                                    if start < updated.len() && end <= updated.len() && start <= end
                                    {
                                        updated.replace_range(start..end, new_name);
                                    }
                                }

                                if fs::write(&file, updated).is_ok() {
                                    modified.push((rel, count));
                                    continue;
                                }
                            }
                        }
                    }
                }

                // Fallback for non-AST supported files: regex with word boundary
                let pattern = format!(r"\b{}\b", regex::escape(old_name));
                if let Ok(re) = regex::Regex::new(&pattern) {
                    if re.is_match(&content) {
                        let count = re.find_iter(&content).count();
                        let replaced = re
                            .replace_all(&content, regex::NoExpand(new_name))
                            .to_string();
                        if fs::write(&file, replaced).is_ok() {
                            modified.push((rel, count));
                        }
                    }
                }
            }
        }

        Ok(modified)
    }

    /// Explicitly removes a file from the AST cache
    pub fn invalidate_file(&self, file_path: &Path) {
        if let Ok(mut cache) = self.cache.write() {
            cache.remove(file_path);
        }
    }

    /// Pre-warms the AST index across all source files in the workspace.
    /// Returns (indexed_files, total_symbols, elapsed_ms).
    pub fn warm_cache(&self, path_filter: Option<&str>) -> (usize, usize, u128) {
        let start = std::time::Instant::now();
        let files = self.collect_source_files(path_filter);
        let mut total_files = 0;
        let mut total_symbols = 0;

        for file in &files {
            if let Ok(analysis) = self.analyze_file(file) {
                total_files += 1;
                total_symbols += analysis.symbols.len();
            }
        }

        (total_files, total_symbols, start.elapsed().as_millis())
    }

    /// Computes a deterministic workspace Merkle root (SHA-256) across all source files
    /// using bottom-up directory aggregation matching Git tree-hashing semantics.
    pub fn compute_merkle_root(&self) -> String {
        let files = self.collect_source_files(None);
        if files.is_empty() {
            let mut hasher = Sha256::new();
            hasher.update(b"empty_workspace_tree");
            return format!("{:x}", hasher.finalize());
        }

        let mut root_node = DirNode::default();

        for file in &files {
            let rel = file
                .strip_prefix(&self.workspace_root)
                .unwrap_or(file)
                .to_string_lossy()
                .replace('\\', "/");

            let hash = if let Ok(cache) = self.cache.read() {
                cache.get(file).map(|e| e.content_hash.clone())
            } else {
                None
            };

            let content_hash = match hash {
                Some(h) => h,
                None => {
                    if let Ok(bytes) = fs::read(file) {
                        let mut hasher = Sha256::new();
                        hasher.update(&bytes);
                        format!("{:x}", hasher.finalize())
                    } else {
                        continue;
                    }
                }
            };

            let parts: Vec<&str> = rel.split('/').filter(|s| !s.is_empty()).collect();
            root_node.insert(&parts, content_hash);
        }

        root_node.compute_hash()
    }

    /// Retrieves hit/miss statistics and Merkle root for the index
    pub fn get_cache_stats(&self) -> IndexCacheStats {
        let (hits, misses) = self.stats.read().map(|s| *s).unwrap_or((0, 0));
        let (cached_files, total_symbols) = if let Ok(cache) = self.cache.read() {
            let files = cache.len();
            let syms: usize = cache.values().map(|c| c.analysis.symbols.len()).sum();
            (files, syms)
        } else {
            (0, 0)
        };
        let merkle_root = self.compute_merkle_root();

        IndexCacheStats {
            hits,
            misses,
            cached_files,
            total_symbols,
            merkle_root,
        }
    }

    /// Returns all indexed symbol names across the workspace for autocomplete (#mentions)
    pub fn get_all_symbol_names(&self) -> Vec<String> {
        let is_empty = if let Ok(cache) = self.cache.read() {
            cache.is_empty()
        } else {
            true
        };
        if is_empty {
            let _ = self.warm_cache(None);
        }

        if let Ok(cache) = self.cache.read() {
            let mut names = HashSet::new();
            for entry in cache.values() {
                for sym in &entry.analysis.symbols {
                    names.insert(sym.name.clone());
                }
            }
            let mut list: Vec<String> = names.into_iter().collect();
            list.sort();
            list
        } else {
            Vec::new()
        }
    }

    /// Resolves ambient symbol candidates from natural language queries
    /// to provide Turn-0 zero-token-waste context for LLMs.
    pub fn resolve_ambient_symbols(&self, query: &str, limit: usize) -> Vec<SymbolRecord> {
        let candidates = Self::extract_candidate_identifiers(query);
        if candidates.is_empty() {
            return Vec::new();
        }

        let mut matches = Vec::new();

        // 1. Search cached definitions first (sub-millisecond)
        let cache_has_entries = if let Ok(cache) = self.cache.read() {
            let has_entries = !cache.is_empty();
            for entry in cache.values() {
                for sym in &entry.analysis.symbols {
                    for candidate in &candidates {
                        if sym.name.eq_ignore_ascii_case(candidate) {
                            let exact = sym.name == *candidate;
                            matches.push((exact, sym.clone()));
                        }
                    }
                }
            }
            has_entries
        } else {
            false
        };

        // 2. If cache was empty or found no matches, walk filesystem
        if !cache_has_entries || matches.is_empty() {
            for candidate in &candidates {
                let defs = self.find_symbol_definitions(candidate);
                for def in defs {
                    let exact = def.name == *candidate;
                    matches.push((exact, def));
                }
            }
        }

        // Sort: exact matches first, then public symbols first, then path & line
        matches.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then(b.1.is_public.cmp(&a.1.is_public))
                .then(a.1.file_path.cmp(&b.1.file_path))
                .then(a.1.line_number.cmp(&b.1.line_number))
        });

        let mut seen = HashSet::new();
        let mut deduped = Vec::new();
        for (_, sym) in matches {
            let key = (sym.file_path.clone(), sym.line_number);
            if seen.insert(key) {
                deduped.push(sym);
                if deduped.len() >= limit {
                    break;
                }
            }
        }

        deduped
    }

    /// Extracts potential code identifier tokens from a natural language string.
    pub fn extract_candidate_identifiers(query: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let re = match regex::Regex::new(r"\b[a-zA-Z_][a-zA-Z0-9_]{2,}\b") {
            Ok(r) => r,
            Err(_) => return tokens,
        };

        let stop_words: HashSet<&str> = [
            "the", "and", "for", "with", "from", "that", "this", "file", "function",
            "code", "test", "tests", "make", "help", "view", "read", "write", "patch",
            "find", "look", "search", "show", "give", "need", "want", "some", "more",
            "into", "over", "under", "when", "what", "which", "where", "how", "why",
            "then", "also", "have", "been", "will", "would", "could", "should", "true",
            "false", "none", "null", "self", "item", "data", "list", "type", "name",
            "path", "error", "line", "mode", "call", "used", "does", "done", "work",
            "best", "like", "just", "them", "both", "each", "other", "there",
        ]
        .iter()
        .cloned()
        .collect();

        for cap in re.find_iter(query) {
            let word = cap.as_str();
            let lower = word.to_ascii_lowercase();
            if !stop_words.contains(lower.as_str()) && !tokens.contains(&word.to_string()) {
                tokens.push(word.to_string());
            }
        }
        tokens
    }

    fn collect_identifier_ranges(
        node: tree_sitter::Node,
        content: &str,
        target_name: &str,
        ranges: &mut Vec<(usize, usize)>,
    ) {
        let kind = node.kind();
        let is_ident_kind = matches!(
            kind,
            "identifier"
                | "type_identifier"
                | "field_identifier"
                | "property_identifier"
                | "shorthand_property_identifier_pattern"
                | "variable_name"
        );

        if is_ident_kind {
            if let Ok(text) = node.utf8_text(content.as_bytes()) {
                if text == target_name {
                    ranges.push((node.start_byte(), node.end_byte()));
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::collect_identifier_ranges(child, content, target_name, ranges);
        }
    }

    fn walk_and_collect_symbols(
        &self,
        dir: &Path,
        symbol_name: &str,
        results: &mut Vec<SymbolRecord>,
    ) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Ok(ft) = entry.file_type() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with('.')
                        || name == "target"
                        || name == "node_modules"
                        || name == "vendor"
                        || name == "dist"
                    {
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
                if name.starts_with('.')
                    || name == "target"
                    || name == "node_modules"
                    || name == "vendor"
                    || name == "dist"
                {
                    continue;
                }
                if let Ok(ft) = entry.file_type() {
                    if ft.is_dir() {
                        self.walk_source_files(&p, filter, list);
                    } else if ft.is_file() && AstLanguage::from_path(&p).is_some() {
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

    // --- Language AST Walkers ---

    fn is_rust_node_public(node: tree_sitter::Node, bytes: &[u8]) -> bool {
        if let Some(v) = node.child_by_field_name("visibility") {
            if let Ok(t) = v.utf8_text(bytes) {
                if t.contains("pub") {
                    return true;
                }
            }
        }
        let mut cur = node.walk();
        for child in node.children(&mut cur) {
            if child.kind() == "visibility_modifier" {
                if let Ok(t) = child.utf8_text(bytes) {
                    if t.contains("pub") {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn is_js_ts_node_public(node: tree_sitter::Node) -> bool {
        let mut curr = Some(node);
        while let Some(n) = curr {
            if n.kind() == "export_statement" {
                return true;
            }
            curr = n.parent();
        }
        false
    }

    fn walk_rust_ast(
        node: tree_sitter::Node,
        content: &str,
        rel_path: &str,
        container: Option<&str>,
        symbols: &mut Vec<SymbolRecord>,
        callsites: &mut Vec<CallSiteRecord>,
        imports: &mut Vec<ImportRecord>,
    ) {
        let kind = node.kind();
        let bytes = content.as_bytes();

        match kind {
            "function_item" => {
                let name_node = node.child_by_field_name("name");
                let is_pub = Self::is_rust_node_public(node, bytes);

                if let Some(n) = name_node {
                    let name = n.utf8_text(bytes).unwrap_or("").to_string();
                    let line_number = n.start_position().row + 1;
                    let sig = Self::node_first_line(node, content);

                    symbols.push(SymbolRecord {
                        name: name.clone(),
                        kind: if container.is_some() {
                            SymbolKind::Method.as_str().to_string()
                        } else {
                            SymbolKind::Function.as_str().to_string()
                        },
                        line_number,
                        signature: sig,
                        is_public: is_pub,
                        file_path: rel_path.to_string(),
                        container: container.map(|s| s.to_string()),
                    });

                    // Walk body with this function as container for callsites
                    if let Some(body) = node.child_by_field_name("body") {
                        Self::walk_rust_ast(
                            body,
                            content,
                            rel_path,
                            Some(&name),
                            symbols,
                            callsites,
                            imports,
                        );
                    }
                    return;
                }
            }
            "struct_item" => {
                let name_node = node.child_by_field_name("name");
                let is_pub = Self::is_rust_node_public(node, bytes);

                if let Some(n) = name_node {
                    let name = n.utf8_text(bytes).unwrap_or("").to_string();
                    let line_number = n.start_position().row + 1;
                    let sig = Self::node_first_line(node, content);

                    symbols.push(SymbolRecord {
                        name,
                        kind: SymbolKind::Struct.as_str().to_string(),
                        line_number,
                        signature: sig,
                        is_public: is_pub,
                        file_path: rel_path.to_string(),
                        container: None,
                    });
                }
            }
            "enum_item" => {
                let name_node = node.child_by_field_name("name");
                let is_pub = Self::is_rust_node_public(node, bytes);

                if let Some(n) = name_node {
                    let name = n.utf8_text(bytes).unwrap_or("").to_string();
                    let line_number = n.start_position().row + 1;
                    let sig = Self::node_first_line(node, content);

                    symbols.push(SymbolRecord {
                        name,
                        kind: SymbolKind::Enum.as_str().to_string(),
                        line_number,
                        signature: sig,
                        is_public: is_pub,
                        file_path: rel_path.to_string(),
                        container: None,
                    });
                }
            }
            "trait_item" => {
                let name_node = node.child_by_field_name("name");
                let is_pub = Self::is_rust_node_public(node, bytes);

                if let Some(n) = name_node {
                    let name = n.utf8_text(bytes).unwrap_or("").to_string();
                    let line_number = n.start_position().row + 1;
                    let sig = Self::node_first_line(node, content);

                    symbols.push(SymbolRecord {
                        name,
                        kind: SymbolKind::Trait.as_str().to_string(),
                        line_number,
                        signature: sig,
                        is_public: is_pub,
                        file_path: rel_path.to_string(),
                        container: None,
                    });
                }
            }
            "impl_item" => {
                let type_name = node
                    .child_by_field_name("type")
                    .and_then(|t| t.utf8_text(bytes).ok())
                    .unwrap_or("Impl");
                let trait_name = node
                    .child_by_field_name("trait")
                    .and_then(|t| t.utf8_text(bytes).ok());

                let container_label = if let Some(tr) = trait_name {
                    format!("impl {} for {}", tr, type_name)
                } else {
                    format!("impl {}", type_name)
                };

                if let Some(body) = node.child_by_field_name("body") {
                    Self::walk_rust_ast(
                        body,
                        content,
                        rel_path,
                        Some(&container_label),
                        symbols,
                        callsites,
                        imports,
                    );
                    return;
                }
            }
            "type_item" => {
                let name_node = node.child_by_field_name("name");
                let is_pub = Self::is_rust_node_public(node, bytes);

                if let Some(n) = name_node {
                    let name = n.utf8_text(bytes).unwrap_or("").to_string();
                    let line_number = n.start_position().row + 1;
                    let sig = Self::node_first_line(node, content);

                    symbols.push(SymbolRecord {
                        name,
                        kind: SymbolKind::TypeAlias.as_str().to_string(),
                        line_number,
                        signature: sig,
                        is_public: is_pub,
                        file_path: rel_path.to_string(),
                        container: None,
                    });
                }
            }
            "const_item" | "static_item" => {
                let name_node = node.child_by_field_name("name");
                let is_pub = Self::is_rust_node_public(node, bytes);

                if let Some(n) = name_node {
                    let name = n.utf8_text(bytes).unwrap_or("").to_string();
                    let line_number = n.start_position().row + 1;
                    let sig = Self::node_first_line(node, content);

                    symbols.push(SymbolRecord {
                        name,
                        kind: SymbolKind::Constant.as_str().to_string(),
                        line_number,
                        signature: sig,
                        is_public: is_pub,
                        file_path: rel_path.to_string(),
                        container: None,
                    });
                }
            }
            "use_declaration" => {
                let line_number = node.start_position().row + 1;
                let text = node.utf8_text(bytes).unwrap_or("");
                let clean_text = text.trim_start_matches("use ").trim_end_matches(';').trim();
                let imported = vec![clean_text.to_string()];
                imports.push(ImportRecord {
                    source: clean_text.to_string(),
                    imported_names: imported,
                    line_number,
                    file_path: rel_path.to_string(),
                });
            }
            "call_expression" => {
                if let Some(fn_node) = node.child_by_field_name("function") {
                    let callee = fn_node.utf8_text(bytes).unwrap_or("").trim().to_string();
                    let line_number = node.start_position().row + 1;
                    if !callee.is_empty() {
                        callsites.push(CallSiteRecord {
                            caller: container.map(|s| s.to_string()),
                            callee,
                            line_number,
                            file_path: rel_path.to_string(),
                        });
                    }
                }
            }
            _ => {}
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_rust_ast(
                child, content, rel_path, container, symbols, callsites, imports,
            );
        }
    }

    fn walk_python_ast(
        node: tree_sitter::Node,
        content: &str,
        rel_path: &str,
        container: Option<&str>,
        symbols: &mut Vec<SymbolRecord>,
        callsites: &mut Vec<CallSiteRecord>,
        imports: &mut Vec<ImportRecord>,
    ) {
        let kind = node.kind();
        let bytes = content.as_bytes();

        match kind {
            "function_definition" => {
                if let Some(n) = node.child_by_field_name("name") {
                    let name = n.utf8_text(bytes).unwrap_or("").to_string();
                    let line_number = n.start_position().row + 1;
                    let sig = Self::node_first_line(node, content);
                    let is_pub = !name.starts_with('_');

                    symbols.push(SymbolRecord {
                        name: name.clone(),
                        kind: if container.is_some() {
                            SymbolKind::Method.as_str().to_string()
                        } else {
                            SymbolKind::Function.as_str().to_string()
                        },
                        line_number,
                        signature: sig,
                        is_public: is_pub,
                        file_path: rel_path.to_string(),
                        container: container.map(|s| s.to_string()),
                    });

                    if let Some(body) = node.child_by_field_name("body") {
                        Self::walk_python_ast(
                            body,
                            content,
                            rel_path,
                            Some(&name),
                            symbols,
                            callsites,
                            imports,
                        );
                    }
                    return;
                }
            }
            "class_definition" => {
                if let Some(n) = node.child_by_field_name("name") {
                    let name = n.utf8_text(bytes).unwrap_or("").to_string();
                    let line_number = n.start_position().row + 1;
                    let sig = Self::node_first_line(node, content);
                    let is_pub = !name.starts_with('_');

                    symbols.push(SymbolRecord {
                        name: name.clone(),
                        kind: SymbolKind::Class.as_str().to_string(),
                        line_number,
                        signature: sig,
                        is_public: is_pub,
                        file_path: rel_path.to_string(),
                        container: container.map(|s| s.to_string()),
                    });

                    if let Some(body) = node.child_by_field_name("body") {
                        Self::walk_python_ast(
                            body,
                            content,
                            rel_path,
                            Some(&name),
                            symbols,
                            callsites,
                            imports,
                        );
                    }
                    return;
                }
            }
            "import_statement" | "import_from_statement" => {
                let line_number = node.start_position().row + 1;
                let text = node.utf8_text(bytes).unwrap_or("").trim().to_string();
                imports.push(ImportRecord {
                    source: text.clone(),
                    imported_names: vec![text],
                    line_number,
                    file_path: rel_path.to_string(),
                });
            }
            "call" => {
                if let Some(fn_node) = node.child_by_field_name("function") {
                    let callee = fn_node.utf8_text(bytes).unwrap_or("").trim().to_string();
                    let line_number = node.start_position().row + 1;
                    if !callee.is_empty() {
                        callsites.push(CallSiteRecord {
                            caller: container.map(|s| s.to_string()),
                            callee,
                            line_number,
                            file_path: rel_path.to_string(),
                        });
                    }
                }
            }
            _ => {}
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_python_ast(
                child, content, rel_path, container, symbols, callsites, imports,
            );
        }
    }

    fn walk_js_ts_ast(
        node: tree_sitter::Node,
        content: &str,
        rel_path: &str,
        container: Option<&str>,
        symbols: &mut Vec<SymbolRecord>,
        callsites: &mut Vec<CallSiteRecord>,
        imports: &mut Vec<ImportRecord>,
    ) {
        let kind = node.kind();
        let bytes = content.as_bytes();

        let is_exported = Self::is_js_ts_node_public(node);

        match kind {
            "function_declaration" | "method_definition" => {
                if let Some(n) = node.child_by_field_name("name") {
                    let name = n.utf8_text(bytes).unwrap_or("").to_string();
                    let line_number = n.start_position().row + 1;
                    let sig = Self::node_first_line(node, content);

                    symbols.push(SymbolRecord {
                        name: name.clone(),
                        kind: if container.is_some() {
                            SymbolKind::Method.as_str().to_string()
                        } else {
                            SymbolKind::Function.as_str().to_string()
                        },
                        line_number,
                        signature: sig,
                        is_public: is_exported || !name.starts_with('_'),
                        file_path: rel_path.to_string(),
                        container: container.map(|s| s.to_string()),
                    });

                    if let Some(body) = node.child_by_field_name("body") {
                        Self::walk_js_ts_ast(
                            body,
                            content,
                            rel_path,
                            Some(&name),
                            symbols,
                            callsites,
                            imports,
                        );
                    }
                    return;
                }
            }
            "class_declaration" => {
                if let Some(n) = node.child_by_field_name("name") {
                    let name = n.utf8_text(bytes).unwrap_or("").to_string();
                    let line_number = n.start_position().row + 1;
                    let sig = Self::node_first_line(node, content);

                    symbols.push(SymbolRecord {
                        name: name.clone(),
                        kind: SymbolKind::Class.as_str().to_string(),
                        line_number,
                        signature: sig,
                        is_public: is_exported,
                        file_path: rel_path.to_string(),
                        container: container.map(|s| s.to_string()),
                    });

                    if let Some(body) = node.child_by_field_name("body") {
                        Self::walk_js_ts_ast(
                            body,
                            content,
                            rel_path,
                            Some(&name),
                            symbols,
                            callsites,
                            imports,
                        );
                    }
                    return;
                }
            }
            "interface_declaration" => {
                if let Some(n) = node.child_by_field_name("name") {
                    let name = n.utf8_text(bytes).unwrap_or("").to_string();
                    let line_number = n.start_position().row + 1;
                    let sig = Self::node_first_line(node, content);

                    symbols.push(SymbolRecord {
                        name,
                        kind: SymbolKind::Interface.as_str().to_string(),
                        line_number,
                        signature: sig,
                        is_public: is_exported,
                        file_path: rel_path.to_string(),
                        container: None,
                    });
                }
            }
            "type_alias_declaration" => {
                if let Some(n) = node.child_by_field_name("name") {
                    let name = n.utf8_text(bytes).unwrap_or("").to_string();
                    let line_number = n.start_position().row + 1;
                    let sig = Self::node_first_line(node, content);

                    symbols.push(SymbolRecord {
                        name,
                        kind: SymbolKind::TypeAlias.as_str().to_string(),
                        line_number,
                        signature: sig,
                        is_public: is_exported,
                        file_path: rel_path.to_string(),
                        container: None,
                    });
                }
            }
            "enum_declaration" => {
                if let Some(n) = node.child_by_field_name("name") {
                    let name = n.utf8_text(bytes).unwrap_or("").to_string();
                    let line_number = n.start_position().row + 1;
                    let sig = Self::node_first_line(node, content);

                    symbols.push(SymbolRecord {
                        name,
                        kind: SymbolKind::Enum.as_str().to_string(),
                        line_number,
                        signature: sig,
                        is_public: is_exported,
                        file_path: rel_path.to_string(),
                        container: None,
                    });
                }
            }
            "import_statement" => {
                let line_number = node.start_position().row + 1;
                let text = node.utf8_text(bytes).unwrap_or("").trim().to_string();
                let source = node
                    .child_by_field_name("source")
                    .and_then(|s| s.utf8_text(bytes).ok())
                    .unwrap_or("")
                    .trim_matches('\'')
                    .trim_matches('"')
                    .to_string();

                imports.push(ImportRecord {
                    source: if source.is_empty() {
                        text.clone()
                    } else {
                        source
                    },
                    imported_names: vec![text],
                    line_number,
                    file_path: rel_path.to_string(),
                });
            }
            "call_expression" => {
                if let Some(fn_node) = node.child_by_field_name("function") {
                    let callee = fn_node.utf8_text(bytes).unwrap_or("").trim().to_string();
                    let line_number = node.start_position().row + 1;
                    if !callee.is_empty() {
                        callsites.push(CallSiteRecord {
                            caller: container.map(|s| s.to_string()),
                            callee,
                            line_number,
                            file_path: rel_path.to_string(),
                        });
                    }
                }
            }
            _ => {}
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_js_ts_ast(
                child, content, rel_path, container, symbols, callsites, imports,
            );
        }
    }

    fn walk_go_ast(
        node: tree_sitter::Node,
        content: &str,
        rel_path: &str,
        container: Option<&str>,
        symbols: &mut Vec<SymbolRecord>,
        callsites: &mut Vec<CallSiteRecord>,
        imports: &mut Vec<ImportRecord>,
    ) {
        let kind = node.kind();
        let bytes = content.as_bytes();

        match kind {
            "function_declaration" => {
                if let Some(n) = node.child_by_field_name("name") {
                    let name = n.utf8_text(bytes).unwrap_or("").to_string();
                    let line_number = n.start_position().row + 1;
                    let sig = Self::node_first_line(node, content);
                    let is_pub = name
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false);

                    symbols.push(SymbolRecord {
                        name: name.clone(),
                        kind: SymbolKind::Function.as_str().to_string(),
                        line_number,
                        signature: sig,
                        is_public: is_pub,
                        file_path: rel_path.to_string(),
                        container: None,
                    });

                    if let Some(body) = node.child_by_field_name("body") {
                        Self::walk_go_ast(
                            body,
                            content,
                            rel_path,
                            Some(&name),
                            symbols,
                            callsites,
                            imports,
                        );
                    }
                    return;
                }
            }
            "method_declaration" => {
                let receiver = node
                    .child_by_field_name("receiver")
                    .and_then(|r| r.utf8_text(bytes).ok())
                    .unwrap_or("()");

                if let Some(n) = node.child_by_field_name("name") {
                    let name = n.utf8_text(bytes).unwrap_or("").to_string();
                    let line_number = n.start_position().row + 1;
                    let sig = Self::node_first_line(node, content);
                    let is_pub = name
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false);

                    symbols.push(SymbolRecord {
                        name: name.clone(),
                        kind: SymbolKind::Method.as_str().to_string(),
                        line_number,
                        signature: sig,
                        is_public: is_pub,
                        file_path: rel_path.to_string(),
                        container: Some(receiver.to_string()),
                    });

                    if let Some(body) = node.child_by_field_name("body") {
                        Self::walk_go_ast(
                            body,
                            content,
                            rel_path,
                            Some(&name),
                            symbols,
                            callsites,
                            imports,
                        );
                    }
                    return;
                }
            }
            "type_spec" => {
                if let Some(n) = node.child_by_field_name("name") {
                    let name = n.utf8_text(bytes).unwrap_or("").to_string();
                    let line_number = n.start_position().row + 1;
                    let sig = Self::node_first_line(node, content);
                    let is_pub = name
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false);

                    let type_kind = node
                        .child_by_field_name("type")
                        .map(|t| match t.kind() {
                            "struct_type" => SymbolKind::Struct.as_str(),
                            "interface_type" => SymbolKind::Interface.as_str(),
                            _ => SymbolKind::TypeAlias.as_str(),
                        })
                        .unwrap_or("type");

                    symbols.push(SymbolRecord {
                        name,
                        kind: type_kind.to_string(),
                        line_number,
                        signature: sig,
                        is_public: is_pub,
                        file_path: rel_path.to_string(),
                        container: None,
                    });
                }
            }
            "import_declaration" => {
                let line_number = node.start_position().row + 1;
                let text = node.utf8_text(bytes).unwrap_or("").trim().to_string();
                imports.push(ImportRecord {
                    source: text.clone(),
                    imported_names: vec![text],
                    line_number,
                    file_path: rel_path.to_string(),
                });
            }
            "call_expression" => {
                if let Some(fn_node) = node.child_by_field_name("function") {
                    let callee = fn_node.utf8_text(bytes).unwrap_or("").trim().to_string();
                    let line_number = node.start_position().row + 1;
                    if !callee.is_empty() {
                        callsites.push(CallSiteRecord {
                            caller: container.map(|s| s.to_string()),
                            callee,
                            line_number,
                            file_path: rel_path.to_string(),
                        });
                    }
                }
            }
            _ => {}
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_go_ast(
                child, content, rel_path, container, symbols, callsites, imports,
            );
        }
    }

    fn node_first_line(node: tree_sitter::Node, content: &str) -> String {
        let start = node.start_byte();
        let end = node.end_byte();
        if start < content.len() {
            let slice = &content[start..end.min(content.len())];
            slice.lines().next().unwrap_or("").trim().to_string()
        } else {
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rust_symbols_ast() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("agent.rs");
        let code = r#"
use std::path::Path;
use crate::tools::registry::ToolRegistry;

pub struct EngineContext {
    pub id: String,
}

pub fn initialize_agent(name: &str) -> bool {
    let reg = ToolRegistry::new();
    true
}

enum PriorityState {
    High,
}
"#;
        fs::write(&file_path, code).unwrap();

        let engine = SymbolEngine::new(temp.path());
        let analysis = engine.analyze_file(&file_path).unwrap();

        assert_eq!(analysis.language, "Rust");
        assert_eq!(analysis.imports.len(), 2);
        assert_eq!(analysis.symbols.len(), 3);

        assert_eq!(analysis.symbols[0].name, "EngineContext");
        assert_eq!(analysis.symbols[0].kind, "struct");
        assert!(analysis.symbols[0].is_public);

        assert_eq!(analysis.symbols[1].name, "initialize_agent");
        assert_eq!(analysis.symbols[1].kind, "function");
        assert!(analysis.symbols[1].is_public);

        assert_eq!(analysis.symbols[2].name, "PriorityState");
        assert_eq!(analysis.symbols[2].kind, "enum");
        assert!(!analysis.symbols[2].is_public);

        assert!(!analysis.callsites.is_empty());
        assert!(analysis
            .callsites
            .iter()
            .any(|c| c.callee.contains("ToolRegistry::new")));
    }

    #[test]
    fn test_parse_ts_symbols_ast() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("api.ts");
        let code = r#"
import { AxiosResponse } from 'axios';

export interface UserPayload {
    id: string;
}

export function fetchUserData(id: string) {
    console.log(id);
    return id;
}

class LocalBuffer {}
"#;
        fs::write(&file_path, code).unwrap();

        let engine = SymbolEngine::new(temp.path());
        let analysis = engine.analyze_file(&file_path).unwrap();

        assert_eq!(analysis.language, "TypeScript");
        assert_eq!(analysis.imports.len(), 1);
        assert_eq!(analysis.symbols.len(), 3);

        assert_eq!(analysis.symbols[0].name, "UserPayload");
        assert_eq!(analysis.symbols[0].kind, "interface");
        assert!(analysis.symbols[0].is_public);

        assert_eq!(analysis.symbols[1].name, "fetchUserData");
        assert_eq!(analysis.symbols[1].kind, "function");
        assert!(analysis.symbols[1].is_public);

        assert_eq!(analysis.symbols[2].name, "LocalBuffer");
        assert_eq!(analysis.symbols[2].kind, "class");
        assert!(!analysis.symbols[2].is_public);

        assert!(analysis.callsites.iter().any(|c| c.callee == "console.log"));
    }

    #[test]
    fn test_ast_repository_map_generation() {
        let temp = tempfile::tempdir().unwrap();
        let file_rs = temp.path().join("main.rs");
        let code_rs = "pub fn execute_flow() {}\n";
        fs::write(&file_rs, code_rs).unwrap();

        let file_py = temp.path().join("script.py");
        let code_py = "def run_task():\n    pass\n";
        fs::write(&file_py, code_py).unwrap();

        let engine = SymbolEngine::new(temp.path());
        let map = engine.generate_repo_map(None).unwrap();

        assert!(map.contains("[AST REPOSITORY MAP:"));
        assert!(map.contains("main.rs (Rust"));
        assert!(map.contains("execute_flow"));
        assert!(map.contains("script.py (Python"));
        assert!(map.contains("run_task"));
    }

    #[test]
    fn test_ast_safe_symbol_rename() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("test.rs");
        let code = r#"
fn calculate_score(score: i32) -> i32 {
    let msg = "calculate_score should not change here"; // comment about calculate_score
    score * 2
}
"#;
        fs::write(&file_path, code).unwrap();

        let engine = SymbolEngine::new(temp.path());
        let modified = engine
            .rename_symbol("calculate_score", "compute_rating", None)
            .unwrap();
        assert_eq!(modified.len(), 1);

        let updated = fs::read_to_string(&file_path).unwrap();
        assert!(updated.contains("fn compute_rating(score: i32) -> i32"));
        // String literal and comment must remain untouched by AST renaming!
        assert!(updated.contains("\"calculate_score should not change here\""));
        assert!(updated.contains("// comment about calculate_score"));
    }

    #[test]
    fn test_cached_ast_entry_hit_and_invalidation() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("service.rs");
        let code_v1 = r#"
pub struct ServiceContext {
    pub id: String,
}

pub fn handle_request() -> bool {
    true
}
"#;
        fs::write(&file_path, code_v1).unwrap();

        let engine = SymbolEngine::new(temp.path());

        // 1. Initial parse -> Cache miss
        let analysis1 = engine.analyze_file(&file_path).unwrap();
        assert_eq!(analysis1.symbols.len(), 2);
        let stats1 = engine.get_cache_stats();
        assert_eq!(stats1.misses, 1);
        assert_eq!(stats1.hits, 0);
        assert_eq!(stats1.cached_files, 1);

        // 2. Second parse -> Immediate sub-millisecond cache hit
        let analysis2 = engine.analyze_file(&file_path).unwrap();
        assert_eq!(analysis2.symbols.len(), 2);
        let stats2 = engine.get_cache_stats();
        assert_eq!(stats2.hits, 1);
        assert_eq!(stats2.misses, 1);

        // 3. Modify file content -> Cache invalidates and re-parses
        let code_v2 = r#"
pub struct ServiceContext {
    pub id: String,
}

pub fn handle_request() -> bool {
    true
}

pub fn terminate_session() {}
"#;
        fs::write(&file_path, code_v2).unwrap();

        let analysis3 = engine.analyze_file(&file_path).unwrap();
        assert_eq!(analysis3.symbols.len(), 3);
        assert!(analysis3.symbols.iter().any(|s| s.name == "terminate_session"));
        let stats3 = engine.get_cache_stats();
        assert_eq!(stats3.misses, 2);
        assert_eq!(stats3.hits, 1);
    }

    #[test]
    fn test_merkle_tree_computation() {
        let temp = tempfile::tempdir().unwrap();
        let src_dir = temp.path().join("src");
        let tools_dir = src_dir.join("tools");
        fs::create_dir_all(&tools_dir).unwrap();

        let file_a = src_dir.join("main.rs");
        let file_b = tools_dir.join("engine.rs");
        fs::write(&file_a, "fn main() {}\n").unwrap();
        fs::write(&file_b, "pub struct Engine;\n").unwrap();

        let engine = SymbolEngine::new(temp.path());
        let root_1 = engine.compute_merkle_root();
        assert!(!root_1.is_empty());

        // Identical state yields identical Merkle root
        let root_1_repeat = engine.compute_merkle_root();
        assert_eq!(root_1, root_1_repeat);

        // Modify one file -> Merkle root changes deterministically
        fs::write(&file_b, "pub struct Engine;\npub fn run() {}\n").unwrap();
        let root_2 = engine.compute_merkle_root();
        assert_ne!(root_1, root_2);

        // Revert modification -> Merkle root reverts back
        fs::write(&file_b, "pub struct Engine;\n").unwrap();
        let root_reverted = engine.compute_merkle_root();
        assert_eq!(root_1, root_reverted);
    }

    #[test]
    fn test_ambient_symbol_resolution() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("pipeline.rs");
        let code = r#"
pub struct DataPipeline;

impl DataPipeline {
    pub fn execute_batch_flow() {}
}

pub fn transform_stream() {}
"#;
        fs::write(&file_path, code).unwrap();

        let engine = SymbolEngine::new(temp.path());
        engine.warm_cache(None);

        let query = "Can you help optimize execute_batch_flow and transform_stream in the codebase?";
        let resolved = engine.resolve_ambient_symbols(query, 5);

        assert!(!resolved.is_empty());
        let names: Vec<&str> = resolved.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"execute_batch_flow"));
        assert!(names.contains(&"transform_stream"));
    }
}
