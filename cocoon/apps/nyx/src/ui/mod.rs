mod palette;
mod widgets;

use palette::*;

/// Animated nyx banner with pulsating purple glow.
pub fn banner() {
    widgets::banner::render();
}

/// Status message: "nyx │ msg" on stderr.
pub fn status(msg: &str) {
    let n = ansi_fg(VIOLET);
    let s = ansi_fg(PLUM);
    eprintln!("  {n}{ANSI_BOLD}nyx{ANSI_RESET} {s}{ANSI_DIM}│{ANSI_RESET} {msg}");
}

/// Display an LLM response with a purple accent bar on the left.
pub fn response(text: &str) {
    let hi = ansi_fg(NIGHTSHADE);
    let lo = ansi_fg(PLUM);
    eprintln!("  {hi}{ANSI_BOLD}▌{ANSI_RESET}");
    for line in text.lines() {
        eprintln!("  {lo}▌{ANSI_RESET} {line}");
    }
    eprintln!("  {hi}{ANSI_BOLD}▌{ANSI_RESET}");
    eprintln!();
}

/// Display a commit message in a styled box.
pub fn commit_preview(message: &str) {
    let b = ansi_fg(NIGHTSHADE);
    let a = ansi_fg(AMETHYST);
    eprintln!("  {b}╭──{ANSI_RESET} {a}{ANSI_BOLD}commit{ANSI_RESET}");
    for line in message.lines() {
        eprintln!("  {b}│{ANSI_RESET}  {line}");
    }
    eprintln!("  {b}╰──{ANSI_RESET}");
    eprintln!();
}

/// Render all clarity results (detailed reports + summary).
pub fn clarity_results(reports: &[crate::agents::clarity::FileReport], saved_to: Option<&str>) {
    widgets::clarity::render_all(reports, saved_to);
}

/// Styled prompt for user input.
pub fn prompt(msg: &str) {
    let c = ansi_fg(VIOLET);
    eprint!("  {c}{ANSI_BOLD}▸{ANSI_RESET} {msg} ");
}

/// Launch an interactive full-screen file picker.
pub fn pick_files_interactive(
    entries: &[String],
    max_select: usize,
) -> std::io::Result<Vec<usize>> {
    widgets::file_picker::pick(entries, max_select)
}
