# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-10-22

### Added

- A quick transfer: `t` runs a Source → Destination once without saving a
  profile or a task first. It appears in the Runs panel marked `»` and stays
  there until `a` keeps it as a real task or `d` discards it — so the one-off
  that turns out to be worth repeating does not have to be retyped. Quitting
  with one still marked `»` throws it away.
- SSH port, key file and extra arguments on a quick transfer, shown only when
  a path is remote. They are the same options a saved task offers under `s`,
  and they carry over when the transfer is kept.
- The reason a remote transfer could not start, instead of a bare failure.
  lazyrsync probes the connection with `ssh -o BatchMode=yes -o
  ConnectTimeout=5` and reports what ssh said — wrong port, refused key,
  unknown host.
- A record of every finished run, written from the TUI and from
  `lazyrsync run` alike to `$XDG_STATE_HOME/lazyrsync/history.toml`. The new
  `lazyrsync log` subcommand prints them, and the Command log replays the
  recent ones at startup. The 200 most recent are kept.
- `history`, `log`, `log_size` and `show_log` in `settings.toml`, plus `@` to
  hide or show the Command log for the session. The `log` level is `quiet`,
  `normal` or `verbose`; no level hides an error or a warning.

## [0.3.0] - 2026-08-10

### Added

- `rsync_path` in `settings.toml` picks which rsync binary runs instead of the
  first one on `$PATH`. Useful on macOS 15.4 and later, where `/usr/bin/rsync`
  is openrsync and shadows Homebrew's rsync, and in any launch context with a
  minimal `$PATH` such as cron. The configured binary is what the resolved
  command in the TUI and `lazyrsync list` reports.
- A startup warning when the rsync found on `$PATH` is older than 3.1. The
  preview needs `--itemize-changes`, the progress bar needs `--info=progress2`
  and snapshots need `--link-dest`; openrsync has none of the first two and
  accepts `--link-dest` without reliably hardlinking.
- Statically linked musl release binaries for `x86_64-unknown-linux-musl` and
  `aarch64-unknown-linux-musl`, so distributions carrying a glibc older than
  2.34 can run a prebuilt binary. `cargo binstall` resolves them on musl hosts.

### Changed

- Saving `settings.toml` preserves comments and every hand-edited key; only
  `hints` and `last_profile` are rewritten. Editing the file while the TUI is
  open no longer loses those edits on exit.
- `settings.toml` rejects unknown keys, the rule `profiles.toml` already
  followed. One consequence for both: a file written by a newer lazyrsync may
  fail to load on an older binary rather than being partially ignored.

### Fixed

- A `settings.toml` that exists but cannot be read or parsed is now a loud
  error instead of being silently discarded and replaced with defaults.

## [0.2.0] - 2026-07-28

### Added

- Headless runs: `lazyrsync run PROFILE` runs every task in a profile without
  the TUI, and `PROFILE/TASK` runs a single task by the id `list` prints. Suited
  to cron and systemd timers — Snapshot tasks resolve their `--link-dest` chain
  fresh on every run, which no static crontab line can express.
- `run -n` for a real dry run, `--yes` to allow tasks that delete at the
  destination, and `-v` to include rsync's own output per task.
- Documented exit codes: `0` success, `1` refused for want of `--yes`, `2`
  unknown target or unloadable config, `3` a task that could not start or was
  killed, otherwise the first failing task's rsync exit code.
- Dynamic paths: `{now}`, `{now:FORMAT}`, `{utcnow}`, `{hostname}`, `{user}`,
  `$VAR`/`${VAR}` and `~` expand every time a task runs, so one saved task can
  write to a new dated folder each night. Unknown placeholders and unset
  variables are left as typed; `{{` and `$$` escape.
- A separate opt-out per confirmation prompt in `settings.toml`
  (`skip_delete_warning`, `skip_run_confirm`, `skip_remove_confirm`), all
  `false` by default.

### Changed

- A headless real run passes rsync `-q` and reports one line per task; a dry run
  is never quiet, and neither carries the TUI-only `--info=progress2` or
  `--stats` flags.
- `profiles.toml` rejects unknown keys instead of ignoring them, so a typo in a
  field name fails loudly at load time rather than silently dropping a setting.

### Fixed

- A task that enables `--delete-excluded` on its own now warns, matching the
  existing `--delete` warning.
- Placeholders resolve before a path is classified as remote, so a placeholder
  that expands to contain a colon no longer looks like an SSH target.

## [0.1.1] - 2026-07-13

### Changed

- Feature GIFs are committed in the repo and referenced relatively, so they
  render on both GitHub and the crates.io page.

## [0.1.0] - 2026-07-10

### Added

- Initial release: an rsync terminal UI with reusable profiles and tasks,
  dry-run diff preview, live run progress with cancellation, a filters/flags
  editor, SSH remotes (`user@host:/path`), and local snapshots.

[Unreleased]: https://github.com/westpoint-io/lazyrsync/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/westpoint-io/lazyrsync/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/westpoint-io/lazyrsync/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/westpoint-io/lazyrsync/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/westpoint-io/lazyrsync/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/westpoint-io/lazyrsync/releases/tag/v0.1.0
