//! Atlas Daedalus — Semantic AST Symbol & Repository Map Engine (Tree-sitter Architecture)
//! Parses symbols (functions, structs, traits, classes, interfaces, enums, callsites, imports)
//! across Rust, TypeScript, JavaScript, Python, and Go for structural navigation,
//! AST repository map generation, and AST-precise cross-file renaming.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

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

pub struct SymbolEngine {
    pub workspace_root: PathBuf,
}

impl SymbolEngine {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
    }

    /// Extracts all symbol definitions from a specific source file using Tree-sitter AST
    pub fn extract_symbols_from_file(&self, file_path: &Path) -> Result<Vec<SymbolRecord>, String> {
        let analysis = self.analyze_file(file_path)?;
        Ok(analysis.symbols)
    }

    /// Fully analyzes a source file extracting symbols, callsites, and imports via AST
    pub fn analyze_file(&self, file_path: &Path) -> Result<AstFileAnalysis, String> {
        if !file_path.exists() {
            return Err(format!("File '{}' does not exist.", file_path.display()));
        }

        let content = fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read file '{}': {}", file_path.display(), e))?;

        let rel_path = file_path
            .strip_prefix(&self.workspace_root)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        let ast_lang = AstLanguage::from_path(file_path);
        match ast_lang {
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

                Ok(AstFileAnalysis {
                    file_path: rel_path,
                    language: lang.name().to_string(),
                    symbols,
                    callsites,
                    imports,
                })
            }
            None => Ok(AstFileAnalysis {
                file_path: rel_path,
                language: "Unsupported".to_string(),
                symbols: Vec::new(),
                callsites: Vec::new(),
                imports: Vec::new(),
            }),
        }
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
}
