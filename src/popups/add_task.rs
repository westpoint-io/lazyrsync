use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph, Wrap},
    Frame,
};

use crate::app::{Cmd, Ctx};
use crate::profile::{Action, Task};
use crate::rsync;
use crate::ui::{accent, centered, deleted, field_box, field_status, hint_line, with_footer};

const WIDTH: u16 = 62;
const BOX_H: u16 = 18;
const ROWS: usize = 4;

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn basename(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("task")
        .to_string()
}

pub struct AddTask {
    label: String,
    action: Action,
    source: String,
    dest: String,
    row: usize,
    cursor: usize,
    id_taken: bool,
    attempted: bool,
}

impl AddTask {
    pub fn new() -> Self {
        Self {
            label: String::new(),
            action: Action::Sync,
            source: String::new(),
            dest: String::new(),
            row: 0,
            cursor: 0,
            id_taken: false,
            attempted: false,
        }
    }

    fn text_mut(&mut self) -> Option<&mut String> {
        match self.row {
            0 => Some(&mut self.label),
            2 => Some(&mut self.source),
            3 => Some(&mut self.dest),
            _ => None,
        }
    }

    fn text_len(&self) -> usize {
        match self.row {
            0 => self.label.chars().count(),
            2 => self.source.chars().count(),
            3 => self.dest.chars().count(),
            _ => 0,
        }
    }

    fn text(&self) -> Option<&str> {
        match self.row {
            0 => Some(&self.label),
            2 => Some(&self.source),
            3 => Some(&self.dest),
            _ => None,
        }
    }

    fn prev_word(&self) -> usize {
        let Some(t) = self.text() else { return 0 };
        let chars: Vec<char> = t.chars().collect();
        let mut i = self.cursor.min(chars.len());
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        i
    }

    fn next_word(&self) -> usize {
        let Some(t) = self.text() else {
            return self.cursor;
        };
        let chars: Vec<char> = t.chars().collect();
        let n = chars.len();
        let mut i = self.cursor.min(n);
        while i < n && !chars[i].is_whitespace() {
            i += 1;
        }
        while i < n && chars[i].is_whitespace() {
            i += 1;
        }
        i
    }

    fn delete_forward(&mut self) {
        let at = self.cursor;
        if let Some(b) = self.text_mut() {
            if at < b.chars().count() {
                let byte = Self::byte_at(b, at);
                b.remove(byte);
            }
        }
    }

    fn delete_to_start(&mut self) {
        let at = self.cursor;
        if let Some(b) = self.text_mut() {
            let end = Self::byte_at(b, at);
            b.replace_range(..end, "");
        }
        self.cursor = 0;
    }

    fn delete_to_end(&mut self) {
        let at = self.cursor;
        if let Some(b) = self.text_mut() {
            let start = Self::byte_at(b, at);
            b.truncate(start);
        }
    }

    fn cursor_to_end(&mut self) {
        self.cursor = self.text_len();
    }

    fn byte_at(s: &str, ci: usize) -> usize {
        s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len())
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            let c = self.cursor;
            if let Some(b) = self.text_mut() {
                let byte = Self::byte_at(b, c - 1);
                b.remove(byte);
            }
            self.cursor -= 1;
        }
    }

    fn delete_word(&mut self) {
        let cursor = self.cursor;
        let new_cursor = {
            let Some(text) = self.text_mut() else {
                return;
            };
            let chars: Vec<char> = text.chars().collect();
            let mut i = cursor.min(chars.len());
            while i > 0 && chars[i - 1].is_whitespace() {
                i -= 1;
            }
            while i > 0 && !chars[i - 1].is_whitespace() {
                i -= 1;
            }
            let (start, end) = (Self::byte_at(text, i), Self::byte_at(text, cursor));
            text.replace_range(start..end, "");
            i
        };
        self.cursor = new_cursor;
    }

    fn preview_task(&self) -> Task {
        let src = self.source.trim();
        let label = match self.label.trim() {
            "" if src.is_empty() => "task".to_string(),
            "" => basename(src),
            name => name.to_string(),
        };
        let mut t = Task::new(label, src, self.dest.trim());
        t.action = self.action;
        t
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, cx: &Ctx) {
        let footer = if cx.settings.hints {
            vec![
                hint_line(&[("<tab>", "Move"), ("<ctrl+n>", "Complete")]),
                hint_line(&[("<enter>", "Save"), ("<esc>", "Cancel")]),
            ]
        } else {
            vec![]
        };
        let area = with_footer(frame, cx, area, WIDTH, BOX_H, footer);
        let rows = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(6),
        ])
        .split(area);

        let cur = |row: usize| (self.row == row).then_some(self.cursor);

        let name_line = if self.label.is_empty() {
            Line::from(vec![
                Span::raw(" "),
                self.preview_task().candidate_id().dim(),
            ])
        } else {
            Line::from(format!(" {}", self.label))
        };
        let (id_border, id_badge) = if self.id_taken {
            (Some(deleted()), Some("taken".to_string()))
        } else {
            (None, None)
        };
        field_box(
            frame,
            rows[0],
            "ID",
            self.row == 0,
            name_line,
            id_badge,
            id_border,
            cur(0),
        );
        field_box(
            frame,
            rows[1],
            "Action",
            self.row == 1,
            Line::from(vec![
                " ◂ ".dim(),
                self.action.label().fg(accent()).bold(),
                " ▸".dim(),
            ]),
            None,
            None,
            None,
        );
        let (mut src_color, mut src_badge) = field_status(&self.source, self.row == 2, false);
        if self.attempted && self.source.trim().is_empty() {
            src_color = deleted();
            src_badge = Some("required".to_string());
        }
        field_box(
