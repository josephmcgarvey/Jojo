# Jojo Plan 2b: Carry-Over Into Plan 3

This document records the decisions made while executing Plan 2b (the Tauri app shell, branch `plan-2b-app-shell`, commits `6162921..3908873` on top of `a64dbc9`) and the items Plan 3 (pane layout) and later plans must act on. It replaces the per-task progress ledger, which was scratch state. The Plan 1 and Plan 2a carry-over documents remain the source for the items they list that are not repeated here.

## 1. Decisions Made During Plan 2b

1. **The frontend envelope is a Tauri Channel of raw bytes.** Byte 0 of each message selects the kind: `0` is a daemon frame body (`[type][payload]`, the frame without its 4-byte length prefix), and `1` is a `ConnectionStatus` JSON document. `send_frame` takes a raw invoke body.
2. **The connection loop reads frames in a separate task**, because `read_frame` is not cancel-safe inside `select!`. A restart generation counter (a `watch` channel) lets the Retry and Restart buttons interrupt backoff.
3. **A terminal pane is keyed on `${session.id}:${session.attachable}`** (`app/src/lib/terminal/pane.ts`, `paneKey`). When a replacement jojod reports an old session as not attachable, the active pane remounts, shows the no-screen message, and stops sending input.
4. **Terminal links open in the default browser through the `open_url` command** (`app/src-tauri/src/links.rs`). The command accepts only `http://` and `https://` URLs with a host, rejects control characters, whitespace and backslashes, caps the length at 4096 bytes, and calls `ShellExecuteW`. The main window is created in `setup` (`"create": false` in `tauri.conf.json`) so that it can register `on_new_window` returning `Deny`; no WebView2 popup window can open.
5. **A failed spawn is recognized by state:** state `exited`, `exit_code` -1, and `state_changed_at === created_at` (`pane.ts`, `failedToStart`). The daemon's failed-spawn branch never updates `state_changed_at`, and `on_session_exit` always does.
6. **`Hello` and `Events` errors are cleared when the connection status becomes `connected`** (`reducer.ts`, `clearConnectionErrors`). Errors for other requests stay until dismissed.
7. **Packaging:** `beforeBuildCommand` runs `scripts/stage-daemon.ps1 -Profile release` before the frontend build. The bundle uses the `resources/daemon/*` glob, which matches the tracked `.gitkeep`, so an unstaged checkout still builds and tests. The staging script deletes every file in `app/src-tauri/resources/daemon` except `.gitkeep` before copying. The installer contains a zero-byte `.gitkeep`.

## 2. Corrections to Plan 2b's Coverage Note

The self-review section of the Plan 2b document claims three Plan 2a carry-over items that Plan 2b did not deliver. They remain open:

1. **2.1.8:** no remove-project UI exists, so the "kill each session, wait for `SessionExited`, then `RemoveProject`" flow is not implemented.
2. **2.4.2:** the repository has no CI, so `scripts/check-protocol.ps1` is not run automatically.
3. **2.4.3:** only `ConnectionStatus` is generated with ts-rs. The envelope kind bytes are duplicated by hand in `app/src-tauri/src/bridge.rs` and `app/src/lib/wire/bridge.ts`.

## 3. Items for Plan 3

### 3.1 Window and terminal behavior

1. Windows 11 snap layouts: the custom title bar's maximize button does not answer `WM_NCHITTEST` with `HTMAXBUTTON`, so the snap layout flyout does not appear.
2. OSC 8 hyperlinks open nothing. xterm.js 6's default handler shows `confirm()` and then calls `window.open()`, which the new-window handler denies. Set `linkHandler.activate` to invoke `open_url`, keeping a confirmation step, because OSC 8 link text can differ from its target.
3. `open_url` failures are swallowed (`.catch(() => {})` in `TerminalPane.svelte`); show feedback when a URL is rejected or `ShellExecuteW` fails.
4. `open_url` is a synchronous command and runs on the main thread; `ShellExecuteW` can block there (for example on the "Open with" dialog). Moving it to a worker thread requires `CoInitializeEx` on that thread.
5. xterm.js measures cells at `term.open()`, and nothing waits for the Geist Mono font (`font-display: swap`). Await `document.fonts.load('13px "Geist Mono"')` before mounting the app.
6. Theme changes are not propagated to mounted panes (only one theme exists today).
7. DECRQSS is still answered by xterm.js. The daemon's grid does not answer it, so there is one reply, not two.
8. Session rename and the session switcher hint are not implemented.

