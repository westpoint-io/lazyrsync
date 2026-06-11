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
