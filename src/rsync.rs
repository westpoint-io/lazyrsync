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
    crate::paths::expand_path(path)
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

pub struct ArgOpts {
    pub dry_run: bool,
    pub stats: bool,
    pub quiet: bool,
}

pub fn build_args(task: &Task, dry_run: bool) -> Vec<String> {
    build_args_with(
        task,
        ArgOpts {
            dry_run,
            stats: true,
            quiet: false,
        },
    )
}

pub fn build_args_with(task: &Task, opts: ArgOpts) -> Vec<String> {
    assemble(task, &resolve(task), opts)
}

fn host_of(p: &str) -> Option<&str> {
    if !is_remote_path(p) {
        return None;
    }
    let host = p.split(':').next().unwrap_or(p);
    (!host.is_empty()).then_some(host)
}

pub fn uses_ssh(task: &Task) -> bool {
    remote_host(task).is_some()
}

pub fn remote_host(task: &Task) -> Option<String> {
    let ep = resolve(task);
    host_of(&ep.dst)
        .or_else(|| host_of(&ep.src))
        .map(str::to_string)
}

pub fn ssh_probe_args(task: &Task) -> Option<Vec<String>> {
    let host = remote_host(task)?;
    let ssh = &task.ssh;
    let mut args = vec![
        "-o".into(),
        "BatchMode=yes".into(),
        "-o".into(),
        "ConnectTimeout=5".into(),
    ];
    if ssh.port != 22 {
        args.push("-p".into());
        args.push(ssh.port.to_string());
    }
    if !ssh.keyfile.is_empty() {
        args.push("-i".into());
        args.push(expand_local(&ssh.keyfile));
    }
    args.extend(split_args(ssh.extra.trim()));
    args.push(host);
    args.push("true".into());
    Some(args)
}

fn ssh_reason(stderr: &str) -> String {
    stderr
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty() && !l.starts_with("Warning:"))
        .unwrap_or("no further detail")
        .to_string()
}

pub fn check_ssh(task: &Task) -> Result<(), String> {
    let Some(args) = ssh_probe_args(task) else {
        return Ok(());
    };
    let host = remote_host(task).unwrap_or_default();
    let out = std::process::Command::new("ssh")
        .args(&args)
        .stdin(std::process::Stdio::null())
        .output();
    match out {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            let detail = ssh_reason(&stderr);
            Err(format!("cannot reach {host} over ssh: {detail}"))
        }
        Err(e) => Err(format!("cannot run ssh to reach {host}: {e}")),
    }
}

