# Jojo: Design Specification

**Date:** 2026-11-06
**Status:** Approved in brainstorming; awaiting written-spec review
**Author:** Joe McGarvey, with Claude Code

## 1. Overview

Jojo is a Windows desktop terminal application for running and supervising many Claude Code sessions at once. It combines a terminal emulator and a terminal multiplexer in one native window, in the same category as Superlogical's Rex (macOS only) and Herdr (a text-mode multiplexer that runs inside another terminal).

Jojo has four defining properties:

1. **Sessions outlive the window.** Every terminal session is owned by a background daemon (`jojod`). Closing, crashing or restarting the Jojo window does not stop any session. Reopening the window repaints every pane exactly as it was.
2. **Sessions are aware of Claude Code.** Claude Code hooks report each session's state (working, needs you, done, error) to the daemon. The sidebar shows the state as an animated badge, and Windows toast notifications announce changes in sessions you are not looking at.
3. **Machines.** Linux servers reachable over SSH can be added as machines. Jojo installs `jojod` on the server, and sessions on that server appear in the same sidebar as local sessions.
4. **Visual design is a primary requirement.** The interface is built in HTML and CSS inside WebView2, so it has full control over typography, color, glow and animation. Version 1 ships one theme, Neon Glass, on a token system that allows more themes later.

Jojo is a personal tool for a single user on a single Windows 11 PC. It is not designed for distribution, multiple users or teams.

### 1.1 Goals

1. Run, view and switch between at least 20 concurrent sessions (Claude Code or shell) across This PC and remote Linux servers.
2. Show the state of every Claude Code session at a glance and notify the user when one needs attention.
3. Survive window closure, window crashes and network interruptions to remote machines without losing session output.
4. Provide tabs, split panes (up to 4 per tab) and right-click control of pane layout.

### 1.2 Non-goals for version 1

1. macOS or Linux builds of the Jojo window. The daemon builds for Linux only so that servers can be added as machines.
2. Windows PCs or Macs as remote machines.
3. WSL distributions as machines.
4. A control CLI that lets agents open panes or read other panes.
5. Per-session metadata in the sidebar beyond label, badge and elapsed time (for example current tool, git branch, token use).
6. Drag-and-drop rearrangement of panes.
7. Themes other than Neon Glass.
8. Automated end-to-end UI tests.
9. Code signing and auto-update.

## 2. Decision Summary

| Topic | Decision |
|---|---|
| Application shell | Tauri 2 (Rust backend, WebView2 frontend) |
| Frontend | Svelte 5 with TypeScript, built with Vite |
| Terminal rendering | xterm.js with the WebGL renderer |
| PTY layer | `portable-pty` (ConPTY on Windows, Unix PTYs on Linux) |
| Session persistence | Rust daemon `jojod`, one per machine, with an `alacritty_terminal` grid per session |
| Repaint on reattach | Daemon serializes its grid to VT sequences (a Snapshot) |
| Organization | Machine → Project → Session |
| Layout | Tabs per project; each tab holds a split tree of 1–4 panes |
| Agent awareness (v1) | State badges and Windows toasts, driven by Claude Code hooks |
| Machines (v1) | This PC plus Linux servers (x86_64, aarch64) over SSH |
| Remote install | The app uploads and upgrades `jojod` on the server automatically |
| Visual direction | Neon Glass; Window layout A (sidebar full height, window controls in the tab row) |
| Theming | One JSON file per theme; all UI values are CSS custom properties |

## 3. Architecture

### 3.1 Processes

Jojo consists of three programs. The second and third are subcommands of the same Rust binary.

```
┌────────────────────────────── jojo.exe (Tauri) ─────────────────────────────┐
│  Frontend (WebView2): Svelte 5 + TS, xterm.js per visible pane               │
│  Backend (Rust): machine manager, frame router, toast notifier               │
└───────────────┬──────────────────────────────────────────┬──────────────────┘
                │ named pipe \\.\pipe\jojo-<user>          │ ssh.exe <host> jojod attach --stdio
                ▼                                          ▼
┌──────── jojod.exe (This PC) ────────┐     ┌──────── jojod (Linux server) ────────┐
│ ConPTY sessions + grids             │     │ Unix PTY sessions + grids             │
│ state.json in %LOCALAPPDATA%\Jojo   │     │ socket ~/.jojo/jojod.sock, state.json │
└─────────────▲───────────────────────┘     └─────────────▲─────────────────────────┘
              │ pipe                                       │ socket
     jojod hook  (run by Claude Code hooks, reads JOJO_SESSION_ID)
```

