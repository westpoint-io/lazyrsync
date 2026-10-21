use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const CAP: usize = 200;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub at: String,
    pub task: String,
    pub outcome: Outcome,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub code: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<u64>,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub secs: u64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dry_run: bool,
}

fn is_zero(n: &i32) -> bool {
    *n == 0
}

fn is_zero_u64(n: &u64) -> bool {
    *n == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Ok,
    Failed,
    Cancelled,
}

impl Outcome {
    pub fn label(self) -> &'static str {
        match self {
            Outcome::Ok => "ok",
            Outcome::Failed => "failed",
            Outcome::Cancelled => "cancelled",
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct File {
    #[serde(default, rename = "run")]
    runs: Vec<Record>,
}

pub fn path() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default();
            home.join(".local").join("state")
        });
    base.join("lazyrsync").join("history.toml")
}

pub fn load() -> Vec<Record> {
    if cfg!(test) {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(path()) else {
        return Vec::new();
    };
    toml::from_str::<File>(&text)
        .map(|f| f.runs)
        .unwrap_or_default()
}

fn trim(mut runs: Vec<Record>, cap: usize) -> Vec<Record> {
    let n = runs.len();
    if n > cap {
        runs.drain(0..n - cap);
    }
    runs
}

pub fn append(record: Record, enabled: bool) {
    if cfg!(test) || !enabled {
        return;
    }
    let runs = trim(
        {
            let mut runs = load();
            runs.push(record);
            runs
        },
        CAP,
    );
    let p = path();
    if let Some(dir) = p.parent() {
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
    }
    if let Ok(text) = toml::to_string_pretty(&File { runs }) {
        let _ = std::fs::write(&p, text);
    }
}

pub fn line(r: &Record) -> String {
    format!("{}  {}", r.at, summary(r))
}

pub fn summary(r: &Record) -> String {
    let kind = if r.dry_run { "dry-run" } else { "run" };
    let mut s = format!("{kind} {}  {}", r.task, r.outcome.label());
    if r.outcome == Outcome::Failed && r.code != 0 {
        s.push_str(&format!(" (exit {})", r.code));
    }
    if let Some(files) = r.files {
        s.push_str(&format!("  {files} files"));
    }
    if r.secs > 0 {
        s.push_str(&format!("  {}s", r.secs));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(task: &str) -> Record {
        Record {
            at: "10:00:00".into(),
            task: task.into(),
            outcome: Outcome::Ok,
            code: 0,
            files: None,
            secs: 0,
            dry_run: false,
        }
    }

    #[test]
    fn trimming_drops_the_oldest_and_keeps_the_newest() {
        let runs: Vec<Record> = (0..5).map(|i| rec(&format!("t{i}"))).collect();
        let kept = trim(runs, 3);
        let ids: Vec<&str> = kept.iter().map(|r| r.task.as_str()).collect();
        assert_eq!(ids, ["t2", "t3", "t4"]);
    }

    #[test]
    fn trimming_leaves_a_short_history_alone() {
        let runs = vec![rec("a"), rec("b")];
        assert_eq!(trim(runs.clone(), 200), runs);
    }

    #[test]
    fn a_record_survives_a_round_trip() {
        let mut r = rec("nightly");
        r.outcome = Outcome::Failed;
        r.code = 23;
        r.files = Some(142);
        r.secs = 9;
        r.dry_run = true;
        let text = toml::to_string_pretty(&File {
            runs: vec![r.clone()],
        })
        .unwrap();
        let back = toml::from_str::<File>(&text).unwrap();
        assert_eq!(back.runs, vec![r]);
    }

    #[test]
    fn an_unreadable_history_is_empty_not_fatal() {
        assert!(toml::from_str::<File>("this is not toml {{{")
            .map(|f| f.runs)
            .unwrap_or_default()
            .is_empty());
    }

    #[test]
    fn a_failed_line_names_the_exit_code() {
        let mut r = rec("backup");
        r.outcome = Outcome::Failed;
        r.code = 23;
        assert_eq!(line(&r), "10:00:00  run backup  failed (exit 23)");
    }

    #[test]
    fn a_summary_leaves_the_timestamp_to_the_caller() {
        assert_eq!(summary(&rec("docs")), "run docs  ok");
    }
}
