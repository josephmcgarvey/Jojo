# Jojo Plan 1: Carry-Over Into Plans 2–5

This document records the decisions made while executing Plan 1 (daemon core, branch `plan-1-daemon-core`, commits `876b4fc..e0c14bb`) and the findings that later plans must act on. It replaces the per-task progress ledger, which was scratch state.

## 1. Decisions That Changed the Plan's Code

1. **`vte` is a direct dependency.** `alacritty_terminal` 0.25.1 does not re-export `vte`, so `jojo-grid` depends on `vte` 0.15.0 (the same version `alacritty_terminal` uses) and imports `vte::ansi::{Processor, Color, CursorShape, NamedColor}`.
2. **Snapshot corrections (user-approved).** Double underline is emitted as SGR `4:2` (vte parses `21` as cancel-bold). Every trimmed row ends with `ESC[0m ESC[K`. A cursor in the pending-wrap state is restored by re-emitting the last cell. DECSCUSR is emitted only when the cursor style differs from the default. A row that follows a wrapped row is always written with at least one cell, so a blank continuation row is not lost.
3. **The daemon is the sole responder to terminal queries.** The session reader always writes `alacritty_terminal`'s query replies (DSR, DA, DECRQM, `CSI 18 t`) back to the PTY. The plan's "only when no window is attached" rule deadlocked `cmd.exe`, which sends `ESC[6n` at startup under ConPTY and waits for the answer.
4. **Accept errors no longer stop the daemon.** The server logs the error, waits 100 ms and keeps accepting. The Windows listener recreates a pipe instance when the previous one was consumed.
5. **Exit is reported after the final output.** The waiter thread reports `on_exit` only after the reader thread has drained, bounded by `READER_DRAIN_TIMEOUT` (2 s) so a grandchild process holding the Linux PTY open cannot suppress `SessionExited`.
6. **Size limits.** `CreateSession` and `Resize` outside 2–1000 columns × 1–1000 rows are rejected with `Error`; `TermGrid` also clamps to those bounds.
7. **Poison-tolerant daemon locks.** The daemon-wide `store` and `sessions` mutexes recover from poisoning instead of panicking.
8. **`jojod shutdown` waits** up to 5 s for the daemon to close the connection, so "restart daemon" can run `serve` immediately afterwards.

## 2. Requirements for Plan 2 (App Shell)

### 2.1 Protocol changes to make before `ts-rs` types are generated

Changing these after TypeScript types are committed requires a `PROTOCOL_VERSION` bump.

1. **Attach carries the pane size.** Change `Attach { session_id }` to `Attach { session_id, cols, rows }`. The daemon resizes the grid (first) and the PTY, then takes the snapshot, under the session lock. A snapshot is laid out for one grid size; replaying it into an xterm.js of a different size corrupts the screen.
2. **Snapshot size bound.** A pathological snapshot measured 72 MB against `MAX_FRAME_LEN` (16 MiB); the client rejects the frame and drops the whole connection, which then reconnects into the same failure. Either drop the oldest history rows until the snapshot fits, or split it into continuation frames. `Frame::encode` must refuse frames over the limit.
3. **Spawn failures identify their session.** Add `session_id: Option<SessionId>` to `Error` (or carry the failure text in `SessionExited`), so the app can show the error and the `PATH` searched in the right pane (spec section 10).
4. **`attachable` flag.** Mark in `SessionInfo` whether the daemon still holds a grid for the session (sessions from a previous daemon run do not).
5. **Malformed control frame.** Reply with `Error { request: "Frame" }` and keep the connection open instead of disconnecting.

### 2.2 App behavior