**`jojo.exe`, the Tauri application.** The window. It holds no terminal state, so closing or crashing it loses nothing.

- The frontend renders the sidebar, tabs, split panes, menus and settings. It creates one xterm.js instance per *visible* pane. Panes in hidden tabs have no xterm.js instance, so background sessions cost no rendering work.
- The Rust backend maintains one connection per enabled machine, routes frames between panes and daemons, and raises Windows toast notifications.
- Tauri's single-instance plugin allows only one Jojo window. Launching `jojo.exe` again focuses the existing window.

**`jojod`, the daemon.** One instance runs per machine. It owns every PTY on that machine and an `alacritty_terminal` grid per session that records screen contents and scrollback. The daemon is the same Rust code on Windows and Linux. `portable-pty` selects ConPTY or Unix PTYs at compile time. The only platform-specific code is the IPC transport (named pipe or Unix socket) and process daemonization.

**`jojod hook`, the hook relay.** Claude Code runs this command for each registered hook event. It reads the hook JSON on stdin and the `JOJO_SESSION_ID` environment variable, sends one event to the daemon on the same machine, and exits with code 0.

### 3.2 `jojod` subcommands

| Subcommand | Purpose |
|---|---|
| `jojod serve` | Run the daemon in the foreground (used on Windows, where the app launches it detached) |
| `jojod serve --daemonize` | Linux: `setsid`, fork, write the PID, and run in the background |
| `jojod attach --stdio` | Connect to the local daemon's socket and relay stdin/stdout to it. Used as the remote end of the SSH connection |
| `jojod hook` | Hook relay (section 7.1) |
| `jojod install-hooks` / `jojod uninstall-hooks` | Merge or remove Jojo's entries in `~/.claude/settings.json` on this machine |
| `jojod shutdown` | Ask the running daemon to kill all sessions and exit |
| `jojod --version` | Print the daemon version and protocol version |

## 4. Data Model and Storage

| Entity | Fields | Stored in |
|---|---|---|
| Machine | `id`, `label`, `kind` (`local` \| `ssh`), `ssh_host` (alias from `~/.ssh/config` or `user@host`), `enabled` | App: `%APPDATA%\Jojo\machines.toml` |
| Project | `id`, `path` (absolute, on that machine), `name` | Daemon: `state.json` |
| Session | `id` (ULID), `project_id`, `kind` (`claude` \| `shell`), `command`, `args`, `label`, `created_at`, `state`, `state_changed_at`, `exit_code`, `claude_session_id` | Daemon: `state.json` (metadata only; no screen contents) |
| Layout | Keyed by `(machine id, project id)`, because project ids are unique only within one daemon. Per project: ordered tabs; per tab: name and split tree of session ids; focused pane. Session ids that no longer exist on the daemon are dropped when the layout is loaded, and daemon sessions missing from the layout are each placed in a new tab. | App: `%APPDATA%\Jojo\layout.json` |
| Settings | See section 9.10 | App: `%APPDATA%\Jojo\settings.json` |

Daemon `state.json` locations: `%LOCALAPPDATA%\Jojo\state.json` on Windows and `~/.jojo/state.json` on Linux. The daemon writes it atomically (write to `state.json.tmp`, then rename) on every project or session metadata change.

Projects and sessions are stored by the daemon, not the app, so the sidebar contents depend only on which daemons the app is connected to.

## 5. Session Lifecycle

### 5.1 Creation

The app sends `CreateSession { project_id, kind, cols, rows }`. The daemon spawns the process in the project directory with these environment additions:

- `TERM=xterm-256color`
- `COLORTERM=truecolor`
- `JOJO_SESSION_ID=<session id>`

Commands by kind:

| Kind | Windows | Linux |
|---|---|---|
| `claude` | `claude` resolved on `PATH`. If the resolved file is `claude.cmd` (npm installation) rather than `claude.exe`, it is launched as `cmd.exe /d /c claude` because ConPTY spawns executables, not batch files. | `claude` resolved on `PATH` |
| `shell` | The `defaultShell` setting, default `pwsh` | `$SHELL`, falling back to `/bin/bash` |

Sessions inherit the daemon's environment. The daemon's environment is fixed when the daemon starts, so a `PATH` change made after that is visible only to sessions created after a daemon restart. The settings screen states this next to the "Restart daemon" action.

### 5.2 Running

The daemon reads PTY output continuously, whether or not any window is attached, and feeds every byte into the session's grid. Scrollback is capped at the `scrollbackLines` setting (default 10,000 lines per session).

### 5.3 Exit

When the process exits, the session moves to `exited` with its exit code. The daemon keeps the grid so the final output stays readable. The session is removed only when the user closes its pane.

