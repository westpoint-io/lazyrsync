use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{Block, BorderType, Paragraph},
    Frame,
};

use crate::app::{Cmd, Ctx};
use crate::ui::{accent, hint_line, truncate, with_footer};

enum Target {
    Profile(String),
    Tasks(Vec<String>),
}

pub struct ConfirmDelete {
    target: Target,
}

impl ConfirmDelete {
    pub fn profile(name: String) -> Self {
        Self {
            target: Target::Profile(name),
        }
    }

    pub fn tasks(ids: Vec<String>) -> Self {
        Self {
            target: Target::Tasks(ids),
        }
    }

    fn copy(&self) -> (&'static str, Line<'static>, &'static str) {
        match &self.target {
            Target::Profile(name) => (
                "profile",
                Line::from(vec![
                    "Delete ".into(),
                    truncate(name, 48).fg(accent()).bold(),
                    " ?".into(),
                ]),
                "This removes the profile and its tasks.",
            ),
            Target::Tasks(ids) if ids.len() == 1 => (
                "task",
                Line::from(vec![
                    "Delete ".into(),
                    truncate(&ids[0], 48).fg(accent()).bold(),
                    " ?".into(),
                ]),
                "This removes the task from the profile.",
            ),
            Target::Tasks(ids) => (
                "tasks",
                Line::from(vec![
                    "Delete ".into(),
                    format!("{} tasks", ids.len()).fg(accent()).bold(),
                    " ?".into(),
                ]),
                "This removes them from the profile.",
            ),
        }
    }
