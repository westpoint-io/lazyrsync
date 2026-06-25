use ratatui::{
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

use super::{Browse, SUBTABS};
use crate::app::Ctx;
use crate::editor;
use crate::profile::{Profile, Task};
use crate::rsync;
use crate::ui::{
    accent, added, kvc, log_rows, muted, on_accent, pad_row, rail_constraints, rounded, secondary,
    task_age, truncate, warn,
};

fn empty_state(caption: &str, h: usize) -> Text<'static> {
    let bar = |s: &'static str| Line::from(s.fg(muted())).centered();
    let mut lines = vec![
        bar("┌ ─ ─ ─ ─ ─ ┐"),
        bar("╎           ╎"),
        Line::from(vec![
            "╎     ".fg(muted()),
            "+".fg(accent()).bold(),
            "     ╎".fg(muted()),
        ])
        .centered(),
        bar("╎           ╎"),
        bar("└ ─ ─ ─ ─ ─ ┘"),
        Line::from(""),
        Line::from(caption.to_string().fg(secondary())).centered(),
    ];
    let pad = h.saturating_sub(lines.len()) / 2;
    let mut out = vec![Line::from(""); pad];
    out.append(&mut lines);
    Text::from(out)
}

fn justify(left: Vec<Span<'static>>, right: Vec<Span<'static>>, w: usize) -> Line<'static> {
    let span_w = |s: &[Span]| {
        s.iter()
            .flat_map(|s| s.content.chars())
            .map(|c| if c == '⚠' { 2 } else { 1 })
            .sum::<usize>()
    };
    let pad = w.saturating_sub(span_w(&left) + span_w(&right) + 1);
    let mut spans = left;
    spans.push(" ".repeat(pad).into());
    spans.extend(right);
    spans.push(" ".into());
    Line::from(spans)
}

impl Browse {
    pub(super) fn render(&mut self, frame: &mut Frame, area: Rect, cx: &Ctx) {
        let [body, status] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);

        if self.zoom {
            self.draw_context(frame, body, cx);
            self.draw_status(frame, status, cx);
            return;
        }

        let [rail, right] =
            Layout::horizontal([Constraint::Length(34), Constraint::Min(30)]).areas(body);

        self.draw_rail(frame, rail, cx);

        let [main, log] =
            Layout::vertical([Constraint::Min(6), Constraint::Length(8)]).areas(right);
        self.draw_context(frame, main, cx);

        let inner_w = (log.width as usize).saturating_sub(2);
        let inner_h = (log.height as usize).saturating_sub(2);
        if cx.log.is_empty() {
            self.log_max_scroll = 0;
            frame.render_widget(
                Paragraph::new(Line::from("No activity yet".fg(Color::Reset)))
                    .block(rounded(false).title("Command log")),
                log,
            );
        } else {
            let rows = log_rows(&cx.log, inner_w);
            let max_scroll = rows.len().saturating_sub(inner_h);
            self.log_max_scroll = max_scroll;
            let off = max_scroll.saturating_sub(self.log_scroll);
            let body: Vec<Line> = rows.into_iter().skip(off).take(inner_h).collect();
            frame.render_widget(
                Paragraph::new(Text::from(body)).block(rounded(false).title("Command log")),
                log,
            );
            if max_scroll > 0 {
                let mut sb = ScrollbarState::new(max_scroll).position(off);
                frame.render_stateful_widget(
