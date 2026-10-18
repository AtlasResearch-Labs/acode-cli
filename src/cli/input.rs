//! Atlas Daedalus — Terminal Raw-Mode Input Engine & Kitty Keyboard Protocol
//! (C) 2026 Atlas Research Labs — MIT License

use std::io::{self, stdout, Write};
use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crate::cli::completion::CompletionEngine;
use crate::cli::menu::FloatingMenu;

/// Actions emitted by the raw-mode input engine upon key events
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputAction {
    /// User submitted the prompt (plain Enter)
    Submit(String),
    /// Redraw the active line buffer and ghost text
    Redraw,
    /// User accepted the ghost text autocomplete
    AcceptGhost,
    /// Slash command menu was triggered
    OpenMenu,
    /// Cancel operation (Ctrl+C)
    Cancel,
    /// Exit REPL session (Ctrl+D on empty line)
    Exit,
}

/// Raw-mode line editing engine supporting multiline, ghost-text, and Kitty protocol
pub struct TerminalInputEngine {
    pub buffer: String,
    pub cursor_pos: usize,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    pub ghost_text: Option<String>,
    saved_input_before_history: String,
}

impl TerminalInputEngine {
    /// Create a new input engine initialized with session history
    pub fn new(history: Vec<String>) -> Self {
        Self {
            buffer: String::new(),
            cursor_pos: 0,
            history,
            history_index: None,
            ghost_text: None,
            saved_input_before_history: String::new(),
        }
    }

    /// Check if terminal emulator safely supports Kitty progressive keyboard protocol
    pub fn is_kitty_protocol_supported() -> bool {
        #[cfg(windows)]
        {
            false
        }
        #[cfg(not(windows))]
        {
            if std::env::var("KITTY_WINDOW_ID").is_ok()
                || std::env::var("GHOSTTY_RESOURCES_DIR").is_ok()
                || std::env::var("WEZTERM_EXECUTABLE").is_ok()
            {
                matches!(crossterm::terminal::supports_keyboard_enhancement(), Ok(true))
            } else {
                false
            }
        }
    }

    /// Enter raw mode and request Kitty Keyboard Protocol (CSI u) for Shift+Enter disambiguation if supported
    pub fn enter_raw_mode() -> io::Result<()> {
        enable_raw_mode()?;
        if Self::is_kitty_protocol_supported() {
            let _ = crossterm::execute!(
                stdout(),
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            );
        }
        Ok(())
    }

    /// Restore standard terminal mode and pop Kitty protocol flags if pushed
    pub fn exit_raw_mode() -> io::Result<()> {
        if Self::is_kitty_protocol_supported() {
            let _ = crossterm::execute!(stdout(), PopKeyboardEnhancementFlags);
        }
        disable_raw_mode()?;
        Ok(())
    }

