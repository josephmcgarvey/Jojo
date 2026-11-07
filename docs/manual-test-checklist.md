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
