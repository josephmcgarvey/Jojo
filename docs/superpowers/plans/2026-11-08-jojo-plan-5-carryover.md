# Jojo Plan 5: Carry-Over Into Later Plans

This document records the decisions made while executing Plan 5 (machines over SSH, branch `plan-5-machines`, commits `088b698..2b06dcc` on top of `7eb04dc`) and the open items from its final review.

## 1. Decisions Made During Plan 5

1. **The Linux daemon is the same `jojod`**, cross-compiled on Windows to static musl binaries (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`) with `cargo-zigbuild` and Zig. Its code needed no changes for Linux; two test races surfaced and were fixed. The Linux tests run in WSL from cross-built executables (`scripts/run-linux-tests.sh`).
2. **`serve --daemonize`** forks, calls `setsid`, forks again, redirects stdio to `/dev/null` and changes to `/`, before the tokio runtime exists. The parent returns once the socket accepts a connection. `~/.jojo/jojod.lock` (an exclusive `flock`) makes a second `serve` exit 0.
3. **`attach --stdio`** relays bytes between stdio and the socket and exits (with `std::process::exit`) as soon as either direction ends, because tokio's blocking stdin read cannot be cancelled.
4. **The remote daemon starts through the user's login shell** (`exec "${SHELL:-/bin/sh}" -lc '~/.local/bin/jojod serve --daemonize'`), so sessions get the PATH a login gives. On the test server this put `~/.local/bin` (where Claude Code is installed) on the daemon's PATH.
5. **Bridge envelope:** `[kind][machine id length][machine id][body]`; one shared `Frontend` sink and one `Bridge` per machine; `send_frame` takes the machine from the `jojo-machine` invoke header.
6. **Frontend:** one model per machine merged into `app.model` (project and session ids are ULIDs); `reconcileMachine` touches only that machine's layout keys, so an unconnected machine keeps its tabs.
7. **Install:** a jojod of the same protocol is replaced without stopping the running daemon (the new binary runs from its next start); another protocol stops the daemon only when allowed (Add machine asks with "Install and end sessions"; Upgrade confirms first). The upload limit is 30 s plus 32 KB/s.
8. **Remote shells** get no program from the app, so the daemon starts `$SHELL`; the Windows shell list is offered for This PC only.
9. **`machines.toml`** uses the `toml` crate already in the build through Tauri.

## 2. Open Items From the Final Review

1. **Logout survival (spec-level):** with `KillUserProcesses=yes`, logind kills processes in an SSH session's scope even when lingering is on; surviving logout needs the daemon started under `systemd-run --user`. The install step also warns on every `Linger=no` system without checking `KillUserProcesses` (spec 8.2 step 6), and does not offer to run `loginctl enable-linger`.
2. **"Open SSH in a pane" and retry** (spec 8.2 step 2) are not implemented; the dialog explains the failure and asks the user to run `ssh <host>` once.
3. **Remote mismatch prompt** names the sessions last seen in this app run only; after an app update the count is usually unknown. Upgrade progress is not shown (it runs the install without a progress view).
4. **This PC "Later"** (spec 8.4) is not implemented; the banner offers only "Restart jojod (ends all running sessions)".
5. **`unix.rs`:** a relative `--state-dir` or `--endpoint` breaks `--daemonize` (the child changes to `/` before binding; the parent waits on a path relative to its own directory). The app always passes absolute defaults. Errors in the child before logging starts are not reported; taking the `flock` in the parent before `fork` would report "already running" and lock errors synchronously.
6. **The 200 ms wait for ssh's stderr** after a failed handshake (`ssh.rs`) could await the collector task instead.
7. **`add_machine` re-add path:** a disabled existing entry is not started, and two concurrent adds of the same host can both append an entry.
8. **The banner** shows This PC's status only; a remote machine's auth failure appears in its header and pane overlay, and its Reconnect is in the header's right-click menu.
9. **Leaked test daemons:** a test that panics before shutting its daemon down leaves the daemon running, and the daemon holds the test's output pipe, so a piped test command never returns. A drop guard that shuts the daemon down would fix both.
