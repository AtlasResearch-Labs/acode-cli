//! Atlas Daedalus — TUI Renderer & Animation Engine (TrueColor / OLED Black / Claude Code Aesthetic)
//! (C) 2026 Atlas Research Labs — MIT License

use std::io::Write;
use std::time::{Duration, Instant};

use crate::cli::viewport::{pad_box_line, LayoutTier, Viewport};

pub struct TuiRenderer;

impl TuiRenderer {
    /// Trigger a subtle audible bell alert upon task completion
    pub fn bell() {
        print!("\x07");
        let _ = std::io::stdout().flush();
    }

    // Standard ANSI Text Styles
    pub const RESET: &'static str = "\x1b[0m";
    pub const BOLD: &'static str = "\x1b[1m";
    pub const DIM: &'static str = "\x1b[2m";
    pub const ITALIC: &'static str = "\x1b[3m";
    pub const UNDERLINE: &'static str = "\x1b[4m";

    // Standard 16-color ANSI
    pub const RED: &'static str = "\x1b[31m";
    pub const GREEN: &'static str = "\x1b[32m";
    pub const YELLOW: &'static str = "\x1b[33m";
    pub const MAGENTA: &'static str = "\x1b[35m";
    pub const CYAN: &'static str = "\x1b[36m";
    pub const WHITE: &'static str = "\x1b[37m";
    pub const GRAY: &'static str = "\x1b[90m";

    // Extended TrueColor / ANSI-256 Palette (Claude Code & Codex aesthetic)
    pub const SLATE: &'static str = "\x1b[38;5;244m";
    pub const CHARCOAL: &'static str = "\x1b[38;5;239m";
    pub const INDIGO: &'static str = "\x1b[38;5;68m";
    pub const CYAN_ACCENT: &'static str = "\x1b[38;5;44m";
    pub const EMERALD: &'static str = "\x1b[38;5;42m";
    pub const AMBER: &'static str = "\x1b[38;5;215m";
    pub const ROSE: &'static str = "\x1b[38;5;204m";
    pub const VIOLET: &'static str = "\x1b[38;5;141m";

    pub const BG_PILL: &'static str = "\x1b[48;5;236m";
    pub const BG_HIGHLIGHT: &'static str = "\x1b[48;5;238m";

    // Unicode Box-Drawing Elements
    pub const BOX_TL: &'static str = "╭";
    pub const BOX_TR: &'static str = "╮";
    pub const BOX_BL: &'static str = "╰";
    pub const BOX_BR: &'static str = "╯";
    pub const BOX_H: &'static str = "─";
    pub const BOX_V: &'static str = "│";

    // Braille Spinner Frames
    pub const SPINNER_FRAMES: [&'static str; 10] =
        ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    /// Get spinner frame by tick count
    pub fn spinner_frame(tick: usize) -> &'static str {
        Self::SPINNER_FRAMES[tick % Self::SPINNER_FRAMES.len()]
    }

    /// Set dynamic terminal window title using ANSI OSC escape sequence
    pub fn set_terminal_title(title: &str) {
        print!("\x1b]0;{}\x07", title);
        let _ = std::io::stdout().flush();
    }

    /// Format duration into human-readable stopwatch text (e.g. "42ms", "3.4s", "01:14s")
    pub fn format_duration(d: Duration) -> String {
        let millis = d.as_millis();
        if millis < 1000 {
            format!("{}ms", millis)
        } else if millis < 60_000 {
            format!("{:.1}s", d.as_secs_f64())
        } else {
            let mins = millis / 60_000;
            let secs = (millis % 60_000) / 1000;
            format!("{:02}:{:02}s", mins, secs)
        }
    }

    /// Truncate string cleanly on unicode character boundaries
    pub fn safe_truncate(s: &str, max_chars: usize) -> &str {
        match s.char_indices().nth(max_chars) {
            Some((idx, _)) => &s[..idx],
            None => s,
        }
    }

