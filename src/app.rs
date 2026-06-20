use std::time::{Duration, Instant};

use chrono::Local;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent};
use ratatui::{layout::Rect, DefaultTerminal, Frame};

use crate::profile::{Profile, Task};
use crate::store::{Settings, Store};

use crate::popups::{
    AddTask, Alert, ConfirmClearFilters, ConfirmDelete, ConfirmRun, Help, Prompt, SectionEdit,
};
use crate::screens::browse::Browse;

#[derive(Clone, Copy)]
pub enum LogKind {
    Info,
    Active,
    Done,
    Warn,
    Error,
    Command,
}

pub struct LogEntry {
    pub(crate) at: String,
    pub(crate) kind: LogKind,
    pub(crate) text: String,
}

pub struct Ctx {
    pub store: Store,
    pub settings: Settings,
    pub log: Vec<LogEntry>,
    pub area: Rect,
    pub tick: usize,
    pub shake: u8,
    pub profile: usize,
    pub pcursor: usize,
    pub task: usize,
    pub subtab: usize,
}

impl Ctx {
    pub fn active_profile(&self) -> Option<&Profile> {
        self.store.profiles.get(self.profile)
    }

    pub fn active_task(&self) -> Option<&Task> {
        self.active_profile().and_then(|p| p.tasks.get(self.task))
    }

    pub fn clamp(&mut self) {
        let np = self.store.profiles.len();
        if np == 0 {
            self.profile = 0;
            self.pcursor = 0;
            self.task = 0;
            return;
        }
        self.profile = self.profile.min(np - 1);
        self.pcursor = self.pcursor.min(np - 1);
        let nt = self.store.profiles[self.profile].tasks.len();
        self.task = self.task.min(nt.saturating_sub(1));
    }
