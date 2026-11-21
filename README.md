<h1 align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/jojo-logo.svg">
    <img src="assets/jojo-logo-light.svg" alt="Jojo" height="64">
  </picture>
</h1>

Jojo is a desktop terminal application for running and supervising many
Claude Code sessions at once, on your own computer and on Linux servers over
SSH. It combines a terminal emulator and a terminal multiplexer in one
window: sessions are grouped by location and project, each project has tabs,
and each tab holds up to four split panes. Jojo runs on Windows 11 and macOS
11 or later.

Jojo has four defining properties:

1. Sessions outlive the window. Every terminal session is owned by a
   background daemon, `jojod`, not by the window. Closing, crashing or
   restarting the window does not stop any session, and reopening it repaints
   every pane as it was.
2. Sessions report their Claude Code state. Claude Code hooks tell the daemon
   whether each session is starting, working, waiting for the user ("needs
   you"), done or in error. The window shows the state on every pane, tab and
   sidebar row, and sends a desktop notification when a session you are not
   looking at changes state.
3. Linux servers can be added as remote locations. Jojo installs `jojod` on
   the server, and the server's sessions appear in the same sidebar as local
   ones.
4. The interface is HTML and CSS, so every color, font, border, blur and glow
   is a theme token. Jojo ships seven themes.

## Installing

Installers are attached to each release on the
[Releases](https://github.com/josephmcgarvey/Jojo/releases) page.

| File | Computer |
|---|---|
| `Jojo_<version>_x64-setup.exe` | Windows 11, 64-bit |
| `Jojo_<version>_aarch64.dmg` | Mac with Apple Silicon (M1 and later) |
| `Jojo_<version>_x64.dmg` | Mac with an Intel processor |

### Windows

The quickest way to install or upgrade is this command in PowerShell:

```powershell
irm https://raw.githubusercontent.com/josephmcgarvey/Jojo/master/scripts/install-windows.ps1 | iex
```

The script (`scripts/install-windows.ps1`) downloads the latest release's
installer, closes Jojo's window if it is open (sessions keep running in the
daemon), runs the installer silently for the current user into
`%LOCALAPPDATA%\Jojo` without an administrator prompt, and opens Jojo.
Because the installer is downloaded by PowerShell rather than a browser, it
carries no "downloaded from the internet" mark, so SmartScreen does not stop
it. Set `$env:JOJO_VERSION = "0.2.5"` first to install a specific release.

To install by hand instead:

1. Run `Jojo_<version>_x64-setup.exe`.
2. The installer is not code-signed, so Windows SmartScreen shows "Windows
   protected your PC". Choose **More info**, then **Run anyway**.

### macOS

The quickest way to install or upgrade is this command in Terminal:

```sh
curl -fsSL https://raw.githubusercontent.com/josephmcgarvey/Jojo/master/scripts/install-mac.sh | bash
```

The script (`scripts/install-mac.sh`) downloads the latest release's `.dmg`
for your processor, quits Jojo if it is running (sessions keep running in the
daemon), installs `Jojo.app` into Applications and opens it. Because the file
is downloaded with `curl` rather than a browser, macOS does not block the
first launch. Set `JOJO_VERSION=0.2.5` before `bash` to install a specific
release.

To install from the `.dmg` by hand instead:

1. Open the `.dmg` for your Mac and drag **Jojo** into **Applications**.
2. The app is not signed with an Apple developer certificate, so macOS blocks
   the first launch. Open **System Settings ▸ Privacy & Security** and choose
   **Open Anyway**, or run this in Terminal:

   ```sh
   xattr -dr com.apple.quarantine /Applications/Jojo.app
   ```

### First start and upgrades

On first start, the app copies its daemon into place, starts it, and
registers Jojo's hooks in Claude Code's `~/.claude/settings.json` (see
[Claude Code hooks](#claude-code-hooks)). On macOS the daemon is copied to
`~/.local/bin/jojod` and started through your login shell, so it finds
programs installed with Homebrew, nvm or shell aliases.

Installing a new version over an older one keeps running sessions when the
protocol between the window and the daemon is unchanged, which is true of
every 0.2.x release. When the protocol changes, the window reports that the
daemon is outdated and offers to restart it, which ends its sessions. With
**Settings ▸ Restore sessions** on, they are brought back in the new daemon
and keep their names.

## Using Jojo

### Layout

The sidebar lists locations (**This PC** or **This Mac**, then each remote
location), the projects under each location, and the sessions in each
project. A project is a folder on its location; new sessions in a project
start in that folder. Only the list scrolls: the JOJO logo at the top of the
sidebar and the footer (**Remote location** and Settings) stay in place.

1. The **Needs you** queue at the top of the sidebar lists every session
   waiting for the user, from every location, longest wait first. Clicking a
   row focuses that session's pane.
2. Each session row shows the session's state badge, name, time in its
   current state and, for Claude Code sessions, a context meter. A shell
   session without Claude Code has no hook-driven states, so its badge is a
   terminal icon instead of a state circle. The meter
   shows how full Claude's context window is; it turns amber at 70% and red
   at 85%. The window size is inferred from the model name and token count
   (200,000 or 1,000,000 tokens).
3. Each tab is a card for its focused session: name, time in state, the
   state word in its color, and what the session is asking (for example a
   permission prompt) or its project name. Right of the cards is **+** (a
   new terminal).
4. Each pane has a border in its session's state color. Panes that need the
   user pulse. A bar along the bottom of each pane, under the terminal where
   the prompt is, shows the session's state badge, name and project, and the
   state word with its time while the session is working, needs you, done
   or failed.
5. The state strip at the bottom of the window has one segment per session
   that has not exited, from every location, grouped by location and colored
   by state, with counts per state at its right. The focused session's
   segment is taller, and a flagged session's segment carries a pennant.
   Clicking a segment focuses that session.

The sidebar's logo row also holds three buttons: the sidebar button, Snippets
(a command-prompt icon) and Search (a magnifier). The sidebar's right edge can
be dragged to resize it (200 to 480 pixels); double-clicking the edge returns
it to the default width of 268 pixels.

The sidebar can be hidden so the panes take the full width: click the sidebar
button, press Ctrl+Shift+B (⌘B on macOS), or drag the sidebar's edge to the
left edge of the window. While it is hidden, the three buttons sit at the left
end of the tab row, and the sidebar button there shows it again at its saved
width. Every session's state stays visible in the state strip. Jojo remembers
whether the sidebar was hidden.

Locations and projects fold. A chevron appears at the left of a location or
project row when the pointer is over it; clicking it folds or unfolds the
rows beneath, and Alt+click on a project's chevron folds or unfolds every
project of that location. Clicking a location's name also folds it; clicking
a project's name selects the project. With a location or project name
focused, Left folds and Right unfolds it. A folded row shows one small dot
per session that is working, needs you, is done or failed, in that state's
color, and a session that needs you keeps pulsing; a folded location also
shows its project count. Choosing a session from the Needs you queue, the
switcher or Jump unfolds its location and project. Folded groups are saved
in `layout.json`.

### Keyboard shortcuts

| Action | Windows | macOS |
|---|---|---|
| New Claude Code tab | Ctrl+Shift+T | ⌘T |
| New shell tab | Ctrl+Shift+N | ⌘N |
| Split right (new shell) | Alt+Shift+= | ⌘D |
| Split down (new shell) | Alt+Shift+- | ⇧⌘D |
| Zoom the focused pane | Alt+Shift+Z | ⇧⌘Return |
| Move focus between panes | Alt+Arrow keys | ⌥⌘Arrow keys |
| Next and previous tab | Ctrl+Tab, Ctrl+Shift+Tab | ⇧⌘], ⇧⌘[ |
| Close pane | Ctrl+Shift+W | ⌘W |
| Copy, paste | Ctrl+Shift+C, Ctrl+Shift+V | ⌘C, ⌘V |
| Switcher and output search | Ctrl+Shift+P | ⌘P |
| Jump to the next session that needs you | Ctrl+Shift+J | ⌘J |
| Hide or show the sidebar | Ctrl+Shift+B | ⌘B |

Right-clicking a location, project, session, tab or pane name bar opens a menu
with the actions for it, including split, zoom, swap, move, layout presets,
rename, restart, resume and remove. An item that cannot apply is grayed out,
and hovering it shows the reason. Other interactions:

1. Drag a tab to reorder tabs; drag a project to reorder projects.
2. Double-click a tab, project or session name to rename it.
3. Typing `exit` in a shell closes its pane.

### Compacting a Claude conversation

Right-click a Claude Code session's context meter or row in the sidebar and
choose **Compact conversation**. Jojo types `/compact` and Enter into the
session. The item is disabled while Claude is starting, working or waiting
for an answer, because the typed text would then interrupt the turn or answer
the prompt on screen.

### Searching output

The switcher (Ctrl+Shift+P, or the magnifier in the tab row) searches
session names, projects, actions and snippets, and its **In output** section
searches the terminal output of every session on every connected location,
including scrollback. Output search is plain text and case-insensitive,
needs at least 2 characters, and returns up to 20 matches per session and 200
in total, newest first. Choosing a match focuses the session's pane and
scrolls to the line.

### Flagging a session for later

**Flag for later** in a session's right-click menu (in the sidebar or on its
pane's name bar) marks the session to come back to. The Needs you and Done
states clear when the session is clicked; a flag stays until **Remove flag**
in the same menu, or until the session is removed.

A flagged session shows a pennant in the theme's accent color on its sidebar
row, its pane's name bar, its tab card and its segment in the state strip. A
folded location or project shows the pennant when any session it hides is
flagged. Flags are saved in `layout.json` on each computer, so they are not
shared between a PC and a
Mac.

### Snippets

The snippets library holds commands you use often. The command-prompt icon
in the tab row opens it; the switcher also lists the snippets for the
focused terminal.

1. Clicking a snippet types it into the focused terminal without pressing
   Enter, so it can be checked or edited first. **Run** types it and presses
   Enter. **Copy** puts it on the clipboard. **Edit** changes or deletes it.
   In the list, Up and Down move the highlight, Enter inserts and Ctrl+Enter
   (⌘Enter on macOS) runs.
2. A snippet can span several lines. It is typed as one paste, so a shell
   with bracketed paste (PowerShell 7, zsh, bash 5.1 and later) receives the
   whole block before running anything.
3. **New snippet** in the list creates one. Selecting text in a terminal,
   right-clicking the pane's name bar and choosing **Save as snippet…** creates
   one from the selection.
4. Each snippet has a name, a command, an optional folder (shown as a heading
   in the list) and a **Show for** setting: every location and project, only
   terminals on one location, or only terminals in one project. The list
   shows the snippets for the focused terminal; a checkbox at its foot shows
   the rest.

A snippet's command can contain placeholders: `{{name}}`, or
`{{name:default}}` with a default value. Inserting or running such a snippet
first opens a form with one field per placeholder name, a preview of the
command as it will be typed, and **Insert** and **Run** buttons. Each field
starts with the value last used for that snippet, else the default. A name
used several times is asked for once and fills every use. Text in double
braces that is not a single name (for example `{{ two words }}` or
`${{ secrets.TOKEN }}`) is typed as written. **Copy** copies the command with
its placeholders unfilled.

Snippets are saved in `snippets.json` (see [Files](#files)). The window reads
the file when it starts and rewrites it after every change, so edit it by
hand only while Jojo is closed. Snippets belong to one computer; to copy them
to another, use **Export configuration…** and **Import configuration…** (see
[Moving a setup to another computer](#moving-a-setup-to-another-computer)).

### Remote locations

**Remote location** at the bottom of the sidebar adds a Linux server
(x86_64 or aarch64). Jojo runs the system `ssh`, and the dialog's
**Sign in with** setting offers two ways in:

1. **SSH key**: a host alias from `~/.ssh/config` (or `user@host`) whose key
   is loaded in the SSH agent, with its host key accepted once. The Host
   list has one row per `Host` line. OpenSSH treats every name on a `Host`
   line as an alias of the same server, so a line such as
   `Host myserver 203.0.113.5` is one row, `myserver (203.0.113.5)`, and
   Jojo connects with the first name.
2. **Username and password**: the server's host name or address, port,
   username and password. With **Set up an SSH key** checked (the default),
   Jojo creates `~/.ssh/jojo_ed25519` if it does not exist, adds its public
   key to the server's `~/.ssh/authorized_keys` over the password
   connection, and checks that the key signs in. From then on the location
   uses the key and the password is not saved. When the server does not
   accept the key, or the box is unchecked, Jojo keeps signing in with the
   password, which it saves in Windows Credential Manager or the macOS
   Keychain (never in `machines.toml`) and gives to `ssh` through its own
   password helper (`SSH_ASKPASS`, OpenSSH 8.4 or later). A new server's host
   key is accepted and recorded in `known_hosts` on first connection.

The server account needs
a normal login shell such as `/bin/bash`: a shell that runs each SSH
connection in its own temporary container, such as RunCloud's jailed shell
(`/bin/rc-shell`), cannot keep the daemon running.

Adding a location runs these steps over SSH and reports each one:

1. Connect: checks that `ssh <host>` works without a password prompt
   (`ssh -o BatchMode=yes`).
2. Platform: reads `uname -sm` to choose the x86_64 or aarch64 daemon.
3. Install: uploads the daemon to `~/.local/bin/jojod`, or upgrades an
   older one.
4. Start: starts it with `jojod serve --daemonize` through the account's
   login shell.
5. Linger: checks `loginctl` lingering. Without it, systemd may end the
   daemon and its sessions when the account's last SSH session closes; the
   step warns but does not stop the install.
6. Hooks: registers Jojo's hooks in the server's `~/.claude/settings.json`,
   and warns when Claude Code is not installed there.

Adding a location that is already listed upgrades the daemon binary and
reconnects, but a daemon that is already running keeps running its old code
until it restarts. A daemon older than the app then reports actions it does
not know, such as renaming a project, as "This jojod is older than the app".
Right-clicking the location and choosing **Update jojod to (version)…**
(shown as **Restart jojod…** when the versions match) installs the app's
daemon when the server has another version, stops the running daemon with
`jojod shutdown`, which ends that location's sessions, and reconnects, which
starts the new daemon. The confirmation lists the sessions that end.

When adding a project on a remote location, the dialog browses the server's
folders. The remote daemon keeps running when the window disconnects, and the
window reattaches when the connection returns. Any computer that adds the
same server sees the same projects and running sessions.

### Settings

**Settings** (the gear at the bottom of the sidebar) controls the theme, base
color, terminal and interface fonts, scrollback length, copy on select,
notifications and sound, the default shell, session restore, and naming
sessions from Claude's first prompt.

Jojo has seven themes:

| Theme | Mode | Description |
|---|---|---|
| Neon Glass | Dark | Translucent glass panels with blur and glow, and a violet accent. The default. |
| Prism | Dark | Neon Glass with an animated rainbow: working sessions, the focused pane, the selected tab and session, the logo, dialog glows and the background glow cycle through the color wheel every 12 seconds. An aurora of soft, streaked glows drifts slowly behind the window. Both animations pause while the Jojo window is not the active window. |
| Daylight | Light | The Neon Glass layout in light colors. |
| Graphite | Dark | Flat and matte, gray throughout, color only for session states. |
| Phosphor | Dark | Green-screen terminal: square corners, text glow, monospace interface font. |
| Porcelain | Light | Solid white panels with soft shadows and a cobalt accent. |
| Contrast | Dark | Black and white, 2-pixel borders, yellow focus and orange "needs you". |

The base color setting recolors the accent of any theme. The resulting colors
are adjusted to keep text readable (at least 4.5:1 contrast for
accent-colored text). The interface font option **Theme default** uses each
theme's own font; any other choice applies to every theme. Prism's rainbow
does not follow the base color, and animations stop when reduced motion is
on (Windows **Show animations** off, macOS **Reduce motion** on).

### Moving a setup to another computer

**Settings ▸ Export configuration…** saves one `jojo-config.json` file, and
**Settings ▸ Import configuration…** on the other computer reads it. The file
holds:

1. The settings, except the default shell, which names a program on the
   exporting computer.
2. The remote locations, with each one's label, SSH host, port and sign-in
   method.
3. The snippets.
4. The tab layouts and sidebar order of remote projects.

The file never holds a password or an SSH key. Remote projects and their
sessions are not in the file either: they live in each server's jojod, so the
other computer sees them once it adds the same location. This PC's projects
and sessions stay on the computer that runs them.

Import first shows what the file would change, and writes nothing until
**Import** is chosen. It merges instead of replacing:

1. A location whose SSH host and port match one already listed is not added
   again.
2. A location that signs in with a saved password asks for the password in
   the import dialog and saves it in Windows Credential Manager or the macOS
   Keychain. A location whose password is left empty is not added.
3. A location that signs in with the key Jojo set up uses
   `~/.ssh/jojo_ed25519` on the receiving computer. When that file is
   missing, the dialog says so: copy `jojo_ed25519` and `jojo_ed25519.pub`
   from the other computer's `~/.ssh` folder, or remove the location and add
   it again with its username and password.
4. A location that uses an alias from `~/.ssh/config` is reported when the
   receiving computer's `~/.ssh/config` lacks that alias. Copy the `Host`
   entry and its key file.
5. Snippets are matched by id, and an imported snippet replaces an older copy.
   A snippet listed only for one location follows that location.
6. Tab layouts are added only for remote projects that have no tabs on the
   receiving computer yet.

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

Jojo keeps its data in plain text files; it uses no database.

| File | Location (Windows) | Location (macOS) | Contents |
|---|---|---|---|
| Settings | `%APPDATA%\Jojo\settings.json` | `~/Library/Application Support/Jojo/` | The values on the Settings screen. |
| Locations | `%APPDATA%\Jojo\machines.toml` | same folder | Remote locations, with the port, key file and sign-in method of those added with a username and password. Passwords are kept in Windows Credential Manager or the macOS Keychain. |
| Layout | `%APPDATA%\Jojo\layout.json` | same folder | Project order, tabs, split layouts and folded groups. |
| Snippets | `%APPDATA%\Jojo\snippets.json` | same folder | The snippets library, with each snippet's last placeholder values. |
| Daemon state | `%LOCALAPPDATA%\Jojo\state.json` | `~/.jojo/state.json` | Projects and session records. |
| Daemon endpoint | `\\.\pipe\jojo-<username>` | `~/.jojo/jojod.sock` | The connection between window and daemon. |

On remote locations, the daemon is `~/.local/bin/jojod` and its state and
socket are in `~/.jojo/`. A `layout.json`, `machines.toml` or `snippets.json`
that cannot be parsed is renamed to `<name>.corrupt-<unix milliseconds>` and
replaced with defaults, so the app still starts and the file can be repaired
by hand.

### Profiles

Setting the environment variable `JOJO_PROFILE` to a name (1 to 32 letters,
digits, `-` or `_`) before starting Jojo runs a separate Jojo with its own
settings, locations, layout and snippets in `profiles\<name>` under the
settings folder, and its own daemon, sessions and logs in `profiles\<name>`
under the daemon's folder, listening on its own pipe (`jojo-<user>-<name>`)
or socket. Sessions in a profile pass the variable on, so Claude Code's hooks
reach that profile's daemon. Profiles are meant for recording demos without
showing real projects or servers. Restart daemon in a profile ends only that
profile's daemon. Only one Jojo window runs at a time, so close the default
profile's window first; its sessions keep running in its daemon.

On Windows, from PowerShell:

```powershell
$env:JOJO_PROFILE = "demo"; & "$env:LOCALAPPDATA\Jojo\Jojo.exe"
```

## Architecture

Jojo consists of the window and the daemon. The repository is a Cargo
workspace plus a Svelte frontend:

| Path | Contents |
|---|---|
| `app/` | The window: Tauri 2, Svelte 5, TypeScript and xterm.js 6 (WebGL renderer). |
| `app/src-tauri/` | The window's Rust side (`jojo-app`): daemon start-up, SSH connections, remote install, settings, layout, snippets and notifications. |
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

### Prerequisites

1. Rust, stable toolchain (`rust-toolchain.toml`). On Windows, with the MSVC
   build tools; on macOS, with the Xcode command line tools.
2. Node.js 22 and pnpm 10.
3. On Windows, PowerShell 7 (`pwsh`), used by the staging scripts.
4. For remote locations: cargo-zigbuild (`cargo install cargo-zigbuild
   --locked`), Zig, and the Rust targets `x86_64-unknown-linux-musl` and
   `aarch64-unknown-linux-musl`. Without them a debug staging warns and
   remote locations are unavailable; a release build fails.

### Windows

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

```sh
pnpm --dir app install

# Build jojod and the Linux daemons and copy them into
# app/src-tauri/resources/daemon.
bash scripts/stage-daemon.sh

# Run the app in development mode.
pnpm --dir app tauri dev

# Build the app and .dmg under target/release/bundle/.
pnpm --dir app tauri build
```

On macOS, `tauri build` runs `scripts/stage-daemon.sh release` itself
(`app/src-tauri/tauri.macos.conf.json`). The `.dmg` is ad-hoc signed and not
notarized.

## Testing

```sh
cargo test --workspace            # Rust unit and integration tests
pnpm --dir app test               # Frontend tests (Vitest)
pnpm --dir app check              # Svelte and TypeScript type check
pnpm --dir app lint:colors        # No hard-coded colors outside theme files
pwsh scripts/check-protocol.ps1   # Generated TypeScript types match the Rust protocol
```

Tests use temporary directories and never touch the real
`~/.claude/settings.json`, `machines.toml`, `layout.json` or `snippets.json`.
Tests against a real SSH server are opt-in: set `JOJO_SSH_TEST_HOST` to a
host alias. `docs/manual-test-checklist.md` lists the manual checks to run
before tagging a build.

## Status

Jojo is in alpha; the current release is 0.2.15. Postponed work is listed in
[ROADMAP.md](ROADMAP.md). Jojo is released under the MIT License; see
[LICENSE](LICENSE).
