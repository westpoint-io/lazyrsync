use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    text::Line,
    Frame,
};

use crate::app::{Cmd, Ctx};
use crate::profile::{Profile, Task};
use crate::ui::{field_box, hint_line_in, warn, with_footer};

const WIDTH: u16 = 62;

fn basename(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .map(|s| s.trim_start_matches('.'))
        .filter(|s| !s.is_empty())
        .unwrap_or("transfer")
        .to_string()
}

pub struct Keep {
    index: usize,
    profiles: Vec<String>,
    profile: usize,
    name: String,
    id: String,
    id_edited: bool,
    row: usize,
}

impl Keep {
    pub fn new(cx: &Ctx, index: usize) -> Self {
        let profiles: Vec<String> = cx.store.profiles.iter().map(|p| p.name.clone()).collect();
        let profile = if profiles.is_empty() {
            0
        } else {
            cx.profile.min(profiles.len() - 1)
        };
        let mut task = cx
            .quick
            .get(index)
            .cloned()
            .unwrap_or_else(|| Task::new("", "", ""));
        task.label = basename(&task.source);
        Self {
            index,
            profile,
            name: String::new(),
            id: task.candidate_id(),
            id_edited: false,
            row: 0,
            profiles,
        }
    }

    fn making_profile(&self) -> bool {
        self.profile == self.profiles.len()
    }

    fn rows(&self) -> usize {
        if self.making_profile() {
            3
        } else {
            2
        }
    }

    fn id_row(&self) -> usize {
        self.rows() - 1
    }

    fn buf(&mut self) -> &mut String {
        if self.row == 1 && self.making_profile() {
            &mut self.name
        } else {
            &mut self.id
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, cx: &Ctx) {
        let footer = if cx.settings.hints {
            vec![hint_line_in(
                &[("<tab>", "Move"), ("<enter>", "Save"), ("<esc>", "Discard")],
                warn(),
            )]
        } else {
            vec![]
        };
        let box_h = self.rows() as u16 * 3;
        let area = with_footer(frame, cx, area, WIDTH, box_h, footer);
        let slots = Layout::vertical(vec![Constraint::Length(3); self.rows()]).split(area);

        let label = if self.making_profile() {
            "+ new profile"
        } else {
            self.profiles
                .get(self.profile)
                .map(String::as_str)
                .unwrap_or("+ new profile")
        };
        field_box(
            frame,
            slots[0],
            "Profile",
            self.row == 0,
            Line::from(vec![
                " ◂ ".dim(),
                label.to_string().fg(warn()).bold(),
                " ▸".dim(),
            ]),
            None,
            Some(warn()),
            None,
        );

        let mut n = 1;
        if self.making_profile() {
            let content = if self.name.is_empty() {
                Line::from(vec![" ".into(), "name it".dim()])
            } else {
                Line::from(format!(" {}", self.name))
            };
            field_box(
                frame,
                slots[1],
                "Name",
                self.row == 1,
                content,
                None,
                Some(warn()),
                (self.row == 1).then_some(self.name.chars().count()),
            );
            n = 2;
        }

        let focused = self.row == self.id_row();
        let (content, badge) = if self.id_edited {
            (Line::from(format!(" {}", self.id)), None)
        } else {
            (
                Line::from(vec![" ".into(), self.id.clone().dim()]),
                Some("auto".to_string()),
            )
        };
        field_box(
            frame,
            slots[n],
            "Task id",
            focused,
            content,
            badge,
            Some(warn()),
            focused.then_some(self.id.chars().count()),
        );
    }

    pub fn on_key(&mut self, key: KeyEvent, cx: &mut Ctx) -> Cmd {
        match key.code {
            KeyCode::Esc => Cmd::Close,
            KeyCode::Tab | KeyCode::Down => {
                self.row = (self.row + 1) % self.rows();
                Cmd::None
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.row = (self.row + self.rows() - 1) % self.rows();
                Cmd::None
            }
            KeyCode::Left if self.row == 0 => {
                let n = self.profiles.len() + 1;
                self.profile = (self.profile + n - 1) % n;
                self.row = self.row.min(self.rows() - 1);
                Cmd::None
            }
            KeyCode::Right if self.row == 0 => {
                self.profile = (self.profile + 1) % (self.profiles.len() + 1);
                self.row = self.row.min(self.rows() - 1);
                Cmd::None
            }
            KeyCode::Backspace => {
                if self.row == self.id_row() {
                    self.id_edited = true;
                }
                self.buf().pop();
                Cmd::None
            }
            KeyCode::Char(c) => {
                if self.row == self.id_row() {
                    self.id_edited = true;
                }
                self.buf().push(c);
                Cmd::None
            }
            KeyCode::Enter => self.save(cx),
            _ => Cmd::None,
        }
    }

    fn save(&mut self, cx: &mut Ctx) -> Cmd {
        if self.index >= cx.quick.len() {
            return Cmd::Close;
        }
        let mut task = cx.quick.remove(self.index);
        let index = if self.making_profile() {
            let name = if self.name.trim().is_empty() {
                "default".to_string()
            } else {
                self.name.trim().to_string()
            };
            cx.store.profiles.push(Profile::new(name));
            cx.store.profiles.len() - 1
        } else {
            self.profile
        };
        cx.profile = index;
        cx.pcursor = index;
        let p = &mut cx.store.profiles[index];
        let base = if self.id.trim().is_empty() {
            task.candidate_id()
        } else {
            self.id.trim().to_string()
        };
        let id = Task::unique_id(&base, p.tasks.iter().map(|t| t.id.as_str()));
        task.id = id.clone();
        task.label = id.clone();
        p.tasks.insert(0, task);
        p.sort_tasks_by_recency();
        cx.task = p.tasks.iter().position(|t| t.id == id).unwrap_or(0);
        cx.subtab = 0;
        cx.save(&format!("kept '{id}'"));
        Cmd::CloseToTasks
    }
}