    /// Print header card with rounded unicode corners and model pills
    pub fn print_header(workspace_root: &str, model: &str, provider: &str, security: &str) {
        let sec_color = match security {
            "full" => Self::EMERALD,
            "sandboxed" => Self::AMBER,
            "strict" => Self::ROSE,
            _ => Self::CYAN_ACCENT,
        };

        if std::env::var("ACODE_MASCOT").is_ok() {
            crate::cli::art::MascotEngine::print_mascot(crate::cli::art::MascotVariant::Automaton);
        }

        let vp = Viewport::current();
        let width = vp.card_width(82);
        let top_border = Self::BOX_H.repeat(width.saturating_sub(2));
        let bot_border = Self::BOX_H.repeat(width.saturating_sub(2));

        println!();
        println!(
            "{}{}{}{}{}",
            Self::INDIGO,
            Self::BOX_TL,
            top_border,
            Self::BOX_TR,
            Self::RESET
        );

        if vp.is_compact() {
            // Stacked rows for Termux / narrow split panes (< 60 cols)
            println!(
                "{}",
                pad_box_line(
                    &format!(
                        "  {}{}ATLAS DAEDALUS{} v{}",
                        Self::BOLD,
                        Self::WHITE,
                        Self::RESET,
                        env!("CARGO_PKG_VERSION")
                    ),
                    width,
                    Self::INDIGO,
                    Self::BOX_V,
                    Self::RESET
                )
            );
            println!(
                "{}",
                pad_box_line(
                    &format!(
                        "  {}Dir: {}{}",
                        Self::SLATE,
                        Self::WHITE,
                        Self::safe_truncate(workspace_root, width.saturating_sub(10))
                    ),
                    width,
                    Self::INDIGO,
                    Self::BOX_V,
                    Self::RESET
                )
            );
            println!(
                "{}",
                pad_box_line(
                    &format!("  {}Model: {}{}", Self::SLATE, Self::CYAN_ACCENT, model),
                    width,
                    Self::INDIGO,
                    Self::BOX_V,
                    Self::RESET
                )
            );
            println!(
                "{}",
                pad_box_line(
                    &format!("  {}Sec: {}{}", Self::SLATE, sec_color, security),
                    width,
                    Self::INDIGO,
                    Self::BOX_V,
                    Self::RESET
                )
            );
            println!(
                "{}",
                pad_box_line(
                    &format!("  {}Help: {}/help{}", Self::SLATE, Self::WHITE, Self::RESET),
                    width,
                    Self::INDIGO,
                    Self::BOX_V,
                    Self::RESET
                )
            );
        } else {
            // Full multi-column layout for Standard & Wide screens
            println!(
                "{}",
                pad_box_line(
                    &format!(
                        "  {}{}ATLAS DAEDALUS{} {}v{}-industrial{} · Pure Rust Engine · Atlas Research Labs",
                        Self::BOLD,
                        Self::WHITE,
                        Self::RESET,
                        Self::CYAN_ACCENT,
                        env!("CARGO_PKG_VERSION"),
                        Self::SLATE
                    ),
                    width,
                    Self::INDIGO,
                    Self::BOX_V,
                    Self::RESET
                )
            );
            println!(
                "{}",
                pad_box_line(
                    &format!(
                        "  {}Workspace: {}{}",
                        Self::SLATE,
                        Self::WHITE,
                        workspace_root
                    ),
                    width,
                    Self::INDIGO,
                    Self::BOX_V,
                    Self::RESET
                )
            );
            println!(
                "{}",
                pad_box_line(
                    &format!(
                        "  {}Security:  {}{}{}  {}Provider: {}{}{}  {}Model: {}{}",
                        Self::SLATE,
                        sec_color,
                        security,
                        Self::RESET,
                        Self::SLATE,
                        Self::EMERALD,
                        provider,
                        Self::RESET,
                        Self::SLATE,
                        Self::CYAN_ACCENT,
                        model
                    ),
                    width,
                    Self::INDIGO,
                    Self::BOX_V,
                    Self::RESET
                )
            );
            println!(
                "{}",
                pad_box_line(
                    &format!(
                        "  {}Commands: {}/help{} · Shell: {}!<cmd>{} · Daemon: {}!&<cmd>{}",
                        Self::SLATE,
                        Self::WHITE,
                        Self::RESET,
                        Self::AMBER,
                        Self::RESET,
                        Self::VIOLET,
                        Self::RESET
                    ),
                    width,
                    Self::INDIGO,
                    Self::BOX_V,
                    Self::RESET
                )
            );
        }

        println!(
            "{}{}{}{}{}\n",
            Self::INDIGO,
            Self::BOX_BL,
            bot_border,
            Self::BOX_BR,
            Self::RESET
        );
    }

