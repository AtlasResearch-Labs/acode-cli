//! Atlas Daedalus — Terminal Mascot & Binary Art Engine
//! (C) 2026 Atlas Research Labs — MIT License
//!
//! Compliant with Atlas Research Labs Permanent Engineering & Design Laws:
//! - Zero Emoji Policy: Pure ASCII, UTF-8 geometric box-drawing, binary numbers, and typographic tokens.
//! - OLED Pitch Black Palette: Indigo, Slate, Amber, and brilliant White ANSI TrueColor formatting.
//! - Multi-Tier Viewport Responsiveness: Automatically adapts between Compact (<60), Standard (60-99), and Ultrawide (100+).

use crate::cli::tui::TuiRenderer;
use crate::cli::viewport::{pad_box_line, Viewport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MascotVariant {
    Automaton,
    WingedCrest,
    BinaryMatrix,
}

impl MascotVariant {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().trim() {
            "automaton" | "raptor" | "1" => Some(Self::Automaton),
            "winged" | "crest" | "wings" | "2" => Some(Self::WingedCrest),
            "binary" | "matrix" | "math" | "3" => Some(Self::BinaryMatrix),
            _ => None,
        }
    }
}

pub struct MascotEngine;

impl MascotEngine {
    /// Render the Greek Daedalus Mascot to stdout
    pub fn print_mascot(variant: MascotVariant) {
        let vp = Viewport::current();
        let width = vp.card_width(82);

        match variant {
            MascotVariant::Automaton => Self::render_automaton(width, vp.is_compact()),
            MascotVariant::WingedCrest => Self::render_winged_crest(width, vp.is_compact()),
            MascotVariant::BinaryMatrix => Self::render_binary_matrix(width, vp.is_compact()),
        }
    }

    /// Concept 1: The Mechanical Automaton Raptor (Clockwork Sentinel of Knossos)
    fn render_automaton(width: usize, is_compact: bool) {
        if is_compact {
            println!(
                "{}",
                pad_box_line(
                    &format!(
                        "  {}[D.E.D]{} DAEDALUS AUTOMATON · RUST ENGINE",
                        TuiRenderer::AMBER,
                        TuiRenderer::RESET
                    ),
                    width,
                    TuiRenderer::INDIGO,
                    TuiRenderer::BOX_V,
                    TuiRenderer::RESET
                )
            );
            return;
        }

        let art = [
            "               .---.             .---.               ",
            "              /     \\           /     \\              ",
            "             | [O] [O] |       | [O] [O] |             ",
            "              \\   ^   /         \\   ^   /              ",
            "         __..--'\"\"'--..__   __..--'\"\"'--..__         ",
            "       .'   /  \\     /  \\ '.'   /  \\     /  \\ '.       ",
            "      /    |    |   |    | \\   |    |   |    |  \\      ",
            "     |     |    |   |    |  |  |    |   |    |   |     ",
            "     |     /    /   \\    \\  |  /    /   \\    \\   |     ",
            "      \\___/    /     \\    \\_|_/    /     \\    \\_/      ",
            "       \\______/       \\___________/       \\______/     ",
            "            \\           /       \\           /          ",
            "             |   <+>   | [D.A.E] |   <+>   |           ",
            "             |         |DAEDALUS |         |           ",
            "              \\       / \\_______/ \\       /            ",
            "               '-----'             '-----'             ",
        ];

        println!();
        for line in art {
            println!("  {}{}{}", TuiRenderer::SLATE, line, TuiRenderer::RESET);
        }
        println!(
            "  {}[DAEDALUS_AUTOMATON]{} Greek Clockwork Raptor · Native Rust Terminal Sentinel\n",
            TuiRenderer::AMBER,
            TuiRenderer::RESET
        );
    }

    /// Concept 2: The Architectural Winged Crest (Daedalus Wings spanning Knossos)
    fn render_winged_crest(width: usize, is_compact: bool) {
        if is_compact {
            println!(
                "{}",
                pad_box_line(
                    &format!(
                        "  {}/\\/\\{} ATLAS DAEDALUS v{}",
                        TuiRenderer::AMBER,
                        TuiRenderer::RESET,
                        env!("CARGO_PKG_VERSION")
                    ),
                    width,
                    TuiRenderer::INDIGO,
                    TuiRenderer::BOX_V,
                    TuiRenderer::RESET
                )
            );
            return;
        }

        let art = [
            "                 /\\                 /\\                 ",
            "                /  \\               /  \\                ",
            "               /    \\     _._     /    \\               ",
            "              /  /\\  \\  .'   '.  /  /\\  \\              ",
            "             /  /  \\  \\/   +   \\/  /  \\  \\             ",
            "            /  /    \\  |   D   |  /    \\  \\            ",
            "           /  /      \\_\\   *  /_/      \\  \\           ",
            "          /  /     ___/ '---' \\___     \\  \\          ",
            "         /  /____/                 \\____\\  \\         ",
            "        /___________________________________\\        ",
            "        \\=====[ ATLAS DAEDALUS v1.7.3 ]=====/        ",
            "         \\       SOVEREIGN RUST ENGINE     /         ",
            "          \\_______________________________/          ",
        ];

        println!();
        for line in art {
            println!("  {}{}{}", TuiRenderer::WHITE, line, TuiRenderer::RESET);
        }
        println!(
            "  {}[WINGED_CREST]{} Mechanical Wings of Daedalus · Sovereign AST Primitive\n",
            TuiRenderer::AMBER,
            TuiRenderer::RESET
        );
    }

