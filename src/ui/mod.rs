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
