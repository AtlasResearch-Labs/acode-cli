//! Atlas Daedalus — Floating Slash Command Popover Menu
//! (C) 2026 Atlas Research Labs — MIT License

use std::fmt::Write as FmtWrite;

/// A command item displayed in the floating menu
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    pub name: String,
    pub args: Option<String>,
    pub description: String,
}

/// Floating popover menu state and renderer
pub struct FloatingMenu {
    pub items: Vec<MenuItem>,
    pub selected_index: usize,
    pub is_open: bool,
    pub max_visible: usize,
    pub last_rendered_lines: usize,
}

impl FloatingMenu {
    /// Create a new floating popover menu with default system commands
    pub fn new() -> Self {
        let items = vec![
            MenuItem {
                name: "/help".to_string(),
                args: None,
                description: "Interactive command cheat sheet and cockpit guide".to_string(),
            },
            MenuItem {
                name: "/reset".to_string(),
                args: None,
                description: "Wipe working conversation memory (preserves Merkle AST)".to_string(),
            },
            MenuItem {
                name: "/diff".to_string(),
                args: None,
                description: "Review pending workspace file changes as unified diff".to_string(),
            },
            MenuItem {
                name: "/stats".to_string(),
                args: None,
                description: "Display session token telemetry and AST metrics".to_string(),
            },
            MenuItem {
                name: "/compact".to_string(),
                args: None,
                description: "Compact conversation memory to minimize context burn".to_string(),
            },
            MenuItem {
                name: "/model".to_string(),
                args: Some("<model-id>".to_string()),
                description: "Inspect or switch active LLM provider and inference model".to_string(),
            },
            MenuItem {
                name: "/swarm".to_string(),
                args: None,
                description: "Show status of background agent worker tasks and locks".to_string(),
            },
            MenuItem {
                name: "/cost".to_string(),
                args: None,
                description: "Token expenditure breakdown across all tool calls".to_string(),
            },
            MenuItem {
                name: "/review".to_string(),
                args: None,
                description: "Run automated security and style audit on git changes".to_string(),
            },
            MenuItem {
                name: "/doctor".to_string(),
                args: None,
                description: "Check environment, tool chains, and security shields".to_string(),
            },
            MenuItem {
                name: "/clear".to_string(),
                args: None,
                description: "Clear terminal buffer and re-anchor cockpit header".to_string(),
            },
            MenuItem {
                name: "/exit".to_string(),
                args: None,
                description: "Gracefully quit session and persist command history".to_string(),
            },
        ];

        Self {
            items,
            selected_index: 0,
            is_open: false,
            max_visible: 6,
            last_rendered_lines: 0,
        }
    }

