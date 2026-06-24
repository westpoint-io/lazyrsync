use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{layout::Rect, Frame};

use crate::app::{Cmd, Component, Ctx};
use crate::editor;

mod input;
mod render;
mod run;

use crate::profile::Task;
use run::Runs;

const SUBTABS: [&str; 2] = ["Tasks", "Profiles"];

fn search_match(q: &str, s: &str) -> bool {
    if q.chars().any(|c| c.is_uppercase()) {
        s.contains(q)
    } else {
        s.to_lowercase().contains(&q.to_lowercase())
    }
}

pub struct Browse {
    focus: usize,
    flag: usize,
    filter: usize,
    zoom: bool,
    tcursor: usize,
    visual: Option<usize>,
    visual_len: usize,
    list_filter: String,
    filtering: bool,
    log_scroll: usize,
    log_max_scroll: usize,
    scroll_drag: bool,
    runs: Runs,
}

impl Browse {
    pub fn new() -> Self {
        Self {
            focus: 0,
            flag: 0,
            filter: 0,
            zoom: false,
            tcursor: 0,
            visual: None,
            visual_len: 0,
            list_filter: String::new(),
            filtering: false,
            log_scroll: 0,
            log_max_scroll: 0,
            scroll_drag: false,
            runs: Runs::new(),
        }
    }

    pub fn start_run(&mut self, batch: Vec<Task>, cx: &mut Ctx) {
        self.visual = None;
        self.runs.enqueue(batch, cx);
        self.focus = 3;
    }

    pub fn text_entry(&self) -> bool {
        self.filtering || self.runs.searching()
    }

    const RAIL_ORDER: [usize; 4] = [3, 0, 1, 2];

    fn rail_pos(&self) -> usize {
        Self::RAIL_ORDER
            .iter()
            .position(|f| *f == self.focus)
            .unwrap_or(0)
    }

    fn focus_at(pos: usize) -> usize {
        Self::RAIL_ORDER[pos % 4]
    }

    pub fn on_resume(&mut self, cx: &Ctx) {
        let ntasks = cx.active_profile().map_or(0, |p| p.tasks.len());
        if self.visual.is_some() && ntasks != self.visual_len {
            self.visual = None;
        }
        self.tcursor = self.tcursor.min(ntasks.saturating_sub(1));
        let nfilters = cx.active_task().map_or(0, |t| t.filters.excludes.len());
        self.filter = self.filter.min(nfilters.saturating_sub(1));
    }

    fn visual_range(&self) -> Option<(usize, usize)> {
        self.visual
            .map(|a| (a.min(self.tcursor), a.max(self.tcursor)))
    }

    fn is_filtering(&self) -> bool {
        self.filtering
    }

    fn filter_active(&self) -> bool {
        !self.list_filter.is_empty()
    }

    fn row_matches(&self, name: &str) -> bool {
        self.list_filter.is_empty() || search_match(&self.list_filter, name)
    }

    fn visible_rows(&self, cx: &Ctx) -> Vec<usize> {
        if cx.subtab == 1 {
            cx.store
                .profiles
                .iter()
                .enumerate()
                .filter(|(_, p)| self.row_matches(&p.name))
                .map(|(i, _)| i)
                .collect()
        } else {
            cx.active_profile().map_or(Vec::new(), |p| {
                p.tasks
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| self.row_matches(&t.id))
                    .map(|(i, _)| i)
                    .collect()
            })
        }
    }

    fn list_cursor(&self, cx: &Ctx) -> usize {
        if cx.subtab == 1 {
            cx.pcursor
        } else {
            self.tcursor
        }
    }

    fn set_list_cursor(&mut self, cx: &mut Ctx, real: usize) {
        if cx.subtab == 1 {
            cx.pcursor = real;
        } else {
            self.tcursor = real;
        }
    }

    fn cursor_pos(&self, cx: &Ctx) -> usize {
        let cur = self.list_cursor(cx);
        self.visible_rows(cx)
            .iter()
            .position(|&i| i == cur)
            .unwrap_or(0)
    }

    fn snap_cursor(&mut self, cx: &mut Ctx) {
        let vis = self.visible_rows(cx);
        if !vis.contains(&self.list_cursor(cx)) {
            let first = vis.first().copied().unwrap_or(0);
            self.set_list_cursor(cx, first);
        }
    }

    fn context_task<'a>(&self, cx: &'a Ctx) -> Option<&'a crate::profile::Task> {
        if self.focus == 0 && cx.subtab == 0 {
            cx.active_profile().and_then(|p| p.tasks.get(self.tcursor))
        } else {
            cx.active_task()
        }
    }

    fn rows_in(&self, cx: &Ctx, panel: usize) -> usize {
        match panel {
            0 => self.visible_rows(cx).len().max(1),
            1 => editor::bool_flag_count(),
            _ => cx
                .active_task()
                .map_or(1, |t| t.filters.excludes.len().max(1)),
        }
    }

    fn panel_count(&self, cx: &Ctx, panel: usize) -> (usize, usize) {
        let n = self.rows_in(cx, panel);
        let cur = match panel {
            0 => self.cursor_pos(cx),
            1 => self.flag,
            _ => self.filter,
        };
        (cur.min(n.saturating_sub(1)), n)
    }

    fn scroll_preview(&mut self, delta: i32) {
        self.runs.scroll(delta);
    }

    fn move_sel(&mut self, cx: &mut Ctx, delta: i32) {
        if self.focus == 3 {
            self.runs.select(delta);
            return;
        }
        if self.focus == 0 {
            let vis = self.visible_rows(cx);
            if vis.is_empty() {
                return;
            }
            let pos = self.cursor_pos(cx);
            let np = (pos as i32 + delta).rem_euclid(vis.len() as i32) as usize;
            self.set_list_cursor(cx, vis[np]);
            return;
        }
        if self.focus == 1 {
            self.move_flag(delta, 0);
            return;
        }
        let n = self.rows_in(cx, self.focus) as i32;
        self.filter = (self.filter as i32 + delta).rem_euclid(n) as usize;
    }

    fn move_flag(&mut self, dr: i32, dc: i32) {
        const COLS: i32 = 2;
        let n = editor::bool_flag_count() as i32;
        if n == 0 {
            return;
        }
        let i = self.flag as i32;
        let rows = (n + COLS - 1) / COLS;
        let row = (i / COLS + dr).clamp(0, rows - 1);
        let col = (i % COLS + dc).clamp(0, COLS - 1);
        let target = row * COLS + col;
        self.flag = target.min(n - 1) as usize;
    }
}

impl Component for Browse {
    fn busy(&self) -> bool {
        self.runs.running() || self.runs.scanning()
    }

    fn tick(&mut self, cx: &mut Ctx) {
        self.runs.tick(cx);
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, cx: &Ctx) {
        self.render(frame, area, cx);
    }

    fn on_key(&mut self, key: KeyEvent, cx: &mut Ctx) -> Cmd {
        self.handle_key(key, cx)
    }

    fn on_mouse(&mut self, m: MouseEvent, cx: &mut Ctx) -> Cmd {
        self.handle_mouse(m, cx)
    }
}