### 5.4 Daemon restart

When the daemon stops (Windows reboot, crash, or explicit restart), all processes end, but session metadata survives in `state.json`. On the next start, every session that was not already `exited` is marked `exited` with `exit_code = null`. The sidebar offers:

- **Resume** for `claude` sessions with a recorded `claude_session_id`. This starts `claude --resume <claude_session_id>` in the same project directory, as a new session that replaces the old row.
- **Restart** for shell sessions and for `claude` sessions without a recorded id. This starts the same command in the same directory.

### 5.5 Attach, snapshot and detach

1. On connecting to a daemon, the app sends `ListProjects` and `ListSessions` and subscribes to state events for every session. These events are small and drive the sidebar and toasts.
2. When a pane becomes visible, the app sends `Attach { session_id }`. The daemon replies with `Snapshot`: the scrollback and visible screen serialized as VT sequences, followed by the sequences that restore cursor position, cursor style, SGR attributes and terminal modes (alternate screen, application cursor keys, bracketed paste, mouse tracking modes, origin mode, auto-wrap). Live `Output` frames follow.
3. The snapshot is produced while holding the same lock the daemon uses to feed PTY output into the grid. No byte can fall between the end of the snapshot and the first live `Output` frame, and no byte appears in both.
4. When a pane becomes hidden (its tab is not the active tab, or its project is not selected), the app sends `Detach`. Hidden panes therefore produce no traffic over SSH.

### 5.6 Resize

The app sends `Resize { session_id, cols, rows }` when a pane's size changes. Only one Jojo window can exist, so there is never more than one client size per session.

## 6. Wire Protocol

The protocol is identical over the Windows named pipe and over SSH stdio.

### 6.1 Framing

Each frame is:

```
[u32 little-endian: length of type + payload][u8: frame type][payload]
```

- **Control frames** carry UTF-8 JSON payloads. Rust structs (with `serde`) are the single source of truth. The `ts-rs` crate generates TypeScript types from them into the frontend source tree.
- **Data frames** (`Input`, `Output`, `Snapshot` body) carry a 16-byte binary session id (ULID) followed by raw bytes, with no JSON or base64 wrapping.

### 6.2 Messages

| Message | Direction | Purpose |
|---|---|---|
| `Hello { protocol_version, jojod_version }` | both | First frame on every connection. The app requires an exact `protocol_version` match. |
| `ListProjects` / `Projects` | app → daemon / reply | |
| `AddProject { path, name }` / `RemoveProject { id }` | app → daemon | |
| `ListDir { path }` / `DirEntries` | app → daemon / reply | Directory autocomplete when adding a project on a remote machine |
| `ListSessions` / `Sessions` | app → daemon / reply | |
| `CreateSession { project_id, kind, cols, rows }` | app → daemon | |
| `Attach { session_id }` / `Snapshot` | app → daemon / reply | Section 5.5 |
| `Detach { session_id }` | app → daemon | |
| `Input { session_id, bytes }` | app → daemon | |
| `Output { session_id, bytes }` | daemon → app | Sent only for attached sessions |
| `Resize { session_id, cols, rows }` | app → daemon | |
| `Kill { session_id }` | app → daemon | |
| `RenameSession { session_id, label }` | app → daemon | |
| `StateChanged { session_id, state, detail }` | daemon → app | `detail` carries tool name and summary, or error type, for toasts and tooltips |
| `SessionExited { session_id, exit_code }` | daemon → app | |
| `HookEvent { session_id, payload }` | hook relay → daemon | The hook relay uses the same framing, after `Hello` |
| `Error { request, message }` | daemon → app | Any request that fails |

## 7. Agent State Detection

Event names and fields below come from the Claude Code hooks reference at https://code.claude.com/docs/en/hooks, as read on 2026-11-06.

### 7.1 Hook registration

`jojod install-hooks` merges one command hook, `<absolute path to jojod> hook`, into `~/.claude/settings.json` for these events:

`SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PermissionRequest`, `Notification`, `Elicitation`, `Stop`, `StopFailure`, `SessionEnd`.

Rules for the merge:

1. Before writing, the existing file is copied to `settings.json.jojo-bak`.
2. Existing hook entries are never removed or reordered. Jojo's entries are appended.
3. Jojo identifies its own entries by the command path ending in `jojod hook` or `jojod.exe hook`. Running `install-hooks` again replaces those entries and changes nothing else, so it is safe to repeat.
4. Each entry is registered with `"timeout": 5` and without `"async": true`.

