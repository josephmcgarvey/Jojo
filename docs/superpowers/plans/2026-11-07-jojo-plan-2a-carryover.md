# Jojo Plan 2a: Carry-Over Into Plan 2b

This document records the decisions made while executing Plan 2a (protocol v2 and daemon preparation, branch `plan-2a-protocol-and-daemon-prep`, commits `5b3a0a3..d21cccb`) and the requirements Plan 2b (the Tauri app shell) must act on. It replaces the per-task progress ledger, which was scratch state. Plan 1's carry-over (`2026-11-06-jojo-plan-1-carryover.md`) remains the source for items it lists that are not repeated here.

## 1. Decisions Made During Plan 2a

1. **Only live output counts toward the per-client queue limit** (user decision). `MAX_CLIENT_QUEUE_BYTES` = 8 MiB of queued `Output`; `Snapshot` and control frames are never counted, so a large snapshot cannot cause a re-attach loop. The limit check is an exact compare-and-swap.
2. **`CreateSession` carries a client-chosen `session_id`** (user decision). The daemon uses it; a duplicate is rejected. Every `Error` for a `CreateSession` echoes that id, so a pane can match `SessionCreated`, `SessionExited` and `Error` to its own request. `PROTOCOL_VERSION` stays 2.
3. **Agent-state messages are deferred to Plan 4** (user decision). `StateChanged` and `HookEvent` are not in protocol v2; Plan 4 adds them with a `PROTOCOL_VERSION` bump (restarting the daemon ends open sessions; acceptable because nothing is released).
4. **Every broadcast happens while the daemon holds the store lock**, in every handler, so the order of events always matches the order of state changes.
5. **The Linux double-kill risk is closed** (user decision): a session records when its process has exited, and `kill()` does nothing afterwards.
6. **The daemon refuses to run elevated**, and the client verifies the pipe's server process belongs to the current user.
7. **Known snapshot gap 4 (documented, pinned by an ignored test):** on the alternate screen, a column-growing resize without reflow leaves a pending-wrap cursor at a column that is no longer the last; it cannot be restored exactly.
8. **The sideload test runs in an isolated copy of the test binary**, so `conpty.dll` never lands in the shared test folder.
9. **`NO_COLOR` is removed only in the recorder, not the daemon.** Claude Code's PowerShell tool injects `NO_COLOR=1`; PowerShell honors it with plain-text output. Real Jojo sessions launched normally are in color. Stripping it in the daemon would override a user preference.
10. **Logs** are written to `<state_dir>/logs/jojod.<YYYY-MM-DD>.log`, rotated daily, at most 7 kept (`%LOCALAPPDATA%\Jojo\logs` on Windows, `~/.jojo/logs` on Linux, where the state directory is mode 0700). Spec section 10's `jojod.log` name is superseded.

## 2. Requirements for Plan 2b

### 2.1 How the app consumes the protocol

1. **Apply events as idempotent upserts and ignore events for unknown ids.** Events are ordered among themselves; events and direct replies are not ordered relative to each other. `SessionCreated` can arrive after a `Sessions` reply that already contains that session.
2. **Output after Detach:** drop `Output` for a session until the `Snapshot` that follows its latest `Attach` arrives. Keep at most one outstanding `Attach` per session. A snapshot starts with `ESC c ESC[3J`, so it can be written into the same xterm.js instance.
3. **Handle the special `Error.request` values** (documented on the field in the generated TypeScript): `"Output"` (client fell behind; attach again), `"Events"` (missed events; the connection is closing; reconnect and list again), `"Hello"` (protocol mismatch; the connection is closing), `"Frame"` (a malformed frame was sent).
4. **xterm.js settings must match the cross-check:** load the Unicode 11 addon and activate it, and set `scrollback` to at least 10,000.
5. **xterm.js must not answer terminal queries** (Plan 1 carry-over 2.2 item 1): register CSI handlers that consume DSR, DA1, DA2 and DECRQM.
6. **Never send a size outside 2–1000 columns × 1–1000 rows** (hidden panes report no size).
7. **Optional fields in the generated types are required keys that accept `null`** (for example `shell: null` on `CreateSession`).
8. **`RemoveProject` is refused while a session in the project is running**: Kill each session and wait for `SessionExited` first.

### 2.2 Connecting and running the daemon

1. Connect from the Tauri backend with `jojod::transport::connect` (owner check, 5-second deadline). Add a timeout on the `Hello` exchange (none exists in `Client::connect` or the server's Hello read).
2. If the app runs elevated, `jojod serve` refuses to start; show that message, or start jojod without elevation.
3. Ship `conpty.dll` and `OpenConsole.exe` next to `jojod.exe`, verified against `resources/conpty/SHA256SUMS` at package time (`scripts/fetch-conpty.ps1` now verifies before copying). Call `SetDefaultDllDirectories` with application directory and System32 at daemon startup so a missing `conpty.dll` is not searched for on `PATH`.
4. **Development rule:** never start jojod from Claude Code's PowerShell tool (it inherits `NO_COLOR=1`, so PowerShell sessions are colorless). Check whether an inherited `CLAUDECODE` variable stops `claude` from starting inside a session; if it does, remove it when spawning sessions.
5. The error banner shows the dated log path pattern.

### 2.3 Daemon work deferred into Plan 2b

1. Move blocking work (`Session::spawn`, snapshotting, `store.save()`, `read_dir`) off tokio worker threads (`spawn_blocking` or an asynchronous `Daemon::handle`), keeping the guarantee that `SessionCreated` precedes `SessionExited`.
2. Cap the number of exited sessions whose grids stay in memory (each can hold ~29 MB).
3. Add a hard per-client cap on all queued bytes (snapshots and control frames included) that closes the connection, and a write timeout so a stalled client cannot hang its connection task.
4. Make `CloseSession` hold the `sessions` lock only briefly (today the `if let` guard lives through `kill()` and `Drop`).
5. Make `scripts/check-protocol.ps1` detect generated files for types that were removed (ts-rs never deletes them).

### 2.4 Tooling

1. Exclude `app/src/lib/protocol` from Prettier (ts-rs writes each type on one line), or enable ts-rs's `format` feature.
2. Run `scripts/check-protocol.ps1` in CI (it assumes `cargo` on `PATH`).
3. Define the frontend envelope (machine id plus message, and a binary channel for `Output`/`Snapshot`) in Rust and generate it with ts-rs, so the drift check covers it.

### 2.5 Test gaps still open

1. Read the pipe's DACL back in a test; a protocol-version-mismatch test; a test that `Shutdown` terminates session processes.
2. `attach_with_a_new_size_resizes_the_session` checks the grid size, not the PTY size (echo the console size from the shell).
3. The xterm.js cross-check compares visible text only, not attributes or cursor position.
4. A Claude Code recording (needs the app to save a session's raw output).
5. Linux: compile and run the `#[cfg(unix)]` code and tests (Plan 5).

## 3. Accepted As-Is

- The snapshot binary search runs while holding the session lock; it only runs for snapshots over 16 MiB.
- A spawn failure leaves `state_changed_at` equal to the creation time.
- `snapshot_bounded`'s fallback can exceed `max_len` only when `max_len` is under 14 bytes (unreachable).
- The `pwsh-unicode-color` recording is 946 bytes; its content is complete.
