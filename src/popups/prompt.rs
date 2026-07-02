use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    text::Line,
    widgets::Paragraph,
    Frame,
};

use crate::app::{Cmd, Ctx, LogKind};
use crate::profile::Profile;
use crate::ui::{accent, hint_line, rounded, with_footer};

enum PromptKind {
    NewProfile,
    RenameProfile,
}

pub struct Prompt {
    title: String,
    buffer: String,
    cursor: usize,
    kind: PromptKind,
}

impl Prompt {
    pub fn new_profile(name: String) -> Self {
        Self {
            title: " New profile ".into(),
            cursor: name.chars().count(),
            buffer: name,
            kind: PromptKind::NewProfile,
        }
    }

    pub fn rename_profile(name: String) -> Self {
        Self {
            title: " Rename profile ".into(),
            cursor: name.chars().count(),
            buffer: name,
            kind: PromptKind::RenameProfile,
        }
    }

    fn byte_at(&self, ci: usize) -> usize {
        self.buffer
            .char_indices()
            .nth(ci)
            .map(|(b, _)| b)
            .unwrap_or(self.buffer.len())
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            let b = self.byte_at(self.cursor - 1);
            self.buffer.remove(b);
            self.cursor -= 1;
        }
    }

    fn delete_forward(&mut self) {
        if self.cursor < self.buffer.chars().count() {
            let b = self.byte_at(self.cursor);
            self.buffer.remove(b);
        }
    }

    fn delete_to_start(&mut self) {
        let end = self.byte_at(self.cursor);
        self.buffer.replace_range(..end, "");
        self.cursor = 0;
    }

    fn delete_to_end(&mut self) {
        let start = self.byte_at(self.cursor);
        self.buffer.truncate(start);
    }

    fn prev_word(&self) -> usize {
        let chars: Vec<char> = self.buffer.chars().collect();
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
        let chars: Vec<char> = self.buffer.chars().collect();
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

    fn delete_word(&mut self) {
        let chars: Vec<char> = self.buffer.chars().collect();
        let mut i = self.cursor.min(chars.len());
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        let (start, end) = (self.byte_at(i), self.byte_at(self.cursor));
        self.buffer.replace_range(start..end, "");
        self.cursor = i;
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, cx: &Ctx) {
        let width = (area.width * 4 / 7).clamp(40, 72);
        let footer = if cx.settings.hints {
            vec![hint_line(&[("<enter>", "Confirm"), ("<esc>", "Cancel")])]
        } else {
            vec![]
        };
        let area = with_footer(frame, cx, area, width, 3, footer);
        let bc = if cx.shake > 0 { Color::Red } else { accent() };
        let input = Line::from(format!(" {}", self.buffer));
        let inner_w = area.width.saturating_sub(2) as usize;
        let col = self.cursor + 1;
        let scroll_x = if inner_w > 0 && col >= inner_w {
            (col - inner_w + 1) as u16
        } else {
            0
        };
        frame.render_widget(
            Paragraph::new(input).scroll((0, scroll_x)).block(
                rounded(true)
                    .border_style(Style::new().fg(bc))
                    .title(self.title.clone().fg(bc).bold()),
            ),
            area,
        );
        frame.set_cursor_position((area.x + 1 + col as u16 - scroll_x, area.y + 1));
    }