Hooks are **synchronous** on purpose. Asynchronous hooks run in the background, so their events can reach the daemon out of order. A `PreToolUse` event arriving after a `PermissionRequest` event would set the state back to `working` while Claude is waiting for the user.

Because Claude Code waits for synchronous hooks, the relay must be fast:

1. If `JOJO_SESSION_ID` is not set, exit immediately with code 0. Claude Code sessions started outside Jojo are unaffected.
2. Connect to the local daemon with a 500 ms timeout.
3. Send `Hello` and one `HookEvent` frame containing the hook JSON read from stdin.
4. Print nothing to stdout or stderr. Exit with code 0 in every case, including failure.

The worst-case delay added to Claude Code is therefore about 500 ms per event, and only when the daemon is unreachable. The relay never prints JSON, so it never changes Claude Code's decisions.

`JOJO_SESSION_ID` is inherited by every process in a pane. A `claude` started by hand inside a shell pane therefore reports its state and the pane shows agent badges.

### 7.2 States

| State | Neon Glass badge | Entered on |
|---|---|---|
| `starting` | Dim ring | Process spawned; no `SessionStart` received yet |
| `idle` | Dim ring | `SessionStart`; or `done` when its pane receives focus; or the silence fallback (7.3, rule 3) |
| `working` | Cyan ring, spinning | `UserPromptSubmit`, `PreToolUse`, `PostToolUse` |
| `needs_you` | Amber glow, pulsing | `PermissionRequest`; `Elicitation`; `Notification` whose `notification_type` is `permission_prompt`, `elicitation_dialog`, `elicitation_url_dialog` or `agent_needs_input` |
| `done` | Solid teal disc | `Stop` |
| `error` | Solid red disc | `StopFailure`; the `error_type` (for example `rate_limit`) is shown in the tooltip and toast |
| `exited` | Hollow gray ring with exit code | PTY process exited (detected by the daemon, not a hook) |

Shell sessions without a Claude Code process show `running` (dim ring) or `exited` only.

On `SessionStart`, the daemon records `session_id` from the payload as the session's `claude_session_id` (used by Resume). On the first `UserPromptSubmit`, if the user has not renamed the session, the daemon sets the label to the first 40 characters of the `prompt` field.

### 7.3 Transition rules

1. **Permission prompt ordering.** `PreToolUse` fires before the permission check and `PermissionRequest` fires after it. A tool call that needs approval moves `working` → `needs_you`. Approval leads to `PostToolUse` (`working`). Choosing "No, tell Claude what to do differently" ends the turn with `Stop` (`done`).
2. **`done` means unread.** A `done` session changes to `idle` when its pane receives focus in the foreground window.
3. **Silence fallback.** Pressing Esc to interrupt Claude Code does not fire a hook. If a session is `working` and its PTY produces no output for 10 seconds, the daemon sets it to `idle`. While Claude Code is actually working, its spinner redraws several times per second, so 10 seconds without output indicates the turn ended.
4. The state machine is a pure function `(state, event, now) → state` in its own crate (section 12), with the clock injected for tests.

### 7.4 Toast notifications

1. A toast fires when a session enters `needs_you`, `done` or `error` **and** that session is not the focused pane in the foreground Jojo window.
2. Title: `<session label> · <project name>`. Body: `Needs permission: <tool_name>(<abbreviated tool_input>)`, `Finished`, or `Error: <error_type>`.
3. Clicking a toast brings the Jojo window to the front and focuses the session's pane.
4. At most one toast per session per 10 seconds.
5. Implementation: `tauri-winrt-notification`, with the app registered under an AppUserModelID so toast activation reaches the running instance.
6. Toasts can be disabled in settings.

## 8. Machines

### 8.1 This PC

"This PC" is always the first machine and cannot be removed.

- The app ships `jojod.exe` and copies it to `%LOCALAPPDATA%\Jojo\bin\jojod.exe`.
- If the pipe `\\.\pipe\jojo-<username>` does not exist, the app starts `jojod.exe serve` with the process creation flags `DETACHED_PROCESS | CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB`. `CREATE_BREAKAWAY_FROM_JOB` prevents the daemon from being terminated with the app's job object when the window closes.
- The pipe's security descriptor grants access only to the current user's SID.
- The daemon runs until the user restarts it from settings, runs `jojod shutdown`, or Windows shuts down.

### 8.2 Adding a Linux server

The **Add machine** dialog runs these steps in order and shows the status of each.

