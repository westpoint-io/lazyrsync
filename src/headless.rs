use std::process::{Command, Stdio};

use anstyle::{AnsiColor, Color, Style};
use anyhow::{anyhow, Result};

use crate::history;
use crate::profile::{Profile, Task};
use crate::rsync;

const PROSE: Style = Style::new().dimmed();
const IDENT: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Cyan)));
const VALUE: Style = Style::new().bold();
const ALERT: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Red)));
const ALERT_STRONG: Style = ALERT.bold();
const GOOD: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));

pub fn list(profiles: &[Profile]) {
    if profiles.is_empty() {
        println!("No profiles. Add one in the TUI (run `lazyrsync`).");
    }
    for p in profiles {
        println!("{}", p.name);
        for t in &p.tasks {
            println!("  {}", t.id);
            println!("      {}", rsync::resolved_command(t, false));
        }
        println!();
    }
}

fn select<'a>(profiles: &'a [Profile], target: &str) -> Result<Vec<&'a Task>> {
    if profiles.is_empty() {
        return Err(anyhow!(
            "no profiles configured (add one in the TUI: lazyrsync)"
        ));
    }
    let (name, task_id) = match target.split_once('/') {
        Some((n, t)) => (n, Some(t)),
        None => (target, None),
    };
    let profile = profiles.iter().find(|p| p.name == name).ok_or_else(|| {
        let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
        anyhow!("no profile named '{name}' (have: {})", names.join(", "))
    })?;
    match task_id {
        None => Ok(profile.tasks.iter().collect()),
        Some(id) => profile
            .tasks
            .iter()
            .find(|t| t.id == id)
            .map(|t| vec![t])
            .ok_or_else(|| {
                let ids: Vec<&str> = profile.tasks.iter().map(|t| t.id.as_str()).collect();
                anyhow!(
                    "no task '{id}' in profile '{name}' (have: {})",
                    ids.join(", ")
                )
            }),
    }
}

fn headless_args(task: &Task, dry_run: bool, verbose: bool) -> Vec<String> {
    let quiet = !dry_run && !verbose;
    let mut t = task.clone();
    t.flags.progress = false;
    if verbose {
        t.flags.verbose = true;
    } else if quiet {
        t.flags.verbose = false;
    }
    rsync::build_args_with(
        &t,
        rsync::ArgOpts {
            dry_run,
            stats: false,
            quiet,
        },
    )
}

const COULD_NOT_START: i32 = 3;
const VANISHED_SOURCE_FILES: i32 = 24;

fn succeeded(code: i32) -> bool {
    code == 0 || code == VANISHED_SOURCE_FILES
}

fn spawn_task(task: &Task, dry_run: bool, verbose: bool) -> Result<i32> {
    rsync::check_ssh(task).map_err(|e| anyhow::anyhow!(e))?;
    if !dry_run {
        rsync::prepare_dest(task)?;
    }
    let status = Command::new(rsync::binary())
        .args(headless_args(task, dry_run, verbose))
        .stdin(Stdio::null())
        .status()?;
    Ok(status.code().unwrap_or(COULD_NOT_START))
}

pub fn log(limit: usize) -> i32 {
    let runs = history::load();
    if runs.is_empty() {
        anstream::eprintln!("{PROSE}no runs recorded yet{PROSE:#}");
        return 0;
    }
    for r in runs.iter().rev().take(limit).rev() {
        println!("{}", history::line(r));
    }
    0
}

