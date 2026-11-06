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
