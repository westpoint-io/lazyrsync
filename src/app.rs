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

    pub fn push_log(&mut self, kind: LogKind, text: impl Into<String>) {
        let at = Local::now().format("%H:%M:%S").to_string();
        self.log.push(LogEntry {
            at,
            kind,
            text: text.into(),
        });
        let n = self.log.len();
        if n > 200 {
            self.log.drain(0..n - 200);
        }
    }

    pub fn reject(&mut self) {
        self.shake = 6;
    }

    pub fn shake_dx(&self) -> i16 {
        const WAVE: [i16; 7] = [0, 1, -1, 2, -2, 3, -3];
        WAVE[(self.shake as usize).min(WAVE.len() - 1)]
    }

    pub fn save(&mut self, ok_msg: &str) {
        match self.store.save() {
            Ok(()) => self.push_log(LogKind::Info, ok_msg.to_string()),
            Err(e) => self.push_log(LogKind::Error, format!("save failed: {e}")),
        }
    }
}

pub enum Cmd {
    None,
    Quit,
    Overlay(Overlay),
    Close,
    RequestRun(Vec<Task>),
    StartRun(Vec<Task>),
}

pub trait Component {
    fn draw(&mut self, frame: &mut Frame, area: Rect, cx: &Ctx);
    fn on_key(&mut self, key: KeyEvent, cx: &mut Ctx) -> Cmd;
    fn on_mouse(&mut self, _m: MouseEvent, _cx: &mut Ctx) -> Cmd {
        Cmd::None
    }
    fn busy(&self) -> bool {
        false
    }
    fn tick(&mut self, _cx: &mut Ctx) {}
}

pub enum Overlay {
    Prompt(Prompt),
    AddTask(AddTask),
    Edit(Box<SectionEdit>),
    ConfirmDelete(ConfirmDelete),
    ConfirmClearFilters(ConfirmClearFilters),
    ConfirmRun(ConfirmRun),
    Alert(Alert),
    Help(Help),
}

impl Overlay {
    fn is_text_input(&self) -> bool {
        matches!(
            self,
            Overlay::Prompt(_) | Overlay::AddTask(_) | Overlay::Edit(_)
        )
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, cx: &Ctx) {
        match self {
            Overlay::Prompt(p) => p.draw(frame, area, cx),
            Overlay::AddTask(a) => a.draw(frame, area, cx),
            Overlay::Edit(e) => e.draw(frame, area, cx),
            Overlay::ConfirmDelete(c) => c.draw(frame, area, cx),
            Overlay::ConfirmClearFilters(c) => c.draw(frame, area, cx),
            Overlay::ConfirmRun(c) => c.draw(frame, area, cx),
            Overlay::Alert(a) => a.draw(frame, area, cx),
            Overlay::Help(h) => h.draw(frame, area),
        }
    }

    fn on_key(&mut self, key: KeyEvent, cx: &mut Ctx) -> Cmd {
        match self {
            Overlay::Prompt(p) => p.on_key(key, cx),
            Overlay::AddTask(a) => a.on_key(key, cx),
            Overlay::Edit(e) => e.on_key(key, cx),
            Overlay::ConfirmDelete(c) => c.on_key(key, cx),
            Overlay::ConfirmClearFilters(c) => c.on_key(key, cx),
            Overlay::ConfirmRun(c) => c.on_key(key, cx),
            Overlay::Alert(a) => a.on_key(key, cx),
            Overlay::Help(h) => h.on_key(key),
        }
    }

    fn on_mouse(&mut self, m: MouseEvent, cx: &mut Ctx) -> Cmd {
        match self {
            Overlay::AddTask(a) => a.on_mouse(m, cx),
            Overlay::Edit(e) => e.on_mouse(m, cx),
            Overlay::Help(h) => h.on_mouse(m, cx),
            _ => Cmd::None,
        }
    }
}