### 3.2 Attach and flow control

1. **AttachGate is a boolean, not a generation.** Neither the gate nor `TerminalPane.svelte` enforces "at most one outstanding Attach per session"; `fellBehind` and `reattachAll` can each send a second Attach. The daemon's total-cap accounting (`client_queue.rs`, a Snapshot counted as 256 bytes) assumes the app keeps one outstanding Attach. Replace the boolean with a generation counter and skip `attach()` while one is outstanding.
2. **No end-to-end flow control.** The app's reader task drains the pipe as fast as possible, so the daemon's "Output fell behind" path never triggers for the app. xterm.js throws once more than 50 MB of writes are pending, and the Channel's `deliver` swallows the error, so data is lost without a resync. Track bytes written per pane with the `term.write` callback and send an Attach when a watermark is crossed.
3. Two quick `CreateSession` requests activate only the second session.

### 3.3 Connection and daemon management

1. A Retry click while `connect_or_start` is running (up to about 10 s) is lost, because `since` is read after that call returns. Read `since` at the top of each loop iteration.
2. A user restart within 2 s of connecting counts as a short connection and adds backoff.
3. An aborted reader task can emit one frame after the `Reconnecting` status. The reader task is detached if `serve()` is dropped (tests only).
4. After a version mismatch, `connection.rs` sleeps a fixed 500 ms after `taskkill`, and `daemon_manager.rs` silently skips a copy that fails with `ERROR_SHARING_VIOLATION`. If the old image is still locked, the old binary starts again. Poll until no `jojod.exe` remains, or retry the copy.
5. `kill_daemons` kills every `jojod.exe` of the user, including a development daemon.
6. The start timeout reports only the last connect error. `stage_binaries` compares full file contents on every start.
7. On a first launch that never connects, the banner says "Disconnected … Sessions keep running". Show a distinct state until a connection has succeeded once.
8. A subscribe failure reuses the `failed` status, where Retry does nothing useful.
9. One key of the wrong type in `settings.json` resets every setting to its default.
10. `tauri build --debug` still stages a release jojod, because `-Profile release` is hard-coded in `beforeBuildCommand`.

### 3.4 Daemon items still open from Plan 2a

1. Move blocking work (`Session::spawn`, snapshotting, `store.save()`, `read_dir`) off tokio worker threads with `spawn_blocking` or an asynchronous `Daemon::handle`, keeping `SessionCreated` before `SessionExited`.
2. Cap the number of exited sessions whose grids stay in memory (each can hold about 29 MB).
3. `CONTROL_FRAME_WEIGHT` (256 bytes) undercounts large `Sessions` and `Projects` replies.
4. The overflow `Notify` permit serves one waiter (one `select!` arm per connection waits on it today).
5. The `WRITE_TIMEOUT` path has no end-to-end test.
6. `session::tests::a_client_that_falls_behind_is_detached_with_an_error` hit its 60 s flood deadline once during a full `cargo test -p jojod` run under load, and passed alone (21 s) and on a second full run.

### 3.5 Security and tooling

1. The content security policy is `null`. Suggested value: `default-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self' data:; img-src 'self' data:; connect-src ipc: http://ipc.localhost`.
2. `scripts/check-protocol.ps1` leaves generated files deleted when `cargo test` fails.
3. The color lint's `style:{...}` capture stops at the first `}`.
4. `Channel<unknown>` in the frontend bridge could be `Channel<ArrayBuffer>`.

### 3.6 Test gaps

1. A Claude Code recording for the xterm.js cross-check still needs the app to save a session's raw output.
2. The frontend model does not update `state_changed_at` on `SessionExited` (`reducer.ts`). No user sees it today, because the failed-to-start message is shown only for non-attachable sessions, but any future use of `failedToStart` on an attachable session would misclassify a session that exited with code -1.
3. Plan 2a carry-over 2.5 items 1–3 and 5 remain open.
