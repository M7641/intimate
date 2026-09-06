#![allow(dead_code)]

use ratatui::style::{Color, Modifier, Style};

// ── Royal purple core ───────────────────────────
pub const VIOLET: Color = Color::Rgb(130, 50, 180);
pub const AMETHYST: Color = Color::Rgb(160, 80, 200);
pub const NIGHTSHADE: Color = Color::Rgb(90, 30, 130);
pub const PLUM: Color = Color::Rgb(55, 15, 75);
pub const SHADOW: Color = Color::Rgb(35, 8, 50);

// ── Warm accents (the tang) ─────────────────────
pub const ROUGE: Color = Color::Rgb(190, 35, 55);
pub const EMBER: Color = Color::Rgb(255, 70, 70);

// ── Neutrals ────────────────────────────────────
pub const SMOKE: Color = Color::Rgb(110, 100, 120);
pub const BONE: Color = Color::Rgb(225, 218, 230);

pub fn bold(color: Color) -> Style {
    Style::new().fg(color).add_modifier(Modifier::BOLD)
}

pub fn dim(color: Color) -> Style {
    Style::new().fg(color).add_modifier(Modifier::DIM)
}

pub fn fg(color: Color) -> Style {
    Style::new().fg(color)
}

/// Convert a ratatui Color::Rgb to a raw ANSI escape sequence.
pub fn ansi_fg(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("\x1b[38;2;{r};{g};{b}m"),
        _ => String::new(),
    }
}

pub const ANSI_BOLD: &str = "\x1b[1m";
pub const ANSI_DIM: &str = "\x1b[2m";
pub const ANSI_RESET: &str = "\x1b[0m";