1. **Host.** The user chooses an alias parsed from `~/.ssh/config` or types `user@host`, and enters a label.
2. **Connectivity.** Run `ssh -o BatchMode=yes -o ConnectTimeout=10 <host> uname -sm`. `BatchMode` prevents interactive prompts, so keys must be loaded in the Windows "OpenSSH Authentication Agent" service. On failure, show SSH's stderr and offer **Open SSH in a pane**, which runs `ssh <host>` interactively in a local shell pane so the user can accept the host key or enter a passphrase. The dialog then retries.
3. **Platform.** Accept `Linux x86_64` and `Linux aarch64`. Otherwise stop with "Unsupported platform: <uname output>".
4. **Install or upgrade.** Run `~/.local/bin/jojod --version`. If missing or different from the bundled version, stream the matching binary from the app's resources (`jojod-linux-x86_64` or `jojod-linux-aarch64`) via `ssh <host> 'mkdir -p ~/.local/bin && cat > ~/.local/bin/jojod.new'`, then run `chmod +x ~/.local/bin/jojod.new && mv ~/.local/bin/jojod.new ~/.local/bin/jojod`. Streaming over stdin avoids depending on `scp` or SFTP on the server; `mv` makes the replacement atomic.
5. **Start.** Run `~/.local/bin/jojod serve --daemonize`. The daemon calls `setsid` and forks so it survives the SSH session ending. It holds `~/.jojo/jojod.lock`, so a second `serve` exits cleanly. The socket is `~/.jojo/jojod.sock`; `~/.jojo` has mode `0700` and the socket `0600`.
6. **Logout survival.** Check `loginctl show-user $USER -p Linger` and the system's `KillUserProcesses` setting. If processes would be killed at logout and lingering is off, warn and offer to run `loginctl enable-linger`. Without lingering on such a system, every session on the server ends when the user's last SSH session closes.
7. **Hooks.** Run `~/.local/bin/jojod install-hooks`. If `command -v claude` finds nothing, warn that Claude Code is not installed on the server; shell sessions still work.

### 8.3 Live connection

For each enabled remote machine, the app runs one long-lived process:

```
ssh -o BatchMode=yes -o ServerAliveInterval=15 -o ServerAliveCountMax=3 <host> ~/.local/bin/jojod attach --stdio
```

With these keepalive values, a dead connection is detected within about 45 seconds.

When the connection drops:

1. The machine header shows "reconnecting"; its sessions keep their last badge, dimmed.
2. Open panes for that machine show the overlay "Disconnected. Sessions keep running on <label>."
3. The app reconnects with exponential backoff starting at 1 second and capped at 30 seconds.
4. **Keystrokes typed while disconnected are discarded, not queued.** Replaying input typed into a terminal the user could not see could approve a permission prompt or run an unintended command.
5. If `ssh` exits with code 255 and its stderr contains "Permission denied", the app stops retrying and shows "Auth failed. Load your key with ssh-add, then Reconnect."

On reconnect, the app sends `Hello`, re-reads projects and sessions, and re-attaches every visible pane, each receiving a fresh `Snapshot`.

### 8.4 Versions and removal

- `protocol_version` is a single integer and must match exactly.
- **Remote mismatch:** the app offers to upgrade. The prompt states the number of sessions on that machine that will end, because upgrading restarts the daemon.
- **This PC mismatch** (after an app update): the app offers **Restart now (ends N sessions)** or **Later**. With **Later**, the old daemon keeps running and its sessions keep running, but the app cannot attach until the daemon is restarted. The sidebar shows the machine as "Restart required".
- **Remove machine** disconnects and deletes the entry from `machines.toml`. The checkbox **Also uninstall from server** runs `jojod shutdown`, `jojod uninstall-hooks`, and deletes `~/.local/bin/jojod` and `~/.jojo/`.

## 9. User Interface

### 9.1 Window

- **Frame.** Tauri `decorations: false`. Jojo draws minimize, maximize and close buttons at the right end of the tab row (Layout A). The empty area of the tab row is a window drag region. The maximize button's area is reported to Windows as the maximize button (`HTMAXBUTTON` hit-testing) so that Windows 11 snap layouts appear when hovering it.
- **Background.** The application paints the Neon Glass gradients itself on an opaque window. Windows acrylic is not used, because the `window-vibrancy` crate documents severe lag with acrylic while dragging or resizing a window on Windows 11. A Mica option may be added later as a theme setting.
- **Fonts.** Geist and Geist Mono are bundled under the SIL Open Font License and loaded locally. The terminal font family, size and line height are settings.

### 9.2 Sidebar

