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

fn config_dir() -> PathBuf {
    Store::global_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub skip_delete_warning: bool,
    pub hints: bool,
    pub theme: crate::ui::ThemeSpec,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            skip_delete_warning: false,
            hints: true,
            theme: crate::ui::ThemeSpec::default(),
        }
    }
}

impl Settings {
    fn path() -> PathBuf {
        config_dir().join("settings.toml")
    }

    pub fn load() -> Self {
        let path = Self::path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let text = toml::to_string_pretty(self).context("serializing settings")?;
        fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }
}

pub struct Store {
    pub profiles: Vec<Profile>,
    global_path: PathBuf,
}

impl Store {
    pub fn global_path() -> PathBuf {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let home = std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_default();
                home.join(".config")
            });
        base.join("lazyrsync").join("profiles.toml")
    }

    pub fn load() -> Result<Self> {
        let global_path = Self::global_path();
        let mut profiles = read_file(&global_path)?;
        let now = now_unix();
        let mut backfilled = false;
        for p in &mut profiles {
            if p.created.is_none() {
                p.created = Some(now);
                backfilled = true;
            }
            for t in &mut p.tasks {
                if t.created.is_none() {
                    t.created = Some(now);
                    backfilled = true;
                }
            }
            p.sort_tasks_by_recency();
        }
        let store = Self {
            profiles,
            global_path,
        };
        if backfilled {
            let _ = store.save();
        }
        Ok(store)
    }

    pub fn save(&self) -> Result<()> {
        if let Some(dir) = self.global_path.parent() {
            fs::create_dir_all(dir)
                .with_context(|| format!("creating config dir {}", dir.display()))?;
        }
        let file = ProfileFile {
            profiles: self.profiles.clone(),
        };
        let text = toml::to_string_pretty(&file).context("serializing profiles")?;
        fs::write(&self.global_path, text)
            .with_context(|| format!("writing {}", self.global_path.display()))?;
        Ok(())
    }
}

fn read_file(path: &Path) -> Result<Vec<Profile>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let file: StoredFile =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(file.profiles.into_iter().map(Profile::from).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_round_trips_through_toml() {
        let mut t = Task::new("data → nas", "/home/me/data/", "me@nas:/backup/data/");
        t.flags.delete = true;
        t.flags.bwlimit_kbps = 4000;
        t.filters.excludes = vec!["*.tmp".into(), ".git/".into()];
        t.ssh.port = 2222;
        let mut p = Profile::new("backup");
        p.description = "nightly".into();
        p.tasks = vec![t];

        let text = toml::to_string_pretty(&ProfileFile { profiles: vec![p] }).unwrap();
        let back: StoredFile = toml::from_str(&text).unwrap();
        let q: Profile = back.profiles.into_iter().next().unwrap().into();

        assert_eq!(q.name, "backup");
        assert_eq!(q.description, "nightly");
        assert_eq!(q.tasks.len(), 1);
        let task = &q.tasks[0];
        assert_eq!(task.label, "data → nas");
        assert_eq!(task.source, "/home/me/data/");
        assert_eq!(task.dest, "me@nas:/backup/data/");
        assert!(task.flags.delete);
        assert_eq!(task.flags.bwlimit_kbps, 4000);
        assert_eq!(task.filters.excludes, vec!["*.tmp", ".git/"]);
        assert_eq!(task.ssh.port, 2222);
    }

    #[test]
    fn legacy_flat_profile_keeps_host_inline() {
        let legacy = r#"
[[profile]]
name = "old-backup"
source = "/home/me/data/"
destinations = ["me@nas:/backup/"]

[profile.flags]
delete = true
"#;
        let parsed: StoredFile = toml::from_str(legacy).unwrap();
        let p: Profile = parsed.profiles.into_iter().next().unwrap().into();
        assert_eq!(p.name, "old-backup");
        assert_eq!(p.tasks.len(), 1, "legacy profile should fold into one task");
        assert_eq!(p.tasks[0].label, "old-backup");
        assert_eq!(p.tasks[0].source, "/home/me/data/");
        assert_eq!(
            p.tasks[0].dest, "me@nas:/backup/",
            "host kept inline in dest"
        );
        assert!(p.tasks[0].flags.delete);
    }

    #[test]
    fn legacy_push_folds_remote_into_dest() {
        let legacy = r#"
[[profile]]
name = "p"
[[profile.task]]
label = "t"
action = "push"
source = "/home/me/data/"
dest = "/backup/"
remote = "me@nas"
"#;
        let parsed: StoredFile = toml::from_str(legacy).unwrap();
        let p: Profile = parsed.profiles.into_iter().next().unwrap().into();
        assert_eq!(p.tasks[0].source, "/home/me/data/");
        assert_eq!(p.tasks[0].dest, "me@nas:/backup/");
        assert_eq!(p.tasks[0].action, Action::Sync);
    }
