//! Atlas Daedalus — Context SpillGuard Engine (Kiri Architecture)
//! Prevents oversized tool outputs (shell logs, file reads, directory scans, web pages)
//! from overflowing the LLM context window. Outputs exceeding max_chars (default: 12,000)
//! are written to a unique local spill file and replaced with a preview containing head + tail snippets.

use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_MAX_CHARS: usize = 12_000;
pub const DEFAULT_HEAD_CHARS: usize = 600;
pub const DEFAULT_TAIL_CHARS: usize = 900;

pub struct SpillGuard;

impl SpillGuard {
    pub fn spill_if_oversized<P: AsRef<Path>>(
        workspace: P,
        source: &str,
        text: &str,
    ) -> (String, bool, Option<PathBuf>) {
        Self::spill_with_limits(
            workspace,
            source,
            text,
            DEFAULT_MAX_CHARS,
            DEFAULT_HEAD_CHARS,
            DEFAULT_TAIL_CHARS,
        )
    }

    pub fn spill_with_limits<P: AsRef<Path>>(
        workspace: P,
        source: &str,
        text: &str,
        max_chars: usize,
        head_chars: usize,
        tail_chars: usize,
    ) -> (String, bool, Option<PathBuf>) {
        if text.len() <= max_chars {
            return (text.to_string(), false, None);
        }

        let spill_dir = workspace.as_ref().join(".daedalus").join("spill");
        let _ = fs::create_dir_all(&spill_dir);

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let safe_source = source
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .take(24)
            .collect::<String>();
        let file_name = format!("{}_{}_{:04x}.log", safe_source, timestamp, rand_suffix());
        let spill_file = spill_dir.join(&file_name);

        let _ = fs::write(&spill_file, text);

        let head = if text.len() > head_chars {
            let mut end = head_chars;
            while !text.is_char_boundary(end) && end > 0 {
                end -= 1;
            }
            &text[..end]
        } else {
            text
        };

        let tail = if text.len() > tail_chars {
            let start = text.len() - tail_chars;
            let mut char_start = start;
            while !text.is_char_boundary(char_start) && char_start < text.len() {
                char_start += 1;
            }
            &text[char_start..]
        } else {
            text
        };

        let preview = format!(
            "{}\n\n\
            [... {} CHARACTERS SPILLED TO DISK ...]\n\n\
            {}\n\n\
            [SPILL OVERFLOW DETECTED: Total {} characters truncated to protect LLM context window. Full unedited log preserved at: {}. Use view_file to inspect line ranges if necessary.]",
            head,
            text.len().saturating_sub(head_chars + tail_chars),
            tail,
            text.len(),
            spill_file.display()
        );

        (preview, true, Some(spill_file))
    }
}

fn rand_suffix() -> u16 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_nanos() & 0xFFFF) as u16)
        .unwrap_or(0x4242)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_spill_small_text() {
        let (out, spilled, path) =
            SpillGuard::spill_with_limits(Path::new("."), "test", "small text", 100, 10, 10);
        assert!(!spilled);
        assert_eq!(out, "small text");
        assert!(path.is_none());
    }

    #[test]
    fn test_spill_large_text() {
        let large = "A".repeat(500);
        let (out, spilled, path) =
            SpillGuard::spill_with_limits(Path::new("target"), "test_large", &large, 50, 10, 10);
        assert!(spilled);
        assert!(path.is_some());
        assert!(out.contains("SPILL OVERFLOW DETECTED"));
    }
}
