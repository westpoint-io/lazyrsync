use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    style::{Style, Stylize},
    text::{Line, Span},
};

use crate::profile::{Filters, Flags, Task};
use crate::ui::{accent, added, muted, on_accent};

const LABEL_W: usize = 13;

pub enum Outcome {
    Continue,
    Cancel,
    Save,
    Rejected,
}

pub struct FieldView {
    pub label: &'static str,
    pub value: String,
    pub focused: bool,
    pub is_path: bool,
    pub is_dest: bool,
    pub is_file: bool,
    pub dirty: bool,
    pub error: Option<&'static str>,
    pub cursor: usize,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Section {
    Basics,
    Flags,
    Filters,
    Ssh,
    Advanced,
}

pub const SECTIONS: [Section; 5] = [
    Section::Flags,
    Section::Filters,
    Section::Basics,
    Section::Ssh,
    Section::Advanced,
];

impl Section {
    fn fields(self) -> &'static [F] {
        use F::*;
        match self {
            Section::Basics => &[Name, Source, Dest],
            Section::Flags => &[
                Archive,
                Compress,
                Verbose,
                Human,
                Progress,
                Delete,
                DeleteExcluded,
                Backup,
                Update,
                Checksum,
                Partial,
                SizeOnly,
                Existing,
                IgnoreExisting,
                Hardlinks,
                Acls,
                Xattrs,
            ],
            Section::Filters => &[
                Includes,
                IncludeFrom,
                Excludes,
                ExcludeFrom,
                Filter,
                FilesFrom,
            ],
            Section::Ssh => &[SshPort, SshKey, SshExtra],
            Section::Advanced => &[RawArgs],
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum F {
    Name,
    Source,
    Dest,
    Archive,
    Compress,
    Verbose,
    Human,
    Progress,
    Delete,
    DeleteExcluded,
    Backup,
    Update,
    Checksum,
    Partial,
    SizeOnly,
    Existing,
    IgnoreExisting,
    Excludes,
    Includes,
    ExcludeFrom,
    IncludeFrom,
    FilesFrom,
    Filter,
    Hardlinks,
    Acls,
    Xattrs,
    SshPort,
    SshKey,
    SshExtra,
    RawArgs,
}

enum Kind {
    Bool,
    Text,
    Number,
    List,
}

impl F {
    fn short(self) -> &'static str {
        use F::*;
        match self {
            Archive => "archive",
            Compress => "compress",
            Verbose => "verbose",
            Human => "human",
            Progress => "progress",
            Delete => "delete",
            DeleteExcluded => "del-excl",
            Backup => "backup",
            Update => "update",
            Checksum => "checksum",
            Partial => "partial",
            SizeOnly => "size-only",
            Existing => "existing",
            IgnoreExisting => "ign-exist",
            Hardlinks => "hardlinks",
            Acls => "acls",
            Xattrs => "xattrs",
            _ => "",
        }
    }

    fn label(self) -> &'static str {
        use F::*;
        match self {
            Name => "Name",
            Source => "Source",
            Dest => "Destination",
            Archive => "-a archive",
            Compress => "-z compress",
            Verbose => "-v verbose",
            Human => "-h human",
            Progress => "--info=progress2",
            Delete => "--delete",
            DeleteExcluded => "--delete-excluded",
            Backup => "--backup",
            Update => "-u update",
            Checksum => "-c checksum",
            Partial => "--partial",
            SizeOnly => "--size-only",
            Existing => "--existing",
            IgnoreExisting => "--ignore-existing",
            Excludes => "Excludes",
            Includes => "Includes",
            ExcludeFrom => "Exclude-from",
            IncludeFrom => "Include-from",
            FilesFrom => "Files-from",
            Filter => "Filter",
            Hardlinks => "-H hardlinks",
            Acls => "-A acls",
            Xattrs => "-X xattrs",
            SshPort => "SSH port",
            SshKey => "SSH key file",
            SshExtra => "SSH extra",
            RawArgs => "Raw args",
        }
    }

