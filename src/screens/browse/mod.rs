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
