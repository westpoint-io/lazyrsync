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
