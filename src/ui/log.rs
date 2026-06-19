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

fn wrap_line(line: &Line, width: usize) -> Vec<Line<'static>> {
    let cells: Vec<(char, Style)> = line
        .spans
        .iter()
        .flat_map(|s| s.content.chars().map(move |c| (c, s.style)))
        .collect();
    if width == 0 || cells.len() <= width {
        return vec![build_line(cells)];
    }
    let indent = cells.iter().take_while(|(c, _)| *c == ' ').count();
    let pad_style = cells.first().map(|(_, s)| *s).unwrap_or_default();
    let avail = width.saturating_sub(indent).max(1);
    wrap_cells(&cells[indent..], avail)
        .into_iter()
        .map(|chunk| {
            let mut row: Vec<(char, Style)> = vec![(' ', pad_style); indent];
            row.extend(chunk);
            build_line(row)
        })
        .collect()
}

pub(crate) fn log_rows(log: &[LogEntry], width: usize) -> Vec<Line<'static>> {
    let start = log.len().saturating_sub(MAX_ENTRIES);
    log[start..]
        .iter()
        .flat_map(|e| wrap_line(&entry_line(e), width))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(text: &str) -> LogEntry {
        LogEntry {
            at: "10:00".into(),
            kind: LogKind::Command,
            text: text.into(),
        }
    }

    #[test]
    fn wraps_long_command_within_width() {
        let long =
            "rsync -a -z --include-from=/home/light/lazyrsync-test/include.txt -- /src/ /dst/";
        let rows = log_rows(&[entry(long)], 30);
        assert!(rows.len() > 1, "long line should wrap to multiple rows");
        for r in &rows {
            assert!(r.width() <= 30, "row {:?} exceeds width", r);
        }
    }

    #[test]
    fn short_line_stays_one_row() {
        assert_eq!(log_rows(&[entry("ok")], 30).len(), 1);
    }

    #[test]
    fn continuation_rows_keep_hanging_indent() {
        let long =
            "rsync -a -z --include-from=/home/light/lazyrsync-test/include.txt -- /src/ /dst/";
        let rows = log_rows(&[entry(long)], 30);
        assert!(rows.len() > 1);
        for r in &rows {
            let s: String = r.spans.iter().map(|sp| sp.content.as_ref()).collect();
            assert!(s.starts_with("  "), "row not indented: {s:?}");
        }
    }
}