- Hierarchy: Machine header → Project → Session rows.
- A session row shows its badge, label, and the time since its last state change (`now`, `2m`, `1h`).
- A machine header shows a connection dot (teal connected, amber reconnecting, red auth failed) and a **+** that adds a project. On This PC, **+** opens the Windows folder picker. On a remote machine, it opens a path field with autocomplete from `ListDir`.
- Hovering a project row reveals **+**, which offers **Claude** or **Shell** and creates the session in a new tab.
- Double-clicking a session row renames it.
- The bottom of the sidebar holds **+ Machine** and the settings button.

### 9.3 Sessions, tabs and panes

1. Each session is displayed in exactly one pane.
2. Each project has its own ordered set of tabs. Selecting a project in the sidebar shows that project's tabs.
3. Each tab holds a split tree of 1 to 4 panes, using horizontal and vertical splits. Dividers are draggable.
4. Each pane has a header strip showing the badge and label. The focused pane has a 1 px accent outline.
5. Clicking a session in the sidebar selects its project, activates the tab that contains it, and focuses its pane.
6. Closing a pane ends its session. If the session is `working` or `needs_you`, Jojo asks for confirmation first.

### 9.4 Right-click menus

Menus are drawn by Jojo in Svelte, styled with theme tokens, and show the shortcut next to each item. Submenus open on hover. Arrow keys, Enter and Esc operate every menu.

**Mouse reporting rule.** When the program in a pane has enabled mouse reporting (for example vim, htop, or any TUI in mouse mode), a plain right-click is sent to that program. **Shift+right-click always opens Jojo's menu.** When mouse reporting is off, a plain right-click opens Jojo's menu.

**Pane menu** (right-click in a pane or on its header strip):

| Item | Behavior |
|---|---|
| Copy | Enabled when text is selected |
| Paste | Pastes the clipboard into the pane |
| Split right ▸ New Claude · New shell | Adds a pane to the right, in the same project |
| Split down ▸ New Claude · New shell | Adds a pane below |
| Zoom pane | Toggles this pane filling the tab; other panes continue running and return on toggle. Shortcut Alt+Shift+Z |
| Swap with ▸ *other panes in this tab, by label* | Exchanges two panes' positions |
| Move to ▸ New tab · *other tabs in this project* | Removes the pane from this tab's split tree and adds it to the target tab |
| Layout ▸ Columns · Rows · Grid 2×2 · Main + stack | Rearranges all panes in the tab. Main + stack places the focused pane at 60% width on the left and stacks the others vertically on the right |
| Equalize sizes | Resets every divider in the tab to equal shares |
| Rename session… | Inline edit in the header strip |
| Close pane | Section 9.3, rule 6 |

**Tab menu:** Rename tab; Layout ▸ (same presets); Close tab. Close tab lists the sessions it will end and how many are `working` or `needs_you` before confirming. Tabs cannot move between projects, because every session in a tab belongs to that tab's project (its `project_id` and working directory).