    fn is_path(self) -> bool {
        use F::*;
        matches!(
            self,
            Source | Dest | ExcludeFrom | IncludeFrom | FilesFrom | SshKey
        )
    }

    fn kind(self) -> Kind {
        use F::*;
        match self {
            Name | Source | Dest | ExcludeFrom | IncludeFrom | FilesFrom | SshKey | SshExtra
            | RawArgs => Kind::Text,
            Excludes | Includes | Filter => Kind::List,
            SshPort => Kind::Number,
            _ => Kind::Bool,
        }
    }
}

pub struct Editor {
    task: Task,
    orig: Task,
    taken: Vec<String>,

    section_idx: usize,
    field_idx: usize,
    editing: bool,
    buffer: String,
    cursor: usize,
    attempted: bool,
}

impl Editor {
    pub fn edit(task: Task, taken: Vec<String>) -> Self {
        Self {
            orig: task.clone(),
            task,
            taken,
            section_idx: 0,
            field_idx: 0,
            editing: false,
            buffer: String::new(),
            cursor: 0,
            attempted: false,
        }
    }

    pub fn orig_id(&self) -> &str {
        &self.orig.id
    }

    pub fn current_section(&self) -> Section {
        SECTIONS[self.section_idx]
    }

    pub fn focus_section(&mut self, s: Section) {
        if let Some(i) = SECTIONS.iter().position(|x| *x == s) {
            self.section_idx = i;
            self.field_idx = 0;
        }
    }

    pub fn field_count(&self) -> usize {
        self.current_section().fields().len()
    }

    pub fn focus_field(&mut self, idx: usize) {
        if idx < self.field_count() && idx != self.field_idx {
            self.commit_edit();
            self.field_idx = idx;
            self.enter_form();
        }
    }

    pub fn task(&self) -> &Task {
        &self.task
    }

    fn current_field(&self) -> F {
        self.current_section().fields()[self.field_idx]
    }

    pub fn enter_form(&mut self) {
        self.buffer = self.get_text(self.current_field());
        self.cursor = self.buffer.chars().count();
        self.editing = true;
    }

