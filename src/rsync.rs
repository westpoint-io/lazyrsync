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
    }
    if f.compress {
        args.push("-z".into());
    }
    if f.verbose {
        args.push("-v".into());
    }
    if f.human && !dry_run {
        args.push("-h".into());
    }
    if f.update {
        args.push("-u".into());
    }
    if f.checksum {
        args.push("-c".into());
    }

    if f.delete {
        args.push("--delete".into());
    }
    if f.delete_excluded {
        args.push("--delete-excluded".into());
    }
    if f.backup {
        args.push("--backup".into());
    }
    if f.partial {
        args.push("--partial".into());
    }
    if f.size_only {
        args.push("--size-only".into());
    }
    if f.existing {
        args.push("--existing".into());
    }
    if f.ignore_existing {
        args.push("--ignore-existing".into());
    }
    if f.bwlimit_kbps > 0 {
        args.push(format!("--bwlimit={}", f.bwlimit_kbps));
    }

    if f.progress {
        args.push("--info=progress2".into());
    }

    if let Some(link) = &ep.link_dest {
        args.push(format!("--link-dest={link}"));
    }

    if dry_run {
        args.push("-n".into());
        args.push("--itemize-changes".into());
        args.push("--stats".into());
    }

    for rule in &task.filters.filter {
        args.push(format!("--filter={rule}"));
    }
    for inc in &task.filters.includes {
        args.push(format!("--include={inc}"));
    }
    if !task.filters.include_from.is_empty() {
        args.push(format!(
            "--include-from={}",
            expand_local(&task.filters.include_from)
        ));
    }
    for exc in &task.filters.excludes {
        args.push(format!("--exclude={exc}"));
    }
    if !task.filters.exclude_from.is_empty() {
        args.push(format!(
            "--exclude-from={}",
            expand_local(&task.filters.exclude_from)
        ));
    }
    if !task.filters.files_from.is_empty() {
        args.push(format!(
            "--files-from={}",
            expand_local(&task.filters.files_from)
        ));
    }

    if let Some(rsh) = build_rsh(task, ep) {
        args.push(format!("--rsh={rsh}"));
    }

    if !task.advanced.raw_args.trim().is_empty() {
        args.extend(split_args(&task.advanced.raw_args));
    }

    args.push("--".into());
    args.push(ep.src.clone());
    args.push(ep.dst.clone());

    args
}

fn build_rsh(task: &Task, ep: &Endpoints) -> Option<String> {
    let is_remote = is_remote_path(&ep.src) || is_remote_path(&ep.dst);
    if !is_remote {
        return None;
    }
    let ssh = &task.ssh;
    let mut parts = vec![
        "ssh".to_string(),
        "-o".to_string(),
        "BatchMode=yes".to_string(),
    ];
    if ssh.port != 22 {
        parts.push(format!("-p {}", ssh.port));
    }
    if !ssh.keyfile.is_empty() {
        parts.push(format!("-i {}", ssh.keyfile));
    }
    if !ssh.extra.trim().is_empty() {
        parts.push(ssh.extra.trim().to_string());
    }
    Some(parts.join(" "))
}

pub fn resolved_command(task: &Task, dry_run: bool) -> String {
    format!("rsync {}", build_args(task, dry_run).join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{Action, Task};

    #[test]
    fn default_sync_is_safe_archive() {
        let p = Task::new("t", "/src/", "/dst/");
        let args = build_args(&p, false);
        assert!(args.contains(&"-a".to_string()));
        assert!(args.contains(&"-z".to_string()));
        assert!(args.contains(&"--info=progress2".to_string()));
        assert_eq!(args[args.len() - 2], "/src/");
        assert_eq!(args[args.len() - 1], "/dst/");
        assert!(!args.contains(&"--delete".to_string()));
    }

    #[test]
    fn filter_flags_all_emit_before_path_guard() {
        let mut p = Task::new("t", "/src/", "/dst/");
        p.filters.includes = vec!["*.txt".into()];
        p.filters.excludes = vec!["*.log".into()];
        p.filters.filter = vec!["- .git".into()];
        p.filters.include_from = "/inc.txt".into();
        p.filters.exclude_from = "/exc.txt".into();
        p.filters.files_from = "/files.txt".into();
        let args = build_args(&p, false);
        for expected in [
            "--filter=- .git",
            "--include=*.txt",
            "--include-from=/inc.txt",
            "--exclude=*.log",
            "--exclude-from=/exc.txt",
            "--files-from=/files.txt",
        ] {
            assert!(args.contains(&expected.to_string()), "missing {expected}");
        }
        let pos = |s: &str| args.iter().position(|a| a == s).unwrap();
        assert!(pos("--filter=- .git") < pos("--include=*.txt"));
        assert!(pos("--include=*.txt") < pos("--exclude=*.log"));
        assert_eq!(args[args.len() - 3], "--");
    }

    #[test]
    fn paths_are_guarded_by_end_of_options() {
        let p = Task::new("t", "-n", "/dst/");
        let args = build_args(&p, false);
        assert_eq!(args[args.len() - 3], "--");
        assert_eq!(args[args.len() - 2], "-n");
        assert_eq!(args[args.len() - 1], "/dst/");
    }

    #[test]
    fn raw_args_respects_quotes() {
        let mut p = Task::new("t", "/src/", "/dst/");
        p.advanced.raw_args = "--rsync-path='/opt/my rsync/rsync' --stats".into();
        let args = build_args(&p, false);
        assert!(args.contains(&"--rsync-path=/opt/my rsync/rsync".to_string()));
        assert!(args.contains(&"--stats".to_string()));
    }

    #[test]
    fn split_args_handles_quotes_and_whitespace() {
        assert_eq!(split_args("a  b\tc"), vec!["a", "b", "c"]);
        assert_eq!(split_args("\"a b\" c"), vec!["a b", "c"]);
        assert_eq!(split_args("--x='y z'"), vec!["--x=y z"]);
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn prepare_dest_creates_snapshot_root() {
        let base = std::env::temp_dir().join(format!("lr-prep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("snaps");
        let mut p = Task::new("t", "/src/", root.to_str().unwrap());
        p.action = Action::Snapshot;
        prepare_dest(&p).unwrap();
        assert!(root.is_dir(), "snapshot root should be created");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn filter_file_paths_expand_tilde() {
        std::env::set_var("HOME", "/home/tester");
        let mut p = Task::new("t", "/src/", "/dst/");
        p.filters.exclude_from = "~/exclude.txt".into();
        p.filters.include_from = "~/inc.txt".into();
        p.filters.files_from = "~/files.txt".into();
        let args = build_args(&p, false);
        assert!(args.contains(&"--exclude-from=/home/tester/exclude.txt".to_string()));
        assert!(args.contains(&"--include-from=/home/tester/inc.txt".to_string()));
        assert!(args.contains(&"--files-from=/home/tester/files.txt".to_string()));
    }

    #[test]
    fn dry_run_adds_n_itemize_and_stats() {
        let p = Task::new("t", "/src/", "/dst/");
        let args = build_args(&p, true);
        assert!(args.contains(&"-n".to_string()));
        assert!(args.contains(&"--itemize-changes".to_string()));
        assert!(args.contains(&"--stats".to_string()));
    }
