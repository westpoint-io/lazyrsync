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
