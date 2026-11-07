# Terminal Recordings

Raw PTY output captured with `cargo run -p jojod --example record`, used by
`crates/jojo-grid/tests/recordings.rs` and `tests/xterm-crosscheck`. The file
name encodes the terminal size: `<name>.<cols>x<rows>.bin`.

| File | What it captures |
|---|---|
| `pwsh-redraw.80x24.bin` | PSReadLine typing a 150-character command, deleting 120 characters, clearing the line, then running a command. Exercises wrapped-line redraws. |
| `cmd-dir.100x30.bin` | `cmd.exe` listing a directory at 100 columns. |
| `pwsh-unicode-color.80x24.bin` | CJK, emoji and a real combining accent (`e` + U+0301, not the precomposed `é`); a colored `Write-Host`; a 200-column line with a blue background that wraps. |

There is no Claude Code recording yet. The recorder sends only scripted
keystrokes and cannot forward your keyboard, so it cannot drive an
interactive Claude Code session to a permission prompt. Add one once the app
(Plan 2b) can save a session's raw output.

## Recording command deviations from the task brief

Both pwsh recordings were adjusted from the brief's exact commands:

- Both pwsh recordings start with a `Set-PSReadLineOption -PredictionSource
  None` step (with a startup delay before it and another delay before the
  next step), so the recording does not depend on whatever command history
  PSReadLine has accumulated on the machine that made it. PSReadLine's
  history persists across separate `pwsh.exe` launches (it is read from
  `ConsoleHost_history.txt`, not just the current session), so without this
  step a later run of the same recorder command on a machine with different
  history could render a gray inline prediction after a line the brief's own
  steps type, producing a different, non-deterministic recording bytes.
- `pwsh-redraw.80x24.bin` also gained a fifth `--step` (an extra
  `Write-Output` line) and a longer `--duration-ms`, because the brief's
  four content steps alone produced a complete, round-tripping 876-byte
  recording — comfortably below the "well above 1000" guideline, with the
  extra idle time in the original duration not adding bytes since nothing
  happens at an idle prompt.
- `pwsh-unicode-color.80x24.bin`'s combining accent is written in the
  `--step` text as the hex escapes `\x65\xcc\x81` (the recorder's existing
  `\xHH` escape, already used elsewhere for the DEL byte) rather than typed
  directly as a Unicode character. The recorder's `unescape` function turns
  each `\xHH` into one raw byte before the bytes reach the pty, so this
  produces exactly the 3-byte UTF-8 encoding of `e` (U+0065) followed by
  COMBINING ACUTE ACCENT (U+0301) as actual keystrokes typed into `pwsh`,
  which is what the earlier recording's `é` should have been but was not:
  that recording's "é" was the single precomposed codepoint U+00E9 (UTF-8
  `C3 A9`), which exercises no combining-mark handling at all. The
  recording now contains the literal bytes `65 CC 81` (verified with
  `xxd`), not `C3 A9`.
  - The recorder no longer sets `$PSStyle.OutputRendering = 'Ansi'`, which an
    earlier version of this recording used to force colored output. The real
    cause of that missing color was not PowerShell's output rendering mode:
    it was `NO_COLOR=1`, which Claude Code's PowerShell tool (the shell this
    recording was made from) injects into every command it runs, including
    `cargo run --example record`, which `portable-pty`'s `CommandBuilder`
    then inherited into the spawned `pwsh.exe`. `pwsh` 7.6.9 honors
    `NO_COLOR` by selecting `PlainText` output rendering; without it, under
    ConPTY, it selects `Host` rendering with VT/ANSI support on its own, and
    colors do not need to be forced. `crates/jojod/examples/record.rs` now
    calls `cmd.env_remove("NO_COLOR")` before spawning the program being
    recorded, so a recording is no longer shaped by the preferences of
    whatever tool happened to launch the recorder. A real Jojo session is
    never launched with `NO_COLOR` set this way, so recording under it would
    not have reflected what the daemon actually sees.
