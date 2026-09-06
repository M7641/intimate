//! Tiny terminal output helpers for `warden`.
//!
//! Deliberately dependency-free: `warden` is a small CLI, so instead of pulling
//! in `service-kit`'s styling (which drags axum + a database backend) it carries
//! its own. Colour is applied only when stdout is a real terminal; piped or CI
//! output stays plain.

use std::io::IsTerminal;

/// Whether to emit ANSI colour — true only on an interactive terminal.
fn coloured() -> bool {
    std::io::stdout().is_terminal()
}

/// Wrap `text` in an ANSI SGR code when colour is enabled, else return it plain.
fn paint(code: &str, text: &str) -> String {
    if coloured() {
        format!("\u{1b}[{code}m{text}\u{1b}[0m")
    } else {
        text.to_string()
    }
}

/// A bold section header, e.g. `warden — deploy`.
pub fn header(subtitle: &str) {
    println!();
    println!("{}", paint("1;35", &format!("warden — {subtitle}")));
}

/// A neutral informational line.
pub fn note(message: &str) {
    println!("  {message}");
}

/// A success line (green check).
pub fn success(message: &str) {
    println!("{} {message}", paint("1;32", "✓"));
}

/// A warning line (yellow), for the gaps an operator must close.
pub fn warn(message: &str) {
    println!("{} {message}", paint("1;33", "!"));
}

/// An error line (red), printed to stderr.
pub fn error(message: &str) {
    eprintln!("{} {message}", paint("1;31", "✗"));
}
