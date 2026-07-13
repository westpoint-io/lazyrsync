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
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .style(Style::new().fg(accent()))
                        .begin_symbol(Some("▲"))
                        .end_symbol(Some("▼")),
                    log.inner(Margin::new(0, 1)),
                    &mut sb,
                );
            }
        }

        self.draw_status(frame, status, cx);
    }

    fn draw_rail(&self, frame: &mut Frame, area: Rect, cx: &Ctx) {
        let areas = Layout::vertical(rail_constraints()).split(area);
        let num_style = |focused: bool| {
            if focused {
                Style::new().fg(accent()).bold()
            } else {
                Style::new()
            }
        };
        let simple = |num: usize, name: &'static str, focused: bool| {
            let label = if focused {
                name.fg(accent()).bold()
            } else {
                name.into()
            };
            Line::from(vec![
                Span::styled(format!("[{num}] "), num_style(focused)),
                label,
            ])
        };

        for (i, &f) in Self::RAIL_ORDER.iter().enumerate() {
            let focused = f == self.focus;
            let num = i + 1;
            let mut block = rounded(focused);
            let mut offset = 0u16;

            match f {
                3 => {
                    block = block.title(simple(num, "Runs", focused));
                    if let Some((cur, total)) = self.runs.position() {
                        block = block.title_bottom(
                            Line::from(Span::styled(format!("{cur}/{total} "), num_style(focused)))
                                .right_aligned(),
                        );
                    }
                }
                0 => {
                    let mut spans = vec![Span::styled(format!("[{num}] "), num_style(focused))];
                    for (j, name) in SUBTABS.iter().enumerate() {
                        if j > 0 {
                            spans.push(" - ".fg(Color::Reset).not_bold());
                        }
                        let s = *name;
                        spans.push(if j == cx.subtab {
                            s.fg(accent()).bold()
                        } else {
                            s.fg(Color::Reset).not_bold()
                        });
                    }
                    block = block.title(Line::from(spans));
                    let (cur, n) = self.panel_count(cx, 0);
                    block = block.title_bottom(
                        Line::from(Span::styled(
                            format!("{} of {} ", cur + 1, n),
                            num_style(focused),
                        ))
                        .right_aligned(),
                    );
                    let visible = (areas[i].height as usize).saturating_sub(2);
                    offset = (cur + 1).saturating_sub(visible) as u16;
                }
                1 => {
                    let mut spans = simple(num, "Flags", focused).spans;
                    let raw = cx
                        .active_task()
                        .map_or(0, |t| rsync::split_args(&t.advanced.raw_args).len());
                    if raw > 0 {
                        let c = if focused { accent() } else { Color::Reset };
                        spans.push(format!(" ({raw} raw)").fg(c));
                    }
                    block = block.title(Line::from(spans));
                }
                _ => block = block.title(simple(num, "Filters", focused)),
            }

            let w = (areas[i].width as usize).saturating_sub(2);
            let h = (areas[i].height as usize).saturating_sub(2);
            let panel_body = if f == 3 {
                Text::from(self.runs.rail_line(cx))
            } else {
                self.panel_body(cx, f, w, h)
            };
            frame.render_widget(
                Paragraph::new(panel_body).block(block).scroll((offset, 0)),
                areas[i],
            );
        }
    }

    fn panel_body(&self, cx: &Ctx, panel: usize, w: usize, h: usize) -> Text<'static> {
        let focused = panel == self.focus;
        let hl = move |i: usize, cur: usize, line: Line<'static>| {
            if i == cur && focused {
                line.patch_style(Style::new().fg(on_accent()).bg(accent()).bold())
            } else {
                line
            }
        };
        match panel {
            0 if cx.subtab == 1 => {
                if cx.store.profiles.is_empty() {
                    return empty_state("Add your first profile", h);
                }
                let vis = self.visible_rows(cx);
                if vis.is_empty() {
                    return empty_state(&format!("No matches for '{}'", self.list_filter), h);
                }
                Text::from(
                    vis.iter()
                        .map(|&i| {
                            let p = &cx.store.profiles[i];
                            let hot = i == cx.pcursor && focused;
                            let (age, acolor) = task_age(p.created);
                            let gutter = if i == cx.profile {
                                let c = if hot { on_accent() } else { accent() };
                                Span::styled(format!("{:>3} ", "*"), Style::new().fg(c).bold())
                            } else if hot {
                                Span::raw(format!("{age:>3} "))
                            } else {
                                Span::styled(format!("{age:>3} "), Style::new().fg(acolor))
                            };
                            let ntxt = p.tasks.len().to_string();
                            let right_w = ntxt.chars().count() + " tasks".len();
                            let name_max = w.saturating_sub(4 + right_w + 2);
                            let left = vec![gutter, truncate(&p.name, name_max).into()];
                            let right = vec![ntxt.fg(secondary()).bold(), " tasks".fg(secondary())];
                            hl(i, cx.pcursor, justify(left, right, w))
                        })
                        .collect::<Vec<_>>(),
                )
            }
            0 => match cx.active_profile() {
                Some(p) if !p.tasks.is_empty() => {
                    let vis = self.visible_rows(cx);
                    if vis.is_empty() {
                        return empty_state(&format!("No matches for '{}'", self.list_filter), h);
                    }
                    let (lo, hi) = self.visual_range().unwrap_or((usize::MAX, 0));
                    Text::from(
                        vis.iter()
                            .map(|&i| {
                                let t = &p.tasks[i];
                                let hot = focused && (i == self.tcursor || (i >= lo && i <= hi));
                                let selected = i == cx.task;
                                let (age, acolor) = task_age(t.created);
                                let gutter = if selected {
                                    let c = if hot { on_accent() } else { accent() };
                                    Span::styled(format!("{:>3} ", "*"), Style::new().fg(c).bold())
                                } else if hot {
                                    Span::raw(format!("{age:>3} "))
                                } else {
                                    Span::styled(format!("{age:>3} "), Style::new().fg(acolor))
                                };
                                let id_max = w.saturating_sub(4);
                                let mut line =
                                    Line::from(vec![gutter, truncate(&t.id, id_max).into()]);
                                pad_row(&mut line, hot, w);
                                if hot {
                                    line.patch_style(
                                        Style::new().fg(on_accent()).bg(accent()).bold(),
                                    )
                                } else {
                                    line
                                }
                            })
                            .collect::<Vec<_>>(),
                    )
                }
                _ => empty_state("Add your first task", h),
            },
            1 => match cx.active_task() {
                Some(t) => {
                    let sel = if focused { Some(self.flag) } else { None };
                    Text::from(editor::flags_display(&t.flags, sel, w))
                }
                None => Text::from(Line::from("—".fg(Color::Reset))),
            },
            _ => match cx.active_task() {
                Some(t) => Text::from(editor::filters_display(&t.filters, w)),
                None => Text::from(Line::from("—".fg(Color::Reset))),
            },
        }
    }

    fn detail_profile(&self, p: &Profile) -> Text<'static> {
        let n = p.tasks.len();
        let plural = if n == 1 { "task" } else { "tasks" };
        let mut lines = vec![kvc("tasks", format!("{n} {plural}"), added())];

        let mut dests: Vec<&str> = Vec::new();
        for t in &p.tasks {
            if !dests.contains(&t.dest.as_str()) {
                dests.push(&t.dest);
            }
        }
        if !dests.is_empty() {
            const CAP: usize = 3;
            let shown = dests
                .iter()
                .take(CAP)
                .copied()
                .collect::<Vec<_>>()
                .join(", ");
            let extra = dests.len().saturating_sub(CAP);
            let val = if extra > 0 {
                format!("{shown}, +{extra} more")
            } else {
                shown
            };
            lines.push(kvc("destinations", val, secondary()));
        }
        Text::from(lines)
    }

    fn detail_task(&self, t: &Task) -> Text<'static> {
        let mut lines = vec![
            kvc("action", t.action.label().to_string(), added()),
            kvc("source", t.source.clone(), secondary()),
            kvc("destination", t.dest.clone(), secondary()),
        ];
        if let Some(si) = rsync::snapshot_info(t) {
            lines.push(kvc("snapshots", si.count.to_string(), added()));
            if si.count > 0 {
                let (age, c) = task_age(si.newest_unix);
                lines.push(kvc("latest", format!("#{}  ·  {age}", si.latest), c));
            }
            lines.push(kvc("next run", format!("#{}", si.next), secondary()));
        }
        Text::from(lines)
    }

    fn draw_status(&self, frame: &mut Frame, area: Rect, cx: &Ctx) {
        let [left, right] =
            Layout::horizontal([Constraint::Min(10), Constraint::Length(17)]).areas(area);
        frame.render_widget(
            Line::from(format!("lazyrsync {} ", env!("CARGO_PKG_VERSION")).fg(Color::Reset))
                .right_aligned(),
            right,
        );

        if self.focus == 0 && self.is_filtering() {
            frame.render_widget(
                Line::from(vec![
                    " Filter: ".fg(accent()),
                    self.list_filter.clone().fg(Color::Reset),
                    "█".fg(accent()),
                ]),
                left,
            );
            return;
        }

        if self.focus == 0 && self.filter_active() {
            let shown = self.visible_rows(cx).len();
            frame.render_widget(
                Line::from(vec![
                    " Filter: ".fg(accent()),
                    format!("'{}' ({shown} shown)", self.list_filter).fg(Color::Reset),
                    "   ".into(),
                    "<esc>".fg(accent()),
                    ": clear filter".fg(secondary()),
                ]),
                left,
            );
            return;
        }

        if self.focus == 3 && (self.runs.searching() || self.runs.search_on()) {
            frame.render_widget(Line::from(self.search_status_spans(cx.tick)), left);
            return;
        }

        if self.focus == 0 && cx.subtab == 0 && self.visual.is_some() {
            let n = self.visual_range().map_or(0, |(lo, hi)| hi - lo + 1);
            let mut spans = vec![format!(" {n} selected").fg(accent()).bold(), "   ".into()];
            for (i, (label, k)) in [
                ("Run", "r"),
                ("Delete", "d"),
                ("Dismiss", "<esc>"),
                ("Keybindings", "?"),
            ]
            .iter()
            .enumerate()
            {
                if i > 0 {
                    spans.push(" │ ".dim());
                }
                spans.push(format!("{label}: ").fg(secondary()));
                spans.push((*k).fg(accent()));
            }
            frame.render_widget(Line::from(spans), left);
            return;
        }

        let mut keys: Vec<(&str, &str)> = match (self.focus, cx.subtab) {
            (3, _) if self.runs.running() => {
                vec![("Cancel", "c"), ("Select", "j/k")]
            }
            (3, _) => vec![("Select", "j/k"), ("Run", "r"), ("Search", "/")],
            (0, 1) => vec![
                ("Add", "a"),
                ("Rename", "r"),
                ("Delete", "d"),
                ("Filter", "/"),
            ],
            (0, _) => vec![
                ("Select", "␣"),
                ("Run", "r"),
                ("All", "R"),
                ("Preview", "p"),
                ("Edit", "e"),
                ("SSH", "s"),
                ("Add", "a"),
                ("Del", "d"),
                ("Filter", "/"),
            ],
            (1, _) => vec![("Toggle", "␣"), ("Extra flags", "x"), ("Preview", "p")],
            _ => vec![("Edit", "i"), ("Clear", "<ctrl+r>"), ("Preview", "p")],
        };
        keys.push(("Keybindings", "?"));
        keys.push(("Quit", "q"));

        let mut spans = vec![" ".into()];
        for (i, (label, k)) in keys.iter().enumerate() {
            if i > 0 {
                spans.push(" │ ".dim());
            }
            spans.push(format!("{label}: ").fg(secondary()));
            spans.push(k.fg(accent()));
        }
        frame.render_widget(Line::from(spans), left);
    }

    fn search_status_spans(&self, tick: usize) -> Vec<Span<'static>> {
        let q = self.runs.search_query().to_string();
        if self.runs.searching() {
            return vec![
                " Search: ".fg(accent()),
                q.fg(Color::Reset),
                "█".fg(accent()),
            ];
        }
        if self.runs.scanning() {
            let dots = format!("{:<3}", ".".repeat(1 + (tick / 3) % 3));
            return vec![
                " Search: ".fg(accent()),
                format!("searching for '{q}'{dots} ").fg(Color::Reset),
                "<esc>".fg(accent()),
                ": cancel".fg(secondary()),
            ];
        }
        let (pos, count) = self.runs.match_pos();
        let mut spans = vec![" Search: ".fg(accent())];
        if count == 0 {
            spans.push(format!("No matches for '{q}'").fg(warn()));
        } else {
            spans.push(format!("matches for '{q}' ({pos} of {count})").fg(Color::Reset));
        }
        let hints = [
            ("n", "Next match"),
            ("N", "Previous match"),
            ("<esc>", "Exit search mode"),
        ];
        for (i, (k, label)) in hints.iter().enumerate() {
            spans.push(if i == 0 {
                "   ".into()
            } else {
                ", ".fg(secondary())
            });
            spans.push(k.fg(accent()));
            spans.push(format!(": {label}").fg(secondary()));
        }
        spans
    }

    fn draw_context(&self, frame: &mut Frame, area: Rect, cx: &Ctx) {
        let visible = (area.height as usize).saturating_sub(2);
        let (ctx_label, text) = self.context_view(cx, visible);
        let name = match self.focus {
            3 => "Run",
            1 => "Command",
            2 => "Filters",
            _ => "Status",
        };
        let mut title = vec!["[0] ".into(), name.into()];
        if self.focus == 3 {
            if let Some((tag, color)) = self.runs.sel_mode() {
                title.push(format!(" [{tag}]").fg(color).bold());
            }
        }
        let block = rounded(false)
            .title(Line::from(title))
            .title(Line::from(ctx_label.fg(secondary())).right_aligned());
        let para = Paragraph::new(text).block(block);
        let para = if self.focus == 3 {
            para
        } else {
            para.wrap(Wrap { trim: false })
        };
        frame.render_widget(para, area);
        if self.focus == 3 {
            if let Some((total, top)) = self.runs.overflow(visible) {
                let mut sb = ScrollbarState::new(total.saturating_sub(visible)).position(top);
                frame.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .style(Style::new().fg(accent()))
                        .begin_symbol(Some("▲"))
                        .end_symbol(Some("▼")),
                    area.inner(Margin::new(0, 1)),
                    &mut sb,
                );
            }
        }
    }

    fn context_view(&self, cx: &Ctx, visible: usize) -> (String, Text<'static>) {
        if self.focus == 3 {
            let label = self
                .runs
                .sel_label()
                .map(|l| format!(" {l} "))
                .unwrap_or_default();
            return (label, self.runs.main_text(cx, visible));
        }
        if self.focus == 0 && cx.subtab == 1 {
            return match cx.store.profiles.get(cx.pcursor) {
                Some(p) => (format!(" {} ", p.name), self.detail_profile(p)),
                None => (" ".into(), empty_state("Add your first profile", visible)),
            };
        }
        let Some(t) = self.context_task(cx) else {
            return (" ".into(), empty_state("Add your first task", visible));
        };
        let label = format!(" {} ", t.id);
        match self.focus {
            1 => (label, self.command_view(t)),
            2 => (label, self.filters_view(t)),
            _ => (label, self.detail_task(t)),
        }
    }

    fn command_view(&self, t: &Task) -> Text<'static> {
        Text::from(vec![
            Line::from(rsync::resolved_command(t, false)),
            Line::from(""),
            Line::from("toggle flags in [2] — the command updates live".fg(secondary())),
        ])
    }

    fn filters_view(&self, t: &Task) -> Text<'static> {
        let f = &t.filters;
        let rows: [(&str, String, Color); 6] = [
            ("includes", f.includes.join(", "), added()),
            ("include-from", f.include_from.clone(), added()),
            ("excludes", f.excludes.join(", "), warn()),
            ("exclude-from", f.exclude_from.clone(), warn()),
            ("filter", f.filter.join(", "), accent()),
            ("files-from", f.files_from.clone(), secondary()),
        ];
        let lines: Vec<Line> = rows
            .into_iter()
            .map(|(k, v, c)| {
                if v.is_empty() {
                    kvc(k, "None", Color::Reset)
                } else {
                    kvc(k, v, c)
                }
            })
            .collect();
        Text::from(lines)
    }
}
