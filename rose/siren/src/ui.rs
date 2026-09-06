//! Minimal styled terminal output. Kept dependency-free on purpose — no
//! ratatui here, since siren is interactive at the line level (read prompt,
//! print status, open viewer), not a full-screen TUI.

use std::io::{self, Write};

// Coral/cyan palette to keep visual identity distinct from nyx (purple).
const CORAL: &str = "\x1b[38;2;255;120;100m";
const ROSE: &str = "\x1b[38;2;220;90;120m";
const CYAN: &str = "\x1b[38;2;120;200;220m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

pub fn banner() {
    eprintln!();
    eprintln!("  {CORAL}{BOLD}siren{RESET}  {DIM}text → image, in a loop{RESET}");
    eprintln!();
}

/// Single-line progress / informational note. Stderr so stdout stays clean
/// for any pipeable output (saved paths, etc.).
pub fn status(msg: &str) {
    eprintln!("  {CORAL}{BOLD}siren{RESET} {DIM}│{RESET} {msg}");
}

pub fn warn(msg: &str) {
    eprintln!("  {ROSE}{BOLD}!{RESET} {msg}");
}

pub fn success(msg: &str) {
    eprintln!("  {CYAN}{BOLD}✓{RESET} {msg}");
}

/// Read a single line from stdin with a styled prompt prefix. Trims trailing
/// newline. Returns `Ok(None)` on EOF (Ctrl-D) so the caller can gracefully
/// exit the session loop.
pub fn read_line(prefix: &str) -> anyhow::Result<Option<String>> {
    eprint!("  {CORAL}{BOLD}▸{RESET} {prefix} ");
    io::stderr().flush()?;
    let mut buf = String::new();
    let n = io::stdin().read_line(&mut buf)?;
    if n == 0 {
        return Ok(None);
    }
    Ok(Some(buf.trim().to_string()))
}

/// Read one keystroke-ish answer where empty input means the default.
/// Used for `[K]eep / [r]eprompt / [s]ave` style menus.
pub fn read_choice(prefix: &str, default: char) -> anyhow::Result<char> {
    let answer = read_line(prefix)?.unwrap_or_default();
    Ok(answer
        .chars()
        .next()
        .unwrap_or(default)
        .to_ascii_lowercase())
}