    /// Concept 3: The Binary & Math Labyrinth Sentinel (Matrix Cipher)
    fn render_binary_matrix(width: usize, is_compact: bool) {
        if is_compact {
            println!(
                "{}",
                pad_box_line(
                    "  01000100 01000001 [DAEDALUS_CIPHER]",
                    width,
                    TuiRenderer::INDIGO,
                    TuiRenderer::BOX_V,
                    TuiRenderer::RESET
                )
            );
            return;
        }

        println!();
        println!(
            "  {}01000100 01000001 01000101 01000100 01000001 01001100 01010101 01010011 [DAEDALUS]{}",
            TuiRenderer::SLATE,
            TuiRenderer::RESET
        );
        println!(
            "  {}╭────────────────────────────────────────────────────────────────────────────────╮{}",
            TuiRenderer::INDIGO,
            TuiRenderer::RESET
        );
        println!(
            "  {}│{} 01  {}╭───────────╮{}  10  {}╭───╮{}  00  {}┌───┐{}  11  {}╭───╮{}  01  │  00  {}┌───┐{}  01  │ 10 {}│{}",
            TuiRenderer::INDIGO,
            TuiRenderer::RESET,
            TuiRenderer::WHITE,
            TuiRenderer::RESET,
            TuiRenderer::WHITE,
            TuiRenderer::RESET,
            TuiRenderer::WHITE,
            TuiRenderer::RESET,
            TuiRenderer::WHITE,
            TuiRenderer::RESET,
            TuiRenderer::WHITE,
            TuiRenderer::RESET,
            TuiRenderer::INDIGO,
            TuiRenderer::RESET
        );
        println!(
            "  {}│{} 10  {}│ 01000100  │{}  01  {}│ + │{}  11  {}│ * │{}  00  {}│ ^ │{}  10  │  01  {}│ 01 │{}  11  │ 01 {}│{}",
            TuiRenderer::INDIGO,
            TuiRenderer::RESET,
            TuiRenderer::WHITE,
            TuiRenderer::RESET,
            TuiRenderer::AMBER,
            TuiRenderer::RESET,
            TuiRenderer::AMBER,
            TuiRenderer::RESET,
            TuiRenderer::AMBER,
            TuiRenderer::RESET,
            TuiRenderer::WHITE,
            TuiRenderer::RESET,
            TuiRenderer::INDIGO,
            TuiRenderer::RESET
        );
        println!(
            "  {}│{} 00  {}│ DAEDALUS  │{}  11  {}│   │{}  01  {}│   │{}  10  {}│   │{}  00  ┴─ 10  {}└───┘{}  00  ┴─ 11 {}│{}",
            TuiRenderer::INDIGO,
            TuiRenderer::RESET,
            TuiRenderer::WHITE,
            TuiRenderer::RESET,
            TuiRenderer::WHITE,
            TuiRenderer::RESET,
            TuiRenderer::WHITE,
            TuiRenderer::RESET,
            TuiRenderer::WHITE,
            TuiRenderer::RESET,
            TuiRenderer::WHITE,
            TuiRenderer::RESET,
            TuiRenderer::INDIGO,
            TuiRenderer::RESET
        );
        println!(
            "  {}│{} 11  {}╰───────────╯{}  00  {}╰───╯{}  10  {}└───┘{}  01  {}╰───╯{}  11  ── 01  01000100   10 {}│{}",
            TuiRenderer::INDIGO,
            TuiRenderer::RESET,
            TuiRenderer::WHITE,
            TuiRenderer::RESET,
            TuiRenderer::WHITE,
            TuiRenderer::RESET,
            TuiRenderer::WHITE,
            TuiRenderer::RESET,
            TuiRenderer::WHITE,
            TuiRenderer::RESET,
            TuiRenderer::INDIGO,
            TuiRenderer::RESET
        );
        println!(
            "  {}╰────────────────────────────────────────────────────────────────────────────────╯{}",
            TuiRenderer::INDIGO,
            TuiRenderer::RESET
        );
        println!(
            "  {}0xDAED4105 · SHA-256 [PURE RUST RUNTIME] · SUBAGENT SWARM DISPATCH ACTIVE{}\n",
            TuiRenderer::AMBER,
            TuiRenderer::RESET
        );
    }
}
