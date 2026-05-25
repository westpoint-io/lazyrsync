use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in s.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !out.is_empty() && !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let s = out.trim_matches('-').to_string();
    if s.is_empty() {
        "task".to_string()
    } else {
        s
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<i64>,
    #[serde(default, rename = "task")]
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    #[default]
    Sync,
    Snapshot,
}

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Action::Sync => "Sync",
            Action::Snapshot => "Snapshot",
        }
    }

    pub fn next(self) -> Action {
        match self {
            Action::Sync => Action::Snapshot,
            Action::Snapshot => Action::Sync,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    #[serde(default)]
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub action: Action,
    pub source: String,
    #[serde(default)]
    pub dest: String,
    #[serde(default)]
    pub flags: Flags,
    #[serde(default)]
    pub filters: Filters,
    #[serde(default)]
    pub ssh: Ssh,
    #[serde(default)]
    pub advanced: Advanced,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_files: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Flags {
    pub archive: bool,
    pub compress: bool,
    pub verbose: bool,
    pub human: bool,
    pub progress: bool,

    pub delete: bool,
    pub delete_excluded: bool,
    pub backup: bool,

    pub update: bool,
    pub checksum: bool,
    pub partial: bool,
    pub size_only: bool,
    pub existing: bool,
    pub ignore_existing: bool,
    pub bwlimit_kbps: u32,

    pub hardlinks: bool,
    pub acls: bool,
    pub xattrs: bool,
}

impl Default for Flags {
    fn default() -> Self {
        Self {
            archive: true,
            compress: true,
            verbose: true,
            human: true,
            progress: true,
            delete: false,
            delete_excluded: false,
            backup: false,
            update: false,
            checksum: false,
            partial: true,
            size_only: false,
            existing: false,
            ignore_existing: false,
            bwlimit_kbps: 0,
            hardlinks: false,
            acls: false,
            xattrs: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Filters {
    pub excludes: Vec<String>,
    pub includes: Vec<String>,
    pub exclude_from: String,
    #[serde(default)]
    pub include_from: String,
    #[serde(default)]
    pub files_from: String,
    #[serde(default)]
    pub filter: Vec<String>,
}

impl Filters {
    pub fn is_empty(&self) -> bool {
        self.excludes.is_empty()
            && self.includes.is_empty()
            && self.filter.is_empty()
            && self.exclude_from.is_empty()
            && self.include_from.is_empty()
            && self.files_from.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Ssh {
    pub port: u16,
    pub keyfile: String,
    pub extra: String,
}

impl Default for Ssh {
    fn default() -> Self {
        Self {
            port: 22,
            keyfile: String::new(),
            extra: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Advanced {
    pub raw_args: String,
}

impl Profile {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            created: None,
            tasks: Vec::new(),
        }
    }

    pub fn ensure_ids(&mut self) {
        let mut used: HashSet<String> = self
            .tasks
            .iter()
            .map(|t| t.id.clone())
            .filter(|s| !s.is_empty())
            .collect();
        for t in &mut self.tasks {
            if !t.id.is_empty() {
                continue;
            }
            let base = t.candidate_id();
            let mut cand = base.clone();
            let mut n = 2;
            while used.contains(&cand) {
                cand = format!("{base}-{n}");
                n += 1;
            }
            used.insert(cand.clone());
            t.id = cand;
        }
    }

    pub fn sort_tasks_by_recency(&mut self) {
        if self.tasks.len() < 2 {
            return;
        }
        self.tasks[1..].sort_by(|a, b| b.created.cmp(&a.created));
    }
}
