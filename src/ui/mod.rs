use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear},
    Frame,
};

use crate::app::Ctx;

mod age;
mod fields;
mod log;
mod preview;
mod theme;

pub(crate) use age::task_age;
pub(crate) use fields::{field_box, field_status, file_status, kvc};
pub(crate) use log::log_rows;
pub(crate) use preview::{human_bytes, preview_text};
pub(crate) use theme::{
    accent, added, apply as apply_theme, border, bytes, deleted, modified, muted, on_accent,
    secondary, warn, ThemeSpec,
};

pub(crate) fn highlight_spans(
    text: &str,
    base: Style,
    q_lower: &str,
    current: bool,
) -> Vec<Span<'static>> {
    if q_lower.is_empty() {
        return vec![Span::styled(text.to_string(), base)];
    }
    let hl = if current {
        Style::new().fg(Color::Black).bg(accent()).bold()
    } else {
        Style::new().fg(Color::Black).bg(theme::warn())
    };
    let lower = text.to_lowercase();
    let mut spans = Vec::new();
    let mut i = 0;
    while let Some(rel) = lower[i..].find(q_lower) {
        let start = i + rel;
        let end = start + q_lower.len();
        if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
            return vec![Span::styled(text.to_string(), base)];
        }
        if start > i {
            spans.push(Span::styled(text[i..start].to_string(), base));
        }
        spans.push(Span::styled(text[start..end].to_string(), hl));
        i = end;
    }
    if i < text.len() {
        spans.push(Span::styled(text[i..].to_string(), base));
    }
    spans
}

pub(crate) fn cap_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

pub(crate) fn hint_line(pairs: &[(&'static str, &'static str)]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, (k, d)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push("   ·   ".dim());
        }
        spans.push((*k).fg(accent()));
        spans.push(format!(" {d}").fg(Color::Reset));
    }
    Line::from(spans).centered()
}

pub(crate) fn with_footer(
    frame: &mut Frame,
    cx: &Ctx,
    area: Rect,
    width: u16,
    box_height: u16,
    footer: Vec<Line<'static>>,
) -> Rect {
    let mut region = centered(area, width, box_height + footer.len() as u16);
    let dx = cx.shake_dx();
    if dx != 0 {
        let max_x = area.width.saturating_sub(region.width) as i16;
        region.x = (region.x as i16 + dx).clamp(0, max_x) as u16;
    }
    frame.render_widget(Clear, region);
    for (i, line) in footer.into_iter().enumerate() {
        frame.render_widget(
            line,
            Rect {
                x: region.x,
                y: region.y + box_height + i as u16,
                width: region.width,
                height: 1,
            },
        );
    }
    Rect {
        height: box_height,
        ..region
    }
}