    /// Print thinking line with dynamic Braille spinner and elapsed time
    pub fn print_thinking_animated(status: &str, tick: usize, start_time: Instant) {
        let spinner = Self::spinner_frame(tick);
        let elapsed = Self::format_duration(start_time.elapsed());
        print!(
            "\r\x1b[2K{}{}{} {}{} {}[{}]{}",
            Self::CYAN_ACCENT,
            spinner,
            Self::RESET,
            Self::SLATE,
            status,
            Self::CHARCOAL,
            elapsed,
            Self::RESET
        );
        let _ = std::io::stdout().flush();
    }

    /// Legacy thinking print fallback
    pub fn print_thinking(status: &str) {
        print!(
            "\r\x1b[2K{}[ACODE] {}{}",
            Self::CYAN_ACCENT,
            status,
            Self::RESET
        );
        let _ = std::io::stdout().flush();
    }

    /// Clear current animated line cleanly
    pub fn clear_line() {
        print!("\r\x1b[2K");
        let _ = std::io::stdout().flush();
    }

    /// Print structured tool execution start card (Claude Code style)
    pub fn print_tool_start(name: &str, args: &serde_json::Value) {
        let vp = Viewport::current();
        let max_args_len = vp.cols.saturating_sub(24).max(15);
        let compact = args.to_string();
        let trimmed = if compact.chars().count() > max_args_len {
            format!(
                "{}...",
                Self::safe_truncate(&compact, max_args_len.saturating_sub(3))
            )
        } else {
            compact
        };

        println!(
            "\n{}{}─ [TOOL]{} {}{}{} {}{}{}",
            Self::INDIGO,
            Self::BOX_TL,
            Self::RESET,
            Self::BOLD,
            name,
            Self::RESET,
            Self::SLATE,
            trimmed,
            Self::RESET
        );
    }

    /// Print structured tool execution finish card with status and timing
    pub fn print_tool_end(name: &str, success: bool, output: &str) {
        Self::print_tool_end_timed(name, success, output, None);
    }

    /// Print structured tool execution finish card with optional duration badge
    pub fn print_tool_end_timed(
        name: &str,
        success: bool,
        output: &str,
        duration: Option<Duration>,
    ) {
        let (tag, tag_color) = if success {
            ("[OK]", Self::EMERALD)
        } else {
            ("[FAIL]", Self::ROSE)
        };

        let time_str = duration
            .map(|d| format!(" ({})", Self::format_duration(d)))
            .unwrap_or_default();

        let vp = Viewport::current();
        let max_out_len = (vp.cols * 3).clamp(80, 240);
        let preview = if output.chars().count() > max_out_len {
            format!(
                "{}...",
                Self::safe_truncate(output, max_out_len.saturating_sub(3))
            )
        } else {
            output.trim().to_string()
        };

        if !preview.is_empty() {
            println!(
                "{}{} {}output:{} {}{}{}",
                Self::INDIGO,
                Self::BOX_V,
                Self::SLATE,
                Self::RESET,
                Self::CHARCOAL,
                preview,
                Self::RESET
            );
        }

        println!(
            "{}{}─ {}{}{}{} {}{}{}",
            Self::INDIGO,
            Self::BOX_BL,
            tag_color,
            tag,
            Self::RESET,
            Self::SLATE,
            name,
            time_str,
            Self::RESET
        );
    }

    /// Print test verification outcome card
    pub fn print_test_result(command: &str, passed: bool, output: &str) {
        let (tag, color) = if passed {
            ("[VERIFICATION PASSED]", Self::EMERALD)
        } else {
            ("[VERIFICATION FAILED]", Self::ROSE)
        };

        println!(
            "\n{}{}{} {}{}{}",
            color,
            Self::BOLD,
            tag,
            Self::CYAN_ACCENT,
            command,
            Self::RESET
        );
        if !passed {
            let snippet = if output.chars().count() > 500 {
                Self::safe_truncate(output, 500)
            } else {
                output
            };
            println!("{}{}{}", Self::ROSE, snippet, Self::RESET);
        }
    }

