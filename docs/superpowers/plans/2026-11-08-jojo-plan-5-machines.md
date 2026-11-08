# Jojo Plan 5: Machines Implementation Plan

> Executed inline in the controlling session (user preference). Each task ends with its tests passing and a commit. The Progress section at the end is the recovery record.

**Goal:** Linux servers reachable over SSH appear as machines in the sidebar. Jojo installs `jojod` on the server, keeps one SSH connection per machine, and shows the server's projects and sessions next to This PC's.

**Architecture:** The Linux daemon is the same `jojod`, cross-compiled to static musl binaries with `cargo-zigbuild`. On the server it runs as `jojod serve --daemonize` on `~/.jojo/jojod.sock`. The app reaches it through `ssh <host> ~/.local/bin/jojod attach --stdio`, which relays bytes between SSH's stdio and the socket, so the app speaks the same framed protocol over the SSH pipes that it speaks over the local named pipe. The app runs one connection loop per enabled machine (`connection::run_with` with a per-machine connect step), tags every frame and status in the bridge envelope with the machine id, and keeps one frontend model per machine.

**Test server:** `onprem-server` from `~/.ssh/config` (Linux x86_64, user `jmcgarvey`, no Claude Code, `Linger=no`), chosen by the user. Daemon tests run in WSL Ubuntu from cross-compiled test binaries.

## Scope

Spec 8.2-8.4 and the Machine row of spec 4 (`%APPDATA%\Jojo\machines.toml`: `id`, `label`, `kind` `local` | `ssh`, `ssh_host`, `enabled`), the sidebar's machine headers and **+ Machine** button (spec 9.2), the pane overlay while disconnected, and discarding keystrokes while disconnected.

Out of scope: the settings screen, the switcher and the "+ opens a terminal" flow (the next functionality plan), the design pass.

## Global Constraints

- Linux targets: `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, built with `cargo zigbuild` (Zig at `C:\Program Files\zig`). Bundled as `jojod-linux-x86_64` and `jojod-linux-aarch64` in the app's `daemon` resources.
- Remote paths: binary `~/.local/bin/jojod`, state `~/.jojo` (mode 0700), socket `~/.jojo/jojod.sock` (0600), lock `~/.jojo/jojod.lock`.
- SSH: `ssh -o BatchMode=yes -o ConnectTimeout=10` for checks; the live connection adds `-o ServerAliveInterval=15 -o ServerAliveCountMax=3` and runs `~/.local/bin/jojod attach --stdio`. Reconnect backoff 1 s doubling to 30 s. Exit code 255 with "Permission denied" stops retrying: "Auth failed. Load your key with ssh-add, then Reconnect."
- Keystrokes typed while a machine is disconnected are discarded.
- `machines.toml` uses the `toml` crate (already in the build through Tauri). This PC is always present, first, and cannot be removed.
- Tests never touch the user's real `~/.claude/settings.json`, `machines.toml`, `layout.json` or any remote server; the server is used only in the final manual check.
- US spelling; factual comments, no aphorisms; cargo `--jobs 4`; never start jojod or the app from the PowerShell tool.

## Tasks

1. **Linux daemon in WSL.** Build the test binaries for `x86_64-unknown-linux-musl` and run them in WSL; fix every `#[cfg(unix)]` failure.
2. **`serve --daemonize` and `attach --stdio`.** Daemonize with `setsid` and fork, hold `~/.jojo/jojod.lock` with `flock` (a second `serve` exits 0), create `~/.jojo` 0700 and the socket 0600. `attach --stdio` connects to the socket and copies stdin to it and it to stdout until either side closes. Tests in WSL.
3. **Linux release binaries.** `scripts/build-linux-daemon.ps1` builds both targets in release; `stage-daemon.ps1` copies them as `jojod-linux-x86_64` and `jojod-linux-aarch64`.
4. **Machines file and bridge envelope.** `machines.rs` (load/save `machines.toml`, This PC always first); the bridge envelope gains the machine id; `send_frame` takes it; one connection loop per enabled machine; SSH connect step (spawn `ssh.exe`, handshake over its stdio, map exit 255 + "Permission denied" to an auth failure).
5. **Frontend per-machine models.** One model per machine; layout keys use the real machine id; sends and input go to the session's machine; input to a disconnected machine is dropped; machine headers with status in the sidebar; pane overlay "Disconnected. Sessions keep running on <label>."
6. **Add machine.** Backend commands for `~/.ssh/config` host aliases and for the install steps of spec 8.2 (connectivity, platform, install or upgrade by streaming the binary over stdin, start, linger check, hooks), reported step by step; the Add machine dialog; **+ Machine** at the bottom of the sidebar.
7. **Remove machine and remote upgrade.** Machine header menu: Reconnect, Remove (with "Also uninstall from server"); a remote protocol mismatch offers Upgrade, naming the sessions that end.
8. **Checklist, verification, installer, manual test against `onprem-server`.**

## Progress

- Task 0: branch `plan-5-machines` from master `7eb04dc` (test-hook fix).
- Task 1: complete (088b698, 6d985b7). The Linux code needed no changes; two test races fixed. Linux tests run with `MSYS_NO_PATHCONV=1 wsl.exe -d Ubuntu -- bash /mnt/c/.../scripts/run-linux-tests.sh` after `cargo zigbuild --tests --bins --target x86_64-unknown-linux-musl`.
- Task 2: complete (see git log: serve --daemonize, state lock, attach --stdio; tests/remote_entry.rs passes on Windows and in WSL).
