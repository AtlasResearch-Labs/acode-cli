//! Atlas Daedalus — Viewport Geometry & ANSI-Aware Layout Engine
//! (C) 2026 Atlas Research Labs — MIT License

use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutTier {
    /// Phone screen in Termux or narrow vertical split pane (< 60 columns)
    Compact,
    /// Standard laptop screen or 80-column terminal (60..=99 columns)
    Standard,
    /// Maximized desktop terminal or wide monitor (>= 100 columns)
    Wide,
}

#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub cols: usize,
    pub rows: usize,
    pub plain_ascii: bool,
}

impl Viewport {
    /// Query current terminal dimensions, falling back gracefully to environment variables or standard 80x24
    pub fn current() -> Self {
        let (cols, rows) = match crossterm::terminal::size() {
            Ok((w, h)) if w > 0 && h > 0 => (w as usize, h as usize),
            _ => {
                let c = env::var("COLUMNS")
                    .ok()
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(80);
                let r = env::var("LINES")
                    .ok()
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(24);
                (c, r)
            }
        };

        let plain_ascii = env::var("ACODE_PLAIN_TUI")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or_else(|_| {
                env::var("ACODE_NO_UNICODE")
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false)
            });

        Self {
            cols,
            rows,
            plain_ascii,
        }
    }

    /// Construct a specific viewport instance for deterministic testing
    pub fn explicit(cols: usize, rows: usize, plain_ascii: bool) -> Self {
        Self {
            cols,
            rows,
            plain_ascii,
        }
    }

    /// Classify active screen tier
    pub fn tier(&self) -> LayoutTier {
        if self.cols < 60 {
            LayoutTier::Compact
        } else if self.cols < 100 {
            LayoutTier::Standard
        } else {
            LayoutTier::Wide
        }
    }

    /// Compute safe card border width clamped between minimum readable bounds and a specified maximum
    pub fn card_width(&self, max_allowed: usize) -> usize {
        let available = self.cols.saturating_sub(2);
        available.clamp(32, max_allowed)
    }

    pub fn is_compact(&self) -> bool {
        self.tier() == LayoutTier::Compact
    }

    pub fn is_wide(&self) -> bool {
        self.tier() == LayoutTier::Wide
    }
}

/// Calculate visible printable column width of a string by stripping ANSI escape sequences
pub fn visible_width(s: &str) -> usize {
    let mut width = 0;
    let mut in_escape = false;

    for ch in s.chars() {
        if in_escape {
            if ch.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else if ch == '\x1b' {
            in_escape = true;
        } else {
            width += 1;
        }
    }

    width
}

/// Truncate text cleanly based on visible width without splitting multibyte chars or bleeding active ANSI colors
pub fn ansi_safe_truncate(s: &str, max_width: usize) -> String {
    if visible_width(s) <= max_width {
        return s.to_string();
    }

    let mut result = String::with_capacity(s.len());
    let mut cur_width = 0;
    let mut in_escape = false;
    let mut active_escape = false;

    for ch in s.chars() {
        if in_escape {
            result.push(ch);
            if ch.is_ascii_alphabetic() {
                in_escape = false;
            }
            continue;
        }

        if ch == '\x1b' {
            in_escape = true;
            active_escape = true;
            result.push(ch);
            continue;
        }

        if cur_width + 1 > max_width {
            break;
        }

        result.push(ch);
        cur_width += 1;
    }

    if active_escape && !result.ends_with("\x1b[0m") {
        result.push_str("\x1b[0m");
    }

    result
}

/// Format a box content row `│  content  ...  │` with exact padding matching visible column width
pub fn pad_box_line(
    content: &str,
    target_width: usize,
    border_color: &str,
    border_char: &str,
    reset_color: &str,
) -> String {
    let vis_len = visible_width(content);
    // target_width includes both vertical borders (e.g. `│ content │`)
    let inner_width = target_width.saturating_sub(2);

    if vis_len >= inner_width {
        let truncated = ansi_safe_truncate(content, inner_width);
        format!(
            "{border_color}{border_char}{reset_color}{truncated}{border_color}{border_char}{reset_color}"
        )
    } else {
        let padding = " ".repeat(inner_width - vis_len);
        format!(
            "{border_color}{border_char}{reset_color}{content}{padding}{border_color}{border_char}{reset_color}"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_tier_classification() {
        let compact = Viewport::explicit(40, 24, false);
        assert_eq!(compact.tier(), LayoutTier::Compact);
        assert!(compact.is_compact());

        let standard = Viewport::explicit(80, 24, false);
        assert_eq!(standard.tier(), LayoutTier::Standard);
        assert!(!standard.is_compact());

        let wide = Viewport::explicit(120, 30, false);
        assert_eq!(wide.tier(), LayoutTier::Wide);
        assert!(wide.is_wide());
    }

    #[test]
    fn test_card_width_clamping() {
        let phone = Viewport::explicit(42, 24, false);
        assert_eq!(phone.card_width(78), 40);

        let ultra_narrow = Viewport::explicit(20, 24, false);
        assert_eq!(ultra_narrow.card_width(78), 32); // clamps to minimum 32

        let wide_monitor = Viewport::explicit(160, 40, false);
        assert_eq!(wide_monitor.card_width(76), 76); // clamps to maximum allowed
    }

    #[test]
    fn test_visible_width_calculation() {
        assert_eq!(visible_width("Hello World"), 11);
        let colored = "\x1b[38;2;99;102;241mHello\x1b[0m World";
        assert_eq!(visible_width(colored), 11);

        let multi_color = "\x1b[31mRed\x1b[0m \x1b[32mGreen\x1b[0m \x1b[34mBlue\x1b[0m";
        assert_eq!(visible_width(multi_color), 14);
    }

    #[test]
    fn test_ansi_safe_truncate() {
        let colored = "\x1b[32mSupercalifragilistic\x1b[0m";
        let truncated = ansi_safe_truncate(colored, 5);
        assert_eq!(visible_width(&truncated), 5);
        assert!(truncated.ends_with("\x1b[0m"));
    }

    #[test]
    fn test_pad_box_line_exact_columns() {
        let colored_content = "  \x1b[32mModified:\x1b[0m file.rs";
        let padded = pad_box_line(colored_content, 40, "", "│", "");
        assert_eq!(visible_width(&padded), 40);
    }
}
