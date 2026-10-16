//! Atlas Daedalus — TUI Renderer (ANSI / OLED Black)

pub struct TuiRenderer;

impl TuiRenderer {
    pub const RESET: &'static str = "\x1b[0m";
    pub const BOLD: &'static str = "\x1b[1m";
    pub const DIM: &'static str = "\x1b[2m";

    pub const RED: &'static str = "\x1b[31m";
    pub const GREEN: &'static str = "\x1b[32m";
    pub const YELLOW: &'static str = "\x1b[33m";
    pub const MAGENTA: &'static str = "\x1b[35m";
    pub const CYAN: &'static str = "\x1b[36m";
    pub const WHITE: &'static str = "\x1b[37m";
    pub const GRAY: &'static str = "\x1b[90m";

    pub fn print_header(workspace_root: &str, model: &str, provider: &str, security: &str) {
        let border = "=".repeat(78);
        println!("\n{}{}{}", Self::CYAN, border, Self::RESET);
        println!(
            "{}{}  ATLAS DAEDALUS (acode){}  {}v{}-industrial · Native Rust Engine · Atlas Research Labs{}",
            Self::BOLD, Self::WHITE, Self::RESET, Self::DIM, env!("CARGO_PKG_VERSION"), Self::RESET
        );
        println!("{}  Workspace: {}{}{}", Self::DIM, Self::WHITE, workspace_root, Self::RESET);
        let sec_color = match security {
            "full" => Self::GREEN,
            "sandboxed" => Self::YELLOW,
            "strict" => Self::RED,
            _ => Self::CYAN,
        };
        println!(
            "{}  Security:  {}{}{}  {}Provider: {}{}{}  {}Model: {}{}{}",
            Self::DIM, sec_color, security, Self::RESET,
            Self::DIM, Self::GREEN, provider, Self::RESET,
            Self::DIM, Self::CYAN, model, Self::RESET
        );
        println!(
            "{}  Type {}/help{} for commands, {}!<cmd>{} for shell, or enter prompt to begin.{}",
            Self::DIM, Self::WHITE, Self::RESET, Self::YELLOW, Self::RESET, Self::RESET
        );
        println!("{}{}{}\n", Self::CYAN, border, Self::RESET);
    }

    pub fn safe_truncate(s: &str, max_chars: usize) -> &str {
        match s.char_indices().nth(max_chars) {
            Some((idx, _)) => &s[..idx],
            None => s,
        }
    }

    pub fn print_thinking(status: &str) {
        print!("\r{}[ACODE] {}{}", Self::DIM, status, Self::RESET);
        let _ = std::io::Write::flush(&mut std::io::stdout());
    }

    pub fn print_tool_start(name: &str, args: &serde_json::Value) {
        let compact = args.to_string();
        let trimmed = if compact.chars().count() > 80 {
            format!("{}...", Self::safe_truncate(&compact, 77))
        } else {
            compact
        };
        println!(
            "\n{}[TOOL CALL]{} {}{}{} {}{}{}",
            Self::MAGENTA, Self::RESET, Self::BOLD, name, Self::RESET, Self::DIM, trimmed, Self::RESET
        );
    }

    pub fn print_tool_end(name: &str, success: bool, output: &str) {
        let tag = if success {
            format!("{}[OK]{}", Self::GREEN, Self::RESET)
        } else {
            format!("{}[FAIL]{}", Self::RED, Self::RESET)
        };
        let preview = if output.chars().count() > 300 {
            format!("{}...", Self::safe_truncate(output, 297))
        } else {
            output.to_string()
        };
        println!("{} {}{} output:{}\n{}{}{}", tag, Self::DIM, name, Self::RESET, Self::GRAY, preview, Self::RESET);
    }

    pub fn print_test_result(command: &str, passed: bool, output: &str) {
        let tag = if passed {
            format!("{}{}[VERIFICATION PASSED]{}", Self::BOLD, Self::GREEN, Self::RESET)
        } else {
            format!("{}{}[VERIFICATION FAILED]{}", Self::BOLD, Self::RED, Self::RESET)
        };
        println!("\n{} {}{}{}", tag, Self::CYAN, command, Self::RESET);
        if !passed {
            let snippet = if output.chars().count() > 500 { Self::safe_truncate(output, 500) } else { output };
            println!("{}{}{}", Self::RED, snippet, Self::RESET);
        }
    }

    pub fn print_diff(diff_text: &str) {
        for line in diff_text.lines() {
            if line.starts_with('+') {
                println!("{}{}{}", Self::GREEN, line, Self::RESET);
            } else if line.starts_with('-') {
                println!("{}{}{}", Self::RED, line, Self::RESET);
            } else if line.starts_with("@@") {
                println!("{}{}{}", Self::CYAN, line, Self::RESET);
            } else {
                println!("{}{}{}", Self::GRAY, line, Self::RESET);
            }
        }
    }

    pub fn print_success(message: &str) {
        println!("\n{}[COMPLETED]{} {}\n", Self::GREEN, Self::RESET, message);
    }

    pub fn print_error(message: &str) {
        println!("\n{}[ERROR]{} {}\n", Self::RED, Self::RESET, message);
    }
}