    /// Print live host metrics status bar
    pub fn print_status_bar(model: &str, provider: &str, security: &str) {
        let metrics = crate::telemetry::host::HostInspector::collect();
        let sec_color = match security {
            "full" => Self::EMERALD,
            "sandboxed" => Self::AMBER,
            "strict" => Self::ROSE,
            _ => Self::CYAN_ACCENT,
        };

        let vp = Viewport::current();
        let width = vp.card_width(82);
        let border = Self::BOX_H.repeat(width);
        println!("{}{}{}", Self::CHARCOAL, border, Self::RESET);

        if vp.is_compact() {
            println!(
                "{}[RAM: {}MB | {} | Sec: {}{}]{}",
                Self::SLATE,
                metrics.process_rss_mb,
                model,
                sec_color,
                security,
                Self::RESET
            );
        } else {
            println!(
                "{}[HOST: {} | CPU: {}C | RAM: {}MB free / {}MB | RSS: {}MB | Model: {}{} ({}) | Sec: {}{}{}]{}",
                Self::SLATE,
                metrics.os_name,
                metrics.cpu_cores,
                metrics.available_memory_mb,
                metrics.total_memory_mb,
                metrics.process_rss_mb,
                Self::CYAN_ACCENT,
                model,
                provider,
                sec_color,
                security,
                Self::SLATE,
                Self::RESET
            );
        }
        println!("{}{}{}\n", Self::CHARCOAL, border, Self::RESET);
    }

    /// Print unified diff with line numbers and addition/deletion visual bar
    pub fn print_diff(diff_text: &str) {
        let mut additions = 0usize;
        let mut deletions = 0usize;

        for line in diff_text.lines() {
            if line.starts_with('+') && !line.starts_with("+++") {
                additions += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                deletions += 1;
            }
        }

        let vp = Viewport::current();
        let total = additions + deletions;
        let bar_width = match vp.tier() {
            LayoutTier::Compact => 6,
            LayoutTier::Standard => 12,
            LayoutTier::Wide => 16,
        };
        let add_bars = (additions * bar_width).checked_div(total).unwrap_or(0);
        let del_bars = bar_width.saturating_sub(add_bars);
        let ratio_bar = format!(
            "{}{}{}{}",
            Self::EMERALD,
            "█".repeat(add_bars),
            Self::ROSE,
            "░".repeat(del_bars)
        );

        if total > 0 {
            println!(
                "\n{}[DIFF]{} {}+{} additions{} / {}-{} deletions{}  [{}{}]",
                Self::BOLD,
                Self::RESET,
                Self::EMERALD,
                additions,
                Self::RESET,
                Self::ROSE,
                deletions,
                Self::RESET,
                ratio_bar,
                Self::RESET
            );
        }

        let max_line_len = vp.cols.saturating_sub(10).max(20);

        for (idx, raw_line) in diff_text.lines().enumerate() {
            let line = if raw_line.chars().count() > max_line_len {
                format!(
                    "{}...",
                    Self::safe_truncate(raw_line, max_line_len.saturating_sub(3))
                )
            } else {
                raw_line.to_string()
            };

            if line.starts_with("+++") || line.starts_with("---") {
                println!(
                    "{}  {:4} │ {}{}",
                    Self::CHARCOAL,
                    idx + 1,
                    line,
                    Self::RESET
                );
            } else if line.starts_with('+') {
                println!("{}+ {:4} │ {}{}", Self::EMERALD, idx + 1, line, Self::RESET);
            } else if line.starts_with('-') {
                println!("{}- {:4} │ {}{}", Self::ROSE, idx + 1, line, Self::RESET);
            } else if line.starts_with("@@") {
                println!(
                    "{}~ {:4} │ {}{}",
                    Self::CYAN_ACCENT,
                    idx + 1,
                    line,
                    Self::RESET
                );
            } else {
                println!("{}  {:4} │ {}{}", Self::SLATE, idx + 1, line, Self::RESET);
            }
        }
        println!();
    }

