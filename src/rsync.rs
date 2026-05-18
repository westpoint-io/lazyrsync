use crate::profile::{Action, Task};

pub struct Endpoints {
    pub src: String,
    pub dst: String,
    pub link_dest: Option<String>,
}

fn is_remote_path(p: &str) -> bool {
    p.contains('@') || (p.contains(':') && !p.starts_with('/'))
}

fn expand_local(path: &str) -> String {
    let home = std::env::var("HOME").ok().filter(|h| !h.is_empty());
    match (path, home) {
        ("~", Some(home)) => home,
        (p, Some(home)) if p.starts_with("~/") => format!("{home}/{}", &p[2..]),
        _ => path.to_string(),
    }
}

pub fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut has = false;
    let mut single = false;
    let mut double = false;
    for c in s.chars() {
        match c {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            c if c.is_whitespace() && !single && !double => {
                if has {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
                continue;
            }
            c => cur.push(c),
        }
        has = true;
    }
    if has {
        out.push(cur);
    }
    out
}

pub struct SnapshotInfo {
    pub count: usize,
    pub latest: u32,
    pub next: u32,
    pub newest_unix: Option<i64>,
}

pub fn snapshot_info(task: &Task) -> Option<SnapshotInfo> {
    if !matches!(task.action, Action::Snapshot) {
        return None;
    }
    let root = expand_local(&task.dest);
    let root = root.trim_end_matches('/').to_string();
    if is_remote_path(&root) {
        return None;
    }
    let mut nums: Vec<u32> = match std::fs::read_dir(&root) {
        Ok(rd) => rd
            .flatten()
            .filter_map(|e| e.file_name().to_string_lossy().parse::<u32>().ok())
            .collect(),
        Err(_) => Vec::new(),
    };
    nums.sort_unstable();
    let latest = nums.last().copied().unwrap_or(0);
    let newest_unix = (latest > 0)
        .then(|| std::fs::metadata(format!("{root}/{latest}")).ok())
        .flatten()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64);
    Some(SnapshotInfo {
        count: nums.len(),
        latest,
        next: latest + 1,
        newest_unix,
    })
}

pub fn next_snapshot_n(root: &str) -> u32 {
    let mut max = 0;
    if let Ok(rd) = std::fs::read_dir(root) {
        for entry in rd.flatten() {
            if let Ok(n) = entry.file_name().to_string_lossy().parse::<u32>() {
                max = max.max(n);
            }
        }
    }
    max + 1
}

pub fn resolve(task: &Task) -> Endpoints {
    match task.action {
        Action::Sync => Endpoints {
            src: expand_local(&task.source),
            dst: expand_local(&task.dest),
            link_dest: None,
        },
        Action::Snapshot => {
            let root = expand_local(&task.dest);
            let root = root.trim_end_matches('/').to_string();
            let n = next_snapshot_n(&root);
            Endpoints {
                src: expand_local(&task.source),
                dst: format!("{root}/{n}"),
                link_dest: (n > 1).then(|| format!("{root}/{}", n - 1)),
            }
        }
    }
}

pub fn build_args(task: &Task, dry_run: bool) -> Vec<String> {
    assemble(task, &resolve(task), dry_run)
}

pub fn prepare_dest(task: &Task) -> std::io::Result<()> {
    if !matches!(task.action, Action::Snapshot) {
        return Ok(());
    }
    let ep = resolve(task);
    if is_remote_path(&ep.dst) {
        return Ok(());
    }
    if let Some(parent) = std::path::Path::new(&ep.dst).parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn assemble(task: &Task, ep: &Endpoints, dry_run: bool) -> Vec<String> {
    let f = &task.flags;
    let mut args: Vec<String> = Vec::new();

    if f.archive {
        args.push("-a".into());
    }
    if f.hardlinks {
        args.push("-H".into());
    }
    if f.acls {
        args.push("-A".into());
    }
    if f.xattrs {
        args.push("-X".into());
