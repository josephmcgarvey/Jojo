# Jojo Plan 6: Remaining Functionality Implementation Plan

> Executed inline in the controlling session (user preference). Each task ends with its tests passing and a commit. The Progress section is the recovery record.

**Goal:** Finish the functionality the spec and the user's requests call for before the design pass: the settings screen, the switcher, the jump shortcut, a + that opens a terminal directly, the status bar of spec 9.7, This PC's "Later" choice on a protocol mismatch, session counts in the sidebar, and path suggestions when adding a project on a server.

**Architecture:** Frontend work in Svelte 5 with pure TypeScript modules for the logic (fuzzy matching, switcher items, jump order, banner choices), plus one backend command, `save_settings`, next to `get_settings`. No protocol change.

## Scope

1. **Settings screen** (spec 9.10): a dialog opened from ⚙ at the bottom of the sidebar (spec 9.2) and from the switcher. It edits every `settings.json` key (`theme` with its one option, terminal font family, size and line height, scrollback lines, copy on select, toasts, default shell from the detected shells) and saves through `save_settings` (atomic write). Font and line-height changes apply to open panes at once; scrollback applies to panes opened afterwards. It also provides **Restart daemon (This PC)**, naming the sessions that end, and **Debug ▸ Rehydrate pane**, which requests a new snapshot for the focused pane (spec 10).
2. **Switcher** (Ctrl+Shift+P, spec 9.6): a fuzzy search over sessions (label, project, machine), projects and actions (New shell, New Claude, Add machine, Add project, Settings, Restart daemon, Rehydrate pane). Up/Down/Enter/Esc.
3. **Ctrl+Shift+J** (spec 9.6): focuses the next session in `needs_you`, then the next in `done`, cycling after the focused one.
4. **+ on a project opens a terminal** (user request, replaces spec 9.2's Claude/Shell menu): a click starts a shell with the default shell (This PC) or the login shell (remote); right-click on + offers New Claude and the other shells.
5. **Status bar** (spec 9.7): the focused session's machine label and project path, and the hint `Ctrl+Shift+P`.
6. **This PC protocol mismatch** (spec 8.4): the banner offers **Restart now (ends N sessions)** and **Later**; Later hides the banner and the header keeps "restart required".
7. **Session count** after each project name in the sidebar (Layout A mockup).
8. **Path suggestions** in Add project on a remote machine (spec 9.2): `ListDir` for the typed path's parent folder fills a suggestion list.

## Global Constraints

- No protocol change, no new dependencies.
- Every color, font, radius and shadow in `.svelte` files from `var(--jojo-*)`; `npm run lint:colors` passes.
- Settings are written atomically (`settings.json.tmp`, then rename); unknown keys in an existing file are not lost.
- US spelling; factual comments, no aphorisms; cargo `--jobs 4`; never start jojod or the app from the PowerShell tool.

## Progress

- Task 0: branch `plan-6-functionality` from master `b8197cb`.
- Scope 1 (settings): b876c63. Scope 2-3 (switcher, jump): 3a58158. Scope 4 and 7 (+ opens a terminal, counts): d410638. Scope 5-6 (status bar, Later): dc8d0d0. Scope 8 (path suggestions): next commit.
- Scope 8 (path suggestions): 044bb04. Final-review fixes: 2aaab17.

## Open Items From the Final Review

1. **Rehydrate overrides the attach gate:** `rehydrate` calls `stream.reattach(true)`, which abandons an outstanding Attach, so repeated clicks send several Attaches (cosmetic: the last Snapshot resets the screen). It is kept because Rehydrate is the recovery for a pane whose gate is stuck; a gentler path would use `reattach(false)` first.
2. **Switcher keyboard details:** the highlighted row is not scrolled into view past the visible rows, and arrow keys work only while the input has focus.
3. **The status bar no longer shows the jojod version** (spec 9.7 lists machine, path and the switcher hint only); the version is visible in the Add machine install step and the protocol-mismatch messages.
4. **User-driven deviations from the spec:** a project's + opens a terminal at once (right-click for Claude and other shells), replacing spec 9.2's Claude/Shell menu, and Alt+Shift+= / Alt+Shift+- split with a terminal instead of a Claude session (spec 9.6).
