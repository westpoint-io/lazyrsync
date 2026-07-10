# Contributing to lazyrsync

Thanks for your interest in lazyrsync! Whether you're reporting a bug, polishing
the docs, or sending a patch, you're welcome here.

## Ways to contribute

- **Report a bug.** Open an [issue](https://github.com/westpoint-io/lazyrsync/issues)
  with what you did, what you expected, and what happened — plus your OS and
  `rsync --version`.
- **Request a feature.** Open an issue describing the problem you're trying to
  solve; that's more useful than a pre-baked solution.
- **Improve the docs.** Fixes to the README or the docs site are always welcome.
- **Send a patch.** See below.

## Getting started

Browse the [open issues](https://github.com/westpoint-io/lazyrsync/issues) —
anything tagged `good first issue` is a gentle way in. Small, focused PRs get
reviewed fastest. For anything larger than a bug fix, open an issue first so we
can agree on the approach before you write the code.

## Development setup

You'll need a stable Rust toolchain and `rsync` on your `$PATH` (the app spawns
the system binary, and the test suite includes a live-rsync integration test).

```bash
cargo build            # build
cargo test             # unit tests + a live-rsync integration test
cargo run              # launch the TUI
cargo clippy           # lint
cargo fmt              # format
```

## Submitting a pull request

Before you open a PR, make sure it:

- builds and passes `cargo test`,
- is `clippy`- and `fmt`-clean,
- uses [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`,
  `fix:`, `docs:`, `refactor:`, `test:`, `chore:`), and
- is squashed — keep `main` free of WIP and fixup commits.

## Code style

A couple of house rules worth knowing up front:

- **No comments.** Write self-explanatory code with clear names.

The architecture, module map, and color conventions live in
[AGENTS.md](AGENTS.md) — read it before a substantial change so your patch fits
the existing grain.

## Getting help

Stuck? Open an [issue](https://github.com/westpoint-io/lazyrsync/issues) — a
question is a perfectly good reason to open one.

## Releasing

Maintainers cut releases by tagging `vX.Y.Z`, which triggers the release workflow
to build and publish binaries. See [RELEASING.md](RELEASING.md).
