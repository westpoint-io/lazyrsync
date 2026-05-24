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
