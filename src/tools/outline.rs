//! Atlas Daedalus — Structural File Outline Tool (Tree-sitter AST)
//! (C) 2026 Atlas Research Labs — MIT License

use std::fs;
use std::path::{Path, PathBuf};

use crate::tools::symbols::SymbolEngine;

pub struct OutlineTools {
    workspace_root: PathBuf,
    symbols: SymbolEngine,
}

impl OutlineTools {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        let p = workspace_root.as_ref().to_path_buf();
        Self {
            symbols: SymbolEngine::new(&p),
            workspace_root: p,
        }
    }

    /// Generate a compact structural AST outline of a file
    pub fn file_outline(&self, rel_path: &str) -> Result<String, String> {
        let full_path = self.workspace_root.join(rel_path);
        if !full_path.exists() {
            return Err(format!("File not found: {}", rel_path));
        }

        let content = fs::read_to_string(&full_path)
            .map_err(|e| format!("Failed to read file {}: {}", rel_path, e))?;

        let analysis = self.symbols.analyze_file(&full_path)?;
        let symbols = analysis.symbols;

        if symbols.is_empty() {
            // Fallback for markdown or simple files: extract markdown headings or lines summary
            let mut headings = Vec::new();
            for (idx, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') {
                    headings.push(format!("  L{:4} │ {}", idx + 1, trimmed));
                }
            }
            if !headings.is_empty() {
                return Ok(format!(
                    "OUTLINE: {} ({} lines, {} headings)\n{}",
                    rel_path,
                    content.lines().count(),
                    headings.len(),
                    headings.join("\n")
                ));
            }
            return Ok(format!(
                "OUTLINE: {} ({} lines, no structural code symbols identified)",
                rel_path,
                content.lines().count()
            ));
        }

        let mut out = format!(
            "OUTLINE: {} ({} lines, {} structural symbols, language: {})\n",
            rel_path,
            content.lines().count(),
            symbols.len(),
            analysis.language
        );

        for sym in &symbols {
            let kind_tag = format!("[{}]", sym.kind);
            out.push_str(&format!(
                "  L{:4} │ {:<12} {}\n",
                sym.line_number, kind_tag, sym.name
            ));
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_outline_rust_source() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src_file = dir.path().join("lib.rs");
        fs::write(
            &src_file,
            "pub struct Worker {\n    id: u32,\n}\n\nimpl Worker {\n    pub fn run(&self) {}\n}\n",
        )
        .expect("write");

        let tools = OutlineTools::new(dir.path());
        let outline = tools.file_outline("lib.rs").expect("outline");
        assert!(outline.contains("Worker"));
        assert!(outline.contains("run"));
    }
}
