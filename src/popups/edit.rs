use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::Line,
    Frame,
};

use crate::app::{Cmd, Ctx};
use crate::editor::{Editor, Outcome, Section};
use crate::profile::Task;
use crate::ui::{centered, deleted, field_box, field_status, file_status, hint_line, with_footer};

const WIDTH: u16 = 64;

pub struct SectionEdit {
    ed: Editor,
}

impl SectionEdit {
    pub fn new(task: Task, section: Section, taken: Vec<String>) -> Self {
        let mut ed = Editor::edit(task, taken);
        ed.focus_section(section);
        ed.enter_form();
        Self { ed }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, cx: &Ctx) {
        let views = self.ed.form_views();
        let footer = if cx.settings.hints {
            vec![
                hint_line(&[("<tab>", "Move"), ("<ctrl+n>", "Complete")]),
                hint_line(&[("<enter>", "Save"), ("<esc>", "Cancel")]),
            ]
        } else {
            vec![]
        };
        let box_a = with_footer(frame, cx, area, WIDTH, views.len() as u16 * 3, footer);
        let rows = Layout::vertical(vec![Constraint::Length(3); views.len()]).split(box_a);

        for (i, v) in views.iter().enumerate() {
            let content = Line::from(format!(" {}", v.value));
            let (border, badge) = if let Some(err) = v.error {
                (Some(deleted()), Some(err.to_string()))
            } else if v.is_path && v.dirty {
                let (c, b) = field_status(&v.value, v.focused, v.is_dest);
                (Some(c), b)
            } else if v.is_file {
                let (c, b) = file_status(&v.value, v.focused);
                let border = (!v.value.trim().is_empty()).then_some(c);
                (border, b)
            } else {
                (None, None)
            };
            let cursor = v.focused.then_some(v.cursor);
            field_box(
                frame, rows[i], v.label, v.focused, content, badge, border, cursor,
            );
        }
    }

    pub fn on_key(&mut self, key: KeyEvent, cx: &mut Ctx) -> Cmd {
        match self.ed.form_key(key) {
            Outcome::Continue => Cmd::None,
            Outcome::Cancel => Cmd::Close,
            Outcome::Rejected => {
                cx.reject();
                Cmd::None
            }
            Outcome::Save => {
                self.save(cx);
                Cmd::Close
            }
        }
    }

    pub fn on_mouse(&mut self, m: MouseEvent, cx: &mut Ctx) -> Cmd {
        if let MouseEventKind::Down(MouseButton::Left) = m.kind {
            let n = self.ed.field_count();
            let footer_h = if cx.settings.hints { 2 } else { 0 };
            let region = centered(cx.area, WIDTH, n as u16 * 3 + footer_h);
            let inside =
                m.column >= region.x && m.column < region.x + region.width && m.row >= region.y;
            if inside {
                self.ed.focus_field(((m.row - region.y) / 3) as usize);
            }
        }
        Cmd::None
    }

    fn save(&self, cx: &mut Ctx) {
        let task = self.ed.task().clone();
        let id = task.id.clone();
        let orig = self.ed.orig_id().to_string();
        if let Some(p) = cx.store.profiles.get_mut(cx.profile) {
            if let Some(slot) = p.tasks.iter_mut().find(|t| t.id == orig) {
                *slot = task;
            }
        }
        cx.save(&format!("saved task '{id}'"));
    }
}
