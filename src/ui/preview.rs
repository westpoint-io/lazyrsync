use ratatui::{
    style::{Style, Stylize},
    text::{Line, Span, Text},
};

use super::{accent, added, bytes, deleted, modified, muted, secondary};
use crate::preview::{ChangeKind, Preview};

pub(crate) fn preview_text(
    pv: &Preview,
    scroll: usize,
    visible: usize,
    search_lower: Option<&str>,
    current_match: Option<usize>,
) -> Text<'static> {
    let s = &pv.stats;
    let mut lines = vec![
        Line::from(vec![
            " ✓ DRY-RUN COMPLETE".fg(accent()).bold(),
            format!(
                "   {} files to transfer ({})",
                commas(s.transferred),
                human_bytes(s.transferred_size)
            )
            .fg(secondary()),
        ]),
        Line::from(format!(" {}", "─".repeat(56)).fg(muted())),
        Line::from(vec![
            "   ".into(),
            format!("+{} new", commas(s.created)).fg(added()),
            "   ".into(),
            format!("-{} deleted", commas(s.deleted)).fg(deleted()),
            "   ·  ".fg(muted()),
            format!(
                "{} files, {} dirs · total {}",
                commas(s.files),
                commas(s.dirs),
                human_bytes(s.total_size)
            )
            .fg(bytes()),
        ]),
        Line::from(""),
    ];
    if pv.changes.is_empty() {
        lines.push(Line::from(vec![
            "   ✓ ".fg(added()),
            "Nothing to transfer — source and destination are in sync.".into(),
        ]));
        return Text::from(lines);
    }
    let list_h = visible.saturating_sub(lines.len()).max(1);
    let total = pv.changes.len();
    let start = scroll.min(total.saturating_sub(1));
    let end = (start + list_h).min(total);
    for (j, c) in pv.changes[start..end].iter().enumerate() {
        let i = start + j;
        let (sym, style) = match c.kind {
            ChangeKind::Added => ("+ ", Style::new().fg(added())),
            ChangeKind::Modified => ("~ ", Style::new().fg(modified())),
            ChangeKind::Deleted => ("- ", Style::new().fg(deleted())),
        };
        let mut spans = vec![Span::raw("   "), Span::styled(sym, style)];
        match search_lower {
            Some(q) => spans.extend(super::highlight_spans(
                &c.path,
                Style::new(),
                q,
                current_match == Some(i),
            )),
            None => spans.push(Span::raw(c.path.to_string())),
        }
        let mut line = Line::from(spans);
        if c.kind == ChangeKind::Deleted {
            line.push_span("  (DELETED on dest)".fg(deleted()));
        }
        lines.push(line);
    }
    Text::from(lines)
}

fn commas(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub(crate) fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}
