# Jojo

Jojo is a desktop terminal application for running and supervising many
Claude Code sessions at once. It combines a terminal emulator and a terminal
multiplexer in one window: sessions are grouped by location and project,
each project has tabs, and each tab holds up to four split panes.

Jojo has four defining properties:

1. **Sessions outlive the window.** Every terminal session is owned by a
   background daemon, `jojod`, not by the window. Closing, crashing or
   restarting the window does not stop any session, and reopening it repaints
   every pane as it was.
2. **Sessions report their Claude Code state.** Claude Code hooks tell the
   daemon whether each session is starting, working, waiting for the user
   ("needs you"), done or in error. The window shows the state on every pane,
   tab and sidebar row, and sends a desktop notification when a session you
   are not looking at changes state.
3. **Remote locations.** Linux servers reachable over SSH can be added as
   remote locations. Jojo installs `jojod` on the server, and the server's
   sessions appear in the same sidebar as local ones.
4. **Themes.** The interface is HTML and CSS, so every color, font, border,
   blur and glow is a theme token.

Jojo is a personal tool. Windows 11 is the primary platform; a macOS build
exists but has not been verified on a real Mac (see [ROADMAP.md](ROADMAP.md)).

## Installing

Download `Jojo_<version>_x64-setup.exe` from the
[Releases](https://github.com/josephmcgarvey/Jojo/releases) page and run it.
The installer is not code-signed, so Windows SmartScreen shows "Windows
protected your PC" the first time: choose **More info**, then **Run anyway**.

On first start, the app copies `jojod.exe` and its ConPTY files into its own
folder, starts the daemon, and registers Jojo's hooks in Claude Code's
`~/.claude/settings.json` (see [Claude Code hooks](#claude-code-hooks)).

Installing a new version over an older one replaces the daemon when the
protocol version changed. Sessions running in the old daemon end when it
stops. With **Settings ▸ Restore sessions** on, Claude Code sessions are
resumed in the new daemon and keep their names.

## Using Jojo

### Layout

The sidebar lists locations (**This PC** and each remote location), the
projects under each location, and the sessions in each project. A project is
a folder on its location; new sessions in a project start in that folder.

- **Needs you queue.** The box at the top of the sidebar lists every session
  waiting for the user, from every location, longest wait first. Clicking a
  row focuses that session's pane.
- **Session rows.** Each row shows the session's state badge, name, time in
  its current state and, for Claude Code sessions, a context meter. The meter
  shows how full Claude's context window is; it turns amber at 70% and red at
  85%, the point at which running `/compact` is advisable. The window size is
  inferred from the model name and token count (200,000 or 1,000,000 tokens).
- **Tabs.** Each tab is a card for its focused session: name, time in state,
  the state word in its color, and what the session is asking (for example a
  permission prompt) or its project name.
- **Panes.** Each pane has a colored edge in its session's state color.
  Panes that need the user pulse.
- **State strip.** The bar at the bottom counts sessions per state.

The sidebar's right edge can be dragged to resize it (200 to 480 pixels);
double-clicking the edge returns it to the default width of 268 pixels.

Locations and projects fold. Clicking a location's name folds or unfolds
its projects; the chevron that appears left of a project's name when the
pointer is over it folds or unfolds its sessions (Alt+click folds or
unfolds every project of that location). With a location or project name
focused, Left folds and Right unfolds it. A folded row shows one small dot
per session that is working, needs you, done or in error, in that state's
color, and a session that needs you keeps pulsing; a folded location also
shows its project count. Choosing a session from the Needs you queue, the
switcher or Jump unfolds its location and project. Folded groups are saved
in `layout.json`.

### Common actions

| Action | Windows | macOS |
|---|---|---|
| New Claude Code tab | Ctrl+Shift+T | ⌘T |
| New shell tab | Ctrl+Shift+N | ⌘N |
| Split right | Alt+Shift+= | ⌘D |
| Split down | Alt+Shift+- | ⇧⌘D |
| Zoom the focused pane | Alt+Shift+Z | ⇧⌘Return |
| Move focus between panes | Alt+Arrow keys | ⌥⌘Arrow keys |
| Next and previous tab | Ctrl+Tab, Ctrl+Shift+Tab | ⇧⌘], ⇧⌘[ |
| Close pane | Ctrl+Shift+W | ⌘W |
| Copy, paste | Ctrl+Shift+C, Ctrl+Shift+V | ⌘C, ⌘V |
| Switcher and output search | Ctrl+Shift+P | ⌘P |
| Jump to the next session that needs you | Ctrl+Shift+J | ⌘J |

Both columns come from `app/src/lib/input/shortcuts.ts`.
Right-clicking a location, project, session, tab or pane header opens a menu with the same
actions plus rename, close and remove.

Other interactions:

1. Drag a tab to reorder tabs; drag a project to reorder projects.
2. Double-click a tab, project or session name to rename it.
3. Typing `exit` in a shell closes its pane.

### Searching output

The switcher (Ctrl+Shift+P) searches session names and commands, and its
**In output** section searches the terminal output of every session on every
connected location, including scrollback. Search is plain text and
case-insensitive, needs at least 2 characters, and returns up to 20 matches
per session and 200 in total, newest first. Choosing a match focuses the
session's pane and scrolls to the line.

### Snippets

The snippets library holds commands you use often. **Snippets** in the tab
row opens it; the switcher (Ctrl+Shift+P) also lists the snippets for the
focused terminal.

1. Clicking a snippet types it into the focused terminal without pressing
   Enter, so it can be checked or edited first. **Run** types it and presses
   Enter. **Copy** puts it on the clipboard. In the list, Up and Down move
   the highlight, Enter inserts and Ctrl+Enter (⌘Enter on macOS) runs.
2. A snippet can span several lines. It is typed as one paste, so a shell
   with bracketed paste (PowerShell 7, zsh, bash 5.1 and later) receives the
   whole block before running anything.
3. **New snippet** in the list creates one. Selecting text in a terminal,
   right-clicking it and choosing **Save as snippet…** creates one from the
   selection.
4. Each snippet has a name, a command, an optional folder (shown as a heading
   in the list) and a **Show for** setting: every location and project, only
   terminals on one location, or only terminals in one project. The list
   shows the snippets for the focused terminal; a checkbox at its foot shows
   the rest.

Snippets are saved in `snippets.json` (see [Files](#files)). The window
reads and writes the file and the daemon never does, so snippets belong to
one computer and are not shared between a PC and a Mac.

### Remote locations

**Remote location** at the bottom of the sidebar adds a Linux server
(x86_64 or aarch64). The dialog offers the host aliases from
`~/.ssh/config`. Authentication uses your existing SSH setup (keys or agent);
Jojo runs the system `ssh` and stores no passwords.

Adding a location runs these steps over SSH and reports each one:

1. **Connect:** checks that `ssh <host>` works without a password prompt
   (`ssh -o BatchMode=yes`).
2. **Platform:** reads `uname -sm` to choose the x86_64 or aarch64 daemon.
3. **Install:** uploads the daemon to `~/.local/bin/jojod`, or upgrades an
   older one.
4. **Start:** starts it with `jojod serve --daemonize` through the user's
   login shell.
5. **Linger:** checks `loginctl` lingering. Without it, systemd may end the
   daemon and its sessions when the user's last SSH session closes; the step
   warns but does not stop the install.
6. **Hooks:** registers Jojo's hooks in the server's
   `~/.claude/settings.json`; warns if Claude Code is not installed there.

When adding a project on a remote location, the dialog browses the server's
folders. The remote daemon keeps running when the window disconnects, and the
window reattaches when the connection returns.

### Settings

**Settings** (the gear at the bottom of the sidebar) controls the theme, base
color, terminal and interface fonts, scrollback length, copy on select,
notifications and sounds, the default shell, and session restore.

Jojo has seven themes:

| Theme | Mode | Description |
|---|---|---|
| Neon Glass | Dark | Translucent glass panels with blur and glow, and a violet accent. The default. |
| Prism | Dark | Neon Glass with an animated rainbow: working sessions (their panes, badges and state words), the focused pane, the selected tab, the selected session, the logo and the background glow cycle through the color wheel every 12 seconds. |
| Daylight | Light | The Neon Glass layout in light colors. |
| Graphite | Dark | Flat and matte, gray throughout, color only for session states. |
| Phosphor | Dark | Green-screen terminal: square corners, text glow, monospace interface font. |
| Porcelain | Light | Solid white panels with soft shadows and a cobalt accent. |
| Contrast | Dark | Black and white, 2-pixel borders, yellow focus and orange "needs you". |

The **base color** setting recolors the accent of any theme. The resulting
colors are adjusted to keep text readable (at least 4.5:1 contrast for
accent-colored text). The **Interface font** option **Theme default** uses
each theme's own font; any other choice applies to every theme. Prism's
rainbow colors do not follow the base color, and they stop moving when
Windows' **Show animations** setting is off (`prefers-reduced-motion`).

## Claude Code hooks

The app runs `jojod install-hooks` on every start. It merges entries for the
`SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`,
`PermissionRequest`, `Notification`, `Elicitation`, `Stop`, `StopFailure` and
`SessionEnd` events into `~/.claude/settings.json`, each running
`jojod hook`. The merge is idempotent and leaves existing hooks in place.

`jojod hook` reads the hook's JSON from standard input and forwards it to the
daemon for the session named in the `JOJO_SESSION_ID` environment variable,
which jojod sets in every session it starts. Outside Jojo the variable is
unset and the hook exits immediately, so the hooks have no effect on Claude
Code sessions started in other terminals. The daemon also reads the end of
the session's transcript file (512 KiB) to compute the context meter.

To remove the hooks, run `jojod uninstall-hooks`.

## Files

| File | Location (Windows) | Location (macOS) | Contents |
|---|---|---|---|
| Settings | `%APPDATA%\Jojo\settings.json` | `~/Library/Application Support/Jojo/` | The values on the Settings screen. |
| Locations | `%APPDATA%\Jojo\machines.toml` | same folder | Remote locations. |
| Layout | `%APPDATA%\Jojo\layout.json` | same folder | Projects' order, tabs and split layouts. |
| Snippets | `%APPDATA%\Jojo\snippets.json` | same folder | The snippets library. |
| Daemon state | `%LOCALAPPDATA%\Jojo\state.json` | `~/.jojo/state.json` | Projects and session records. |
| Daemon endpoint | `\\.\pipe\jojo-<username>` | `~/.jojo/jojod.sock` | The connection between window and daemon. |

On remote locations, the daemon is `~/.local/bin/jojod` and its state and
socket are in `~/.jojo/`. A corrupt `layout.json` or `machines.toml` is
renamed to `<name>.corrupt-<unix milliseconds>` and replaced with defaults.

## Architecture

Jojo consists of the window and the daemon. The repository is a Cargo
workspace plus a Svelte frontend:

| Path | Contents |
|---|---|
| `app/` | The window: Tauri 2, Svelte 5, TypeScript and xterm.js 6 (WebGL renderer). |
| `app/src-tauri/` | The window's Rust side (`jojo-app`): daemon start-up, SSH connections, remote install, settings, layout and notifications. |
| `crates/jojod/` | The daemon: owns every PTY (ConPTY on Windows, Unix PTYs elsewhere), serves the window over a named pipe or Unix socket, and relays hooks. |
| `crates/jojo-grid/` | A terminal grid per session (`alacritty_terminal` 0.25). The daemon serializes it to VT sequences so a reattaching window repaints exactly, and searches it for output search. |
| `crates/jojo-proto/` | The window–daemon protocol (version 3): length-prefixed frames carrying JSON control messages and binary terminal output. TypeScript types are generated with ts-rs into `app/src/lib/protocol/`. |
| `crates/jojo-agent/` | Session state from hook events, and context usage from transcripts. |
| `resources/conpty/` | Hashes of the pinned ConPTY build (`conpty.dll`, `OpenConsole.exe`), fetched by `scripts/fetch-conpty.ps1`. |
| `docs/superpowers/` | The design specifications and implementation plans. |

The window holds no terminal state. It creates an xterm.js instance only for
panes in the visible tab, so background sessions cost no rendering work.
Remote locations are reached by running `ssh <host> ~/.local/bin/jojod attach
--stdio`, which relays the same protocol over the SSH connection.

## Building from source

### Prerequisites (Windows)

1. **Rust**, stable toolchain (`rust-toolchain.toml`), with the MSVC build
   tools.
2. **Node.js** 22 and **pnpm** 10.
3. **PowerShell 7** (`pwsh`), used by the staging scripts.
4. For remote locations: **cargo-zigbuild** (`cargo install cargo-zigbuild
   --locked`), **Zig**, and the Rust targets `x86_64-unknown-linux-musl` and
   `aarch64-unknown-linux-musl`. Without them a debug staging warns and
   remote locations are unavailable; a release build fails.

### Commands

```powershell
pnpm --dir app install

# Build jojod, fetch and verify ConPTY, build the Linux daemons, and copy
# them into app/src-tauri/resources/daemon.
pnpm --dir app stage

# Run the app in development mode.
pnpm --dir app tauri dev

# Build the installer: target\release\bundle\nsis\Jojo_<version>_x64-setup.exe
pnpm --dir app tauri build
```

`tauri build` stages a release daemon itself (`beforeBuildCommand` in
`app/src-tauri/tauri.conf.json`). Run `tauri dev` and the built app from a
normal, non-elevated terminal.

### macOS

`.github/workflows/macos.yml` builds unsigned `.dmg` installers for Apple
Silicon and Intel. It runs only when started by hand:

```sh
gh workflow run macos.yml -R josephmcgarvey/Jojo --ref master
```

Locally, `scripts/stage-daemon.sh` replaces `stage-daemon.ps1`.

## Testing

```powershell
cargo test --workspace            # Rust unit and integration tests
pnpm --dir app test               # Frontend tests (Vitest)
pnpm --dir app check              # Svelte and TypeScript type check
pnpm --dir app lint:colors        # No hard-coded colors outside theme files
pwsh scripts/check-protocol.ps1   # Generated TypeScript types match the Rust protocol
```

Tests use temporary directories and never touch the real
`~/.claude/settings.json`, `machines.toml` or `layout.json`. Tests against a
real SSH server are opt-in: set `JOJO_SSH_TEST_HOST` to a host alias.
`scripts/run-linux-tests.sh` runs the Linux test executables built with
`cargo zigbuild --tests` inside WSL. `docs/manual-test-checklist.md` lists
the manual checks to run before tagging a build.

## Status

The current release is 0.2.0. Postponed work is listed in
[ROADMAP.md](ROADMAP.md). The project has no license; all rights reserved.
