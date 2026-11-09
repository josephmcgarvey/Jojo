#!/usr/bin/env bash
# macOS and Linux counterpart of stage-daemon.ps1. Builds jojod and copies it
# into app/src-tauri/resources/daemon, which Tauri bundles as the `daemon`
# resource folder. Run before `tauri dev` or `tauri build` (tauri.macos.conf.json
# runs it as the beforeBuildCommand).
#
# The target triple comes from TAURI_ENV_TARGET_TRIPLE, which `tauri build
# --target <triple>` sets for the beforeBuildCommand; without it, jojod is
# built for the host.
#
# It also builds the Linux daemons the app installs on remote machines (spec
# 8.2 step 4) with cargo-zigbuild, always in release, and stages them as
# jojod-linux-x86_64 and jojod-linux-aarch64. A release staging fails without
# cargo-zigbuild; a debug staging warns and leaves remote machines unavailable.
set -euo pipefail
profile="${1:-debug}"
case "$profile" in debug | release) ;; *) echo "usage: $0 [debug|release]" >&2; exit 2 ;; esac
target="${TAURI_ENV_TARGET_TRIPLE:-}"
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

args=(-p jojod)
[ "$profile" = release ] && args+=(--release)
[ -n "$target" ] && args+=(--target "$target")
cargo build "${args[@]}"
built="target/${target:+$target/}$profile/jojod"

dest=app/src-tauri/resources/daemon
mkdir -p "$dest"
# The bundle takes every file in $dest (the `resources/daemon/*` glob, which
# also matches the tracked .gitkeep), so remove what a previous run left.
find "$dest" -maxdepth 1 -type f ! -name .gitkeep -delete
cp "$built" "$dest/jojod"

if command -v cargo-zigbuild > /dev/null; then
  for pair in x86_64-unknown-linux-musl:jojod-linux-x86_64 aarch64-unknown-linux-musl:jojod-linux-aarch64; do
    triple="${pair%%:*}"
    cargo zigbuild -p jojod --release --target "$triple"
    cp "target/$triple/release/jojod" "$dest/${pair##*:}"
  done
elif [ "$profile" = release ]; then
  echo "cargo-zigbuild is required to build the Linux daemons (cargo install cargo-zigbuild --locked)." >&2
  exit 1
else
  echo "warning: cargo-zigbuild not found: the Linux daemons are not staged, so remote machines cannot be added." >&2
fi
ls -l "$dest"