    /// Read an interactive prompt line in raw mode with real-time ghost text,
    /// popover slash menu, Kitty Shift+Enter multi-line support, and history.
    pub fn read_line_interactive(
        &mut self,
        prompt: &str,
        completion: &CompletionEngine,
        menu: &mut FloatingMenu,
        ast_symbols: &[String],
    ) -> io::Result<Option<String>> {
        struct RawGuard;
        impl Drop for RawGuard {
            fn drop(&mut self) {
                let _ = TerminalInputEngine::exit_raw_mode();
            }
        }

        Self::enter_raw_mode()?;
        let _guard = RawGuard;

        self.buffer.clear();
        self.cursor_pos = 0;
        self.history_index = None;
        menu.is_open = false;

        let mut menu_lines_rendered = 0usize;
        let mut prompt_lines_rendered = 0usize;

        let redraw = |buf: &str,
                      cur_pos: usize,
                      ghost: Option<&str>,
                      m: &mut FloatingMenu,
                      m_lines: &mut usize,
                      p_lines: &mut usize| {
            let mut out = String::new();

            // Clear previously rendered prompt and menu lines
            let total_prev_lines = *m_lines + *p_lines;
            if total_prev_lines > 0 {
                out.push_str(&format!("\r\x1b[{}A", total_prev_lines));
                for _ in 0..total_prev_lines {
                    out.push_str("\x1b[2K\r\n");
                }
                out.push_str(&format!("\r\x1b[{}A", total_prev_lines));
                *m_lines = 0;
                *p_lines = 0;
            }

            // Draw floating menu if active
            if m.is_open {
                let (term_width, _) = crossterm::terminal::size().unwrap_or((80, 24));
                let popover = m.render_popover(buf, term_width);
                if !popover.is_empty() {
                    out.push_str(&popover);
                    *m_lines = m.last_rendered_lines;
                }
            }

            // Draw prompt and buffer
            out.push_str("\r\x1b[2K");
            if !buf.contains('\n') {
                out.push_str(prompt);
                out.push_str(buf);

                // Draw dimmed ghost text if cursor is at end of buffer
                let count = buf.chars().count();
                if cur_pos == count {
                    if let Some(g) = ghost {
                        out.push_str("\x1b[2m\x1b[38;5;244m");
                        out.push_str(g);
                        out.push_str("\x1b[0m");
                        // Move cursor back so it blinks at the insertion point, not after the ghost text!
                        let g_len = g.chars().count();
                        if g_len > 0 {
                            out.push_str(&format!("\x1b[{}D", g_len));
                        }
                    }
                }

                // Restore cursor position if not at end
                if cur_pos < count {
                    let diff = count - cur_pos;
                    out.push_str(&format!("\x1b[{}D", diff));
                }
                *p_lines = 0;
            } else {
                // Multiline buffer rendering with \r\n and 2D cursor placement
                let lines: Vec<&str> = buf.split('\n').collect();
                let num_lines = lines.len();
                *p_lines = num_lines - 1;

                let mut char_acc = 0usize;
                let mut target_line = 0usize;
                let mut target_col = 0usize;

                for (idx, line) in lines.iter().enumerate() {
                    let line_char_count = line.chars().count();
                    if cur_pos <= char_acc + line_char_count {
                        target_line = idx;
                        target_col = cur_pos - char_acc;
                        break;
                    }
                    char_acc += line_char_count + 1; // +1 for newline
                }

                for (idx, line) in lines.iter().enumerate() {
                    if idx == 0 {
                        out.push_str(prompt);
                        out.push_str(line);
                    } else {
                        out.push_str("  ... ");
                        out.push_str(line);
                    }
                    if idx + 1 < num_lines {
                        out.push_str("\r\n\x1b[2K");
                    }
                }

                // If on last line, display ghost text if available
                if target_line + 1 == num_lines && cur_pos == buf.chars().count() {
                    if let Some(g) = ghost {
                        out.push_str("\x1b[2m\x1b[38;5;244m");
                        out.push_str(g);
                        out.push_str("\x1b[0m");
                        let g_len = g.chars().count();
                        if g_len > 0 {
                            out.push_str(&format!("\x1b[{}D", g_len));
                        }
                    }
                }

                let lines_up = (num_lines - 1) - target_line;
                if lines_up > 0 {
                    out.push_str(&format!("\x1b[{}A", lines_up));
                }
                let prefix_len = if target_line == 0 { 2 } else { 6 };
                out.push_str(&format!("\r\x1b[{}C", prefix_len + target_col));
            }

            print!("{}", out);
            let _ = stdout().flush();
        };

        self.ghost_text = completion.get_ghost_suggestion(&self.buffer, ast_symbols);
        redraw(
            &self.buffer,
            self.cursor_pos,
            self.ghost_text.as_deref(),
            menu,
            &mut menu_lines_rendered,
            &mut prompt_lines_rendered,
        );

        loop {
            let event = match crossterm::event::read() {
                Ok(ev) => ev,
                Err(e) => return Err(e),
            };

            match event {
                crossterm::event::Event::Resize(_, _) => {
                    redraw(
                        &self.buffer,
                        self.cursor_pos,
                        self.ghost_text.as_deref(),
                        menu,
                        &mut menu_lines_rendered,
                        &mut prompt_lines_rendered,
                    );
                }
                crossterm::event::Event::Paste(text) => {
                    for c in text.chars() {
                        if c == '\r' {
                            continue;
                        }
                        self.insert_char(c);
                    }
                    self.ghost_text = completion.get_ghost_suggestion(&self.buffer, ast_symbols);
                    redraw(
                        &self.buffer,
                        self.cursor_pos,
                        self.ghost_text.as_deref(),
                        menu,
                        &mut menu_lines_rendered,
                        &mut prompt_lines_rendered,
                    );
                }
                crossterm::event::Event::Key(key) => {
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }

                    // 1. Floating menu navigation
                    if menu.is_open {
                        let count = menu.filter_indices(&self.buffer).len();
                        match key.code {
                            KeyCode::Up => {
                                menu.select_prev(count);
                                redraw(
                                    &self.buffer,
                                    self.cursor_pos,
                                    self.ghost_text.as_deref(),
                                    menu,
                                    &mut menu_lines_rendered,
                                    &mut prompt_lines_rendered,
                                );
                                continue;
                            }
                            KeyCode::Down => {
                                menu.select_next(count);
                                redraw(
                                    &self.buffer,
                                    self.cursor_pos,
                                    self.ghost_text.as_deref(),
                                    menu,
                                    &mut menu_lines_rendered,
                                    &mut prompt_lines_rendered,
                                );
                                continue;
                            }
                            KeyCode::Esc => {
                                menu.is_open = false;
                                redraw(
                                    &self.buffer,
                                    self.cursor_pos,
                                    self.ghost_text.as_deref(),
                                    menu,
                                    &mut menu_lines_rendered,
                                    &mut prompt_lines_rendered,
                                );
                                continue;
                            }
                            KeyCode::Tab | KeyCode::Enter => {
                                if let Some(item) = menu.get_selected_item(&self.buffer) {
                                    self.buffer = item.name.clone();
                                    if item.args.is_some() {
                                        self.buffer.push(' ');
                                    }
                                    self.cursor_pos = self.char_count();
                                    let is_enter = key.code == KeyCode::Enter;
                                    let no_args = item.args.is_none();
                                    menu.is_open = false;

                                    if is_enter && no_args {
                                        let total = menu_lines_rendered + prompt_lines_rendered;
                                        if total > 0 {
                                            print!("\r\x1b[{}A", total);
                                            for _ in 0..total {
                                                print!("\x1b[2K\r\n");
                                            }
                                            print!("\r\x1b[{}A", total);
                                        }
                                        let submitted = std::mem::take(&mut self.buffer);
                                        self.cursor_pos = 0;
                                        print!("\r\n");
                                        let _ = stdout().flush();
                                        return Ok(Some(submitted));
                                    }

                                    self.ghost_text =
                                        completion.get_ghost_suggestion(&self.buffer, ast_symbols);
                                    redraw(
                                        &self.buffer,
                                        self.cursor_pos,
                                        self.ghost_text.as_deref(),
                                        menu,
                                        &mut menu_lines_rendered,
                                        &mut prompt_lines_rendered,
                                    );
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }

                    // 2. Pass key to input engine
                    if let Some(action) = self.handle_key(key) {
                        match action {
                            InputAction::Submit(line) => {
                                let total = menu_lines_rendered + prompt_lines_rendered;
                                if total > 0 {
                                    print!("\r\x1b[{}A", total);
                                    for _ in 0..total {
                                        print!("\x1b[2K\r\n");
                                    }
                                    print!("\r\x1b[{}A", total);
                                }
                                print!("\r\n");
                                let _ = stdout().flush();
                                return Ok(Some(line));
                            }
                            InputAction::Exit => {
                                print!("\r\n");
                                let _ = stdout().flush();
                                return Ok(None);
                            }
                            InputAction::Cancel => {
                                print!("\r\n");
                                let _ = stdout().flush();
                                return Ok(Some(String::new()));
                            }
                            InputAction::OpenMenu => {
                                menu.is_open = true;
                                menu.selected_index = 0;
                            }
                            InputAction::AcceptGhost | InputAction::Redraw => {}
                        }
                    }

                    // Automatically open menu when typing leading slash
                    if self.buffer.starts_with('/') && !self.buffer.contains(' ') {
                        menu.is_open = true;
                    } else {
                        menu.is_open = false;
                    }

                    self.ghost_text = completion.get_ghost_suggestion(&self.buffer, ast_symbols);
                    redraw(
                        &self.buffer,
                        self.cursor_pos,
                        self.ghost_text.as_deref(),
                        menu,
                        &mut menu_lines_rendered,
                        &mut prompt_lines_rendered,
                    );
                }
                _ => {}
            }
        }
    }

    /// Insert a character at the current cursor position
    pub fn insert_char(&mut self, c: char) {
        let count = self.char_count();
        if self.cursor_pos >= count {
            self.buffer.push(c);
            self.cursor_pos = count + 1;
        } else {
            let byte_idx = self.char_to_byte_index(self.cursor_pos);
            self.buffer.insert(byte_idx, c);
            self.cursor_pos += 1;
        }
    }

    /// Delete from current cursor position to end of buffer (Ctrl+K)
    pub fn kill_to_end(&mut self) {
        if self.cursor_pos < self.char_count() {
            let byte_idx = self.char_to_byte_index(self.cursor_pos);
            self.buffer.truncate(byte_idx);
            self.ghost_text = None;
        }
    }

    /// Insert a clean newline (Shift+Enter or Ctrl+J) without submitting
    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    /// Delete character immediately preceding cursor (Backspace)
    pub fn delete_backspace(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            let byte_idx = self.char_to_byte_index(self.cursor_pos);
            self.buffer.remove(byte_idx);
        }
    }

    /// Delete character immediately after cursor (Delete key)
    pub fn delete_forward(&mut self) {
        if self.cursor_pos < self.char_count() {
            let byte_idx = self.char_to_byte_index(self.cursor_pos);
            self.buffer.remove(byte_idx);
        }
    }

    /// Move cursor left one character
    pub fn move_cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    /// Move cursor right one character
    pub fn move_cursor_right(&mut self) {
        if self.cursor_pos < self.char_count() {
            self.cursor_pos += 1;
        }
    }

    /// Move cursor to beginning of prompt buffer (Home / Ctrl+A)
    pub fn move_cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move cursor to end of prompt buffer (End / Ctrl+E)
    pub fn move_cursor_end(&mut self) {
        self.cursor_pos = self.char_count();
    }

    /// Move cursor backward by one whitespace-delimited word (Alt+B / Ctrl+Left)
    pub fn move_word_left(&mut self) {
        let chars: Vec<char> = self.buffer.chars().collect();
        let mut idx = self.cursor_pos;
        while idx > 0 && chars[idx - 1].is_whitespace() {
            idx -= 1;
        }
        while idx > 0 && !chars[idx - 1].is_whitespace() {
            idx -= 1;
        }
        self.cursor_pos = idx;
    }

    /// Move cursor forward by one whitespace-delimited word (Alt+F / Ctrl+Right)
    pub fn move_word_right(&mut self) {
        let chars: Vec<char> = self.buffer.chars().collect();
        let len = chars.len();
        let mut idx = self.cursor_pos;
        while idx < len && !chars[idx].is_whitespace() {
            idx += 1;
        }
        while idx < len && chars[idx].is_whitespace() {
            idx += 1;
        }
        self.cursor_pos = idx;
    }

    /// Delete previous word (Ctrl+W)
    pub fn delete_word_back(&mut self) {
        let target = {
            let chars: Vec<char> = self.buffer.chars().collect();
            let mut idx = self.cursor_pos;
            while idx > 0 && chars[idx - 1].is_whitespace() {
                idx -= 1;
            }
            while idx > 0 && !chars[idx - 1].is_whitespace() {
                idx -= 1;
            }
            idx
        };

        while self.cursor_pos > target {
            self.delete_backspace();
        }
    }

    /// Clear entire line buffer (Ctrl+U)
    pub fn clear_line(&mut self) {
        self.buffer.clear();
        self.cursor_pos = 0;
        self.ghost_text = None;
    }

    /// Accept the entire inline ghost text suggestion (Tab / Right at end)
    pub fn accept_ghost_full(&mut self) -> bool {
        if let Some(ghost) = self.ghost_text.take() {
            if !ghost.is_empty() {
                self.buffer.push_str(&ghost);
                self.cursor_pos = self.char_count();
                return true;
            }
        }
        false
    }

    /// Accept next word of ghost text (Alt+Right / Ctrl+Right)
    pub fn accept_ghost_word(&mut self) -> bool {
        let to_insert = if let Some(ghost) = &self.ghost_text {
            if ghost.is_empty() {
                None
            } else {
                let word = ghost
                    .split_whitespace()
                    .next()
                    .unwrap_or(ghost.as_str());
                let has_trailing_space = ghost.starts_with(word) && ghost[word.len()..].starts_with(' ');
                let mut ins = word.to_string();
                if has_trailing_space {
                    ins.push(' ');
                }
                Some(ins)
            }
        } else {
            None
        };

        if let Some(ins) = to_insert {
            let count = ins.chars().count();
            for c in ins.chars() {
                self.insert_char(c);
            }
            count > 0
        } else {
            false
        }
    }

    /// Navigate history backward (Up Arrow)
    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.history_index.is_none() {
            self.saved_input_before_history = self.buffer.clone();
            let new_idx = self.history.len() - 1;
            self.history_index = Some(new_idx);
            self.buffer = self.history[new_idx].clone();
            self.cursor_pos = self.char_count();
        } else if let Some(idx) = self.history_index {
            if idx > 0 {
                let new_idx = idx - 1;
                self.history_index = Some(new_idx);
                self.buffer = self.history[new_idx].clone();
                self.cursor_pos = self.char_count();
            }
        }
    }

