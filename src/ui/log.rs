use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use super::{accent, added, cap_first, deleted, warn};
use crate::app::{LogEntry, LogKind};

const MAX_ENTRIES: usize = 1000;

fn entry_line(e: &LogEntry) -> Line<'static> {
    if matches!(e.kind, LogKind::Command) {
        return Line::from(Span::styled(
            format!("  {}", e.text),
            Style::new().fg(Color::Reset),
        ));
    }
    let color = match e.kind {
        LogKind::Active => accent(),
        LogKind::Done => added(),
        LogKind::Warn => warn(),
        LogKind::Error => deleted(),
        _ => Color::Reset,
    };
    Line::from(vec![
        Span::styled(format!("[{}] ", e.at), Style::new().fg(Color::Gray)),
        Span::styled(cap_first(&e.text), Style::new().fg(color)),
    ])
}

fn build_line(cells: Vec<(char, Style)>) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (c, st) in cells {
        match spans.last_mut() {
            Some(s) if s.style == st => s.content.to_mut().push(c),
            _ => spans.push(Span::styled(c.to_string(), st)),
        }
    }
    Line::from(spans)
}

fn wrap_cells(cells: &[(char, Style)], width: usize) -> Vec<Vec<(char, Style)>> {
    let mut rows: Vec<Vec<(char, Style)>> = Vec::new();
    let mut cur: Vec<(char, Style)> = Vec::new();
    let mut last_space: Option<usize> = None;
    for &(c, st) in cells {
        if cur.len() >= width {
            match last_space.filter(|&sp| sp > 0) {
                Some(sp) => {
                    let rest = cur.split_off(sp + 1);
                    rows.push(std::mem::replace(&mut cur, rest));
                    last_space = cur.iter().rposition(|(c, _)| *c == ' ');
                }
                None => {
                    rows.push(std::mem::take(&mut cur));
                    last_space = None;
                }
            }
        }
        if cur.is_empty() && c == ' ' {
            continue;
        }
        cur.push((c, st));
        if c == ' ' {
            last_space = Some(cur.len() - 1);
        }
    }
    if !cur.is_empty() {
        rows.push(cur);
    }
    rows
}
