# Jojo Plan 3: Carry-Over Into Later Plans

This document records the decisions made while executing Plan 3 (pane layout, branch `plan-3-pane-layout`, commits `1548f9b..78d8104` on top of `2193f07`) and the items later plans must act on. It replaces the per-task progress ledger. The Plan 2b carry-over document remains the source for the items it lists that Plan 3 did not take (its sections 3.1 items 6-8, 3.3 items 2-6 and 8-10, and all of 3.4 to 3.6).

## 1. Decisions Made During Plan 3

1. **No protocol change.** `PROTOCOL_VERSION` stays 2. Session rename and Kill-but-keep-the-row use the existing `RenameSession` and `Kill` messages. Restart is done in the app: `CreateSession` with the same project and kind (and `shell: session.command` for a shell), placed in the old pane's position, then `CloseSession` for the old session. Resume for `claude` sessions needs a protocol change and `claude_session_id`, so it moves to Plan 4.
2. **Amendment A (user request): a session that exits with code 0 closes its pane.** On a live `SessionExited` with `exit_code === 0`, the app sends `CloseSession`. Any other code, or none, keeps the pane and its final output (spec 5.3). Sessions that are already exited in a `Sessions` reply are not closed. Kill gives exit code 1 (portable-pty's `TerminateProcess(proc, 1)`), so a killed session keeps its pane. In bash, `exit` returns the status of the last command, so `exit` after a failed command keeps the pane open, as in Windows Terminal's "graceful" mode.
3. **Amendment B (user request): shell picker.** The backend command `available_shells` returns PowerShell 7, Windows PowerShell and Command Prompt when found on `PATH`, and Git Bash when found at `Git\bin\bash.exe` under `%ProgramFiles%`, `%ProgramFiles(x86)%` or `%LOCALAPPDATA%\Programs` (bash.exe on `PATH` is skipped because `System32\bash.exe` is the WSL launcher). Detection runs once per app run. "New shell" in the sidebar + menu and in Split right / Split down lists them with the `defaultShell` entry first, marked "(default)".
4. **Layout reconciliation waits for projects, sessions and `layout.json`.** `reconcile()` runs only after the first `Projects` reply, the first `Sessions` reply and the layout load; it then runs on every `Sessions`, `Projects` and `ProjectRemoved` message and writes `layout.json` 250 ms after the last change.
5. **Panes are absolutely positioned and keyed by session id** (`PaneGrid.svelte`), so rearranging a tab or moving a pane does not remount its terminal; `{#key paneKey(session)}` still remounts a pane whose session loses its screen.
6. **Flow control:** above 24 MiB of unparsed `term.write` data a pane drops Output; at 2 MiB it sends an Attach whose Snapshot replaces the dropped data. At most one Attach is outstanding per session.
7. **Snap layouts** use an invisible `WS_EX_LAYERED` input-sink child window over the maximize button that answers `HTMAXBUTTON`; `build.rs` embeds `windows-app-manifest.xml` (Windows 10/11 `supportedOS`, Common Controls 6). The sink rectangle is re-sent on window resize, button resize and scale-factor change.
8. **Execution note:** from Task 6 on, tasks were implemented directly in the controlling session without per-task reviews (user request for speed). One whole-branch review covered all 16 commits; its three Important findings were fixed in `78d8104`.

## 2. Open Items From the Final Review

1. **M2: with `copyOnSelect` off, a keyboard copy leaves the selection in place,** so each further Ctrl+C copies again and the program is never interrupted until the user clicks. Add `clearSelection` to `TerminalControls` and call it after a keyboard copy.
2. **M3 (dialog part): `ConfirmDialog` does not return keyboard focus to the focused pane** after it closes (OSC 8 link confirmation, Close tab). Restore focus in `answerConfirm`.
3. **M4: a layout change made in the last 250 ms before the window closes is lost.** Flush the pending save from `onCloseRequested`.
4. **M5: duplicate `CloseSession` messages show an "unknown session" error banner** (Restart when the old pane was closed before `SessionCreated`; Restart clicked twice; a manual close racing the amendment-A auto-close). Return `closeSessionId` from `sessionCreated` only when the replacement succeeded and the old session still exists, and consider not showing `CloseSession` "unknown session" errors.
5. **M6: `available_shells`, `get_layout` and `get_settings` are synchronous commands on the main thread.** An unreachable network directory in `PATH` can stall startup. Make them `async` with `spawn_blocking`.
6. **M7: the snap-layout sink takes clicks meant for overlays** drawn over the maximize button's 46×31 px area (the ConfirmDialog scrim, a context menu). Hide the sink (`SWP_HIDEWINDOW`) while a dialog or menu is open.
7. **M8: when the `defaultShell` program is not installed, Ctrl+Shift+N still sends it** and the session fails to start with exit code -1. Fall back to the first detected shell, or send `null` so jojod's `default_shell()` chooses.
8. **M9: `links.rs` calls `CoUninitialize` right after `ShellExecuteW`.** A protocol handler that activates asynchronously could lose its apartment; consider `ShellExecuteExW` with `SEE_MASK_NOASYNC`. No failure observed.
9. **M10: one `pending` workspace entry leaks** when the connection drops after `CreateSession` is sent and before `SessionCreated` arrives. The session is placed by reconciliation; the entry is never removed. Negligible cost.

## 3. Deferred Minors From Per-Task Reviews

1. `layout.rs` builds suffixed sibling paths inline in both `save` and `set_aside`; factor out if a third use appears.
2. The `resizeSplit` doc comment (`tree.ts`) omits the unchanged case when two neighbors' combined share is below `2 × MIN_SHARE`.
3. `focusNeighbor` (`geometry.ts`) uses nested ternaries for distance and overlap; a per-direction table would read better.
4. Reconciliation's removal of a session id repeated inside one tab depends on `filterPanes` traversal order and has no test (only a hand-edited `layout.json` can produce it).

## 4. Manual Verification Still Needed

The agent could not see the window. Every manual step in Plan 3 (Tasks 9-13) and items 1-23 of the Plan 3 section of `docs/manual-test-checklist.md` remain to be checked by hand, including the snap-layout flyout on a mixed-DPI setup (final review I3).