    /// Navigate history forward (Down Arrow)
    pub fn history_down(&mut self) {
        if let Some(idx) = self.history_index {
            if idx + 1 < self.history.len() {
                let new_idx = idx + 1;
                self.history_index = Some(new_idx);
                self.buffer = self.history[new_idx].clone();
                self.cursor_pos = self.char_count();
            } else {
                self.history_index = None;
                self.buffer = self.saved_input_before_history.clone();
                self.cursor_pos = self.char_count();
            }
        }
    }

    /// Handle raw keyboard event and translate into an `InputAction`
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<InputAction> {
        // Discard KeyRelease events on Windows/Crossterm to prevent double-firing, but allow Press and Repeat
        if key.kind == KeyEventKind::Release {
            return None;
        }

        match (key.code, key.modifiers) {
            // Disambiguated Shift+Enter (Kitty CSI 13;2 u) or Ctrl+J fallback
            (KeyCode::Enter, KeyModifiers::SHIFT)
            | (KeyCode::Char('\n'), KeyModifiers::SHIFT)
            | (KeyCode::Char('j'), KeyModifiers::CONTROL) => {
                self.insert_newline();
                Some(InputAction::Redraw)
            }

            // Plain Enter submits prompt (handles KeyCode::Enter, '\r', '\n')
            (KeyCode::Enter, KeyModifiers::NONE)
            | (KeyCode::Char('\r'), _)
            | (KeyCode::Char('\n'), KeyModifiers::NONE) => {
                let submitted = std::mem::take(&mut self.buffer);
                self.cursor_pos = 0;
                self.history_index = None;
                self.ghost_text = None;
                Some(InputAction::Submit(submitted))
            }

            // Tab accepts ghost suggestion if available, otherwise triggers menu
            (KeyCode::Tab, KeyModifiers::NONE) => {
                if self.accept_ghost_full() {
                    Some(InputAction::AcceptGhost)
                } else if self.buffer.starts_with('/') {
                    Some(InputAction::OpenMenu)
                } else {
                    Some(InputAction::Redraw)
                }
            }

            // Right Arrow at buffer end accepts ghost completion
            (KeyCode::Right, KeyModifiers::NONE) if self.cursor_pos == self.char_count() => {
                if self.accept_ghost_full() {
                    Some(InputAction::AcceptGhost)
                } else {
                    Some(InputAction::Redraw)
                }
            }

            // Word navigation forward (Alt+Right / Ctrl+Right / Alt+F)
            (KeyCode::Right, KeyModifiers::ALT)
            | (KeyCode::Right, KeyModifiers::CONTROL)
            | (KeyCode::Char('f'), KeyModifiers::ALT) => {
                if self.cursor_pos == self.char_count() && self.accept_ghost_word() {
                    Some(InputAction::AcceptGhost)
                } else {
                    self.move_word_right();
                    Some(InputAction::Redraw)
                }
            }

            // Word navigation backward (Alt+Left / Ctrl+Left / Alt+B)
            (KeyCode::Left, KeyModifiers::ALT)
            | (KeyCode::Left, KeyModifiers::CONTROL)
            | (KeyCode::Char('b'), KeyModifiers::ALT) => {
                self.move_word_left();
                Some(InputAction::Redraw)
            }

            // Standard cursor navigation
            (KeyCode::Left, KeyModifiers::NONE) => {
                self.move_cursor_left();
                Some(InputAction::Redraw)
            }
            (KeyCode::Right, KeyModifiers::NONE) => {
                self.move_cursor_right();
                Some(InputAction::Redraw)
            }
            (KeyCode::Home, _) | (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
                self.move_cursor_home();
                Some(InputAction::Redraw)
            }
            (KeyCode::End, _) | (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                self.move_cursor_end();
                Some(InputAction::Redraw)
            }

            // Editing operations
            (KeyCode::Backspace, _)
            | (KeyCode::Char('\x08'), _)
            | (KeyCode::Char('\x7f'), _) => {
                self.delete_backspace();
                Some(InputAction::Redraw)
            }
            (KeyCode::Delete, _) => {
                self.delete_forward();
                Some(InputAction::Redraw)
            }
            (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                self.delete_word_back();
                Some(InputAction::Redraw)
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                self.clear_line();
                Some(InputAction::Redraw)
            }
            (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                self.kill_to_end();
                Some(InputAction::Redraw)
            }
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                print!("\x1B[2J\x1B[1;1H");
                let _ = stdout().flush();
                Some(InputAction::Redraw)
            }

            // History navigation
            (KeyCode::Up, KeyModifiers::NONE) => {
                self.history_up();
                Some(InputAction::Redraw)
            }
            (KeyCode::Down, KeyModifiers::NONE) => {
                self.history_down();
                Some(InputAction::Redraw)
            }

            // Cancellation and Exit
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                if !self.buffer.is_empty() {
                    self.clear_line();
                    Some(InputAction::Redraw)
                } else {
                    Some(InputAction::Cancel)
                }
            }
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                if self.buffer.is_empty() {
                    Some(InputAction::Exit)
                } else {
                    None
                }
            }

            // Slash command trigger
            (KeyCode::Char('/'), KeyModifiers::NONE) if self.buffer.is_empty() => {
                self.insert_char('/');
                Some(InputAction::OpenMenu)
            }

            // Normal printable character input (exclude control characters)
            (KeyCode::Char(c), _) if !c.is_control() => {
                self.insert_char(c);
                Some(InputAction::Redraw)
            }

            _ => None,
        }
    }

    /// Total count of Unicode characters in buffer
    pub fn char_count(&self) -> usize {
        self.buffer.chars().count()
    }

    /// Map character index to byte offset
    fn char_to_byte_index(&self, char_idx: usize) -> usize {
        self.buffer
            .char_indices()
            .nth(char_idx)
            .map(|(byte_idx, _)| byte_idx)
            .unwrap_or(self.buffer.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_engine_basic_typing() {
        let mut engine = TerminalInputEngine::new(vec![]);
        engine.insert_char('h');
        engine.insert_char('e');
        engine.insert_char('y');
        assert_eq!(engine.buffer, "hey");
        assert_eq!(engine.cursor_pos, 3);

        engine.delete_backspace();
        assert_eq!(engine.buffer, "he");
        assert_eq!(engine.cursor_pos, 2);
    }

    #[test]
    fn test_input_engine_multiline_newline() {
        let mut engine = TerminalInputEngine::new(vec![]);
        engine.insert_char('a');
        engine.insert_newline();
        engine.insert_char('b');
        assert_eq!(engine.buffer, "a\nb");
        assert_eq!(engine.cursor_pos, 3);
    }

    #[test]
    fn test_input_engine_word_delete() {
        let mut engine = TerminalInputEngine::new(vec![]);
        for c in "cargo test --all".chars() {
            engine.insert_char(c);
        }
        engine.delete_word_back();
        assert_eq!(engine.buffer, "cargo test ");
        engine.delete_word_back();
        assert_eq!(engine.buffer, "cargo ");
    }

    #[test]
    fn test_input_engine_ghost_text_accept() {
        let mut engine = TerminalInputEngine::new(vec![]);
        engine.insert_char('/');
        engine.insert_char('i');
        engine.ghost_text = Some("ndex --symbols".to_string());
        assert!(engine.accept_ghost_full());
        assert_eq!(engine.buffer, "/index --symbols");
        assert_eq!(engine.cursor_pos, 16);
    }

    #[test]
    fn test_input_engine_accept_ghost_word() {
        let mut engine = TerminalInputEngine::new(vec![]);
        engine.insert_char('/');
        engine.ghost_text = Some("index --symbols".to_string());
        assert!(engine.accept_ghost_word());
        assert_eq!(engine.buffer, "/index ");
    }

    #[test]
    fn test_input_engine_key_repeat_press_and_release() {
        let mut engine = TerminalInputEngine::new(vec![]);
        let press_event = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        let mut repeat_event = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
        repeat_event.kind = KeyEventKind::Repeat;
        let mut release_event = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE);
        release_event.kind = KeyEventKind::Release;

        assert_eq!(engine.handle_key(press_event), Some(InputAction::Redraw));
        assert_eq!(engine.handle_key(repeat_event), Some(InputAction::Redraw));
        assert_eq!(engine.handle_key(release_event), None);
        assert_eq!(engine.buffer, "xy");
    }

    #[test]
    fn test_input_engine_history_navigation() {
        let mut engine = TerminalInputEngine::new(vec![
            "git status".to_string(),
            "cargo test".to_string(),
        ]);
        engine.history_up();
        assert_eq!(engine.buffer, "cargo test");
        engine.history_up();
        assert_eq!(engine.buffer, "git status");
        engine.history_down();
        assert_eq!(engine.buffer, "cargo test");
        engine.history_down();
        assert_eq!(engine.buffer, "");
    }

    #[test]
    fn test_input_engine_enter_and_backspace_variations() {
        let mut engine = TerminalInputEngine::new(vec![]);
        engine.insert_char('a');
        engine.insert_char('b');
        assert_eq!(engine.buffer, "ab");

        // Backspace as '\x08'
        let bs_event = KeyEvent::new(KeyCode::Char('\x08'), KeyModifiers::NONE);
        assert_eq!(engine.handle_key(bs_event), Some(InputAction::Redraw));
        assert_eq!(engine.buffer, "a");

        // Backspace as '\x7f'
        let del_event = KeyEvent::new(KeyCode::Char('\x7f'), KeyModifiers::NONE);
        assert_eq!(engine.handle_key(del_event), Some(InputAction::Redraw));
        assert_eq!(engine.buffer, "");

        // Type task and press Enter via '\r'
        engine.insert_char('t');
        let enter_cr = KeyEvent::new(KeyCode::Char('\r'), KeyModifiers::NONE);
        assert_eq!(engine.handle_key(enter_cr), Some(InputAction::Submit("t".to_string())));

        // Type task and press Enter via KeyCode::Enter
        engine.insert_char('o');
        let enter_code = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(engine.handle_key(enter_code), Some(InputAction::Submit("o".to_string())));
    }
}
