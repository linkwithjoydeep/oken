use std::io;
use std::time::Duration;

use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Terminal;

use crate::history;
use crate::hosts;

struct PickerHost {
    host: hosts::Host,
    last_connected: Option<String>,
}

/// Open the fuzzy host picker TUI. Returns the selected host or an error if cancelled.
pub fn run_picker(initial_filter: Option<&str>) -> Result<hosts::Host> {
    let all_hosts = hosts::list_all_hosts().unwrap_or_default();
    if all_hosts.is_empty() {
        bail!("no hosts found in ~/.ssh/config or ~/.config/oken/hosts.toml");
    }

    let recent = history::last_connected_hosts().unwrap_or_default();

    // Build PickerHost list merged with history
    let mut picker_hosts: Vec<PickerHost> = all_hosts
        .into_iter()
        .map(|host| {
            let last_connected = recent
                .iter()
                .find(|r| r.alias == host.alias)
                .map(|r| r.last_connected.clone());
            PickerHost {
                host,
                last_connected,
            }
        })
        .collect();

    // Sort: hosts with history first (most recent first), then the rest alphabetically
    picker_hosts.sort_by(|a, b| {
        match (&a.last_connected, &b.last_connected) {
            (Some(a_ts), Some(b_ts)) => b_ts.cmp(a_ts), // most recent first
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.host.alias.cmp(&b.host.alias),
        }
    });

    let mut search = initial_filter.unwrap_or("").to_string();
    let mut selected: usize = 0;

    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stderr();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_picker_loop(&mut terminal, &picker_hosts, &mut search, &mut selected);

    // Restore terminal
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

fn run_picker_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stderr>>,
    picker_hosts: &[PickerHost],
    search: &mut String,
    selected: &mut usize,
) -> Result<hosts::Host> {
    loop {
        let filtered: Vec<usize> = filter_hosts(picker_hosts, search);
        let total = picker_hosts.len();
        let matched = filtered.len();

        if *selected >= matched && matched > 0 {
            *selected = matched - 1;
        }

        terminal.draw(|frame| {
            let area = frame.area();
            let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(area);

            draw_search_line(frame, chunks[0], search, matched, total);
            draw_host_list(frame, chunks[1], picker_hosts, &filtered, *selected);
        })?;

        // Poll for events
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Esc => bail!("cancelled"),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        bail!("cancelled")
                    }
                    KeyCode::Enter => {
                        if !filtered.is_empty() {
                            return Ok(picker_hosts[filtered[*selected]].host.clone());
                        }
                    }
                    KeyCode::Up => {
                        if *selected > 0 {
                            *selected -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if *selected + 1 < matched {
                            *selected += 1;
                        }
                    }
                    KeyCode::Backspace => {
                        search.pop();
                        *selected = 0;
                    }
                    KeyCode::Char(c) => {
                        search.push(c);
                        *selected = 0;
                    }
                    _ => {}
                }
            }
        }
    }
}

fn filter_hosts(picker_hosts: &[PickerHost], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..picker_hosts.len()).collect();
    }
    let q = query.to_lowercase();
    picker_hosts
        .iter()
        .enumerate()
        .filter(|(_, ph)| {
            let h = &ph.host;
            h.alias.to_lowercase().contains(&q)
                || h.hostname
                    .as_deref()
                    .is_some_and(|hn| hn.to_lowercase().contains(&q))
                || h.user
                    .as_deref()
                    .is_some_and(|u| u.to_lowercase().contains(&q))
                || h.tags.iter().any(|t| t.to_lowercase().contains(&q))
        })
        .map(|(i, _)| i)
        .collect()
}

fn draw_search_line(frame: &mut ratatui::Frame, area: Rect, search: &str, matched: usize, total: usize) {
    let count = format!("{} / {} hosts", matched, total);
    let search_text = format!("  Search: {}\u{2588}", search);
    let padding = area
        .width
        .saturating_sub(search_text.len() as u16 + count.len() as u16 + 2);

    let line = Line::from(vec![
        Span::styled(search_text, Style::default().fg(Color::White)),
        Span::raw(" ".repeat(padding as usize)),
        Span::styled(count, Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_host_list(
    frame: &mut ratatui::Frame,
    area: Rect,
    picker_hosts: &[PickerHost],
    filtered: &[usize],
    selected: usize,
) {
    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, &idx)| {
            let ph = &picker_hosts[idx];
            let h = &ph.host;

            let prefix = if i == selected { "> " } else { "  " };
            let target = match (&h.user, &h.hostname) {
                (Some(u), Some(hn)) => format!("{}@{}", u, hn),
                (None, Some(hn)) => hn.clone(),
                _ => String::new(),
            };
            let tags = if h.tags.is_empty() {
                String::new()
            } else {
                format!("[{}]", h.tags.join(", "))
            };
            let time = ph
                .last_connected
                .as_deref()
                .map(format_relative_time)
                .unwrap_or_default();

            // Pad alias to 16 chars, target to 24 chars, tags to 20 chars
            let text = format!(
                "{}{:<16} {:<24} {:<20} {}",
                prefix, h.alias, target, tags, time,
            );

            let style = if i == selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(Line::styled(text, style))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(list, area);
}

fn format_relative_time(iso: &str) -> String {
    // Parse ISO 8601 timestamp like "2026-02-27T10:30:00Z"
    let parts: Vec<&str> = iso.split('T').collect();
    if parts.len() != 2 {
        return iso.to_string();
    }
    let date_parts: Vec<u32> = parts[0].split('-').filter_map(|s| s.parse().ok()).collect();
    let time_str = parts[1].trim_end_matches('Z');
    let time_parts: Vec<u32> = time_str
        .split(':')
        .filter_map(|s| s.parse().ok())
        .collect();

    if date_parts.len() != 3 || time_parts.len() < 2 {
        return iso.to_string();
    }

    let now = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let dur = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        dur.as_secs() as i64
    };

    // Convert timestamp to proper unix seconds (rough but good enough for relative display)
    // More precise: days since epoch
    let epoch_days = epoch_days(date_parts[0], date_parts[1], date_parts[2]);
    let ts_unix = epoch_days * 86400 + time_parts[0] as i64 * 3600 + time_parts[1] as i64 * 60
        + time_parts.get(2).copied().unwrap_or(0) as i64;

    let diff = now - ts_unix;
    if diff < 0 {
        return "just now".to_string();
    }

    let minutes = diff / 60;
    let hours = diff / 3600;
    let days = diff / 86400;
    let weeks = days / 7;
    let months = days / 30;

    if minutes < 1 {
        "just now".to_string()
    } else if minutes < 60 {
        format!("{}m ago", minutes)
    } else if hours < 24 {
        format!("{}h ago", hours)
    } else if days < 7 {
        format!("{}d ago", days)
    } else if weeks < 5 {
        format!("{}w ago", weeks)
    } else {
        format!("{}mo ago", months)
    }
}

/// Days since Unix epoch for a given date (good enough for relative time).
fn epoch_days(year: u32, month: u32, day: u32) -> i64 {
    // Simplified Julian day calculation
    let y = year as i64;
    let m = month as i64;
    let d = day as i64;

    let a = (14 - m) / 12;
    let y2 = y + 4800 - a;
    let m2 = m + 12 * a - 3;
    let jdn = d + (153 * m2 + 2) / 5 + 365 * y2 + y2 / 4 - y2 / 100 + y2 / 400 - 32045;
    jdn - 2440588 // Unix epoch is Julian day 2440588
}