1. **xterm.js must not answer terminal queries.** Register CSI handlers that consume the queries the daemon already answers: DSR (`final: 'n'`), DA1 (`final: 'c'`), DA2 (`prefix: '>', final: 'c'`), DECRQM (`intermediates: '$', final: 'p'` and `prefix: '?', intermediates: '$', final: 'p'`).
2. **xterm.js `scrollback` ≥ 10,000** so snapshots are not truncated on the client.
3. **Never send a size outside 2–1000 × 1–1000.** xterm.js `FitAddon.proposeDimensions()` returns `undefined` for hidden panes.
4. **Client connect deadline.** `connect` currently retries `ERROR_PIPE_BUSY` forever.
5. **Pipe owner check (security).** After connecting, call `GetNamedPipeServerProcessId`, open the server process token and require that its user SID equals the client's, so another local user cannot squat `\\.\pipe\jojo-<username>` and receive keystrokes.
6. **Elevated daemon policy (security).** Either refuse to `serve` from an elevated token or add a High mandatory label (`S:(ML;;NWNR;;;HI)`) to the pipe.
7. **Events can arrive after later direct replies.** Events travel through a broadcast hop; direct replies do not. The app must apply events idempotently.

### 2.3 Daemon work

1. **Bounded per-client output queue.** Subscriber channels are unbounded; a window that stops reading grows daemon memory without limit. Decide a policy (for example, disconnect a client more than N MB behind; it reattaches with a fresh snapshot).
2. **Broadcast lag.** `RecvError::Lagged` silently drops events. Close the connection so the client re-lists.
3. **Blocking work off tokio workers.** Run `Session::spawn`, snapshotting, `store.save()` and `read_dir` with `spawn_blocking`.
4. **Resize the grid before the PTY** (ConPTY's repaint can otherwise be fed into the old-size grid).
5. **`AddProject` requires an absolute path.**
6. **`RemoveProject` broadcasts `SessionRemoved`** for the rows it deletes, and drops their `Session` objects (each holds a grid of up to ~29 MB). `CloseSession` and `RemoveProject` return `Error` for unknown ids.
7. **Cap the number of exited grids kept in memory**, or document the per-session memory cost in spec section 14.
8. **Logging.** Add `tracing` (spec section 10) and log failed `state.json` saves.
9. **`Session` cleanup.** Add a `Drop` that kills the child, and kill the child on `Session::spawn` error paths after `spawn_command` succeeded.
10. **Corrupt-file rename** uses a millisecond (or unique) suffix so two corrupt loads in one second do not overwrite each other.
11. **`pipe_security`** reads `last_os_error()` before `CloseHandle`.
12. **Session doesn't clamp sizes itself**: only the daemon validates before the PTY is resized.

### 2.4 Tests to add

1. Real recorded fixtures for snapshot round-trips (Claude Code permission prompt, PSReadLine redraws, CJK), a combined-attribute fixture, and the cross-check against `@xterm/headless` (spec section 11, item 1).
2. Pending-wrap edge cases: a wide character at the edge; a tab cell (map `'\t'` to space in the re-emit); restoring at `cursor.point.column` instead of `cols - 1`.
3. A direct test that a query reply reaches the PTY while a subscriber is attached; `detached_client_receives_no_output` must assert an `Output` arrives before detaching.
4. Pipe DACL test (read the security descriptor back); spawn-failure test (`exit_code -1`, `PATH` in the message); protocol-version mismatch; multi-client error isolation; `Shutdown` terminates session processes.

## 3. Requirements for Plan 5 (Machines)

1. Compile and run the `#[cfg(unix)]` code and tests on Linux (session tests, Unix socket transport). None of it has been compiled.

## 4. Accepted As-Is

- `read_frame` treats EOF inside the length prefix as a clean disconnect.
- The snapshot does not restore IRM, LNM, alternate-scroll mode, DECSC saved cursor, active charset or tab stops; kitty keyboard flags are disabled in `alacritty_terminal` by default. Underline color and OSC 8 hyperlinks are not restored.
- A bottom row's WRAPLINE after a reverse scroll is lost.
- `default_state_dir()` panics if `LOCALAPPDATA` / `HOME` is unset.
- No handshake timeout on the user-only pipe.
- `jojod` with no subcommand prints help and exits 0.