pub fn run(profiles: &[Profile], target: &str, dry_run: bool, yes: bool, verbose: bool) -> i32 {
    let history_on = crate::store::Settings::load()
        .map(|s| s.history)
        .unwrap_or(true);
    let tasks = match select(profiles, target) {
        Ok(t) => t,
        Err(e) => {
            anstream::eprintln!("{ALERT_STRONG}error:{ALERT_STRONG:#} {e:#}");
            return 2;
        }
    };
    if !yes && !dry_run {
        let blocked: Vec<&str> = tasks
            .iter()
            .filter(|t| t.destructive())
            .map(|t| t.id.as_str())
            .collect();
        if !blocked.is_empty() {
            anstream::eprintln!(
                "{ALERT_STRONG}error:{ALERT_STRONG:#} nothing ran — these tasks delete files at the destination and need --yes: {IDENT}{}{IDENT:#}",
                blocked.join(", ")
            );
            return 1;
        }
    }
    if tasks.is_empty() {
        anstream::eprintln!("{PROSE}nothing to do: profile '{target}' has no tasks{PROSE:#}");
        return 0;
    }
    if let Some(w) = rsync::capability_warning() {
        anstream::eprintln!("{ALERT_STRONG}warning:{ALERT_STRONG:#} {w}");
    }
    let mut ok = 0;
    let mut failed = 0;
    let mut first_failure = 0;
    let count = tasks.len();
    let width = tasks
        .iter()
        .map(|t| t.label.chars().count())
        .max()
        .unwrap_or(0);
    for (i, t) in tasks.iter().enumerate() {
        let started = std::time::Instant::now();
        let code = match spawn_task(t, dry_run, verbose) {
            Ok(c) => c,
            Err(e) => {
                anstream::eprintln!(
                    "{ALERT_STRONG}error:{ALERT_STRONG:#} {IDENT}{}{IDENT:#}: {e:#}",
                    t.id
                );
                COULD_NOT_START
            }
        };
        let elapsed = started.elapsed().as_secs_f64();
        history::append(
            history::Record {
                at: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
                task: t.id.clone(),
                outcome: if succeeded(code) {
                    history::Outcome::Ok
                } else {
                    history::Outcome::Failed
                },
                code,
                files: None,
                secs: elapsed as u64,
                dry_run,
            },
            history_on,
        );
        let counter = format!("[{}/{count}]", i + 1);
        let label = format!("{:<width$}", t.label);
        if succeeded(code) {
            anstream::println!(
                "{PROSE}{counter}{PROSE:#} {GOOD}✔{GOOD:#} {IDENT}{label}{IDENT:#}  {PROSE}{elapsed:.1}s{PROSE:#}"
            );
            ok += 1;
        } else {
            anstream::eprintln!(
                "{PROSE}{counter}{PROSE:#} {ALERT}✗{ALERT:#} {IDENT}{label}{IDENT:#}  {PROSE}exit {code}  {}{PROSE:#}",
                t.id
            );
            failed += 1;
            if first_failure == 0 {
                first_failure = code;
            }
        }
    }
    let plural = if count == 1 { "" } else { "s" };
    let failed_value = if failed > 0 { ALERT_STRONG } else { VALUE };
    let summary = format!(
        "{VALUE}{count}{VALUE:#} {PROSE}task{plural}:{PROSE:#} {VALUE}{ok}{VALUE:#} {PROSE}ok,{PROSE:#} {failed_value}{failed}{failed_value:#} {PROSE}failed{PROSE:#}"
    );
    if failed > 0 {
        anstream::eprintln!("\n{summary}");
    } else {
        anstream::println!("\n{summary}");
    }
    first_failure
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Task;

    fn task(id: &str, source: &str, dest: &str) -> Task {
        let mut t = Task::new(format!("{id} label"), source, dest);
        t.id = id.to_string();
        t
    }

    fn profile(tasks: Vec<Task>) -> Profile {
        let mut p = Profile::new("backups");
        p.tasks = tasks;
        p
    }

    fn rsync_missing() -> bool {
        std::process::Command::new("rsync")
            .arg("--version")
            .output()
            .is_err()
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!("lr-cli-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        base
    }

    fn seed_source(base: &std::path::Path) -> std::path::PathBuf {
        let src = base.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.txt"), "hello").unwrap();
        src
    }

    #[test]
    fn transfers_a_task_and_returns_0() {
        if rsync_missing() {
            eprintln!("rsync not installed — skipping");
            return;
        }
        let base = scratch("ok");
        let src = seed_source(&base);
        let dst = base.join("dst");
        let ps = vec![profile(vec![task(
            "photos",
            &format!("{}/", src.display()),
            &format!("{}/", dst.display()),
        )])];

        let code = run(&ps, "backups", false, false, false);
        let landed = dst.join("a.txt").is_file();
        let _ = std::fs::remove_dir_all(&base);

        assert_eq!(code, 0);
        assert!(landed, "file should have transferred");
    }

    #[test]
    fn continues_past_a_failing_task_and_returns_its_code() {
        if rsync_missing() {
            eprintln!("rsync not installed — skipping");
            return;
        }
        let base = scratch("mixed");
        let src = seed_source(&base);
        let good_dst = base.join("good");
        let bad_dst = base.join("bad");
        let ps = vec![profile(vec![
            task(
                "missing",
                &format!("{}/nope/", base.display()),
                &format!("{}/", bad_dst.display()),
            ),
            task(
                "photos",
                &format!("{}/", src.display()),
                &format!("{}/", good_dst.display()),
            ),
        ])];

        let code = run(&ps, "backups", false, false, false);
        let later_task_ran = good_dst.join("a.txt").is_file();
        let _ = std::fs::remove_dir_all(&base);

        assert_eq!(code, 23, "rsync's own exit code must be propagated");
        assert!(later_task_ran, "the task after a failure must still run");
    }

    #[test]
    fn dry_run_creates_no_destination_directories() {
        if rsync_missing() {
            eprintln!("rsync not installed — skipping");
            return;
        }
        let base = scratch("dry");
        let src = seed_source(&base);
        let dst = base.join("nested/deep/dst");
        let ps = vec![profile(vec![task(
            "photos",
            &format!("{}/", src.display()),
            &format!("{}/", dst.display()),
        )])];

        let _ = run(&ps, "backups", true, false, false);
        let created = base.join("nested").exists();
        let _ = std::fs::remove_dir_all(&base);

        assert!(!created, "dry run must not create destination directories");
    }

    #[test]
    fn unknown_profile_returns_2() {
        let ps = vec![profile(vec![task("photos", "/src/", "/dst/")])];
        assert_eq!(run(&ps, "nope", false, false, false), 2);
        assert!(select(&ps, "nope")
            .unwrap_err()
            .to_string()
            .contains("backups"));
    }

    #[test]
    fn unknown_task_id_returns_2() {
        let ps = vec![profile(vec![task("photos", "/src/", "/dst/")])];
        assert_eq!(run(&ps, "backups/nope", false, false, false), 2);
        assert!(select(&ps, "backups/nope")
            .unwrap_err()
            .to_string()
            .contains("photos"));
    }

    #[test]
    fn empty_config_says_so_instead_of_dangling() {
        let err = select(&[], "backups").unwrap_err().to_string();
        assert!(err.contains("no profiles configured"), "got: {err}");
    }

    #[test]
    fn whole_profile_selects_every_task() {
        let ps = vec![profile(vec![
            task("photos", "/src/", "/dst/"),
            task("music", "/src2/", "/dst2/"),
        ])];
        let picked = select(&ps, "backups").unwrap();
        let ids: Vec<&str> = picked.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["photos", "music"]);
    }

    #[test]
    fn task_target_selects_only_that_task() {
        let ps = vec![profile(vec![
            task("photos", "/src/", "/dst/"),
            task("music", "/src2/", "/dst2/"),
        ])];
        let picked = select(&ps, "backups/music").unwrap();
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].id, "music");
    }

    #[test]
    fn delete_task_without_yes_returns_1() {
        let mut t = task("photos", "/src/", "/dst/");
        t.flags.delete = true;
        let ps = vec![profile(vec![t])];
        assert_eq!(run(&ps, "backups", false, false, false), 1);
    }

    #[test]
    fn delete_excluded_task_without_yes_returns_1() {
        let mut t = task("photos", "/src/", "/dst/");
        t.flags.delete_excluded = true;
        let ps = vec![profile(vec![t])];
        assert_eq!(run(&ps, "backups", false, false, false), 1);
    }

    #[test]
    fn dry_run_is_not_refused_even_with_delete() {
        let base = scratch("dry-delete");
        let mut t = task(
            "photos",
            &format!("{}/src/", base.display()),
            &format!("{}/dst/", base.display()),
        );
        t.flags.delete = true;
        let ps = vec![profile(vec![t])];

        let code = run(&ps, "backups", true, false, false);
        let _ = std::fs::remove_dir_all(&base);

        assert_ne!(code, 1);
    }

    #[test]
    fn yes_lets_a_destructive_task_past_the_gate() {
        let base = scratch("yes-gate");
        let mut t = task(
            "photos",
            &format!("{}/src/", base.display()),
            &format!("{}/dst/", base.display()),
        );
        t.flags.delete = true;
        let ps = vec![profile(vec![t])];

        let code = run(&ps, "backups", false, true, false);
        let _ = std::fs::remove_dir_all(&base);

        assert_ne!(code, 1);
    }

    #[test]
    fn one_destructive_task_blocks_the_whole_batch() {
        let mut bad = task("photos", "/src/", "/dst/");
        bad.flags.delete = true;
        let ps = vec![profile(vec![task("music", "/src2/", "/dst2/"), bad])];
        assert_eq!(run(&ps, "backups", false, false, false), 1);
    }

    #[test]
    fn headless_args_never_request_progress() {
        let mut t = task("photos", "/src/", "/dst/");
        assert!(t.flags.progress);
        let args = headless_args(&t, false, false);
        assert!(!args.contains(&"--info=progress2".to_string()));
        assert!(args.contains(&"-a".to_string()));
        assert!(!args.contains(&"-n".to_string()));

        t.flags.progress = false;
        assert!(!headless_args(&t, false, false).contains(&"--info=progress2".to_string()));
    }

    #[test]
    fn headless_args_still_pass_dry_run_flags() {
        let t = task("photos", "/src/", "/dst/");
        let args = headless_args(&t, true, false);
        assert!(args.contains(&"-n".to_string()));
        assert!(args.contains(&"--itemize-changes".to_string()));
        assert!(!args.contains(&"--stats".to_string()));
    }

    #[test]
    fn a_real_run_asks_rsync_to_be_quiet() {
        let t = task("photos", "/src/", "/dst/");
        assert!(headless_args(&t, false, false).contains(&"-q".to_string()));
    }

    #[test]
    fn a_dry_run_is_never_quiet_so_the_itemized_diff_survives() {
        let t = task("photos", "/src/", "/dst/");
        assert!(!headless_args(&t, true, false).contains(&"-q".to_string()));
        assert!(!headless_args(&t, true, true).contains(&"-q".to_string()));
    }

    #[test]
    fn verbose_keeps_rsyncs_full_output() {
        let t = task("photos", "/src/", "/dst/");
        let args = headless_args(&t, false, true);
        assert!(!args.contains(&"-q".to_string()));
        assert!(args.contains(&"-v".to_string()));
    }

    #[test]
    fn verbose_restores_output_for_a_task_that_turned_it_off() {
        let mut t = task("photos", "/src/", "/dst/");
        t.flags.verbose = false;
        let args = headless_args(&t, false, true);
        assert!(!args.contains(&"-q".to_string()));
        assert!(args.contains(&"-v".to_string()));
    }

    #[test]
    fn quiet_drops_the_contradictory_verbose_flag() {
        let mut t = task("photos", "/src/", "/dst/");
        t.flags.verbose = true;
        let args = headless_args(&t, false, false);
        assert!(args.contains(&"-q".to_string()));
        assert!(!args.contains(&"-v".to_string()));
    }

    #[test]
    fn quiet_lands_before_the_path_guard() {
        let t = task("photos", "-n", "/dst/");
        let args = headless_args(&t, false, false);
        let q = args.iter().position(|a| a == "-q").expect("missing -q");
        let guard = args.iter().position(|a| a == "--").expect("missing --");
        assert!(q < guard, "-q must not be read as a path: {args:?}");
    }

    #[test]
    fn selecting_a_safe_task_from_a_profile_with_a_destructive_one_is_not_refused() {
        let base = scratch("safe-pick");
        let mut bad = task("photos", "/src/", "/dst/");
        bad.flags.delete = true;
        let safe = task(
            "music",
            &format!("{}/src2/", base.display()),
            &format!("{}/dst2/", base.display()),
        );
        let ps = vec![profile(vec![safe, bad])];

        let code = run(&ps, "backups/music", false, false, false);
        let _ = std::fs::remove_dir_all(&base);

        assert_ne!(code, 1);
    }

    #[test]
    fn empty_profile_says_nothing_to_do_and_returns_0() {
        let ps = vec![profile(vec![])];
        assert_eq!(run(&ps, "backups", false, false, false), 0);
    }

    #[test]
    fn vanished_source_files_still_count_as_success() {
        assert!(succeeded(0));
        assert!(succeeded(24));
        assert!(!succeeded(23));
        assert!(!succeeded(COULD_NOT_START));
    }

    #[test]
    fn dry_run_transfers_nothing_into_an_existing_destination() {
        if rsync_missing() {
            eprintln!("rsync not installed — skipping");
            return;
        }
        let base = scratch("dry-existing");
        let src = seed_source(&base);
        let dst = base.join("dst");
        std::fs::create_dir_all(&dst).unwrap();
        let ps = vec![profile(vec![task(
            "photos",
            &format!("{}/", src.display()),
            &format!("{}/", dst.display()),
        )])];

        let code = run(&ps, "backups", true, false, false);
        let landed = dst.join("a.txt").exists();
        let _ = std::fs::remove_dir_all(&base);

        assert_eq!(code, 0, "a clean dry run should succeed");
        assert!(!landed, "dry run must not transfer files");
    }

    #[test]
    fn the_first_failing_code_is_returned_not_the_last() {
        if rsync_missing() {
            eprintln!("rsync not installed — skipping");
            return;
        }
        let base = scratch("first-code");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("afile"), "not a directory").unwrap();
        let ps = vec![profile(vec![
            task(
                "unstartable",
                &format!("{}/src/", base.display()),
                &format!("{}/afile/sub/", base.display()),
            ),
            task(
                "missing",
                &format!("{}/nope/", base.display()),
                &format!("{}/dst/", base.display()),
            ),
        ])];

        let code = run(&ps, "backups", false, false, false);
        let _ = std::fs::remove_dir_all(&base);

        assert_eq!(code, 3, "a task that could not start must exit 3, never 1");
    }
}
