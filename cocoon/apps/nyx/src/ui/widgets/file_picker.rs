use std::io;
use std::time::Duration;

use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::ExecutableCommand;
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};

use crate::ui::palette::*;

struct PickerState {
    list_state: ListState,
    selected: Vec<bool>,
    max_select: usize,
}

impl PickerState {
    fn new(count: usize, max_select: usize) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            list_state,
            selected: vec![false; count],
            max_select,
        }
    }

    fn next(&mut self, count: usize) {
        let i = self.list_state.selected().map_or(0, |i| (i + 1) % count);
        self.list_state.select(Some(i));
    }

    fn previous(&mut self, count: usize) {
        let i = self
            .list_state
            .selected()
            .map_or(0, |i| if i == 0 { count - 1 } else { i - 1 });
        self.list_state.select(Some(i));
    }

    fn toggle(&mut self) {
        if let Some(i) = self.list_state.selected() {
            if self.selected[i] {
                self.selected[i] = false;
            } else {
                let current = self.selected.iter().filter(|s| **s).count();
                if self.max_select == 0 || current < self.max_select {
                    self.selected[i] = true;
                }
            }
        }
    }

    fn confirmed_indices(&self) -> Vec<usize> {
        self.selected
            .iter()
            .enumerate()
            .filter(|(_, s)| **s)
            .map(|(i, _)| i)
            .collect()
    }

    fn selection_count(&self) -> usize {
        self.selected.iter().filter(|s| **s).count()
    }
}

/// Launch a full-screen interactive file picker.
/// Returns indices of selected entries. Empty vec if cancelled.
pub fn pick(entries: &[String], max_select: usize) -> io::Result<Vec<usize>> {
    enable_raw_mode()?;
    io::stderr().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stderr());
    let mut terminal = Terminal::new(backend)?;
    let mut state = PickerState::new(entries.len(), max_select);

    let result = run_loop(&mut terminal, entries, &mut state);

    // Always restore terminal state
    drop(terminal);
    disable_raw_mode()?;
    io::stderr().execute(LeaveAlternateScreen)?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stderr>>,
    entries: &[String],
    state: &mut PickerState,
) -> io::Result<Vec<usize>> {
    loop {
        terminal.draw(|f| render_picker(f, entries, state))?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => state.next(entries.len()),
                KeyCode::Up | KeyCode::Char('k') => state.previous(entries.len()),
                KeyCode::Char(' ') => state.toggle(),
                KeyCode::Enter => return Ok(state.confirmed_indices()),
                KeyCode::Char('q') | KeyCode::Esc => return Ok(vec![]),
                _ => {}
            }
        }
    }
}

fn render_picker(frame: &mut Frame, entries: &[String], state: &mut PickerState) {
    let area = frame.area();

    let chunks = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(area);

    // Build list items
    let items: Vec<ListItem> = entries
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let marker = if state.selected[i] { "[x]" } else { "[ ]" };
            let style = if state.selected[i] {
                Style::new().fg(VIOLET).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(BONE)
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{marker} "), style),
                Span::styled(path.as_str(), style),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::bordered()
                .title(Span::styled(" nyx ─ select files ", bold(AMETHYST)))
                .border_style(fg(NIGHTSHADE)),
        )
        .highlight_style(
            Style::new()
                .bg(AMETHYST)
                .fg(Color::Rgb(255, 255, 255))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    frame.render_stateful_widget(list, chunks[0], &mut state.list_state);

    // Footer
    let limit_text = if state.max_select > 0 {
        format!(" (max {})", state.max_select)
    } else {
        String::new()
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" Space", bold(VIOLET)),
        Span::styled(": toggle │ ", fg(SMOKE)),
        Span::styled("Enter", bold(VIOLET)),
        Span::styled(": confirm │ ", fg(SMOKE)),
        Span::styled("q", bold(VIOLET)),
        Span::styled(": cancel │ ", fg(SMOKE)),
        Span::styled(
            format!("{} selected{limit_text}", state.selection_count()),
            fg(AMETHYST),
        ),
    ]));

    frame.render_widget(footer, chunks[1]);
}