    /// Filter menu items by query prefix and return indices into self.items
    pub fn filter_indices(&self, query: &str) -> Vec<usize> {
        if query.is_empty() || query == "/" {
            return (0..self.items.len()).collect();
        }

        let q_clean = query.to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                item.name.to_lowercase().starts_with(&q_clean)
                    || item.name.to_lowercase().contains(&q_clean.trim_start_matches('/'))
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Select previous item in filtered view
    pub fn select_prev(&mut self, filtered_count: usize) {
        if filtered_count == 0 {
            self.selected_index = 0;
            return;
        }
        if self.selected_index == 0 {
            self.selected_index = filtered_count - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    /// Select next item in filtered view
    pub fn select_next(&mut self, filtered_count: usize) {
        if filtered_count == 0 {
            self.selected_index = 0;
            return;
        }
        if self.selected_index + 1 >= filtered_count {
            self.selected_index = 0;
        } else {
            self.selected_index += 1;
        }
    }

    /// Clamp selected index if filtered list shrank
    pub fn clamp_selection(&mut self, filtered_count: usize) {
        if filtered_count == 0 {
            self.selected_index = 0;
        } else if self.selected_index >= filtered_count {
            self.selected_index = filtered_count - 1;
        }
    }

    /// Retrieve currently selected item given the active query
    pub fn get_selected_item(&self, query: &str) -> Option<&MenuItem> {
        let indices = self.filter_indices(query);
        if self.selected_index < indices.len() {
            Some(&self.items[indices[self.selected_index]])
        } else {
            None
        }
    }

    /// Render floating menu above prompt into a formatted string
    pub fn render_popover(&mut self, query: &str, term_width: u16) -> String {
        let indices = self.filter_indices(query);
        let count = indices.len();
        self.clamp_selection(count);

        if count == 0 {
            self.last_rendered_lines = 0;
            return String::new();
        }

        let max_vis = self.max_visible.min(count);
        // Calculate sliding window offset
        let start_idx = if self.selected_index >= max_vis {
            self.selected_index + 1 - max_vis
        } else {
            0
        };
        let end_idx = (start_idx + max_vis).min(count);

        let mut out = String::new();
        let border_color = "\x1b[38;2;45;55;72m"; // Subtle slate charcoal
        let active_bg = "\x1b[48;2;26;32;44m";
        let cyan_hl = "\x1b[38;2;56;189;248m";
        let white_txt = "\x1b[38;2;241;245;249m";
        let slate_dim = "\x1b[38;2;148;163;184m";
        let charcoal_hint = "\x1b[38;2;100;116;139m";
        let reset = "\x1b[0m";

        let usable_width = (term_width as usize).saturating_sub(4).max(40).min(90);

        // Header border
        let _ = writeln!(
            out,
            "  {border_color}┌─ Slash Commands ───────────────────────────────────────────┐{reset}"
        );
        let mut lines_count = 1;

        for (i, &item_idx) in indices[start_idx..end_idx].iter().enumerate() {
            let actual_idx = start_idx + i;
            let is_selected = actual_idx == self.selected_index;
            let item = &self.items[item_idx];

            let marker = if is_selected {
                format!("{cyan_hl}►{reset}")
            } else {
                " ".to_string()
            };

            let name_colored = if is_selected {
                format!("{cyan_hl}{:<10}{reset}", item.name)
            } else {
                format!("{white_txt}{:<10}{reset}", item.name)
            };

            let args_part = if let Some(a) = &item.args {
                format!("{charcoal_hint}{:<12}{reset}", a)
            } else {
                format!("{:<12}", "")
            };

            let desc_trunc = if item.description.len() > 36 {
                format!("{}...", &item.description[..33])
            } else {
                item.description.clone()
            };
            let desc_part = format!("{slate_dim}{:<36}{reset}", desc_trunc);

            let row_line = format!(
                "  {border_color}│{reset} {marker} {name_colored} {args_part} {desc_part} {border_color}│{reset}"
            );

            if is_selected {
                let _ = writeln!(out, "{active_bg}{row_line}{reset}");
            } else {
                let _ = writeln!(out, "{row_line}");
            }
            lines_count += 1;
        }

        // Footer / Scroll indicator
        let scroll_info = if count > max_vis {
            format!(" ({}/{} commands) ", self.selected_index + 1, count)
        } else {
            " [↑/↓ Navigate • Tab Select • Esc Close] ".to_string()
        };

        let pad_len = usable_width.saturating_sub(scroll_info.len() + 6);
        let pad_str = "─".repeat(pad_len.min(40));

        let _ = writeln!(
            out,
            "  {border_color}└─{slate_dim}{scroll_info}{border_color}{pad_str}┘{reset}"
        );
        lines_count += 1;

        self.last_rendered_lines = lines_count;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_filtering() {
        let menu = FloatingMenu::new();
        let matches = menu.filter_indices("/h");
        assert_eq!(matches.len(), 1);
        assert_eq!(menu.items[matches[0]].name, "/help");

        let all = menu.filter_indices("/");
        assert!(all.len() >= 10);
    }

    #[test]
    fn test_menu_navigation_wrap() {
        let mut menu = FloatingMenu::new();
        let count = 4;
        menu.select_prev(count);
        assert_eq!(menu.selected_index, 3);
        menu.select_next(count);
        assert_eq!(menu.selected_index, 0);
        menu.select_next(count);
        assert_eq!(menu.selected_index, 1);
    }

    #[test]
    fn test_menu_rendering_output() {
        let mut menu = FloatingMenu::new();
        let popover = menu.render_popover("/", 80);
        assert!(popover.contains("Slash Commands"));
        assert!(popover.contains("/help"));
        assert!(menu.last_rendered_lines > 0);
    }

    #[test]
    fn test_menu_get_selected() {
        let menu = FloatingMenu::new();
        let sel = menu.get_selected_item("/h").unwrap();
        assert_eq!(sel.name, "/help");
    }
}
