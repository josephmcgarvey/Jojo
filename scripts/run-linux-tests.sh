#!/usr/bin/env bash
# Runs the jojod, jojo-grid, jojo-proto and jojo-agent test executables that
# `cargo zigbuild --tests --target x86_64-unknown-linux-musl` built on
# Windows, inside WSL (or any x86_64 Linux with this checkout mounted).
# Usage from Windows: wsl.exe -d Ubuntu -- bash scripts/run-linux-tests.sh
# Prints each executable's failures and its result line; exits non-zero when
# any executable fails.
set -u
cd "$(dirname "$0")/../target/x86_64-unknown-linux-musl/debug/deps" || exit 1
status=0
for test in *; do
  # Test executables have no extension; skip .d, .rlib, .rmeta and .o files
  # and anything that is not executable.
  case "$test" in *.*) continue ;; esac
  [ -x "$test" ] || continue
  # bindings writes TypeScript into the Windows checkout, and recordings
  # reads fixtures from a CARGO_MANIFEST_DIR compiled in as a Windows path;
  # both run on Windows. The jojod executable itself prints its help.
  case "$test" in bindings-*|recordings-*) continue ;; esac
  "./$test" --list >/dev/null 2>&1 || continue
  echo "== $test"
  output="$(timeout 300 "./$test" "$@" 2>&1)"
  code=$?
  printf '%s\n' "$output" | grep -E '^test .*FAILED|panicked at|test result' | head -20
  if [ "$code" -ne 0 ]; then
    echo "exit code $code"
    status=1
  fi
done
exit "$status"
