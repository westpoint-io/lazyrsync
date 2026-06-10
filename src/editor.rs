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
