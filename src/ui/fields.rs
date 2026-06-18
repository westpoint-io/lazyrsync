use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
    Frame,
};

use super::{accent, added, cap_first, deleted, on_accent};

fn cursor_scroll(cursor: usize, inner_w: usize) -> u16 {
    if inner_w == 0 {
        return 0;
    }
    let col = cursor + 1;
    if col >= inner_w {
        (col - inner_w + 1) as u16
    } else {
        0
    }
}

pub(crate) fn kvc(key: &str, value: impl Into<String>, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{:<14}", format!("{}:", cap_first(key))),
            Style::new(),
        ),
        Span::styled(value.into(), Style::new().fg(color)),
    ])
}

pub(crate) fn file_status(buffer: &str, focused: bool) -> (Color, Option<String>) {
    let b = buffer.trim();
    if b.is_empty() {
        return (accent(), focused.then(|| "path to file".into()));
    }
    let path = crate::paths::expand_tilde(b);
    if std::path::Path::new(&path).is_file() {
        (added(), focused.then(|| "found".into()))
    } else {
        (deleted(), focused.then(|| "not found".into()))
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn field_box(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    focused: bool,
    content: Line,
    right: Option<String>,
    border: Option<Color>,
    cursor: Option<usize>,
) {
    let c = border.unwrap_or(accent());
    let bstyle = if focused {
        Style::new().fg(c).bold()
    } else {
        Style::new().fg(c)
    };
    let title_fg = if border.is_none() {
        on_accent()
    } else {
        Color::Black
