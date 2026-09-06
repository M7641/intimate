use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Terminal;

use crate::gym::GymEvent;
use crate::ui::palette::*;

struct DashboardState {
    challenge_name: String,
    max_generations: u32,
    current_generation: u32,
    start_time: Instant,
    best_score: f64,
    ref_time_ns: u64,
    ref_memory_bytes: u64,
    leaderboard: Vec<LeaderboardEntry>,
    activity: Vec<String>,
    finished: bool,
    scroll_offset: usize,
}

struct LeaderboardEntry {
    generation: u32,
    strategy: String,
    score: f64,
    time_ns: u64,
    memory_bytes: u64,
}

impl Default for DashboardState {
    fn default() -> Self {
        Self {
            challenge_name: String::new(),
            max_generations: 0,
            current_generation: 0,
            start_time: Instant::now(),
            best_score: 0.0,
            ref_time_ns: 0,
            ref_memory_bytes: 0,
            leaderboard: Vec::new(),
            activity: Vec::new(),
            finished: false,
            scroll_offset: 0,
        }
    }
}

impl DashboardState {
    fn handle_event(&mut self, event: GymEvent) {
        match event {
            GymEvent::Started {
                challenge_name,
                max_generations,
            } => {
                self.challenge_name = challenge_name;
                self.max_generations = max_generations;
                self.start_time = Instant::now();
            }
            GymEvent::ReferenceCompiled {
                time_ns,
                memory_bytes,
                ..
            } => {
                self.ref_time_ns = time_ns;
                self.ref_memory_bytes = memory_bytes;
                self.activity
                    .push(format!("ref: {time_ns}ns, {memory_bytes}B"));
            }
            GymEvent::GenerationBegin { generation } => {
                self.current_generation = generation;
                self.activity
                    .push(format!("\u{25b8} Generation {generation}"));
            }
            GymEvent::CandidateGenerated {
                index, strategy, ..
            } => {
                self.activity
                    .push(format!("  generating #{index} ({strategy})..."));
            }
            GymEvent::CandidateEvaluated {
                index,
                passed,
                score,
                time_ns,
                memory_bytes,
                generation,
                ..
            } => {
                if passed {
                    self.activity
                        .push(format!("  \u{2713} #{index}: {score:.3}x"));
                    self.leaderboard.push(LeaderboardEntry {
                        generation,
                        strategy: String::new(),
                        score,
                        time_ns,
                        memory_bytes,
                    });
                    self.leaderboard
                        .sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
                    self.leaderboard.truncate(20);
                    self.best_score = self.best_score.max(score);
                } else {
                    self.activity.push(format!("  \u{2717} #{index}: failed"));
                }
            }
            GymEvent::GenerationEnd {
                generation,
                best_score,
            } => {
                self.activity
                    .push(format!("  gen {generation} best: {best_score:.3}x"));
            }
            GymEvent::Finished { .. } => {
                self.finished = true;
                self.activity.push("Training complete!".into());
            }
            GymEvent::Log { message } => {
                self.activity.push(message);
            }
        }

        // Keep activity buffer bounded
        if self.activity.len() > 200 {
            self.activity.drain(..100);
        }
    }

    fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }
}

/// Run the full-screen TUI dashboard.
pub async fn run(mut rx: tokio::sync::mpsc::UnboundedReceiver<GymEvent>) -> anyhow::Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = DashboardState::default();
    let mut tick_interval = tokio::time::interval(Duration::from_millis(100));

    loop {
        terminal.draw(|f| render_dashboard(f, &state))?;

        tokio::select! {
            event = rx.recv() => {
                match event {
                    Some(ev) => state.handle_event(ev),
                    None => state.finished = true,
                }
            }
            _ = tick_interval.tick() => {}
        }

        // Poll keyboard events (non-blocking)
        while event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => {
                        cleanup(&mut terminal)?;
                        return Ok(());
                    }
                    KeyCode::Up => state.scroll_up(),
                    KeyCode::Down => state.scroll_down(),
                    _ => {}
                }
            }
        }
    }
}

fn cleanup(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
    terminal::disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn render_dashboard(f: &mut ratatui::Frame, state: &DashboardState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(8),    // leaderboard
            Constraint::Min(6),    // activity
            Constraint::Length(1), // footer
        ])
        .split(f.area());

    // ── Header ──────────────────────────────────
    let elapsed = state.start_time.elapsed();
    let elapsed_str = format!(
        "{:02}:{:02}",
        elapsed.as_secs() / 60,
        elapsed.as_secs() % 60
    );
    let status = if state.finished {
        "DONE".to_string()
    } else {
        format!("Gen {}/{}", state.current_generation, state.max_generations)
    };

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " \u{25c8} nameless ",
            Style::default().fg(VIOLET).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {status} "), Style::default().fg(AMETHYST)),
        Span::styled(format!(" {elapsed_str} "), Style::default().fg(SMOKE)),
        Span::styled(
            format!(" Best: {:.3}x ", state.best_score),
            Style::default()
                .fg(if state.best_score > 1.0 {
                    AMETHYST
                } else {
                    SMOKE
                })
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(PLUM))
            .title(Span::styled(
                &state.challenge_name,
                Style::default().fg(NIGHTSHADE),
            )),
    );
    f.render_widget(header, chunks[0]);

    // ── Leaderboard ─────────────────────────────
    let header_cells = ["#", "Gen", "Score", "Time(ns)", "Memory(B)"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(SMOKE).add_modifier(Modifier::DIM)));
    let header_row = Row::new(header_cells).height(1);

    let rows: Vec<Row> = state
        .leaderboard
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let style = if i == 0 {
                Style::default().fg(AMETHYST).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(BONE)
            };
            Row::new(vec![
                Cell::from(format!("{}", i + 1)),
                Cell::from(format!("{}", entry.generation)),
                Cell::from(format!("{:.3}", entry.score)),
                Cell::from(format!("{}", entry.time_ns)),
                Cell::from(format!("{}", entry.memory_bytes)),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Length(5),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(header_row)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(PLUM))
            .title(Span::styled("Leaderboard", Style::default().fg(NIGHTSHADE))),
    );
    f.render_widget(table, chunks[1]);

    // ── Activity ────────────────────────────────
    let visible_lines: Vec<Line> = state
        .activity
        .iter()
        .skip(state.scroll_offset)
        .map(|line| {
            if line.contains('\u{2713}') {
                Line::from(Span::styled(line.as_str(), Style::default().fg(VIOLET)))
            } else if line.contains('\u{2717}') {
                Line::from(Span::styled(line.as_str(), Style::default().fg(ROUGE)))
            } else if line.starts_with('\u{25b8}') {
                Line::from(Span::styled(
                    line.as_str(),
                    Style::default().fg(AMETHYST).add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(line.as_str(), Style::default().fg(SMOKE)))
            }
        })
        .collect();

    let activity = Paragraph::new(visible_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(PLUM))
            .title(Span::styled("Activity", Style::default().fg(NIGHTSHADE))),
    );
    f.render_widget(activity, chunks[2]);

    // ── Footer ──────────────────────────────────
    let help = if state.finished {
        " q quit"
    } else {
        " q quit \u{2502} \u{2191}\u{2193} scroll"
    };
    let footer = Paragraph::new(Span::styled(help, Style::default().fg(SMOKE)));
    f.render_widget(footer, chunks[3]);
}
