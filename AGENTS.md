# AGENTS.md

Guidance for AI coding agents working in this repository.

## Project

`lazyrsync` is a terminal UI (TUI) for `rsync`, built with Rust and
[ratatui](https://ratatui.rs). It lets you manage reusable rsync profiles,
preview a transfer as a structured diff before running it, and watch a live
run with progress and cancellation — all from the terminal, including over
SSH where a desktop GUI cannot reach.

## Commands

```bash
cargo build            # build
cargo test             # run the test suite (unit + a live-rsync integration test)
cargo run              # launch the TUI
cargo run -- list      # list profiles and their resolved rsync commands
cargo run -- run NAME  # resolve/run a profile by name (headless)
cargo clippy           # lint
cargo fmt              # format
```

`rsync` must be on `$PATH` (the app shells out to the system binary).

## Architecture

- **Shell out to the system `rsync`** — never reimplement the protocol.
  Arguments are built as a `Vec<String>` (never a shell string).
- **Concurrency: worker thread + `std::sync::mpsc`, no tokio.** The run
  engine spawns rsync on a thread, reads stdout splitting on both `\r` and
  `\n` (progress2 updates in place with `\r`), and sends messages to the UI
  over a channel drained each frame. Cancellation kills the child.
- **State: a `Component` trait per screen + a shared `Ctx`.** Each screen is
  its own struct (`Browse`, `EditorScreen`, `Run`, and the overlays) that owns
  its view-local state and implements `Component` (`draw`/`on_key`/`on_mouse`/
  `busy`/`tick`). Shared model + navigation cursor (`store`, `settings`, `log`,
  `profile`, `task`, `subtab`, `area`, `tick`) live in `Ctx`, passed in to every
  method. Screens never mutate each other or navigate directly — they return a
  `Cmd` (`Goto`, `Overlay`, `Close`, `RequestRun`, `StartRun`, `Quit`) that the
  `App` router applies. Base screens live in a `Screen` enum; popups in an
  `Overlay` enum drawn on top. The main loop blocks on input when idle; while a
  screen is `busy()` it polls input, drains that screen's channel via `tick`,
  and redraws.
- **rsync output parsing** is pure and unit-tested: `--itemize-changes` for
  the dry-run diff, `--info=progress2` for live progress.

## Module map (`src/`)

| File | Responsibility |
|------|----------------|
| `main.rs` | CLI entry (clap); launches the TUI or runs headless subcommands |
| `app.rs` | `App` router + `Ctx`/`Cmd`/`Component` trait + `Screen`/`Overlay` enums + event loop |
| `screens/browse/` | `Browse` component split into `mod.rs` (state + trait), `render.rs` (draw), `input.rs` (keys/mouse/actions) |
| `screens/run.rs` | `Run` component: run dashboard + run engine pump (`tick`) |
| `popups/` | One file per overlay: `prompt`, `menu`, `presets`, `add_task`, `edit` (per-section task editor), `confirm_delete`, `confirm_run`, `help` |
| `ui/` | Shared render helpers split by concern: `mod` (colors/blocks), `log`, `preview`, `age`, `fields` |

Screens/popups/ui are top-level module folders (gitui-style), not nested under `app`. `app.rs`
is a thin flat router. A module is one file until it grows parts, then a folder — `browse` is the
only screen big enough to be a folder.
| `paths.rs` | Path completion / tilde expansion (shared by editor + popups) |
| `profile.rs` | Profile/task data model (serialized as TOML) |
| `store.rs` | Load/save profiles + `Settings` (`$XDG_CONFIG_HOME/lazyrsync/`) |
| `rsync.rs` | The rsync argument builder — the testable core |
| `editor.rs` | Task field engine (per-section fields, get/set, edit buffer, autocomplete) used by the `edit` popup + inline flag toggle |
| `preview.rs` | Dry-run engine (async worker) + itemize/stats parser |
| `run.rs` | Live run engine (worker thread) + progress2 parser |

## Conventions

- **No comments.** Write self-explanatory code with clear names. Do not add
  explanatory or doc comments.
- Keep parsing and argument-building logic in pure functions and unit-test
  them; UI code stays thin.
- Match the existing style and module layout.
- `--delete` is opt-in and gated behind a confirmation; never make
  destructive behavior the default.
- `docs/` is gitignored and must never be committed.

## Colors

- **ANSI named colors only** (`Color::Green`, `Color::Cyan`, …), never
  `Color::Rgb`. Named colors resolve through the user's terminal theme, so the
  app inherits their palette; truecolor would override it and is banned.
- **Color the values, not the labels.** A field is `Label:` in normal-weight
  default fg, followed by a value in a meaningful color (path, state, …). Most
  chrome stays `Color::Reset`; color and bold mark structure, not decoration.
