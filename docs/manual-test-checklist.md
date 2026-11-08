# Jojo Manual Test Checklist

Run these checks before tagging a build. Each line states the action and the
expected result.

## Daemon (Plan 1)

1. `jojod --version` prints `jojod <version> (protocol <n>)`.
2. `jojod serve` keeps running and creates the pipe `\\.\pipe\jojo-<username>`
   (check with `Get-ChildItem \\.\pipe\ | Where-Object Name -like 'jojo-*'`).
3. A second `jojod serve` exits with an error while the first is running.
4. `jojod shutdown` makes the running daemon exit with code 0.
5. `%LOCALAPPDATA%\Jojo\state.json` exists after the daemon has run once.

## App shell (Plan 2b)

Run from a normal terminal (not elevated, not Claude Code's PowerShell tool)
after `pnpm --dir app stage`.

1. `pnpm --dir app tauri dev` opens a frameless Neon Glass window; the banner
   reads "Starting jojod…" and disappears within a few seconds.
2. The status bar shows `jojod <version>`; `%LOCALAPPDATA%\Jojo\bin` holds
   jojod.exe, conpty.dll and OpenConsole.exe.
3. Dragging the empty part of the tab row moves the window; minimize,
   maximize and close work.
4. + next to This PC opens a folder picker; the chosen folder appears as a
   project.
5. Hovering a project shows +; choosing Shell opens a tab with a pwsh prompt
   in that folder; typing works and colors show (for example `Get-ChildItem`).
6. Choosing Claude opens a tab running Claude Code in that folder.
7. Resizing the window reflows the terminal; `$Host.UI.RawUI.WindowSize`
   in pwsh matches the visible size.
8. Switching tabs and back restores the screen exactly, including scrollback.
9. Closing the window and running `pnpm --dir app tauri dev` again shows the
   same projects and sessions, and each running session's screen is intact.
10. `exit` in a shell marks the session exited (hollow badge); × on its tab
    removes it.
11. Ending jojod (`taskkill /IM jojod.exe /F`) shows the reconnecting banner;
    the app starts a new jojod, and old sessions show "This session has no
    screen".
12. A second launch of jojo.exe focuses the existing window instead of
    opening another.
13. Open a project's + menu; Escape closes it; clicking outside closes it;
    clicking the same + again closes it without reopening.
14. Tab to the project + with the keyboard: it becomes visible when focused.


## Pane layout (Plan 3)

Run from a normal terminal (not elevated, not Claude Code's PowerShell tool)
after `pnpm --dir app stage`, with `pnpm --dir app tauri dev` or the
installed build.

1. With `%APPDATA%\Jojo\layout.json` deleted and the app closed, starting
   the app opens every existing session in its own tab, and `layout.json`
   is written within a second.
2. Split a pane (pane menu ▸ Split right ▸ New shell), drag the divider,
   switch to another tab, close the window and start the app again: the
   same tabs, splits, divider position, focused pane and active tab return,
   and every pane shows its screen.
3. With the app closed, add a made-up session id as an extra pane in
   `layout.json`; on start the app ignores it and the tab shows only the
   real sessions.
4. Split right and Split down (menu and Alt+Shift+= / Alt+Shift+-) add a
   pane beside the focused one in the same project. In a tab with 4 panes
   both split items are disabled and hovering them shows
   "Tab is full (4 panes)"; Move to ▸ that tab is disabled from other tabs
   with the same tooltip.
5. Dragging a divider resizes both sides and the terminals reflow; a pane
   cannot be dragged smaller than a tenth of its split.
6. Alt+Arrow moves focus between panes; the focused pane has the accent
   outline and receives typing.
7. Alt+Shift+Z (or Zoom pane) fills the tab with the focused pane; toggling
   again brings the other panes back with their screens intact.
8. Swap with, Move to ▸ New tab, Move to ▸ another tab, Layout ▸ Columns,
   Rows, Grid 2×2 and Main + stack, and Equalize sizes rearrange panes; the
   moved terminals keep their content.
9. Tab menu: Rename tab… (also double-click a tab title) renames the tab and
   the name survives a restart; Layout applies presets; Close tab on a tab
   with two sessions lists both before closing them.
10. Ctrl+Shift+W closes the focused pane and its session.
11. Sidebar session menu: Open in split right/down moves a session of the
    current project into the current tab and is disabled for a session of
    another project; Move to new tab; Rename… and double-click rename the
    session in the sidebar, pane header and tab; Kill marks a running
    session exited and keeps its row and final output; Restart on an exited
    session starts a new one in the same pane position and removes the old
    row.
12. In `vim` with `:set mouse=a`, a plain right-click goes to vim and
    Shift+right-click opens the pane menu; Shift+drag selects text.
13. With copyOnSelect on (default), drag, double-click and triple-click
    selections copy to the clipboard and the header shows "Copied" for
    about a second; Ctrl+C with a selection still visible interrupts
    `ping -t localhost`.
14. With `"copyOnSelect": false` in settings.json, Ctrl+C copies a selection
    and interrupts when nothing is selected.
15. Ctrl+Shift+C copies, Ctrl+Shift+V and pane menu ▸ Paste paste, plain
    Ctrl+V still pastes, and plain Ctrl+T reaches Claude Code.
16. Ctrl+Shift+T and Ctrl+Shift+N open a Claude and a shell tab;
    Ctrl+Tab and Ctrl+Shift+Tab cycle tabs.
17. Clicking Shell twice quickly in a project's + menu opens two tabs.
18. An OSC 8 link
    (`$e = [char]27; Write-Host "$e]8;;https://example.com$e\click here$e]8;;$e\"`)
    asks "Open link?" showing `https://example.com` before opening; a plain
    URL opens without asking; a hostless OSC 8 link
    (`Write-Host "$e]8;;http://$e\broken$e]8;;$e\"`) shows
    "Link not opened: URL has no host" in the pane header.
19. Hovering the maximize button shows the Windows 11 snap layout flyout and
    highlights the button; clicking it maximizes and restores.
20. Printing a large amount of output
    (`$s = 'x' * 200; 1..300000 | ForEach-Object { $s }`) leaves the app
    responsive, ends with the last lines on screen, and the WebView2
    devtools console (Ctrl+Shift+I in `tauri dev`) shows no
    "write data discarded" error.
21. With no staged daemon and no `%LOCALAPPDATA%\Jojo\bin\jojod.exe`, the
    banner reads "Starting jojod (attempt N): …" and, after 10 attempts,
    "jojod did not start: … Logs are in …" with Retry, never
    "Disconnected … Sessions keep running".
22. **Close on exit (amendment A).** In a shell pane, type `exit`: the pane
    closes and the session disappears from the sidebar. In another shell
    pane, run `exit 3`: the pane stays open with its output, and the sidebar
    shows the session as exited. Restart the app with an exited session
    present: it is not closed automatically.
23. **Shell picker (amendment B).** Open the + menu next to a project: it
    lists the shells installed on this PC (PowerShell 7, Windows PowerShell,
    Command Prompt, Git Bash, only those present), with the `defaultShell`
    setting's entry first and marked "(default)". Choosing Git Bash opens a
    bash prompt (not WSL). Right-click a pane ▸ Split right ▸ New shell
    shows the same list, and the chosen shell starts in the new pane.
    Ctrl+Shift+N opens the default shell.

## Agent state (Plan 4)

Plan 4 changes the protocol to version 3. The first start of this build
finds the older jojod running and shows the version banner; Restart jojod
ends the sessions it was running.

1. After the first start, `~/.claude/settings.json` contains one Jojo entry
   (`"<...>/Jojo/bin/jojod.exe" hook`, `"timeout": 5`) for each of
   SessionStart, UserPromptSubmit, PreToolUse, PostToolUse,
   PermissionRequest, Notification, Elicitation, Stop, StopFailure and
   SessionEnd; earlier hooks are still there and still first, and
   `settings.json.jojo-bak` holds the previous file. Restarting the app adds
   no further entries.
2. In a shell pane, run `claude`: the badge changes from running to idle.
   Send a prompt: the badge spins (working) and the sidebar label becomes the
   first 40 characters of the prompt.
3. Ask Claude to run a command that needs permission: the badge glows amber
   (needs you) and its tooltip reads "Needs you: Needs permission:
   Bash(...)". Approve it: the badge spins again. When the turn ends, the
   badge is a solid teal disc (done) if the pane is not focused, and becomes
   idle when the pane is focused.
4. Press Esc during a turn: within about 10 seconds the badge returns to
   idle.
5. With another app in front (or another pane focused), let a turn finish:
   a Windows toast "<label> · <project>" / "Finished" appears. Clicking it
   brings Jojo forward and focuses that pane. A second finish within 10
   seconds raises no second toast. With `"toasts": false` in
   `%APPDATA%\Jojo\settings.json`, no toast appears.
6. Exit Claude with `/exit` inside a shell pane: the badge shows running
   again. Run `claude` from a pane and restart jojod from the banner or with
   `taskkill /IM jojod.exe /F`: the session shows exited, and its sidebar
   menu offers Resume, which continues the conversation in the same pane
   position.
7. A rename (double-click the sidebar row) before the first prompt keeps the
   chosen name after the prompt.
8. Running `claude` in Windows Terminal (outside Jojo) is unaffected: the
   hook exits at once because `JOJO_SESSION_ID` is not set.

## Machines (Plan 5)

Run `scripts/stage-daemon.ps1` (it builds the Linux daemons with
cargo-zigbuild) before building. `JOJO_SSH_TEST_HOST=<host> cargo test -p
jojo-app --test ssh_live -- --nocapture` installs and connects to a real
server without the window.

1. The sidebar shows This PC first. **+ Machine** at the bottom opens Add
   machine; the Host field suggests the aliases from `~/.ssh/config`.
2. Add `onprem-server` with a label: the dialog shows Connect, Platform
   (Linux x86_64), Install jojod (Uploading, then Installed jojod 0.1.0
   (protocol 3)), Start jojod, Logout survival (a warning while
   `Linger=no`) and Claude Code hooks (a warning while Claude Code is not
   installed). Done adds the machine under This PC; `machines.toml` in
   `%APPDATA%\Jojo` lists it.
3. The machine's + asks for a path on the server: `~` (or `~/project`)
   adds a project; a path that is not a directory shows the daemon's error
   in the banner. New shell in that project opens a shell on the server
   (`hostname` prints the server's name).
4. Restart the app: the remote machine connects again, its tabs and panes
   come back with their screens, and the header shows no state text.
5. On the server, `~/.local/bin/jojod shutdown` (or a reboot): the
   machine's header reads "reconnecting", its sessions dim, and its panes
   show "Disconnected. Sessions keep running on <label>."; typing in them
   does nothing. The app starts the daemon again on the next attempt.
6. Disconnect the network (or stop the OpenSSH Authentication Agent and
   remove the key): within about 45 seconds the machine reconnects with
   backoff; with the key removed, the header reads "auth failed", the
   machine's panes say "Auth failed on <label>. Load your key with
   ssh-add, then Reconnect.", and right-click the header ▸ Reconnect
   retries after `ssh-add`.
9. On a server with Claude Code in `~/.local/bin`, the Claude Code hooks
   step reports "registered" and a Claude session starts there (the
   daemon is started through the login shell, so its PATH includes it).
10. New shell in a server project starts the server's login shell, and
    the + menu lists no Windows shells for it.
7. Right-click the machine header ▸ Remove machine: the confirmation names
   the running sessions, which keep running on the server; the machine
   leaves the sidebar and `machines.toml`. Remove and uninstall from server
   also stops jojod there and deletes `~/.local/bin/jojod` and `~/.jojo`.
8. A server whose installed jojod speaks another protocol shows "upgrade
   required"; right-click ▸ Upgrade jojod… names the sessions that end,
   then reinstalls and reconnects.

## Remaining functionality (Plan 6)

1. ⚙ at the bottom of the sidebar opens Settings. Changing the font size
   or line height and saving resizes the text in every open pane at once
   (the shell reflows to the new width); the values are in
   `%APPDATA%\Jojo\settings.json`, and keys added by hand stay there.
2. Settings ▸ Restart daemon (This PC) names the running sessions before
   restarting; Debug ▸ Rehydrate pane repaints the focused pane.
3. Ctrl+Shift+P opens the switcher: typing part of a session label,
   project name or action ("set" for Settings) narrows the list; Up, Down
   and Enter choose; Esc closes. Choosing a session focuses its pane.
4. With one session waiting for permission and one done, Ctrl+Shift+J
   focuses the waiting one, then the done one.
5. Clicking a project's + opens a terminal at once; right-clicking it
   offers New terminal, New Claude session and the other shells. Each
   project name shows its number of sessions. Alt+Shift+= and Alt+Shift+-
   split with a terminal.
6. The status bar shows the focused session's machine and project path and
   "Ctrl+Shift+P to switch".
7. With a jojod of another protocol running on This PC, the banner offers
   "Restart now (ends N sessions)" and "Later"; Later hides the banner and
   This PC's header reads "restart required".
8. Add project on a server: typing `~/` lists the home folder's subfolders
   below the field; typing more narrows them; choosing one fills the path
   and lists its subfolders. A folder that does not exist shows no
   suggestions and no error.