pub fn prepare_dest(task: &Task) -> std::io::Result<()> {
    let ep = resolve(task);
    if is_remote_path(&ep.dst) {
        return Ok(());
    }
    if let Some(parent) = std::path::Path::new(&ep.dst).parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn assemble(task: &Task, ep: &Endpoints, opts: ArgOpts) -> Vec<String> {
    let f = &task.flags;
    let dry_run = opts.dry_run;
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
    if opts.quiet {
        args.push("-q".into());
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
        if opts.stats {
            args.push("--stats".into());
        }
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

pub fn binary() -> &'static str {
    static BINARY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    BINARY.get_or_init(|| {
        let configured = crate::store::Settings::load()
            .map(|s| s.rsync_path)
            .unwrap_or_default();
        let configured = configured.trim();
        if configured.is_empty() {
            "rsync".into()
        } else {
            configured.to_string()
        }
    })
}

pub fn resolved_command(task: &Task, dry_run: bool) -> String {
    format!("{} {}", binary(), build_args(task, dry_run).join(" "))
}

const DOCS: &str = "lazyrsync.westpoint.io/docs/install";

fn needs_rsync31(detected: &str) -> String {
    format!("{detected}; rsync 3.1+ is needed. See: {DOCS}")
}

pub fn version_warning(version_output: &str) -> Option<String> {
    let first = version_output
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default();
    if !first.starts_with("rsync ") {
        return Some(needs_rsync31(&if first.contains("openrsync") {
            "openrsync was detected".to_string()
        } else {
            format!("an unrecognized rsync was detected (\"{first}\")")
        }));
    }
    let version = first.split_whitespace().nth(2)?;
    let mut parts = version.trim_start_matches('v').split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next().and_then(|m| m.parse().ok()).unwrap_or(0);
    ((major, minor) < (3, 1)).then(|| needs_rsync31(&format!("rsync {version} was detected")))
}

pub fn capability_warning() -> Option<String> {
    let out = match std::process::Command::new(binary())
        .arg("--version")
        .output()
    {
        Ok(out) => out,
        Err(_) => return Some(needs_rsync31(&format!("could not run `{}`", binary()))),
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    if stdout.trim().is_empty() {
        version_warning(&String::from_utf8_lossy(&out.stderr))
    } else {
        version_warning(&stdout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{Action, Task};

    #[test]
    fn accepts_rsync_31_and_flags_anything_else() {
        assert!(version_warning("rsync  version 3.1.0  protocol version 30").is_none());
        assert!(version_warning("rsync  version 3.2.7  protocol version 31").is_none());
        assert!(version_warning("rsync  version v3.4.1  protocol version 32").is_none());
        let no_progress2 = version_warning("rsync  version 3.0.9  protocol version 30").unwrap();
        assert!(no_progress2.contains("3.0.9"), "{no_progress2}");
        let old = version_warning("rsync  version 2.6.9  protocol version 29").unwrap();
        assert!(old.contains("2.6.9"), "{old}");
        let open = version_warning("openrsync: protocol version 29").unwrap();
        assert!(open.contains("openrsync") && open.contains(DOCS), "{open}");
        assert!(version_warning("").is_some());
    }

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
    fn prepare_dest_creates_parents_for_a_nested_sync_dest() {
        let base = std::env::temp_dir().join(format!("lr-nest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let dst = base.join("dst/host/2026-07-27");
        let p = Task::new("t", "/src/", format!("{}/", dst.display()));
        prepare_dest(&p).unwrap();
        assert!(
            dst.parent().unwrap().is_dir(),
            "missing parents of a sync dest should be created"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn nested_dest_transfers_against_real_rsync() {
        use std::fs;
        if std::process::Command::new("rsync")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("rsync not installed — skipping live nested-dest test");
            return;
        }
        let base = std::env::temp_dir().join(format!("lr-nest-run-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let src = base.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("a.txt"), "hello").unwrap();

        let dst = base.join("dst/{hostname}/{now:%Y-%m-%d}");
        let p = Task::new(
            "t",
            format!("{}/", src.display()),
            format!("{}/", dst.display()),
        );
        prepare_dest(&p).unwrap();
        let status = std::process::Command::new("rsync")
            .args(build_args(&p, false))
            .status()
            .expect("spawn rsync");

        let landed = resolve(&p).dst;
        let copied = std::path::Path::new(&landed).join("a.txt");
        let ok = status.success() && copied.is_file();
        let _ = fs::remove_dir_all(&base);
        assert!(
            ok,
            "nested dest should transfer: status={status:?} expected file at {}",
            copied.display()
        );
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

    #[test]
    fn without_stats_keeps_the_dry_run_diff_but_drops_the_stats_block() {
        let p = Task::new("t", "/src/", "/dst/");
        let args = build_args_with(
            &p,
            ArgOpts {
                dry_run: true,
                stats: false,
                quiet: false,
            },
        );
        assert!(args.contains(&"-n".to_string()));
        assert!(args.contains(&"--itemize-changes".to_string()));
        assert!(!args.contains(&"--stats".to_string()));
    }

    #[test]
    fn quiet_emits_q_before_the_path_guard() {
        let p = Task::new("t", "/src/", "/dst/");
        let args = build_args_with(
            &p,
            ArgOpts {
                dry_run: false,
                stats: false,
                quiet: true,
            },
        );
        let q = args.iter().position(|a| a == "-q").expect("missing -q");
        let guard = args.iter().position(|a| a == "--").expect("missing --");
        assert!(
            q < guard,
            "-q must precede the end-of-options guard: {args:?}"
        );
        assert!(!build_args(&p, false).contains(&"-q".to_string()));
    }

    #[test]
    fn inline_remote_dest_passes_through() {
        let p = Task::new("t", "/src/", "me@vps:/backup/");
        let args = build_args(&p, false);
        assert_eq!(args[args.len() - 2], "/src/");
        assert_eq!(args[args.len() - 1], "me@vps:/backup/");
    }

    #[test]
    fn inline_remote_source_passes_through() {
        let p = Task::new("t", "me@vps:/data/", "/local/");
        let args = build_args(&p, false);
        assert_eq!(args[args.len() - 2], "me@vps:/data/");
        assert_eq!(args[args.len() - 1], "/local/");
    }

    #[test]
    fn snapshot_builds_numbered_dest_and_link_dest() {
        let base = std::env::temp_dir().join(format!("lr-snap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("1")).unwrap();
        let mut p = Task::new("t", "/src/", base.to_str().unwrap());
        p.action = Action::Snapshot;
        let ep = resolve(&p);
        let _ = std::fs::remove_dir_all(&base);
        assert_eq!(ep.dst, format!("{}/2", base.display()));
        assert_eq!(ep.link_dest, Some(format!("{}/1", base.display())));
    }

    #[test]
    fn preserve_flags_apply_on_top_of_archive() {
        let mut p = Task::new("t", "/src/", "/dst/");
        p.flags.hardlinks = true;
        let args = build_args(&p, false);
        assert!(args.contains(&"-a".to_string()));
        assert!(args.contains(&"-H".to_string()));

        p.flags.archive = false;
        let args = build_args(&p, false);
        assert!(!args.contains(&"-a".to_string()));
        assert!(args.contains(&"-H".to_string()));
    }

    #[test]
    fn the_reason_is_the_last_line_that_is_not_a_warning() {
        let out = "Warning: Permanently added '[127.0.0.1]:2222' (ED25519) to the list of known hosts.\nlight@127.0.0.1: Permission denied (publickey).\n";
        assert_eq!(
            ssh_reason(out),
            "light@127.0.0.1: Permission denied (publickey)."
        );
        assert_eq!(
            ssh_reason("ssh: Could not resolve hostname nope: Name or service not known\n"),
            "ssh: Could not resolve hostname nope: Name or service not known"
        );
        assert_eq!(ssh_reason("   \n"), "no further detail");
    }

    #[test]
    fn a_local_path_with_a_colon_is_not_remote() {
        let t = Task::new("t", "~/src/", "~/another-destination:power/system");
        assert!(
            remote_host(&t).is_none(),
            "a tilde path with a colon is still local"
        );
        assert!(ssh_probe_args(&t).is_none());
    }

    #[test]
    fn a_probe_names_the_host_and_carries_the_ssh_settings() {
        let mut t = Task::new("t", "/src/", "deploy@box:/srv/");
        t.ssh.port = 2222;
        t.ssh.extra = "-o ConnectTimeout=5".into();
        assert_eq!(
            ssh_probe_args(&t).unwrap(),
            [
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=5",
                "-p",
                "2222",
                "-o",
                "ConnectTimeout=5",
                "deploy@box",
                "true"
            ]
        );
    }

    #[test]
    fn a_probe_reads_the_host_from_either_end() {
        let pull = Task::new("t", "box:/srv/", "/dst/");
        assert_eq!(remote_host(&pull).as_deref(), Some("box"));
        let push = Task::new("t", "/src/", "deploy@box:/srv/");
        assert_eq!(remote_host(&push).as_deref(), Some("deploy@box"));
    }

    #[test]
    fn a_local_transfer_is_never_probed() {
        let mut t = Task::new("t", "/src/", "/dst/");
        t.ssh.port = 2222;
        assert!(ssh_probe_args(&t).is_none());
    }

    #[test]
    fn remote_always_builds_rsh_with_batchmode() {
        let p = Task::new("t", "/src/", "user@host:/dst/");
        assert!(build_args(&p, false)
            .iter()
            .any(|a| a == "--rsh=ssh -o BatchMode=yes"));
        let mut q = Task::new("t", "/src/", "user@host:/dst/");
        q.ssh.port = 2222;
        assert!(build_args(&q, false)
            .iter()
            .any(|a| a == "--rsh=ssh -o BatchMode=yes -p 2222"));
    }

    #[test]
    fn local_dest_never_builds_rsh() {
        let mut p = Task::new("t", "/src/", "/dst/");
        p.ssh.port = 2222;
        let args = build_args(&p, false);
        assert!(!args.iter().any(|a| a.starts_with("--rsh")));
    }

    #[test]
    fn bwlimit_only_when_set() {
        let mut p = Task::new("t", "/src/", "/dst/");
        assert!(!build_args(&p, false)
            .iter()
            .any(|a| a.starts_with("--bwlimit")));
        p.flags.bwlimit_kbps = 5000;
        assert!(build_args(&p, false).iter().any(|a| a == "--bwlimit=5000"));
    }
}
