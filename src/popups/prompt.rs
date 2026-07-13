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

    pub fn on_key(&mut self, key: KeyEvent, cx: &mut Ctx) -> Cmd {
        match key.code {
            KeyCode::Esc => Cmd::Close,
            KeyCode::Left | KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.cursor = self.prev_word();
                Cmd::None
            }
            KeyCode::Right | KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.cursor = self.next_word();
                Cmd::None
            }
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                Cmd::None
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = self.cursor.saturating_sub(1);
                Cmd::None
            }
            KeyCode::Right => {
                self.cursor = (self.cursor + 1).min(self.buffer.chars().count());
                Cmd::None
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = (self.cursor + 1).min(self.buffer.chars().count());
                Cmd::None
            }
            KeyCode::Home => {
                self.cursor = 0;
                Cmd::None
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = 0;
                Cmd::None
            }
            KeyCode::End => {
                self.cursor = self.buffer.chars().count();
                Cmd::None
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = self.buffer.chars().count();
                Cmd::None
            }
            KeyCode::Delete => {
                self.delete_forward();
                Cmd::None
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_forward();
                Cmd::None
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_to_start();
                Cmd::None
            }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_to_end();
                Cmd::None
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_word();
                Cmd::None
            }
            KeyCode::Backspace if key.modifiers.contains(KeyModifiers::ALT) => {
                self.delete_word();
                Cmd::None
            }
            KeyCode::Backspace => {
                self.backspace();
                Cmd::None
            }
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.backspace();
                Cmd::None
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                let b = self.byte_at(self.cursor);
                self.buffer.insert(b, c);
                self.cursor += 1;
                Cmd::None
            }
            KeyCode::Enter => self.submit(cx),
            _ => Cmd::None,
        }
    }

    fn submit(&self, cx: &mut Ctx) -> Cmd {
        let name = self.buffer.trim().to_string();
        if name.is_empty() {
            cx.reject();
            return Cmd::None;
        }
        match self.kind {
            PromptKind::NewProfile => {
                if cx.store.profiles.iter().any(|q| q.name == name) {
                    cx.push_log(
                        LogKind::Warn,
                        format!("a profile named '{name}' already exists"),
                    );
                    cx.reject();
                    return Cmd::None;
                }
                let mut p = Profile::new(name);
                p.created = Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0),
                );
                cx.store.profiles.insert(0, p);
                cx.store.sort_profiles_by_recency();
                cx.profile = 0;
                cx.pcursor = 0;
                cx.task = 0;
                cx.subtab = 1;
                cx.save("added profile");
                Cmd::Close
            }
            PromptKind::RenameProfile => {
                let idx = cx.pcursor;
                if cx
                    .store
                    .profiles
                    .iter()
                    .enumerate()
                    .any(|(i, q)| i != idx && q.name == name)
                {
                    cx.push_log(
                        LogKind::Warn,
                        format!("a profile named '{name}' already exists"),
                    );
                    cx.reject();
                    return Cmd::None;
                }
                if let Some(q) = cx.store.profiles.get_mut(idx) {
                    q.name = name;
                }
                cx.save("renamed profile");
                Cmd::Close
            }
        }
    }
}
