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
