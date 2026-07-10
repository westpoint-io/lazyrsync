# Releasing

Releases are cut from `main` by pushing a `vX.Y.Z` tag. The
[`release.yml`](.github/workflows/release.yml) workflow then builds the binaries
and attaches them (with SHA-256 checksums) to the GitHub Release for that tag.

## Steps

1. Bump `version` in `Cargo.toml` following [SemVer](https://semver.org/).
2. In `CHANGELOG.md`, move the `[Unreleased]` notes into a new `## [X.Y.Z]`
   section dated today, and update the compare links at the bottom.
3. Commit: `git commit -am "chore: release vX.Y.Z"`.
4. Tag and push:

   ```bash
   git tag vX.Y.Z
   git push origin main --tags
   ```

5. The workflow builds the targets below and publishes the Release. Verify the
   assets appear, then edit the Release notes if needed.

## Targets

| Platform      | Target triple                |
| ------------- | ---------------------------- |
| Linux x86_64  | `x86_64-unknown-linux-gnu`   |
| Linux aarch64 | `aarch64-unknown-linux-gnu`  |
| macOS x86_64  | `x86_64-apple-darwin`        |
| macOS aarch64 | `aarch64-apple-darwin`       |

Assets are named `lazyrsync-<target>.tar.gz` with the binary at the archive
root, matching `[package.metadata.binstall]` in `Cargo.toml` so
`cargo binstall lazyrsync` resolves them once the crate is published to
crates.io.
