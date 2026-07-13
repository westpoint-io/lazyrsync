#!/usr/bin/env bash
#
# Bump the AUR + Homebrew packages to an already-released version.
#
# Run this AFTER `git push origin vX.Y.Z` has built the GitHub release binaries.
# It downloads the release artifacts, recomputes checksums, and pushes updated
# packaging to the AUR and the Homebrew tap. Publishing to crates.io stays a
# separate `cargo publish` (it uses source, not these binaries).
#
#   scripts/release.sh 0.1.2
#
# Override the local clone locations with AUR_DIR / TAP_DIR if needed.

set -euo pipefail

VERSION="${1:?usage: release.sh <version>   e.g. release.sh 0.1.2}"
REPO="westpoint-io/lazyrsync"
TAG="v$VERSION"
AUR_DIR="${AUR_DIR:-$HOME/aur-lazyrsync}"
TAP_DIR="${TAP_DIR:-$HOME/homebrew-lazyrsync}"

REL="https://github.com/$REPO/releases/download/$TAG"
SRC="https://github.com/$REPO/archive/refs/tags/$TAG.tar.gz"

sha() { curl -fsSL "$1" | sha256sum | cut -d' ' -f1; }

echo "==> verifying release $TAG exists"
gh release view "$TAG" --repo "$REPO" >/dev/null

# clone the packaging repos on first run
[ -d "$AUR_DIR/.git" ] || git clone "ssh://aur@aur.archlinux.org/lazyrsync.git" "$AUR_DIR"
[ -d "$TAP_DIR/.git" ] || git clone "https://github.com/westpoint-io/homebrew-lazyrsync" "$TAP_DIR"

echo "==> AUR: bump to $VERSION (source build)"
src_sha="$(sha "$SRC")"
cd "$AUR_DIR"
git pull --quiet --ff-only origin master 2>/dev/null || true
sed -i \
  -e "s/^pkgver=.*/pkgver=$VERSION/" \
  -e "s/^pkgrel=.*/pkgrel=1/" \
  -e "s/^sha256sums=('.*')/sha256sums=('$src_sha')/" \
  PKGBUILD
makepkg --printsrcinfo > .SRCINFO
git add PKGBUILD .SRCINFO
git -c commit.gpgsign=false commit -m "lazyrsync $VERSION"
git push origin HEAD:master

echo "==> Homebrew: bump to $VERSION (prebuilt binaries)"
arm_mac="$(sha "$REL/lazyrsync-aarch64-apple-darwin.tar.gz")"
intel_mac="$(sha "$REL/lazyrsync-x86_64-apple-darwin.tar.gz")"
arm_linux="$(sha "$REL/lazyrsync-aarch64-unknown-linux-gnu.tar.gz")"
intel_linux="$(sha "$REL/lazyrsync-x86_64-unknown-linux-gnu.tar.gz")"

mkdir -p "$TAP_DIR/Formula"
cat > "$TAP_DIR/Formula/lazyrsync.rb" <<EOF
class Lazyrsync < Formula
  desc "Terminal UI for rsync — profiles, dry-run diff preview, live progress"
  homepage "https://github.com/$REPO"
  version "$VERSION"
  license "MIT"

  on_macos do
    on_arm do
      url "$REL/lazyrsync-aarch64-apple-darwin.tar.gz"
      sha256 "$arm_mac"
    end
    on_intel do
      url "$REL/lazyrsync-x86_64-apple-darwin.tar.gz"
      sha256 "$intel_mac"
    end
  end

  on_linux do
    on_arm do
      url "$REL/lazyrsync-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "$arm_linux"
    end
    on_intel do
      url "$REL/lazyrsync-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "$intel_linux"
    end
  end

  depends_on "rsync"

  def install
    bin.install "lazyrsync"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/lazyrsync --version")
  end
end
EOF
cd "$TAP_DIR"
git pull --quiet --ff-only 2>/dev/null || true
git add Formula/lazyrsync.rb
git -c commit.gpgsign=false commit -m "lazyrsync $VERSION"
git push

echo
echo "==> done. AUR + Homebrew are on $VERSION."
echo "    still to do:  cargo publish   (from the lazyrsync repo, for crates.io)"
