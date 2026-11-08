# Jojo Plan 4: Carry-Over Into Later Plans

This document records the decisions made while executing Plan 4 (agent state, branch `plan-4-agent-state`, commits `3287915..bdb0e3e` on top of `41617d8`) and the open items from its final review.

## 1. Decisions Made During Plan 4

1. **Protocol version 3** adds `HookEvent { session_id, payload }` (relay to daemon, raw hook JSON), `MarkSeen { session_id }` (app to daemon), `CreateSession.resume` and the `SessionInfo` fields `state_detail` and `renamed` (both `#[serde(default)]`, so older `state.json` files load).
2. **All hook interpretation lives in `crates/jojo-agent`.** Unknown events and fields change nothing. Two rules go beyond the spec 7.2 table: `PreToolUse` does not leave `needs_you` (a parallel tool call, for example a subagent, must not hide a pending permission prompt; only `PostToolUse` or `Stop` resolve it), and `SessionStart` with `source: "compact"` keeps the current state.
3. **`renamed` means the label is settled:** either the user renamed the session or its first prompt named it. Resume copies `label` and `renamed` from the row whose `claude_session_id` it continues.
4. **When Claude Code ends inside a shell session** (`SessionEnd`), the session shows `running` again.
5. **Claude session ids are accepted only as 1 to 128 ASCII letters, digits and hyphens** (`jojo_agent::is_valid_claude_session_id`), when recorded from a hook and when resuming, because Resume puts the id on a `cmd.exe` command line for npm installs.
6. **Hook command format:** `C:/.../Jojo/bin/jojod.exe hook`, unquoted when the path needs no quoting (works in Git Bash, cmd.exe and PowerShell), quoted otherwise. No `matcher` field: omitted matchers match every tool.
7. **The app runs `install-hooks`** with `%LOCALAPPDATA%\Jojo\bin\jojod.exe` after its first successful connection in each app run.
8. **Toasts** use `tauri-winrt-notification` under the bundle identifier as AppUserModelID (the NSIS installer sets it on the Start menu shortcut; development builds borrow PowerShell's id). The frontend decides when a toast is due (`app/src/lib/state/toasts.ts`).
9. **The serde_json `preserve_order` feature** is enabled in jojod so `install-hooks` keeps the key order of `settings.json`. Cargo unifies the feature across the workspace, so `jojo-proto` and the app also use ordered maps.

## 2. Open Items From the Final Review

1. **Toast activation from Action Center** after the popup has closed may not reach the app: `tauri-winrt-notification` drops the `ToastNotification` after `show()` and no `ToastActivatorCLSID` is registered. Clicking the popup itself works. Fixing it needs a registered COM activator.
2. **`install-hooks` failures are invisible:** the app reports them with `eprintln!`, which a GUI-subsystem process discards. Log them to a file or show them in the banner.
3. **`settings.json` edge cases:** a symlinked `settings.json` is replaced by a regular file (rename); every changing write overwrites `settings.json.jojo-bak`, so the pre-Jojo backup is lost after the second change; `is_jojo_group` removes a whole hook group when any command in it is Jojo's.
4. **A large `PostToolUse` payload** (a multi-megabyte `tool_response`) may not reach the daemon within the relay's 500 ms budget; a lost `PostToolUse` leaves the session in `needs_you` until the next event. The relay could strip `tool_response`, which the daemon does not use.
5. **The NSIS uninstaller does not run `jojod uninstall-hooks`**, so Jojo's hook entries remain after uninstalling (they exit at once because `JOJO_SESSION_ID` is unset, but point at a deleted file).
6. **Development builds:** when a daemon started from `target/` accepts the first connection, `install-hooks` runs the possibly stale or missing `%LOCALAPPDATA%\Jojo\bin\jojod.exe`.
7. **Spec drift to record in the spec:** the sidebar menu offers both Resume and Restart for a claude session with an id (spec 5.4 offers Restart only without one); `jojo_agent::apply` takes no clock (spec 7.3 rule 4), because no transition depends on time except `silence`, which does take one; a second `StopFailure` with a different `error_type` while already in `error` keeps the first detail.
