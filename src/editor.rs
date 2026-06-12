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
