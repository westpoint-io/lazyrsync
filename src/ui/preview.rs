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