    /// Print turn summary card with duration, tools called, and token usage
    pub fn print_turn_summary(
        duration: Duration,
        tools_called: usize,
        files_modified: &[String],
        tokens: Option<usize>,
        cost: Option<f64>,
    ) {
        let vp = Viewport::current();
        let dur_str = Self::format_duration(duration);

        if vp.is_compact() {
            let files_info = if files_modified.is_empty() {
                String::new()
            } else {
                format!(" · {} modified", files_modified.len())
            };
            println!(
                "\n{}{}{} ✨ Completed in {}{}{} ({} tools{})",
                Self::EMERALD,
                Self::BOX_TL,
                Self::BOX_H.repeat(2),
                Self::BOLD,
                dur_str,
                Self::RESET,
                tools_called,
                files_info,
            );
            if let (Some(t), Some(c)) = (tokens, cost) {
                println!(
                    "{}  └─ {} tokens (${:.4}){}\n",
                    Self::SLATE,
                    t,
                    c,
                    Self::RESET
                );
            } else {
                println!();
            }
        } else {
            let files_str = if files_modified.is_empty() {
                String::new()
            } else {
                format!(" · {} files modified", files_modified.len())
            };

            let token_str = match (tokens, cost) {
                (Some(t), Some(c)) => format!(" · {} tokens (${:.4})", t, c),
                (Some(t), None) => format!(" · {} tokens", t),
                _ => String::new(),
            };

            println!(
                "\n{}{}{} ✨ Completed in {}{}{} · {} tools called{}{}{}\n",
                Self::EMERALD,
                Self::BOX_TL,
                Self::BOX_H.repeat(3),
                Self::BOLD,
                dur_str,
                Self::RESET,
                tools_called,
                files_str,
                token_str,
                Self::RESET
            );
        }
    }

    /// Format Markdown line for streaming terminal display
    pub fn render_markdown_line(line: &str) -> String {
        // Headings
        if let Some(rest) = line.strip_prefix("### ") {
            return format!(
                "\n{}{}{}◆ {}{}\n",
                Self::BOLD,
                Self::CYAN_ACCENT,
                Self::RESET,
                rest,
                Self::RESET
            );
        }
        if let Some(rest) = line.strip_prefix("## ") {
            return format!(
                "\n{}{}{}■ {}{}\n",
                Self::BOLD,
                Self::INDIGO,
                Self::RESET,
                rest,
                Self::RESET
            );
        }
        if let Some(rest) = line.strip_prefix("# ") {
            return format!(
                "\n{}{}{}━ {}{}\n",
                Self::BOLD,
                Self::WHITE,
                Self::RESET,
                rest,
                Self::RESET
            );
        }

        // Fenced code blocks
        if line.starts_with("```") {
            let lang = line.trim_start_matches('`').trim();
            if !lang.is_empty() {
                return format!(
                    "\n{}{}─ [{}] ──────────────────────────────────────╮{}",
                    Self::INDIGO,
                    Self::BOX_TL,
                    lang,
                    Self::RESET
                );
            } else {
                return format!(
                    "{}{}────────────────────────────────────────────────╯{}\n",
                    Self::INDIGO,
                    Self::BOX_BL,
                    Self::RESET
                );
            }
        }

        // Blockquotes and Callouts
        if let Some(rest) = line.strip_prefix("> ") {
            return format!(
                "{}│{} {}{}{}",
                Self::CYAN_ACCENT,
                Self::RESET,
                Self::SLATE,
                rest,
                Self::RESET
            );
        }

        // Bullet lists
        if let Some(rest) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            return format!(
                "  {}•{} {}{}{}",
                Self::CYAN_ACCENT,
                Self::RESET,
                Self::WHITE,
                rest,
                Self::RESET
            );
        }

        // Status badge highlights
        let mut rendered = line.to_string();
        rendered = rendered.replace("[OK]", &format!("{}[OK]{}", Self::EMERALD, Self::RESET));
        rendered = rendered.replace("[FAIL]", &format!("{}[FAIL]{}", Self::ROSE, Self::RESET));
        rendered = rendered.replace("[WARN]", &format!("{}[WARN]{}", Self::AMBER, Self::RESET));
        rendered = rendered.replace(
            "[INFO]",
            &format!("{}[INFO]{}", Self::CYAN_ACCENT, Self::RESET),
        );
        rendered = rendered.replace("[TIP]", &format!("{}[TIP]{}", Self::VIOLET, Self::RESET));

        rendered
    }

    pub fn print_success(message: &str) {
        println!(
            "\n{}[COMPLETED]{} {}\n",
            Self::EMERALD,
            Self::RESET,
            message
        );
    }

    pub fn print_error(message: &str) {
        println!("\n{}[ERROR]{} {}\n", Self::ROSE, Self::RESET, message);
    }
}
