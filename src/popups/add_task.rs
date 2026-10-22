use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph, Wrap},
    Frame,
};

use crate::app::{Cmd, Ctx, LogKind};
use crate::profile::{Action, Task};
use crate::rsync;
use crate::ui::{
    accent, centered, deleted, field_box, field_status, file_status, hint_line_in, warn,
    with_footer,
};

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
    quick: bool,
    ssh_port: String,
    ssh_key: String,
    ssh_extra: String,
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
            quick: false,
            ssh_port: "22".into(),
            ssh_key: String::new(),
            ssh_extra: String::new(),
        }
    }

    pub fn quick() -> Self {
        Self {
            row: 2,
            quick: true,
            ..Self::new()
        }
    }

    fn shows_ssh(&self) -> bool {
        self.quick && rsync::uses_ssh(&self.preview_task())
    }

    fn rows(&self) -> usize {
        if self.shows_ssh() {
            ROWS + 3
        } else {
            ROWS
        }
    }

    fn step_row(&self, step: usize) -> usize {
        let n = self.rows();
        let r = (self.row + step) % n;
        if self.quick && r == 0 {
            (r + step) % n
        } else {
            r
        }
    }

    fn text_mut(&mut self) -> Option<&mut String> {
        match self.row {
            0 => Some(&mut self.label),
            2 => Some(&mut self.source),
            3 => Some(&mut self.dest),
            4 => Some(&mut self.ssh_port),
            5 => Some(&mut self.ssh_key),
            6 => Some(&mut self.ssh_extra),
            _ => None,
        }
    }

    fn text_len(&self) -> usize {
        match self.row {
            0 => self.label.chars().count(),
            2 => self.source.chars().count(),
            3 => self.dest.chars().count(),
            4 => self.ssh_port.chars().count(),
            5 => self.ssh_key.chars().count(),
            6 => self.ssh_extra.chars().count(),
            _ => 0,
        }
    }

    fn text(&self) -> Option<&str> {
        match self.row {
            0 => Some(&self.label),
            2 => Some(&self.source),
            3 => Some(&self.dest),
            4 => Some(&self.ssh_port),
            5 => Some(&self.ssh_key),
            6 => Some(&self.ssh_extra),
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
        if self.quick {
            t.ssh.port = self.ssh_port.trim().parse().unwrap_or(22);
            t.ssh.keyfile = self.ssh_key.trim().to_string();
            t.ssh.extra = self.ssh_extra.trim().to_string();
        }
        t
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, cx: &Ctx) {
        let commit = if self.quick { "Create" } else { "Save" };
        let key_color = if self.quick { warn() } else { accent() };
        let footer = if cx.settings.hints {
            vec![
                hint_line_in(&[("<tab>", "Move"), ("<ctrl+n>", "Complete")], key_color),
                hint_line_in(&[("<enter>", commit), ("<esc>", "Cancel")], key_color),
            ]
        } else {
            vec![]
        };
        let cmd_h = if self.shows_ssh() { 4 } else { 6 };
        let box_h = if self.quick {
            (self.rows() as u16 - 1) * 3 + cmd_h
        } else {
            BOX_H
        };
        let area = with_footer(frame, cx, area, WIDTH, box_h, footer);
        let mut heights = vec![Constraint::Length(3); self.rows()];
        if self.quick {
            heights.remove(0);
        }
        heights.push(Constraint::Length(cmd_h));
        let slots = Layout::vertical(heights).split(area);
        let at = |row: usize| slots[if self.quick { row - 1 } else { row }];

        let cur = |row: usize| (self.row == row).then_some(self.cursor);
        let tint = |c: Option<Color>| if self.quick { Some(warn()) } else { c };

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
        if !self.quick {
            field_box(
                frame,
                at(0),
                "ID",
                self.row == 0,
                name_line,
                id_badge,
                id_border,
                cur(0),
            );
        }
        field_box(
            frame,
            at(1),
            "Action",
            self.row == 1,
            Line::from(vec![
                " ◂ ".dim(),
                self.action
                    .label()
                    .fg(if self.quick { warn() } else { accent() })
                    .bold(),
                " ▸".dim(),
            ]),
            None,
            tint(None),
            None,
        );
        let (mut src_color, mut src_badge) = field_status(&self.source, self.row == 2, false);
        if self.attempted && self.source.trim().is_empty() {
            src_color = deleted();
            src_badge = Some("required".to_string());
        }
        field_box(
            frame,
            at(2),
            "Source",
            self.row == 2,
            Line::from(format!(" {}", self.source)),
            src_badge,
            tint(Some(src_color)),
            cur(2),
        );
        let (mut dst_color, mut dst_badge) = field_status(&self.dest, self.row == 3, true);
        if self.attempted && self.dest.trim().is_empty() {
            dst_color = deleted();
            dst_badge = Some("required".to_string());
        }
        field_box(
            frame,
            at(3),
            "Destination",
            self.row == 3,
            Line::from(format!(" {}", self.dest)),
            dst_badge,
            tint(Some(dst_color)),
            cur(3),
        );

        if self.shows_ssh() {
            let (port_color, port_badge) = match self.ssh_port.trim().parse::<u16>() {
                Ok(_) => (warn(), None),
                Err(_) => (deleted(), Some("not a port".to_string())),
            };
            field_box(
                frame,
                at(4),
                "SSH port",
                self.row == 4,
                Line::from(format!(" {}", self.ssh_port)),
                port_badge,
                Some(port_color),
                cur(4),
            );
            let (key_border, key_badge) = if self.ssh_key.trim().is_empty() {
                (warn(), (self.row == 5).then(|| "optional".to_string()))
            } else {
                file_status(&self.ssh_key, self.row == 5)
            };
            field_box(
                frame,
                at(5),
                "SSH key file",
                self.row == 5,
                Line::from(format!(" {}", self.ssh_key)),
                key_badge,
                Some(key_border),
                cur(5),
            );
            field_box(
                frame,
                at(6),
                "SSH extra",
                self.row == 6,
                Line::from(format!(" {}", self.ssh_extra)),
                (self.row == 6).then(|| "optional".to_string()),
                Some(warn()),
                cur(6),
            );
        }

        let preview = self.preview_task();
        let mut cmd = vec![Span::raw(" → ").dim()];
        cmd.push(Span::raw(rsync::resolved_command(&preview, false)));
        frame.render_widget(
            Paragraph::new(Line::from(cmd))
                .wrap(Wrap { trim: false })
                .block(
                    Block::bordered()
                        .border_type(BorderType::Rounded)
                        .border_style(Style::new().fg(if self.quick { warn() } else { accent() }))
                        .title(Span::raw(" command ").dim()),
                ),
            slots[slots.len() - 1],
        );
    }

    pub fn on_key(&mut self, key: KeyEvent, cx: &mut Ctx) -> Cmd {
        match key.code {
            KeyCode::Esc => Cmd::Close,
            KeyCode::Down | KeyCode::Tab => {
                self.row = self.step_row(1);
                self.cursor_to_end();
                Cmd::None
            }
            KeyCode::Up | KeyCode::BackTab => {
                self.row = self.step_row(self.rows() - 1);
                self.cursor_to_end();
                Cmd::None
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char('h') | KeyCode::Char('l')
                if self.row == 1 =>
            {
                self.action = self.action.next();
                Cmd::None
            }
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
                self.cursor = (self.cursor + 1).min(self.text_len());
                Cmd::None
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = (self.cursor + 1).min(self.text_len());
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
                self.cursor_to_end();
                Cmd::None
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor_to_end();
                Cmd::None
            }
            KeyCode::Delete => {
                self.delete_forward();
                self.clear_errors();
                Cmd::None
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_forward();
                self.clear_errors();
                Cmd::None
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_to_start();
                self.clear_errors();
                Cmd::None
            }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_to_end();
                self.clear_errors();
                Cmd::None
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.row == 2 || self.row == 3 {
                    if let Some(b) = self.text_mut() {
                        *b = crate::paths::complete_path(b);
                    }
                    self.cursor_to_end();
                }
                Cmd::None
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_word();
                self.clear_errors();
                Cmd::None
            }
            KeyCode::Backspace if key.modifiers.contains(KeyModifiers::ALT) => {
                self.delete_word();
                self.clear_errors();
                Cmd::None
            }
            KeyCode::Backspace => {
                self.backspace();
                self.clear_errors();
                Cmd::None
            }
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.backspace();
                self.clear_errors();
                Cmd::None
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                let at = self.cursor;
                let mut inserted = false;
                if let Some(b) = self.text_mut() {
                    let byte = Self::byte_at(b, at);
                    b.insert(byte, c);
                    inserted = true;
                }
                if inserted {
                    self.cursor += 1;
                }
                self.clear_errors();
                Cmd::None
            }
            KeyCode::Enter => self.submit(cx),
            _ => Cmd::None,
        }
    }

    pub fn on_mouse(&mut self, m: MouseEvent, cx: &mut Ctx) -> Cmd {
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let footer_h = if cx.settings.hints { 2 } else { 0 };
                let region = centered(cx.area, WIDTH, BOX_H + footer_h);
                let inside =
                    m.column >= region.x && m.column < region.x + region.width && m.row >= region.y;
                if inside {
                    let idx = ((m.row - region.y) / 3) as usize;
                    if idx < ROWS {
                        self.row = idx;
                        self.cursor_to_end();
                        self.clear_errors();
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                self.row = (self.row + 1) % ROWS;
                self.cursor_to_end();
            }
            MouseEventKind::ScrollUp => {
                self.row = (self.row + ROWS - 1) % ROWS;
                self.cursor_to_end();
            }
            _ => {}
        }
        Cmd::None
    }

    fn clear_errors(&mut self) {
        self.id_taken = false;
        self.attempted = false;
    }

    fn submit(&mut self, cx: &mut Ctx) -> Cmd {
        let source = self.source.trim().to_string();
        let dest = self.dest.trim().to_string();
        let typed = self.label.trim().to_string();
        let mut task = self.preview_task();

        if source.is_empty() || dest.is_empty() {
            self.attempted = true;
            cx.reject();
            return Cmd::None;
        }

        if self.quick && (source.is_empty() || dest.is_empty()) {
            self.attempted = true;
            cx.reject();
            return Cmd::None;
        }
        let existing: Vec<String> = cx
            .store
            .profiles
            .get(cx.profile)
            .map(|p| p.tasks.iter().map(|t| t.id.clone()).collect())
            .unwrap_or_default();
        let taken = |id: &str| existing.iter().any(|e| e == id);

        let id = if typed.is_empty() {
            let base = task.candidate_id();
            let mut cand = base.clone();
            let mut n = 2;
            while taken(&cand) {
                cand = format!("{base}-{n}");
                n += 1;
            }
            cand
        } else if taken(&typed) {
            self.id_taken = true;
            cx.reject();
            return Cmd::None;
        } else {
            typed
        };

        if self.quick {
            task.label = basename(&source);
            task.id = task.candidate_id();
            task.label = task.id.clone();
            task.created = Some(now_unix());
            let id = task.id.clone();
            cx.quick.push(task);
            cx.push_log(
                LogKind::Info,
                format!("quick transfer '{id}' ready — p to preview, r to run"),
            );
            return Cmd::CloseToRuns;
        }
        task.id = id.clone();
        task.label = id.clone();
        task.created = Some(now_unix());
        if let Some(p) = cx.store.profiles.get_mut(cx.profile) {
            p.tasks.insert(0, task);
            p.sort_tasks_by_recency();
            cx.task = p.tasks.iter().position(|t| t.id == id).unwrap_or(0);
        }
        cx.subtab = 0;
        cx.save(&format!("added task '{id}'"));
        Cmd::Close
    }
}
