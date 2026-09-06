pub mod palette;
pub mod widgets;

use palette::*;

use crate::db;
use crate::gym::GymEvent;

/// Animated nameless banner with pulsating purple glow.
pub fn banner() {
    widgets::banner::render();
}

/// Status message: "nameless │ msg" on stderr.
pub fn status(msg: &str) {
    let n = ansi_fg(VIOLET);
    let s = ansi_fg(PLUM);
    eprintln!("  {n}{ANSI_BOLD}nameless{ANSI_RESET} {s}{ANSI_DIM}\u{2502}{ANSI_RESET} {msg}");
}

/// Print GymEvents to stderr (--no-tui mode).
pub async fn print_events(mut rx: tokio::sync::mpsc::UnboundedReceiver<GymEvent>) {
    while let Some(event) = rx.recv().await {
        match event {
            GymEvent::Started {
                challenge_name,
                max_generations,
            } => {
                status(&format!(
                    "challenge: {challenge_name} ({max_generations} generations)"
                ));
            }
            GymEvent::ReferenceCompiled {
                time_ns,
                memory_bytes,
                binary_size,
            } => {
                status(&format!(
                    "reference: {time_ns}ns, {memory_bytes}B mem, {binary_size}B binary"
                ));
            }
            GymEvent::GenerationBegin { generation } => {
                let a = ansi_fg(AMETHYST);
                eprintln!("\n  {a}{ANSI_BOLD}\u{25b8} Generation {generation}{ANSI_RESET}");
            }
            GymEvent::CandidateGenerated {
                index, strategy, ..
            } => {
                let s = ansi_fg(SMOKE);
                eprintln!(
                    "  {s}{ANSI_DIM}    generating candidate {index} ({strategy})...{ANSI_RESET}"
                );
            }
            GymEvent::CandidateEvaluated {
                index,
                passed,
                score,
                time_ns,
                memory_bytes,
                ..
            } => {
                if passed {
                    let v = ansi_fg(VIOLET);
                    eprintln!(
                        "  {v}    \u{2713} candidate {index}: score {score:.3} ({time_ns}ns, {memory_bytes}B){ANSI_RESET}"
                    );
                } else {
                    let r = ansi_fg(ROUGE);
                    eprintln!("  {r}    \u{2717} candidate {index}: failed{ANSI_RESET}");
                }
            }
            GymEvent::GenerationEnd {
                generation,
                best_score,
            } => {
                let a = ansi_fg(AMETHYST);
                eprintln!("  {a}    gen {generation} best: {best_score:.3}x{ANSI_RESET}");
            }
            GymEvent::Finished {
                best_score,
                total_candidates,
                elapsed_secs,
            } => {
                let n = ansi_fg(NIGHTSHADE);
                let v = ansi_fg(VIOLET);
                eprintln!();
                eprintln!("  {n}{ANSI_BOLD}\u{2550}\u{2550}\u{2550} Results \u{2550}\u{2550}\u{2550}{ANSI_RESET}");
                eprintln!("  {v}  best score:  {best_score:.3}x{ANSI_RESET}");
                eprintln!("  {v}  candidates:  {total_candidates}{ANSI_RESET}");
                eprintln!("  {v}  elapsed:     {elapsed_secs:.1}s{ANSI_RESET}");
                eprintln!();
            }
            GymEvent::Log { message } => {
                status(&message);
            }
        }
    }
}

/// Display a summary of all challenges with their best scores.
pub fn leaderboard_summary(rows: &[db::ChallengeSummary]) {
    let n = ansi_fg(NIGHTSHADE);
    let a = ansi_fg(AMETHYST);
    let v = ansi_fg(VIOLET);
    let s = ansi_fg(SMOKE);

    eprintln!();
    eprintln!(
        "  {n}{ANSI_BOLD}\u{2550}\u{2550}\u{2550} Challenges \u{2550}\u{2550}\u{2550}{ANSI_RESET}"
    );
    eprintln!("  {s}{ANSI_DIM}  Name                 Best Score  Candidates{ANSI_RESET}");
    for row in rows {
        let score_str = if row.best_score > 0.0 {
            format!("{:.3}x", row.best_score)
        } else {
            "—".to_string()
        };
        eprintln!(
            "  {v}  {:<20} {a}{:>10}  {s}{:>10}{ANSI_RESET}",
            row.name, score_str, row.total_candidates
        );
    }
    eprintln!();
}

/// Display full leaderboard for a single challenge.
pub fn full_leaderboard(name: &str, rows: &[db::LeaderboardRow]) {
    let n = ansi_fg(NIGHTSHADE);
    let a = ansi_fg(AMETHYST);
    let v = ansi_fg(VIOLET);
    let s = ansi_fg(SMOKE);

    eprintln!();
    eprintln!(
        "  {n}{ANSI_BOLD}\u{2550}\u{2550}\u{2550} {name} \u{2550}\u{2550}\u{2550}{ANSI_RESET}"
    );
    eprintln!("  {s}{ANSI_DIM}  #    Gen  Strategy     Score     Time(ns)  Memory(B){ANSI_RESET}");
    for row in rows.iter().take(20) {
        eprintln!(
            "  {v}  {:<4} {:<4} {:<12} {a}{:>8.3}  {s}{:>9}  {:>9}{ANSI_RESET}",
            row.rank, row.generation, row.strategy, row.score, row.time_ns, row.memory_bytes,
        );
    }
    eprintln!();
}
