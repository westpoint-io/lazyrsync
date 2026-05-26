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
