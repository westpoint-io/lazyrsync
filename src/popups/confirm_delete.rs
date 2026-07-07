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

    pub fn draw(&self, frame: &mut Frame, area: Rect, cx: &Ctx) {
        let (what, headline, note) = self.copy();
        let footer = if cx.settings.hints {
            vec![hint_line(&[("<enter>", "Delete"), ("<esc>", "Cancel")])]
        } else {
            vec![]
        };
        let area = with_footer(frame, cx, area, 60, 8, footer);
        let text = Text::from(vec![
            Line::from(""),
            headline,
            Line::from(""),
            Line::from(note.dim()),
        ]);
        frame.render_widget(
            Paragraph::new(text).centered().block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(accent()).bold())
                    .title(format!(" Delete {what} ").fg(accent()).bold()),
            ),
            area,
        );
    }

    pub fn on_key(&mut self, key: KeyEvent, cx: &mut Ctx) -> Cmd {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                match &self.target {
                    Target::Profile(_) => {
                        let idx = cx.pcursor;
                        if idx < cx.store.profiles.len() {
                            let removed = cx.store.profiles.remove(idx);
                            if cx.profile > idx {
                                cx.profile -= 1;
                            }
                            cx.clamp();
                            cx.save(&format!("deleted profile '{}'", removed.name));
                        }
                    }
                    Target::Tasks(ids) => {
                        if let Some(p) = cx.store.profiles.get_mut(cx.profile) {
                            let before = p.tasks.len();
                            p.tasks.retain(|t| !ids.contains(&t.id));
                            let removed = before - p.tasks.len();
                            cx.clamp();
                            cx.save(&format!(
                                "deleted {removed} task{}",
                                if removed == 1 { "" } else { "s" }
                            ));
                        }
                    }
                }
                Cmd::Close
            }
            KeyCode::Char('n') | KeyCode::Char('q') | KeyCode::Esc => Cmd::Close,
            _ => Cmd::None,
        }
    }
}