**Sidebar session menu:** Open in split right; Open in split down (both move the session into the current tab next to the focused pane, and are enabled only when the current tab belongs to the session's project); Move to new tab; Rename; Resume or Restart (for sessions in `exited`); Kill (ends the process and keeps the row so the final output remains readable).

**Sidebar project menu:** New Claude session; New shell; Reveal in Explorer (This PC only); Remove project.

**Sidebar machine menu:** Reconnect; Disable or Enable; Remove machine.

**Limits.** A tab holds at most 4 panes. Split items and incoming Move to targets are disabled in a full tab, with the tooltip "Tab is full (4 panes)".

### 9.5 Copy on select

When the `copyOnSelect` setting is on (default):

1. Releasing the mouse after a drag selection copies the selected text to the Windows clipboard. Double-click (word) and triple-click (line) selections copy in the same way. Text comes from xterm.js `getSelection()`, which joins wrapped lines and trims trailing whitespace.
2. The pane header strip shows "Copied" for 1.2 seconds.
3. The selection stays highlighted until the user clicks elsewhere in the pane or sends a keystroke to it.
4. In a pane with mouse reporting enabled, a plain drag is sent to the program; **Shift+drag** forces a selection.
5. **Ctrl+C is always sent to the terminal**, with or without a selection, because the selection is already on the clipboard. This prevents a highlighted selection from intercepting Ctrl+C when the user intends to interrupt Claude Code.

When `copyOnSelect` is off, Ctrl+C copies if text is selected and otherwise is sent to the terminal.

### 9.6 Keyboard shortcuts

All application shortcuts use Ctrl+Shift or Alt+Shift. Plain Ctrl combinations are always delivered to the terminal, because Claude Code and shells use them (Claude Code binds Ctrl+T to its todo list; readline binds Ctrl+K to delete to the end of the line).

| Shortcut | Action |
|---|---|
| Ctrl+Shift+P | Switcher: fuzzy search over sessions, projects and actions |
| Ctrl+Shift+J | Jump to the next session in `needs_you`, then the next in `done` |
| Ctrl+Shift+T | New Claude session in the current project (new tab) |
| Ctrl+Shift+N | New shell in the current project (new tab) |
| Alt+Shift+= | Split focused pane right (new Claude session) |
| Alt+Shift+- | Split focused pane down (new Claude session) |
| Alt+Shift+Z | Zoom focused pane |
| Alt+Arrow | Move focus between panes |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab |
| Ctrl+Shift+W | Close focused pane |
| Ctrl+Shift+C | Copy selection |
| Ctrl+Shift+V | Paste |
| Ctrl+C | Section 9.5, rule 5 |

### 9.7 Status bar

Shows the focused session's machine label, project path, and the shortcut hint `Ctrl+Shift+P`. Git branch display is out of scope for version 1 (section 1.2, item 5).

### 9.8 Terminal rendering

xterm.js with `@xterm/addon-webgl` (GPU renderer), `@xterm/addon-fit` (sizes the terminal to the pane), `@xterm/addon-unicode11` (correct emoji and CJK widths) and `@xterm/addon-web-links` (clickable URLs).

### 9.9 Theming

- A theme is one JSON file in `app/src/themes/`, for example `neon-glass.json`, with two keys:
  - `tokens`: interface values (colors, glows, radii, font stacks, badge animation names), applied to `:root` as `--jojo-*` CSS custom properties.
  - `terminal`: the xterm.js palette (16 ANSI colors, foreground, background, cursor, cursor accent, selection background), passed to the xterm.js `theme` option.
- Components use only `var(--jojo-*)` values. A lint check fails the build when a `.svelte` file outside `app/src/themes/` contains a hex, `rgb()`, `rgba()`, `hsl()` or named color literal.
- Version 1 ships `neon-glass.json` only. The Neon Glass values are those in the approved mockup: background `#070a16` with violet (`rgba(155,140,255,.35)`) and teal (`rgba(62,230,176,.18)`) radial glows; accent `#9b8cff`; working `#7fd4ff`; needs-you `#ffb347`; done `#3ee6b0`; error `#ff5c7a`; foreground `#eef1ff`; muted `#7d87b3`.

### 9.10 Settings

`%APPDATA%\Jojo\settings.json`:

| Key | Default |
|---|---|
| `theme` | `"neon-glass"` |
| `terminalFontFamily` | `"Geist Mono"` |
| `terminalFontSize` | `13` |
| `terminalLineHeight` | `1.25` |
| `scrollbackLines` | `10000` |
| `copyOnSelect` | `true` |
| `toasts` | `true` |
| `defaultShell` | `"pwsh"` |

The settings screen also provides **Restart daemon (This PC)** and **Debug ▸ Rehydrate pane** (section 10).

## 10. Error Handling

| Failure | Detection | Behavior |
|---|---|---|
| Local daemon crashes | Pipe closes unexpectedly | The app restarts `jojod.exe` once. Sessions that were running are shown as `exited` with Resume or Restart (section 5.4). A banner states "Jojo daemon restarted; N sessions ended". If the restart also fails, the banner shows the log path. |
| Remote connection drops | `ssh` process exits or keepalive fails | Section 8.3 |
| SSH authentication fails on reconnect | `ssh` exit code 255 with "Permission denied" in stderr | Section 8.3, rule 5 |
| Process fails to spawn | `portable-pty` spawn error (for example `claude` not on `PATH`) | The pane displays the error and the `PATH` searched; the session moves to `exited` with exit code `-1` |
| `state.json` is unreadable | JSON parse error at daemon start | Rename to `state.json.corrupt-<unix timestamp>` and start with empty state |
| Snapshot serialization defect | Visual mismatch noticed by the user | Live output continues after the snapshot. **Debug ▸ Rehydrate pane** requests a new snapshot for the focused pane |
| Hook relay cannot reach the daemon | Connect timeout (500 ms) | Exits 0 silently (section 7.1) |
| Protocol version mismatch | `Hello` exchange | Section 8.4 |

**Logging.** Both binaries log with `tracing`, rotated daily, keeping 7 files: `%LOCALAPPDATA%\Jojo\logs\jojo.log` and `jojod.log` on Windows, and `~/.jojo/jojod.log` on Linux. Logs record metadata (session ids, state transitions, connection events, errors) and never record terminal input or output bytes.

## 11. Testing

1. **Snapshot fidelity.** This module carries the highest risk and gets the most tests.
   - Golden tests feed recorded byte streams into an `alacritty_terminal` grid, serialize a snapshot, replay the snapshot into a fresh grid, and assert both grids are identical cell by cell (characters, attributes, colors), including cursor position, cursor style and terminal modes.
   - Recordings: a Claude Code permission prompt; vim on the alternate screen; 10,000 lines of scrollback; emoji and CJK text; 24-bit color; a session mid-way through a Claude Code redraw.
   - A cross-emulator check replays each snapshot into `@xterm/headless` and compares visible text with the Rust grid, to detect interpretation differences between `alacritty_terminal` and xterm.js.
2. **Agent state machine.** Table tests of `(state, event, now) → state` for every row in section 7.2 and every rule in section 7.3, including permission-prompt ordering and the 10-second silence fallback with an injected clock.
3. **Protocol.** Encode/decode round-trip tests for every message. A CI check fails if the generated TypeScript types differ from the committed ones.
4. **Daemon integration.** Run `jojod` in-process on a temporary pipe (Windows) or socket (Linux). Create a shell session, send `echo hi`, assert `Output` contains `hi`, detach, attach, and assert the `Snapshot` contains `hi`. Runs on Windows and on Linux (WSL or CI).
5. **Remote installation.** A scripted test against a Docker container running `sshd` exercises section 8.2 end to end, including upgrading an older `jojod`.
6. **Frontend.** Vitest unit tests for split-tree operations (split, close, swap, move, zoom, layout presets, equalize, 4-pane limit) and shortcut resolution, including the Ctrl+C rules in section 9.5. UI behavior is verified manually against `docs/manual-test-checklist.md`.

## 12. Repository Layout

```
terminal-app/
├─ Cargo.toml                 # Cargo workspace
├─ crates/
│  ├─ jojo-proto/             # Frame codec, message structs, ts-rs exports
│  ├─ jojo-grid/              # alacritty_terminal wrapper, snapshot serializer, golden tests
│  ├─ jojo-agent/             # Agent state machine (pure)
│  └─ jojod/                  # Daemon binary: serve, attach, hook, install-hooks, shutdown
├─ app/
│  ├─ src-tauri/              # Tauri backend: machine manager, frame router, toasts, single instance
│  ├─ src/                    # Svelte 5 + TypeScript frontend
│  │  ├─ lib/protocol/        # Generated TypeScript types (ts-rs output)
│  │  ├─ lib/layout/          # Split-tree model and operations
│  │  ├─ components/          # Sidebar, TabBar, Pane, Menu, Switcher, Settings
│  │  └─ themes/neon-glass.json
│  └─ package.json
├─ tests/
│  ├─ recordings/             # Byte-stream fixtures for snapshot tests
│  └─ remote/                 # Docker sshd test harness
└─ docs/
   ├─ superpowers/specs/
   └─ manual-test-checklist.md
```

## 13. Toolchain

Installed on the development PC as of 2026-11-06: Node 22.22.0, pnpm 10.33.0, git 2.45.1, Claude Code 2.1.281.

Required and not yet installed:

1. Rust via `rustup` (stable, `x86_64-pc-windows-msvc`).
2. Visual Studio 2022 Build Tools with the "Desktop development with C++" workload (MSVC linker and Windows SDK).
3. WebView2 Runtime (included with Windows 11).
4. Linux daemon builds: `cargo-zigbuild` and Zig, targeting `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, so both static binaries build on Windows without Docker.
5. Docker Desktop, for the remote installation test only.

## 14. Risks

1. **ConPTY output rewriting.** ConPTY maintains its own console buffer and re-emits VT sequences rather than passing a program's bytes through unchanged. Known effects include redundant redraws after resize and altered sequence ordering. Mitigation: bundle a current `conpty.dll` and `OpenConsole.exe` from the Microsoft Windows Terminal project, as WezTerm does, instead of relying on the version in Windows.
2. **Emulator divergence.** The daemon's grid (`alacritty_terminal`) and the window's renderer (xterm.js) are different emulators. A sequence one supports and the other does not produces a snapshot that differs from what the user saw. Mitigation: the cross-emulator test in section 11, item 1, and the Rehydrate pane action.
3. **Hook schema changes.** Claude Code hook names, `notification_type` values and payload fields may change between releases. Mitigation: the relay forwards raw JSON, and all interpretation lives in `jojo-agent`, which ignores unknown events and fields.
4. **Silence fallback misfires.** A Claude Code tool that runs for more than 10 seconds without producing terminal output could be marked `idle` while still working. The next `PostToolUse` event corrects the state. If this proves common, the timeout becomes a setting.
