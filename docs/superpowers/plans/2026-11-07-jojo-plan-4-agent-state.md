# Jojo Plan 4: Agent State Implementation Plan

> Executed inline in the controlling session (user preference, 2026-11-07). Each task ends with its tests passing and a commit.

**Goal:** Sessions show Claude Code's state (starting, idle, working, needs you, done, error) as live badges, Windows toasts announce changes in sessions the user is not looking at, and exited `claude` sessions can be resumed.

**Architecture:** Claude Code runs `jojod hook` for each registered hook event. The relay forwards the raw hook JSON with the session's `JOJO_SESSION_ID` to the local daemon as a `HookEvent` frame. The daemon interprets it with the pure state machine in the new `jojo-agent` crate, updates the session row (state, detail, `claude_session_id`, first-prompt label) and broadcasts `SessionUpdated`. The app renders the state and raises toasts.

**Tech stack:** Rust (jojo-agent, jojod, jojo-app), Svelte 5 + TypeScript, `tauri-winrt-notification` for toasts (named by spec 7.4 item 5).

## Scope

In scope (spec 5.4, 7.1-7.4, 9.4 sidebar session menu Resume, 9.10 `toasts`):

1. `jojo-agent` crate: `(state, event, now) -> state` with injected clock.
2. Protocol v3: `HookEvent`, `MarkSeen`, `CreateSession.resume`, `SessionInfo.state_detail` and `SessionInfo.renamed`.
3. Daemon: hook handling, silence fallback (10 s without output while `working` → `idle`), `MarkSeen` (`done` → `idle`), Resume.
4. `jojod hook`, `jojod install-hooks`, `jojod uninstall-hooks`; the app runs `install-hooks` with the staged `jojod.exe` on every start (the merge is idempotent).
5. App: `MarkSeen` when a `done` session's pane is focused in the foreground window; badge tooltips; toasts with a 10 s per-session limit and click-to-focus; Resume in the sidebar session menu.

Out of scope: machines (Plan 5); the settings screen, switcher and the simplified "+ opens a terminal" flow (a later functionality plan, then the design pass the user asked for).

## Global Constraints

- `PROTOCOL_VERSION` becomes `3`. Regenerate TypeScript with `scripts/check-protocol.ps1`; restarting the daemon ends open sessions (acceptable; nothing is released).
- Hook entries: command `"<absolute jojod path>" hook`, `"timeout": 5`, synchronous (no `async`), for `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PermissionRequest`, `Notification`, `Elicitation`, `Stop`, `StopFailure`, `SessionEnd`. Before writing, copy `settings.json` to `settings.json.jojo-bak`. Jojo's entries are those whose command ends in `jojod hook` or `jojod.exe hook` (quotes ignored); other entries are never removed or reordered.
- The relay exits 0 in every case, prints nothing, returns at once when `JOJO_SESSION_ID` is unset, and connects with a 500 ms timeout.
- Silence fallback: 10 s. Toast limit: one per session per 10 s. First-prompt label: first 40 characters of `prompt`, only when the session was never renamed by the user.
- US spelling; factual comments, no aphorisms; cargo with `--jobs 4`; never start jojod or the app from the PowerShell tool.
- New dependency: `tauri-winrt-notification` (spec 7.4 item 5, approved with the spec). No other new dependencies.

## Tasks

### Task 1: jojo-agent state machine

Create `crates/jojo-agent` (workspace member). `HookEvent::parse(json: &str) -> Option<HookEvent>` reads `hook_event_name`, `session_id`, `prompt`, `notification_type`, `tool_name`, `tool_input`, `error_type`; unknown events parse to `HookEvent::Other`. `apply(current: AgentState, event: &HookEvent) -> Option<Transition>` returns the new state and an optional detail string (`Needs permission: Bash(git push …)`, `Error: rate_limit`) per spec 7.2; `None` means no change. `silence(current, last_output_ms, now_ms) -> Option<AgentState>` implements 7.3 rule 3. `AgentState` comes from `jojo-proto`. Unit tests for every row of the 7.2 table, the permission ordering in 7.3 rule 1, unknown events and fields, and the silence rule.

### Task 2: Protocol v3

`ControlMsg::HookEvent { session_id, payload: String }` (relay → daemon), `ControlMsg::MarkSeen { session_id }` (app → daemon), `CreateSession.resume: Option<String>` (a `claude_session_id`; the daemon runs `claude --resume <id>`), `SessionInfo.state_detail: Option<String>` and `SessionInfo.renamed: bool` (both `#[serde(default)]` so existing state.json files load). `PROTOCOL_VERSION = 3`. Update frame tests and regenerate bindings.

### Task 3: Daemon handling

`HookEvent`: find the session, parse, record `claude_session_id` on `SessionStart`, set the label from the first `UserPromptSubmit` when not `renamed`, apply the transition, save, broadcast `SessionUpdated`. Unknown sessions and unparsable payloads are ignored (no error reply; the relay has disconnected). `RenameSession` sets `renamed = true`. `MarkSeen` changes `done` → `idle`. `Session` records the time of its last PTY output; a 1 s daemon tick applies the silence rule. `CreateSession.resume` builds `claude --resume <id>`. Integration tests in `crates/jojod/tests/daemon_roundtrip.rs`.

### Task 4: Relay and hook installation

`jojod hook`: reads stdin, `JOJO_SESSION_ID`, connects with 500 ms, sends `Hello` and `HookEvent`, exits 0. `jojod install-hooks [--settings <path>] [--command <path>]` and `uninstall-hooks`: the merge rules above, implemented in `crates/jojod/src/hooks.rs` as pure functions over `serde_json::Value` with tests (append, idempotent repeat, foreign entries untouched, uninstall removes only Jojo's entries, missing file, backup written). The app's daemon manager runs `<staged jojod.exe> install-hooks` once per app start on a blocking thread and logs failures.

### Task 5: App state, Resume and toasts

`MarkSeen` when a session in `done` is the focused pane while the window has focus (on focus change, on window focus, and on `SessionUpdated`). Badge `title` shows the state and `state_detail`. Sidebar session menu: Resume for exited `claude` sessions with `claude_session_id` (CreateSession with `resume`, same placement as Restart). Toasts: the backend command `notify(session_id, title, body)` shows a toast through `tauri-winrt-notification` with the app's AppUserModelID; clicking it emits `toast-activated` with the session id, and the app focuses the window and the pane. The frontend decides when to toast (spec 7.4 rules 1, 2, 4, 6) in a pure, tested module `lib/state/toasts.ts`.

### Task 6: Checklist, verification, installer

Plan 4 section in `docs/manual-test-checklist.md`; full Rust and frontend suites, clippy, svelte-check, color lint, `check-protocol.ps1`; build the NSIS installer.
