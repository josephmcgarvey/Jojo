# Terminal Recordings

Raw PTY output captured with `cargo run -p jojod --example record`, used by
`crates/jojo-grid/tests/recordings.rs` and `tests/xterm-crosscheck`. The file
name encodes the terminal size: `<name>.<cols>x<rows>.bin`.

| File | What it captures |
|---|---|
| `pwsh-redraw.80x24.bin` | PSReadLine typing a 150-character command, deleting 120 characters, clearing the line, then running a command. Exercises wrapped-line redraws. |
| `cmd-dir.100x30.bin` | `cmd.exe` listing a directory at 100 columns. |
| `pwsh-unicode-color.80x24.bin` | CJK, emoji and a combining accent; a colored `Write-Host`; a 200-column line with a blue background that wraps. |

There is no Claude Code recording yet. The recorder sends only scripted
keystrokes and cannot forward your keyboard, so it cannot drive an
interactive Claude Code session to a permission prompt. Add one once the app
(Plan 2b) can save a session's raw output.

## Recording command deviations from the task brief

Both pwsh recordings were adjusted from the brief's exact commands; see
`task-7-report.md` for the full reasoning. In short:

- `pwsh-redraw.80x24.bin` gained a fifth `--step` (an extra `Write-Output`
  line) and a longer `--duration-ms` because the brief's four steps alone
  produced a complete, round-tripping 876-byte recording — comfortably below
  the brief's "well above 1000" expectation, with the extra idle time in the
  original duration not adding bytes since nothing happens at an idle prompt.
- `pwsh-unicode-color.80x24.bin` gained a leading
  `$PSStyle.OutputRendering = 'Ansi'` step. This PowerShell 7.6 session's
  default `$PSStyle.OutputRendering` is `PlainText`, which strips embedded
  ANSI escapes from `Write-Output` strings and disables `Write-Host
  -ForegroundColor`/`-BackgroundColor` SGR output; without this step, neither
  the backtick-escaped `` `e[44m ``/`` `e[0m `` sequence around the 200 `x`
  characters nor the `Write-Host` warning's colors reached the recording.