    fn byte_at(&self, char_idx: usize) -> usize {
        self.buffer
            .char_indices()
            .nth(char_idx)
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

    fn delete_forward(&mut self) {
        let len = self.buffer.chars().count();
        if self.cursor < len {
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

    pub fn form_views(&self) -> Vec<FieldView> {
        self.current_section()
            .fields()
            .iter()
            .enumerate()
            .map(|(i, &f)| {
                let focused = i == self.field_idx;
                let value = if focused && self.editing {
                    self.buffer.clone()
                } else {
                    self.get_text(f)
                };
                let dirty = value != text_of(&self.orig, f);
                let error = if matches!(f.kind(), Kind::Number) && !valid_port(&value) {
                    Some("1-65535")
                } else if self.attempted {
                    self.field_error(f, &value)
                } else {
                    None
                };
                let cursor = if focused && self.editing {
                    self.cursor.min(value.chars().count())
                } else {
                    0
                };
                FieldView {
                    label: f.label(),
                    value,
                    focused,
                    is_path: matches!(f, F::Source | F::Dest),
                    is_dest: matches!(f, F::Dest),
                    is_file: matches!(f, F::ExcludeFrom | F::IncludeFrom | F::FilesFrom),
                    dirty,
                    error,
                    cursor,
                }
            })
            .collect()
    }

    fn field_error(&self, f: F, value: &str) -> Option<&'static str> {
        let v = value.trim();
        match f {
            F::Name if v.is_empty() => Some("required"),
            F::Name if self.taken.iter().any(|t| t == v) => Some("taken"),
            F::Source | F::Dest if v.is_empty() => Some("required"),
            _ => None,
        }
    }

    fn is_valid(&self) -> bool {
        let name = self.task.id.trim();
        !name.is_empty()
            && !self.taken.iter().any(|t| t == name)
            && !self.task.source.trim().is_empty()
            && !self.task.dest.trim().is_empty()
    }

    pub fn form_key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Esc => return Outcome::Cancel,
            KeyCode::Enter => {
                if matches!(self.current_field().kind(), Kind::Number) && !valid_port(&self.buffer)
                {
                    self.attempted = true;
                    return Outcome::Rejected;
                }
                self.commit_edit();
                if self.is_valid() {
                    return Outcome::Save;
                }
                self.attempted = true;
                self.enter_form();
                return Outcome::Rejected;
            }
            KeyCode::Up | KeyCode::BackTab => {
                self.commit_edit();
                self.prev_field();
                self.enter_form();
            }
            KeyCode::Down | KeyCode::Tab => {
                self.commit_edit();
                self.next_field();
                self.enter_form();
            }
            KeyCode::Left | KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.cursor = self.prev_word()
            }
            KeyCode::Right | KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.cursor = self.next_word()
            }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = self.cursor.saturating_sub(1)
            }
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.buffer.chars().count()),
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = (self.cursor + 1).min(self.buffer.chars().count())
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => self.cursor = 0,
            KeyCode::End => self.cursor = self.buffer.chars().count(),
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = self.buffer.chars().count()
            }
            KeyCode::Delete => {
                self.delete_forward();
                self.attempted = false;
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_forward();
                self.attempted = false;
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_to_start();
                self.attempted = false;
            }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_to_end();
                self.attempted = false;
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.current_field().is_path() {
                    self.buffer = crate::paths::complete_path(&self.buffer);
                    self.cursor = self.buffer.chars().count();
                }
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_word();
                self.attempted = false;
            }
            KeyCode::Backspace if key.modifiers.contains(KeyModifiers::ALT) => {
                self.delete_word();
                self.attempted = false;
            }
            KeyCode::Backspace => {
                self.backspace();
                self.attempted = false;
            }
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.backspace();
                self.attempted = false;
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if matches!(self.current_field().kind(), Kind::Number) && !c.is_ascii_digit() {
                    return Outcome::Continue;
                }
                let b = self.byte_at(self.cursor);
                self.buffer.insert(b, c);
                self.cursor += 1;
                self.attempted = false;
            }
            _ => {}
        }
        Outcome::Continue
    }

    fn next_field(&mut self) {
        let n = self.current_section().fields().len();
        self.field_idx = (self.field_idx + 1) % n;
    }

    fn prev_field(&mut self) {
        let n = self.current_section().fields().len();
        self.field_idx = (self.field_idx + n - 1) % n;
    }

    fn commit_edit(&mut self) {
        let f = self.current_field();
        let buf = std::mem::take(&mut self.buffer);
        self.set_text(f, &buf);
        self.editing = false;
    }

    fn get_text(&self, f: F) -> String {
        text_of(&self.task, f)
    }

    fn set_text(&mut self, f: F, s: &str) {
        let s = s.trim();
        match f {
            F::Name => {
                self.task.id = s.to_string();
                self.task.label = s.to_string();
            }
            F::Source => self.task.source = s.to_string(),
            F::Dest => self.task.dest = s.to_string(),
            F::Excludes => self.task.filters.excludes = split_list(s),
            F::Includes => self.task.filters.includes = split_list(s),
            F::Filter => self.task.filters.filter = split_list(s),
            F::ExcludeFrom => self.task.filters.exclude_from = s.to_string(),
            F::IncludeFrom => self.task.filters.include_from = s.to_string(),
            F::FilesFrom => self.task.filters.files_from = s.to_string(),
            F::SshKey => self.task.ssh.keyfile = s.to_string(),
            F::SshExtra => self.task.ssh.extra = s.to_string(),
            F::RawArgs => self.task.advanced.raw_args = s.to_string(),
