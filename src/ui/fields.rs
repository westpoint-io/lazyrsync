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
    };
    let tstyle = if focused {
        Style::new().fg(title_fg).bg(c).bold()
    } else {
        Style::new().fg(c).bold()
    };
    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(bstyle)
        .title(Span::styled(format!(" {title} "), tstyle));
    if let Some(r) = right {
        block =
            block.title(Line::from(format!(" {} ──", cap_first(&r)).fg(c).bold()).right_aligned());
    }
    let inner_w = area.width.saturating_sub(2) as usize;
    let scroll_x = match cursor {
        Some(c) => cursor_scroll(c, inner_w),
        None => content.width().saturating_sub(inner_w) as u16,
    };
    frame.render_widget(
        Paragraph::new(content).block(block).scroll((0, scroll_x)),
        area,
    );
    if let Some(c) = cursor {
        let col = (c + 1) as u16;
        frame.set_cursor_position((area.x + 1 + col.saturating_sub(scroll_x), area.y + 1));
    }
}

fn is_remote_buf(b: &str) -> bool {
    b.contains('@') || (b.contains(':') && !b.starts_with('/'))
}

fn plural_matches(n: usize) -> String {
    format!("{n} {}", if n == 1 { "match" } else { "matches" })
}

pub(crate) fn field_status(buffer: &str, focused: bool, is_dest: bool) -> (Color, Option<String>) {
    let b = buffer.trim();
    if b.is_empty() || is_remote_buf(b) {
        return (accent(), None);
    }
    let path = crate::paths::expand_tilde(b);
    if std::path::Path::new(&path).is_dir() {
        return (
            added(),
            focused.then(|| plural_matches(crate::paths::path_hits(buffer).len())),
        );
    }
    let hits = crate::paths::path_hits(buffer);
    if !hits.is_empty() {
        return (accent(), focused.then(|| plural_matches(hits.len())));
    }
    if is_dest {
        let parent_ok = std::path::Path::new(&path)
            .parent()
            .is_none_or(|p| p.as_os_str().is_empty() || p.is_dir());
        if parent_ok {
            (added(), focused.then(|| "new dir".into()))
        } else {
            (deleted(), focused.then(|| "parent missing".into()))
        }
    } else {
        (deleted(), focused.then(|| "not found".into()))
    }
}
