use ratatui::style::Color;

use crate::agents::clarity::{FileReport, Finding};
use crate::ui::palette::*;

const GREEN: Color = Color::Rgb(80, 200, 80);

fn gauge_color(score: u8) -> Color {
    match score {
        1..=3 => Color::Rgb(80, 200, 80),
        4..=6 => Color::Rgb(220, 180, 40),
        7..=8 => Color::Rgb(240, 100, 30),
        _ => Color::Rgb(255, 50, 50),
    }
}

fn gauge_bar(score: u8) -> String {
    let filled = score.min(10) as usize;
    let empty = 10 - filled;
    let gc = ansi_fg(gauge_color(score));
    let sm = ansi_fg(SMOKE);
    format!(
        "{gc}{}{ANSI_RESET}{ANSI_DIM}{sm}{}{ANSI_RESET}",
        "█".repeat(filled),
        "░".repeat(empty),
    )
}

fn print_report(filename: &str, findings: &[Finding]) {
    let b = ansi_fg(NIGHTSHADE);
    let a = ansi_fg(AMETHYST);
    let v = ansi_fg(VIOLET);
    let sm = ansi_fg(SMOKE);
    let bn = ansi_fg(BONE);
    let w = ansi_fg(Color::Rgb(200, 160, 60));

    eprintln!("  {b}╭──{ANSI_RESET} {a}{ANSI_BOLD}clarity scan ── {filename}{ANSI_RESET}");
    eprintln!(
        "  {b}│{ANSI_RESET}  {sm}{ANSI_DIM}found {} items needing explanation{ANSI_RESET}",
        findings.len()
    );
    eprintln!("  {b}│{ANSI_RESET}");

    for (i, f) in findings.iter().enumerate() {
        let bar = gauge_bar(f.confidence);
        eprintln!(
            "  {b}│{ANSI_RESET}  {v}{ANSI_BOLD}{:>2}.{ANSI_RESET} {w}{ANSI_BOLD}L{:<6}{ANSI_RESET} {bar}  {bn}{ANSI_BOLD}{}/10{ANSI_RESET}",
            i + 1,
            f.line,
            f.confidence,
        );
        eprintln!(
            "  {b}│{ANSI_RESET}      {sm}{ANSI_DIM}code:{ANSI_RESET}   {}",
            f.code
        );
        eprintln!(
            "  {b}│{ANSI_RESET}      {sm}{ANSI_DIM}reason:{ANSI_RESET} {}",
            f.reason
        );
        if i < findings.len() - 1 {
            eprintln!("  {b}│{ANSI_RESET}");
        }
    }

    eprintln!("  {b}╰──{ANSI_RESET}");
}

fn print_summary(results: &[(String, usize)], saved_to: Option<&str>) {
    let b = ansi_fg(NIGHTSHADE);
    let a = ansi_fg(AMETHYST);
    let sm = ansi_fg(SMOKE);
    let bn = ansi_fg(BONE);
    let p = ansi_fg(PLUM);
    let g = ansi_fg(GREEN);
    let r = ansi_fg(ROUGE);

    let total_files = results.len();
    let flagged: usize = results.iter().filter(|(_, n)| *n > 0).count();
    let total_findings: usize = results.iter().map(|(_, n)| *n).sum();

    eprintln!(
        "  {b}╭──{ANSI_RESET} {a}{ANSI_BOLD}clarity{ANSI_RESET} {sm}{ANSI_DIM}── {total_files} file{} scanned{ANSI_RESET}",
        if total_files == 1 { "" } else { "s" }
    );
    eprintln!("  {b}│{ANSI_RESET}");

    for (path, count) in results {
        if *count == 0 {
            eprintln!(
                "  {b}│{ANSI_RESET}  {g}{ANSI_BOLD}✓{ANSI_RESET}  {bn}{path}{ANSI_RESET}  {g}{ANSI_DIM}clean{ANSI_RESET}"
            );
        } else {
            eprintln!(
                "  {b}│{ANSI_RESET}  {r}{ANSI_BOLD}⚑{ANSI_RESET}  {bn}{path}{ANSI_RESET}  {r}{ANSI_BOLD}{count} finding{}{ANSI_RESET}",
                if *count == 1 { "" } else { "s" }
            );
        }
    }

    eprintln!("  {b}│{ANSI_RESET}");

    if flagged == 0 {
        eprintln!("  {b}│{ANSI_RESET}  {g}{ANSI_BOLD}all clean{ANSI_RESET}");
    } else {
        eprintln!(
            "  {b}│{ANSI_RESET}  {sm}{ANSI_DIM}{flagged} file{} flagged{ANSI_RESET} {p}{ANSI_DIM}·{ANSI_RESET} {sm}{ANSI_DIM}{total_findings} total finding{}{ANSI_RESET}",
            if flagged == 1 { "" } else { "s" },
            if total_findings == 1 { "" } else { "s" },
        );
    }

    if let Some(path) = saved_to {
        eprintln!("  {b}│{ANSI_RESET}  {sm}{ANSI_DIM}saved to{ANSI_RESET} {bn}{path}{ANSI_RESET}");
    }

    eprintln!("  {b}╰──{ANSI_RESET}");
}

/// Render all clarity results: detailed reports then summary.
pub fn render_all(reports: &[FileReport], saved_to: Option<&str>) {
    for report in reports {
        if !report.findings.is_empty() {
            print_report(&report.path, &report.findings);
            eprintln!();
        }
    }

    let summary: Vec<(String, usize)> = reports
        .iter()
        .map(|r| (r.path.clone(), r.findings.len()))
        .collect();
    print_summary(&summary, saved_to);
}
