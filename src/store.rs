use crate::profile::{Action, Advanced, Filters, Flags, Profile, Ssh, Task};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default, Serialize)]
struct ProfileFile {
    #[serde(rename = "profile")]
    profiles: Vec<Profile>,
}

#[derive(Default, Deserialize)]
struct StoredFile {
    #[serde(default, rename = "profile")]
    profiles: Vec<StoredProfile>,
}

#[derive(Deserialize)]
struct StoredProfile {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    created: Option<i64>,
    #[serde(default, rename = "task")]
    tasks: Vec<StoredTask>,

    source: Option<String>,
    #[serde(default)]
    destinations: Vec<String>,
    flags: Option<Flags>,
    filters: Option<Filters>,
    ssh: Option<Ssh>,
    advanced: Option<Advanced>,
}

#[derive(Deserialize)]
struct StoredTask {
    #[serde(default)]
    id: String,
    label: String,
    #[serde(default)]
    action: Option<String>,
    source: String,
    #[serde(default)]
    dest: String,
    #[serde(default)]
    remote: String,
    #[serde(default)]
    destinations: Vec<String>,
    #[serde(default)]
    flags: Flags,
    #[serde(default)]
    filters: Filters,
    #[serde(default)]
    ssh: Ssh,
    #[serde(default)]
    advanced: Advanced,
    #[serde(default)]
    created: Option<i64>,
    #[serde(default)]
    last_files: Option<u64>,
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn fold_remote(
    remote: &str,
    source: String,
    dest: String,
    legacy_action: &str,
) -> (String, String) {
    let r = remote.trim();
    if r.is_empty() {
        (source, dest)
    } else if legacy_action == "pull" {
        (format!("{r}:{source}"), dest)
    } else {
        (source, format!("{r}:{dest}"))
    }
}

impl From<StoredTask> for Task {
    fn from(s: StoredTask) -> Self {
        let legacy = s.action.as_deref().unwrap_or("");
        let action = if legacy == "snapshot" {
            Action::Snapshot
        } else {
            Action::Sync
        };
        let dest = if s.dest.is_empty() {
            s.destinations.first().cloned().unwrap_or_default()
        } else {
            s.dest
        };
        let (source, dest) = fold_remote(&s.remote, s.source, dest, legacy);
        Task {
            id: s.id,
            label: s.label,
            action,
            source,
            dest,
            flags: s.flags,
            filters: s.filters,
            ssh: s.ssh,
            advanced: s.advanced,
            created: s.created,
            last_files: s.last_files,
        }
    }
}

impl From<StoredProfile> for Profile {
    fn from(s: StoredProfile) -> Self {
        let mut tasks: Vec<Task> = s.tasks.into_iter().map(Task::from).collect();
        if tasks.is_empty() {
            if let Some(source) = s.source {
                tasks.push(Task {
                    id: String::new(),
                    label: s.name.clone(),
                    action: Action::Sync,
                    source,
                    dest: s.destinations.first().cloned().unwrap_or_default(),
                    flags: s.flags.unwrap_or_default(),
                    filters: s.filters.unwrap_or_default(),
                    ssh: s.ssh.unwrap_or_default(),
                    advanced: s.advanced.unwrap_or_default(),
                    created: None,
                    last_files: None,
                });
            }
        }
        let mut p = Profile {
            name: s.name,
            description: s.description,
            created: s.created,
            tasks,
        };
        p.ensure_ids();
        p
    }
}
