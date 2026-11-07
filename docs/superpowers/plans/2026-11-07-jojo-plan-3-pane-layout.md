# Jojo Plan 3: Pane Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Plan 2b's one-session-per-tab window with per-project tabs that each hold a split tree of 1 to 4 panes, persisted in `%APPDATA%\Jojo\layout.json`, controlled by right-click menus and keyboard shortcuts, with copy on select, end-to-end flow control, OSC 8 links, Windows 11 snap layouts, and the connection fixes carried over from Plan 2b.

**Architecture:** All layout logic is pure TypeScript under `app/src/lib/layout/` (split tree, geometry, tabs, reconciliation against the daemon's session list) and `app/src/lib/state/workspace.ts` (placement of newly created sessions), each with vitest tests. The Rust backend stores `layout.json` next to `settings.json` (new `app/src-tauri/src/layout.rs`), writes it atomically, and generates its TypeScript types with ts-rs into `app/src/lib/backend/`. The Svelte UI renders the active tab's panes as absolutely positioned boxes computed from the tree (so restructuring a tab never remounts a terminal), mounts an xterm.js instance only for visible panes (Attach on mount, Detach on unmount), and draws one reusable context-menu component for the pane, tab and sidebar session menus.

**Tech Stack:** Rust 1.99, Tauri 2.12.5 (`tauri` 2.x, `tauri-build` 2.7), `windows-sys` 0.59 (new features only, no new crates), ts-rs 12; Node 22, pnpm 10, Svelte 5.57, TypeScript 5, Vite 8.3, vitest 5.1, svelte-check 4.7, `@xterm/xterm` 6.0.0 (`modes.mouseTrackingMode`, `linkHandler`, `attachCustomKeyEventHandler`, `write(data, callback)`), `@xterm/headless` 6.0.0 (tests).

**Inputs:** Spec `docs/superpowers/specs/2026-11-06-jojo-design.md` (sections 4 Layout row, 5.5, 9.1, 9.3, 9.4, 9.5, 9.6, 9.10). Carry-over `docs/superpowers/plans/2026-11-07-jojo-plan-2b-carryover.md` (sections 3.1 items 1-4, 3.2 items 1-3, 3.3 items 1 and 7; 3.1 item 5 is already done on `master`, see Scope). Style reference: `docs/superpowers/plans/2026-11-07-jojo-plan-2b-app-shell.md`.

## Scope

In scope:

1. `layout.json` storage in the Rust backend (load, atomic save, ts-rs types), keyed by `(machine id, project id)`; reconciliation on load (stale session ids dropped, daemon sessions missing from the layout each placed in a new tab).
2. Pure split-tree operations: split, close, swap, move between tabs, zoom, presets (Columns, Rows, Grid 2×2, Main + stack), equalize, divider resize, directional focus movement, the 4-pane limit.
3. The pane grid: draggable dividers, a header strip per pane (badge, label, "Copied" and error notices, inline rename), a 1 px accent outline on the focused pane, Attach for visible panes and Detach for hidden ones (spec 5.5).
4. One reusable context-menu component (submenus, keyboard operation, disabled items with tooltips) and the pane menu, tab menu and sidebar session menu (spec 9.4), including the mouse-reporting rule for right-click and Shift+right-click.
5. Close confirmation for sessions in `working` or `needs_you`, and the Close tab summary. No session reaches those states until Plan 4 adds hooks, so the confirmation is dormant until then; it reads `SessionInfo.state`, so Plan 4 needs no change here.
6. Session rename (sidebar double-click, sidebar menu, pane menu), tab rename (layout.json), Kill that keeps the row, and Restart for exited sessions.
7. Keyboard shortcuts from spec 9.6 except Ctrl+Shift+P and Ctrl+Shift+J (see the deferral table); copy on select and the Ctrl+C rules (spec 9.5); Paste through the WebView2 clipboard API.
8. Carry-over 3.1 items 1-4: snap layouts (`HTMAXBUTTON`), OSC 8 `linkHandler` through `open_url` with a confirmation, `open_url` failure feedback, `open_url` on a worker thread with `CoInitializeEx`.
9. Carry-over 3.2 items 1-3: at most one outstanding Attach per session, end-to-end flow control with `term.write` callback watermarks and a resync Attach, two quick creates both placed.
10. Carry-over 3.3 items 1 and 7: a Retry click during `connect_or_start` is honored; a distinct banner until the first successful connection.

Deferred, with reasons:

| Item | Moves to | Reason |
|---|---|---|
| Agent state detection: hook-driven badges, toasts, `done` → `idle` on focus, Ctrl+Shift+J (jump to the next `needs_you`/`done` session) | Plan 4 | Those states are produced only by Claude Code hooks |
| Resume for exited `claude` sessions (spec 5.4) | Plan 4 | Needs `claude_session_id`, which only the `SessionStart` hook records, and a way to pass `--resume <id>` to `CreateSession` (a protocol change); see "Protocol Decision" |
| Machines, SSH, the sidebar machine menu | Plan 5 | Roadmap. Plan 3 uses the constant machine id `local` in layout keys |
| Carry-over 3.1 item 5 (wait for Geist Mono before mounting) | Done | Already on `master` in commit `5084b84`: `app/src/main.ts` awaits `document.fonts.load` for the 400 and 700 weights of Geist Mono before `mount`. Plan 3 does not touch it |
| Switcher (Ctrl+Shift+P) and its status bar hint (carry-over 3.1 item 8, second half) | A later plan | It is a separate feature (fuzzy search over sessions, projects and actions); nothing in the pane layout depends on it |
| Sidebar project menu (New Claude, New shell, Reveal in Explorer, Remove project) | A later plan | Remove project needs the kill, wait for `SessionExited`, then `RemoveProject` flow (Plan 2a carry-over 2.1.8); the existing project + menu already offers Claude and Shell |
| Settings screen, Restart daemon button in settings, Debug ▸ Rehydrate pane (spec 9.10, 10) | A later plan | Plan 3 reads `copyOnSelect` and the terminal font settings from `settings.json` only |
| Carry-over 3.1 items 6-7 (theme propagation to mounted panes, DECRQSS) | A later plan | One theme exists; DECRQSS gets exactly one reply today |
| Carry-over 3.3 items 2-6 and 8-10 (backoff after a user restart, late reader frame, taskkill polling, kill scope, start timeout detail, subscribe failure status, per-key settings fallback, debug staging profile) | A later plan | Not needed for pane layout; listed as open in the carry-over document |
| Carry-over 3.4 (daemon blocking work, exited-grid cap, control frame weight, overflow notify, `WRITE_TIMEOUT` test, flaky flood test) | A later plan | Daemon-side; Plan 2b's scope table had pointed `spawn_blocking` at Plan 3, and this plan leaves it open on purpose because a tab shows at most 4 panes and each Attach is one snapshot of at most 16 MiB |
| Carry-over 3.5 (CSP, check-protocol cleanup, color-lint `style:{}` capture, `Channel<ArrayBuffer>`) and 3.6 (test gaps) | A later plan | Unrelated to pane layout |
| Carry-over section 2 corrections (remove-project UI, CI, generated envelope bytes) | A later plan | Unrelated to pane layout |

## Protocol Decision

Plan 3 makes **no protocol change** and keeps `PROTOCOL_VERSION = 2`. The reasons, checked against `crates/jojo-proto/src/msg.rs` and `crates/jojod/src/daemon.rs` at `5084b84`:

1. **Session rename** already exists. `ControlMsg::RenameSession { session_id, label }` is handled in `Daemon::handle_control`: it updates the label in `state.json` and broadcasts `SessionUpdated`, which the frontend reducer already applies.
2. **Kill but keep the row** already exists. `ControlMsg::Kill { session_id }` calls `Session::kill()` and nothing else; the process exit then runs `Daemon::on_session_exit`, which marks the row `exited`, keeps it in `state.json`, and broadcasts `SessionExited`. Only `CloseSession` removes a row.
3. **Restart** is done in the app with existing messages. The app sends `CreateSession` with a new client-chosen id, the same `project_id` and `kind`, and `shell: session.command` for shell sessions (`command_for` in `crates/jojod/src/command.rs` uses `shell` verbatim for `SessionKind::Shell` and ignores it for `SessionKind::Claude`, which re-resolves `claude.exe` or `cmd.exe /d /c claude`). When `SessionCreated` arrives, the new id takes the old pane's position in the layout and the app sends `CloseSession` for the old id. This matches spec 5.4: "a new session that replaces the old row".
4. **Resume** is not possible without a protocol change: `CreateSession` has no field for extra arguments, so `claude --resume <claude_session_id>` cannot be requested, and `claude_session_id` is always `null` until Plan 4's `SessionStart` hook handling records it. Plan 4 adds both together and bumps `PROTOCOL_VERSION` to 3 following the established pattern (change `msg.rs`, regenerate with `scripts/check-protocol.ps1`, commit the regenerated `app/src/lib/protocol/*.ts`). Until then the sidebar menu offers Restart for every exited session, including `claude` sessions, which spec 5.4 already specifies for `claude` sessions without a recorded id.

## Global Constraints

- All Plan 1, 2a and 2b constraints still apply (names, paths, frame format, `PROTOCOL_VERSION = 2`, session size bounds 2-1000 × 1-1000, envelope bytes, generated TypeScript only in `app/src/lib/protocol/` and `app/src/lib/backend/`).
- Layout file: `%APPDATA%\Jojo\layout.json`, written atomically (write `layout.json.tmp`, then rename over `layout.json`). Keys are `"<machine id>:<project id>"`; Plan 3 uses machine id `local`. `version` is `1`.
- A tab holds at most 4 panes (`MAX_PANES = 4`). Disabled split items and full Move-to targets carry the tooltip text exactly `Tab is full (4 panes)`.
- Copy-on-select "Copied" indicator: 1.2 seconds (`COPIED_MS = 1200`).
- Keyboard: application shortcuts use Ctrl+Shift or Alt+Shift (plus Ctrl+Tab, Ctrl+Shift+Tab and Alt+Arrow from spec 9.6); plain Ctrl combinations always reach the terminal, except Ctrl+C when `copyOnSelect` is off and text is selected.
- No new npm or cargo dependencies. New `windows-sys` features on the existing dependency (`Win32_System_Com`, `Win32_Graphics_Gdi`, `Win32_UI_Input_KeyboardAndMouse`, `Win32_System_LibraryLoader`) are not new dependencies. If an implementer finds a new dependency unavoidable, they stop and ask the user for approval first.
- Every color, font, radius, glow and shadow in `.svelte` files comes from `var(--jojo-*)`; `pnpm --dir app lint:colors` must pass. New tokens go in `app/src/themes/neon-glass.json`.
- US spelling in code, comments, commit messages and UI text. Comments state facts (what the code does and why); no aphorisms.
- Cargo: always build with `--jobs 4` (or `CARGO_BUILD_JOBS=4` in the environment for scripts and `pnpm tauri build`), to bound memory use.
- Never start jojod or the app from Claude Code's PowerShell tool: it injects `NO_COLOR=1`, which every session would inherit. Use the Bash tool or a desktop terminal for `pnpm --dir app tauri dev`.
- Line endings follow `.gitattributes`: the generated `app/src/lib/protocol/*.ts` and `app/src/lib/backend/*.ts` are LF; other files follow `core.autocrlf=true`.
- Commit messages end with the trailer lines:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01MJEyVJJDCjpX3WvL67mT1t
  ```
  The `git commit -m` lines in the tasks below show only the subject; add the trailer as the last paragraph of every commit.

## File Structure

```
app/src-tauri/
  Cargo.toml                         # + windows-sys features (Tasks 12, 13)
  build.rs                           # custom Windows manifest (Task 13)
  windows-app-manifest.xml           # NEW: Common Controls 6 + supportedOS Windows 10/11 (Task 13)
  src/lib.rs                         # + pub mod layout; pub mod snap
  src/layout.rs                      # NEW: layout.json types, load, atomic save (Task 1)
  src/snap.rs                        # NEW: maximize-button input sink for snap layouts (Task 13)
  src/links.rs                       # + COM apartment around ShellExecuteW (Task 12)
  src/connection.rs                  # run_with; Retry during connect (Task 14)
  src/main.rs                        # get_layout, save_layout, async open_url, set_maximize_button_rect, clipboard access
  tests/bindings.rs                  # + layout types
app/src/
  App.svelte                         # PaneGrid, ConfirmDialog, ContextMenu, global shortcuts
  themes/neon-glass.json             # + scrim, menu, shadow tokens (Task 9)
  lib/backend/{LayoutFile,ProjectLayout,TabLayout,LayoutNode,SplitDirection}.ts   # generated (Task 1)
  lib/layout/tree.ts (+ test)        # NEW: split tree operations (Task 2)
  lib/layout/geometry.ts (+ test)    # NEW: pane and divider rectangles, focus by direction (Task 3)
  lib/layout/tabs.ts (+ test)        # NEW: tab operations on one project (Task 4)
  lib/layout/reconcile.ts (+ test)   # NEW: layout.json vs daemon sessions (Task 4)
  lib/state/workspace.ts (+ test)    # NEW: placing created sessions, two quick creates (Task 5)
  lib/state/sessionRules.ts (+ test) # NEW: busy states, close-tab summary, restart (Task 5)
  lib/state/app.svelte.ts            # rewritten around the workspace (Task 9)
  lib/state/dialog.svelte.ts         # NEW: confirmation dialog state (Task 9)
  lib/state/ui.svelte.ts             # NEW: open menu and inline rename state (Task 10)
  lib/state/notices.svelte.ts        # NEW: per-pane header notices (Task 10)
  lib/state/clipboard.ts             # NEW: copy and paste for a pane (Task 10)
  lib/state/contextMenus.ts          # NEW: menus bound to app actions (Task 10)
  lib/state/actions.ts               # NEW: shortcut action dispatch (Task 11)
  lib/state/banner.ts (+ test)       # NEW: connection banner text (Task 14)
  lib/state/views.ts (+ test)        # nextActiveAfterRemoval removed (Task 9)
  lib/terminal/attachGate.ts (+ test)   # one outstanding Attach (Task 6)
  lib/terminal/flowControl.ts (+ test)  # NEW: write watermarks (Task 6)
  lib/terminal/paneStream.ts (+ test)   # NEW: gate + flow for one pane (Task 6)
  lib/terminal/router.ts (+ test)       # reattach(afterReconnect), attachFailed (Task 6)
  lib/terminal/registry.ts           # NEW: mounted terminals by session id (Task 9)
  lib/terminal/links.ts              # NEW: open a link with optional confirmation (Task 12)
  lib/input/shortcuts.ts (+ test)    # NEW: shortcut resolution, Ctrl+C rules (Task 7)
  lib/input/mouse.ts (+ test)        # NEW: right-click routing rule (Task 7)
  lib/menu/types.ts                  # NEW: menu entry types (Task 8)
  lib/menu/navigation.ts (+ test)    # NEW: highlight movement, placement (Task 8)
  lib/menu/menus.ts (+ test)         # NEW: pane, tab and session menu builders (Task 8)
  lib/window/snap.ts (+ test)        # NEW: CSS to physical pixel rectangle (Task 13)
  lib/errors.ts                      # NEW: error text from invoke rejections (Task 9)
  ui/Badge.svelte                    # NEW: state badge shared by sidebar, tabs, pane headers (Task 9)
  ui/PaneGrid.svelte                 # NEW: absolutely positioned panes and dividers (Task 9)
  ui/PaneView.svelte                 # NEW: header strip + terminal, focus outline (Tasks 9, 10)
  ui/ConfirmDialog.svelte            # NEW (Task 9)
  ui/ContextMenu.svelte              # NEW (Task 10)
  ui/RenameField.svelte              # NEW (Task 10)
  ui/TerminalPane.svelte             # pane stream, registry, menus, shortcuts, copy, links
  ui/TabBar.svelte  ui/Sidebar.svelte  ui/Banner.svelte  ui/WindowControls.svelte
docs/manual-test-checklist.md        # + Plan 3 section (Task 15)
```

Tasks 2-8 are pure TypeScript with tests and touch no UI. Task 9 switches the UI to the layout model; Tasks 10-13 add menus, shortcuts, links and window chrome on top of it; Task 14 is independent backend and banner work; Task 15 is the checklist and installer.

---

### Task 1: layout.json storage in the backend

**Files:**
- Create: `app/src-tauri/src/layout.rs`
- Modify: `app/src-tauri/src/lib.rs`, `app/src-tauri/src/main.rs`, `app/src-tauri/tests/bindings.rs`
- Generated: `app/src/lib/backend/LayoutFile.ts`, `ProjectLayout.ts`, `TabLayout.ts`, `LayoutNode.ts`, `SplitDirection.ts`
- Test: `app/src-tauri/src/layout.rs` (tests module), `app/src-tauri/tests/bindings.rs`

**Interfaces:**
- Consumes: the `settings.rs` pattern (`default_path()` under `%APPDATA%\Jojo`, `load(&Path)` returning defaults on failure) and the `jojod::store` atomic-write pattern.
- Produces (used by every later task):
  - Rust `jojo_app::layout::{LAYOUT_VERSION: u32 = 1, SplitDirection { Row, Column }, LayoutNode::{Pane { session_id }, Split { direction, children, sizes }}, TabLayout { id, name: Option<String>, root, focused, zoomed: Option<String> }, ProjectLayout { tabs, active_tab: Option<String> }, LayoutFile { version, projects: BTreeMap<String, ProjectLayout> }, default_path() -> PathBuf, load(&Path) -> LayoutFile, save(&Path, &LayoutFile) -> io::Result<()>}`.
  - Tauri commands `get_layout() -> LayoutFile` and `save_layout(layout: LayoutFile) -> Result<(), String>` (async).
  - TypeScript (generated): `SplitDirection = "row" | "column"`; `LayoutNode = { "type": "pane", session_id: string, } | { "type": "split", direction: SplitDirection, children: Array<LayoutNode>, sizes: Array<number>, }`; `TabLayout = { id: string, name: string | null, root: LayoutNode, focused: string, zoomed: string | null, }`; `ProjectLayout = { tabs: Array<TabLayout>, active_tab: string | null, }`; `LayoutFile = { version: number, projects: { [key in string]: ProjectLayout }, }`.

A `"row"` split places its children side by side (left to right); a `"column"` split stacks them (top to bottom). `sizes` holds one positive fraction per child and sums to 1. The backend does not enforce these rules; Task 4's reconciliation does, because only the frontend knows the daemon's current sessions.

- [ ] **Step 1: Write the failing tests**

Create `app/src-tauri/src/layout.rs` with only the tests module and an empty body above it, so the tests fail to compile:
```rust
//! %APPDATA%\Jojo\layout.json (spec section 4, "Layout" row).

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn sample() -> LayoutFile {
        let tab = TabLayout {
            id: "T1".into(),
            name: Some("work".into()),
            root: LayoutNode::Split {
                direction: SplitDirection::Row,
                children: vec![
                    LayoutNode::Pane { session_id: "S1".into() },
                    LayoutNode::Pane { session_id: "S2".into() },
                ],
                sizes: vec![0.6, 0.4],
            },
            focused: "S2".into(),
            zoomed: None,
        };
        let mut projects = BTreeMap::new();
        projects.insert("local:P1".to_string(), ProjectLayout { tabs: vec![tab], active_tab: Some("T1".into()) });
        LayoutFile { version: LAYOUT_VERSION, projects }
    }

    #[test]
    fn missing_file_gives_an_empty_layout() {
        let dir = tempfile::tempdir().unwrap();
        let layout = load(&dir.path().join("layout.json"));
        assert_eq!(layout, LayoutFile::default());
        assert_eq!(layout.version, LAYOUT_VERSION);
        assert!(layout.projects.is_empty());
    }

    #[test]
    fn save_then_load_roundtrips_and_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Jojo").join("layout.json");
        save(&path, &sample()).unwrap();
        assert!(!dir.path().join("Jojo").join("layout.json.tmp").exists());
        assert_eq!(load(&path), sample());
    }

    #[test]
    fn save_replaces_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("layout.json");
        save(&path, &LayoutFile::default()).unwrap();
        save(&path, &sample()).unwrap();
        assert_eq!(load(&path), sample());
    }

    #[test]
    fn unreadable_file_is_set_aside_and_gives_an_empty_layout() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("layout.json");
        std::fs::write(&path, "{ nope").unwrap();
        assert_eq!(load(&path), LayoutFile::default());
        assert!(!path.exists(), "the unreadable file must be renamed, not left for the next save to overwrite");
        let names: Vec<String> =
            std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().any(|n| n.starts_with("layout.json.corrupt-")), "{names:?}");
    }

    #[test]
    fn a_file_with_another_version_is_set_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("layout.json");
        std::fs::write(&path, r#"{ "version": 99, "projects": {} }"#).unwrap();
        assert_eq!(load(&path), LayoutFile::default());
        assert!(!path.exists());
    }

    #[test]
    fn json_shape_matches_the_frontend_types() {
        let json = serde_json::to_value(sample()).unwrap();
        let root = &json["projects"]["local:P1"]["tabs"][0]["root"];
        assert_eq!(root["type"], "split");
        assert_eq!(root["direction"], "row");
        assert_eq!(root["children"][0], serde_json::json!({ "type": "pane", "session_id": "S1" }));
        assert_eq!(json["projects"]["local:P1"]["active_tab"], "T1");
        assert_eq!(json["projects"]["local:P1"]["tabs"][0]["zoomed"], serde_json::Value::Null);
    }
}
```

Add `pub mod layout;` to `app/src-tauri/src/lib.rs` (after `pub mod daemon_manager;`, keeping alphabetical order: `bridge`, `connection`, `daemon_manager`, `layout`, `links`, `settings`).

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jojo-app --jobs 4 --lib layout::`
Expected: FAIL to compile with "cannot find type `LayoutFile` in this scope" (and the other layout names).

- [ ] **Step 3: Write the implementation**

Replace the top of `app/src-tauri/src/layout.rs` (everything above `#[cfg(test)]`) with:
```rust
//! %APPDATA%\Jojo\layout.json (spec section 4, "Layout" row): the ordered
//! tabs of every project, each tab's split tree of session ids, its focused
//! pane, and the active tab. Keys are `"<machine id>:<project id>"`, because a
//! project id is unique only within one daemon.
//!
//! The backend only stores and loads the document. Every semantic rule (at
//! most 4 panes per tab, split sizes that sum to 1, session ids that still
//! exist on the daemon, no session in two tabs) is applied by the frontend in
//! app/src/lib/layout/reconcile.ts, because only the frontend has the
//! daemon's session list.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use ts_rs::TS;

/// Format version written to `version`. A file with any other version is
/// set aside like an unreadable one.
pub const LAYOUT_VERSION: u32 = 1;

/// `Row` places children side by side, left to right; `Column` stacks them,
/// top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SplitDirection {
    Row,
    Column,
}

/// One node of a tab's split tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LayoutNode {
    Pane {
        session_id: String,
    },
    /// `sizes` has one positive fraction per child and sums to 1.
    Split { direction: SplitDirection, children: Vec<LayoutNode>, sizes: Vec<f64> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct TabLayout {
    pub id: String,
    /// A name the user gave the tab; `None` shows the focused session's label.
    #[serde(default)]
    pub name: Option<String>,
    pub root: LayoutNode,
    /// Session id of the focused pane.
    pub focused: String,
    /// Session id of the zoomed pane, which then fills the tab.
    #[serde(default)]
    pub zoomed: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct ProjectLayout {
    pub tabs: Vec<TabLayout>,
    #[serde(default)]
    pub active_tab: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct LayoutFile {
    pub version: u32,
    pub projects: BTreeMap<String, ProjectLayout>,
}

impl Default for LayoutFile {
    fn default() -> Self {
        LayoutFile { version: LAYOUT_VERSION, projects: BTreeMap::new() }
    }
}

pub fn default_path() -> PathBuf {
    let roaming = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    roaming.join("Jojo").join("layout.json")
}

/// Reads the layout. A missing file gives an empty layout. A file that does
/// not parse, or that has a version other than `LAYOUT_VERSION`, is renamed
/// to `layout.json.corrupt-<unix milliseconds>` (so the next save does not
/// overwrite it) and an empty layout is returned. An empty layout is safe:
/// reconciliation then opens one tab per daemon session.
pub fn load(path: &Path) -> LayoutFile {
    let Ok(bytes) = std::fs::read(path) else {
        return LayoutFile::default();
    };
    match serde_json::from_slice::<LayoutFile>(&bytes) {
        Ok(layout) if layout.version == LAYOUT_VERSION => layout,
        _ => {
            set_aside(path);
            LayoutFile::default()
        }
    }
}

fn set_aside(path: &Path) {
    let mut stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    loop {
        let mut name = path.as_os_str().to_owned();
        name.push(format!(".corrupt-{stamp}"));
        let candidate = PathBuf::from(name);
        if !candidate.exists() {
            // A failed rename leaves the file in place; the next save then
            // replaces it, which is the only remaining option.
            let _ = std::fs::rename(path, candidate);
            return;
        }
        stamp += 1;
    }
}

/// Writes the layout atomically: the JSON goes to `layout.json.tmp`, which is
/// then renamed over `layout.json` (`std::fs::rename` replaces an existing
/// file on Windows), so a crash mid-write never leaves a truncated file.
pub fn save(path: &Path, layout: &LayoutFile) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let bytes = serde_json::to_vec_pretty(layout).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jojo-app --jobs 4 --lib layout::`
Expected: PASS, 6 tests.

- [ ] **Step 5: Add the commands**

In `app/src-tauri/src/main.rs`, change the settings import line to:
```rust
use jojo_app::layout::{self, LayoutFile};
use jojo_app::settings::{self, Settings};
```
Add after `get_settings`. The parameter of `save_layout` must be named `layout` (Tauri maps the invoke argument `{ layout }` by name), and it shadows the `layout` module inside that function, so the function calls the module by its full path:
```rust
#[tauri::command]
fn get_layout() -> LayoutFile {
    layout::load(&layout::default_path())
}

/// Async so the file write runs on a blocking worker thread instead of the
/// main thread. The frontend chains its calls, so two saves never overlap.
#[tauri::command]
async fn save_layout(layout: LayoutFile) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || jojo_app::layout::save(&jojo_app::layout::default_path(), &layout))
        .await
        .map_err(|e| format!("the layout writer thread failed: {e}"))?
        .map_err(|e| format!("could not write layout.json: {e}"))
}
```
and change the handler list to:
```rust
        .invoke_handler(tauri::generate_handler![
            subscribe,
            send_frame,
            restart_daemon,
            get_settings,
            get_layout,
            save_layout,
            open_url
        ])
```

- [ ] **Step 6: Generate the TypeScript types**

Replace `app/src-tauri/tests/bindings.rs` with:
```rust
//! Generates the TypeScript types for the app backend's own messages and
//! files (app/src/lib/backend). scripts/check-protocol.ps1 fails when they
//! drift.

use jojo_app::bridge::ConnectionStatus;
use jojo_app::layout::LayoutFile;
use jojo_app::settings::Settings;
use std::path::PathBuf;
use ts_rs::{Config, TS};

#[test]
fn export_backend_bindings() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/lib/backend");
    let cfg = Config::new().with_out_dir(&dir);
    ConnectionStatus::export_all(&cfg).unwrap();
    Settings::export_all(&cfg).unwrap();
    LayoutFile::export_all(&cfg).unwrap();
    let status = std::fs::read_to_string(dir.join("ConnectionStatus.ts")).unwrap();
    assert!(status.contains("\"state\": \"version_mismatch\""), "{status}");
    let settings = std::fs::read_to_string(dir.join("Settings.ts")).unwrap();
    assert!(settings.contains("terminalFontSize: number"), "{settings}");
    assert!(!settings.contains("bigint"), "{settings}");
    let node = std::fs::read_to_string(dir.join("LayoutNode.ts")).unwrap();
    assert!(node.contains("\"type\": \"pane\", session_id: string,"), "{node}");
    assert!(node.contains("children: Array<LayoutNode>"), "{node}");
    assert!(node.contains("sizes: Array<number>"), "{node}");
    let tab = std::fs::read_to_string(dir.join("TabLayout.ts")).unwrap();
    assert!(tab.contains("zoomed: string | null"), "{tab}");
    let file = std::fs::read_to_string(dir.join("LayoutFile.ts")).unwrap();
    assert!(file.contains("ProjectLayout"), "{file}");
    assert!(!file.contains("bigint"), "{file}");
}
```
Run: `cargo test -p jojo-app --jobs 4 --test bindings`
Expected: PASS; `app/src/lib/backend/` now also holds `LayoutFile.ts`, `ProjectLayout.ts`, `TabLayout.ts`, `LayoutNode.ts`, `SplitDirection.ts`. Open `LayoutNode.ts` and confirm the union matches the Interfaces block above.

- [ ] **Step 7: Build and lint**

Run: `cargo clippy -p jojo-app --all-targets --jobs 4` and `pnpm --dir app check`
Expected: no warnings from clippy; svelte-check reports 0 errors.

- [ ] **Step 8: Commit, then verify the generated files are committed**

```bash
git add app/src-tauri/src/layout.rs app/src-tauri/src/lib.rs app/src-tauri/src/main.rs app/src-tauri/tests/bindings.rs app/src/lib/backend/
git commit -m "feat(app): store layout.json in the backend with generated types"
```
Then run: `CARGO_BUILD_JOBS=4 pwsh -NoProfile -File scripts/check-protocol.ps1`
Expected: `Generated types are up to date.`

---

### Task 2: Split tree operations

**Files:**
- Create: `app/src/lib/layout/tree.ts`
- Test: `app/src/lib/layout/tree.test.ts`

**Interfaces:**
- Consumes: generated `LayoutNode` and `SplitDirection` (Task 1).
- Produces (used by Tasks 3, 4, 5, 8, 9):
  - `type LayoutNode`, `type SplitDirection` (re-exported), `type SplitNode`, `type PaneNode`, `type LayoutPreset = "columns" | "rows" | "grid" | "main_stack"`.
  - `MAX_PANES = 4`, `MIN_SHARE = 0.1`.
  - `pane(sessionId: string): PaneNode`; `split(direction, children: LayoutNode[], sizes?: number[]): SplitNode`; `equalSizes(n: number): number[]`.
  - `paneIds(node): string[]` (depth first: left to right, top to bottom); `countPanes(node): number`; `containsPane(node, id): boolean`; `isFull(node): boolean`.
  - `normalize(node): LayoutNode | null`; `filterPanes(node, keep: (id: string) => boolean): LayoutNode | null`.
  - `splitPane(root, targetId, newId, direction): LayoutNode | null` (null when full, target missing, or `newId` already present).
  - `removePane(root, id): LayoutNode | null` (null when the last pane is removed).
  - `swapPanes(root, a, b): LayoutNode`; `replacePane(root, oldId, newId): LayoutNode`; `equalize(root): LayoutNode`.
  - `resizeSplit(root, path: number[], index: number, delta: number): LayoutNode` (moves the boundary after child `index` of the split at `path` by `delta`, a fraction of that split's extent; each side stays at or above `MIN_SHARE`).
  - `applyPreset(root, preset, focusedId): LayoutNode`.

- [ ] **Step 1: Write the failing tests**

`app/src/lib/layout/tree.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import {
  applyPreset,
  containsPane,
  countPanes,
  equalize,
  filterPanes,
  isFull,
  MAX_PANES,
  MIN_SHARE,
  normalize,
  pane,
  paneIds,
  removePane,
  replacePane,
  resizeSplit,
  split,
  splitPane,
  swapPanes,
  type LayoutNode,
} from "./tree";

const a = pane("A");
const b = pane("B");
const c = pane("C");
const d = pane("D");

/** Rounds every size to 3 decimals so trees with float sizes compare with toEqual. */
function rounded(node: LayoutNode | null): LayoutNode | null {
  if (!node || node.type === "pane") return node;
  return {
    ...node,
    children: node.children.map((child) => rounded(child)!),
    sizes: node.sizes.map((s) => Math.round(s * 1000) / 1000),
  };
}

describe("paneIds", () => {
  it("lists panes depth first, left to right and top to bottom", () => {
    const root = split("row", [a, split("column", [b, c])]);
    expect(paneIds(root)).toEqual(["A", "B", "C"]);
    expect(countPanes(root)).toBe(3);
    expect(containsPane(root, "C")).toBe(true);
    expect(containsPane(root, "Z")).toBe(false);
  });

  it("gives a new split equal sizes", () => {
    expect(split("row", [a, b, c]).sizes.map((s) => Math.round(s * 1000))).toEqual([333, 333, 333]);
  });
});

describe("splitPane", () => {
  it("turns a single pane into a split of two halves", () => {
    expect(splitPane(a, "A", "B", "row")).toEqual({ type: "split", direction: "row", children: [a, b], sizes: [0.5, 0.5] });
  });

  it("inserts after the target in a split of the same direction, halving the target's share", () => {
    const root = split("row", [a, b], [0.6, 0.4]);
    expect(rounded(splitPane(root, "A", "C", "row"))).toEqual(split("row", [a, c, b], [0.3, 0.3, 0.4]));
  });

  it("nests a new split when the direction differs", () => {
    const root = split("row", [a, b]);
    expect(splitPane(root, "B", "C", "column")).toEqual(split("row", [a, split("column", [b, c])]));
  });

  it("refuses a fifth pane, an unknown target and an id already in the tree", () => {
    const full = split("row", [a, b, c, d]);
    expect(MAX_PANES).toBe(4);
    expect(isFull(full)).toBe(true);
    expect(splitPane(full, "A", "E", "row")).toBeNull();
    expect(splitPane(a, "X", "B", "row")).toBeNull();
    expect(splitPane(split("row", [a, b]), "A", "B", "column")).toBeNull();
  });
});

describe("removePane", () => {
  it("returns the remaining pane when one of two is removed", () => {
    expect(removePane(split("row", [a, b]), "A")).toEqual(b);
  });

  it("gives the removed share to the siblings in proportion", () => {
    const root = split("row", [a, b, c], [0.5, 0.25, 0.25]);
    expect(rounded(removePane(root, "A"))).toEqual(split("row", [b, c], [0.5, 0.5]));
  });

  it("returns null when the last pane is removed", () => {
    expect(removePane(a, "A")).toBeNull();
  });

  it("collapses a one-child split and merges it into a parent of the same direction", () => {
    const root = split("row", [a, split("column", [b, split("row", [c, d])])], [0.5, 0.5]);
    expect(rounded(removePane(root, "B"))).toEqual(split("row", [a, c, d], [0.5, 0.25, 0.25]));
  });

  it("leaves the tree unchanged for an unknown id", () => {
    const root = split("row", [a, b]);
    expect(removePane(root, "Z")).toEqual(root);
  });
});

describe("normalize", () => {
  it("replaces invalid or mismatched sizes with equal shares", () => {
    expect(normalize({ type: "split", direction: "row", children: [a, b], sizes: [Number.NaN, 1] })).toEqual(split("row", [a, b]));
    expect(normalize({ type: "split", direction: "row", children: [a, b], sizes: [1] })).toEqual(split("row", [a, b]));
    expect(normalize({ type: "split", direction: "row", children: [a, b], sizes: [-1, 2] })).toEqual(split("row", [a, b]));
  });

  it("rescales sizes to sum to 1", () => {
    expect(normalize({ type: "split", direction: "column", children: [a, b], sizes: [3, 1] })).toEqual(split("column", [a, b], [0.75, 0.25]));
  });

  it("replaces a one-child split with the child and drops an empty split", () => {
    expect(normalize({ type: "split", direction: "row", children: [a], sizes: [1] })).toEqual(a);
    expect(normalize({ type: "split", direction: "row", children: [], sizes: [] })).toBeNull();
  });
});

describe("filterPanes", () => {
  it("keeps the panes the predicate accepts, in order", () => {
    const root = split("row", [a, split("column", [b, c]), d]);
    expect(paneIds(filterPanes(root, (id) => id !== "B" && id !== "D")!)).toEqual(["A", "C"]);
    expect(filterPanes(root, () => false)).toBeNull();
  });
});

describe("swap, replace and equalize", () => {
  it("exchanges two panes' positions", () => {
    const root = split("row", [a, split("column", [b, c])]);
    expect(paneIds(swapPanes(root, "A", "C"))).toEqual(["C", "B", "A"]);
    expect(swapPanes(root, "A", "Z")).toBe(root);
  });

  it("replaces one session id with another in place", () => {
    const root = split("row", [a, b], [0.7, 0.3]);
    expect(replacePane(root, "A", "N")).toEqual(split("row", [pane("N"), b], [0.7, 0.3]));
    expect(replacePane(root, "A", "B")).toBe(root);
  });

  it("resets every split to equal shares", () => {
    const root = split("row", [a, split("column", [b, c], [0.9, 0.1])], [0.8, 0.2]);
    expect(equalize(root)).toEqual(split("row", [a, split("column", [b, c])]));
  });
});

describe("resizeSplit", () => {
  it("moves the boundary after the given child", () => {
    const root = split("row", [a, b]);
    expect(rounded(resizeSplit(root, [], 0, 0.2))).toEqual(split("row", [a, b], [0.7, 0.3]));
  });

  it("resizes a nested split by its path", () => {
    const root = split("row", [a, split("column", [b, c])]);
    expect(rounded(resizeSplit(root, [1], 0, -0.25))).toEqual(split("row", [a, split("column", [b, c], [0.25, 0.75])]));
  });

  it("keeps both sides at or above MIN_SHARE", () => {
    const root = split("row", [a, b]);
    const moved = resizeSplit(root, [], 0, 5) as Extract<LayoutNode, { type: "split" }>;
    expect(moved.sizes[1]).toBeCloseTo(MIN_SHARE);
    expect(moved.sizes[0]).toBeCloseTo(1 - MIN_SHARE);
  });

  it("ignores a path or index that does not name a boundary", () => {
    const root = split("row", [a, b]);
    expect(resizeSplit(root, [0], 0, 0.1)).toEqual(root);
    expect(resizeSplit(root, [], 1, 0.1)).toEqual(root);
  });
});

describe("applyPreset", () => {
  const four = split("row", [a, split("column", [b, c]), d]);

  it("lays out columns and rows in pane order", () => {
    expect(applyPreset(four, "columns", "A")).toEqual(split("row", [a, b, c, d]));
    expect(applyPreset(four, "rows", "A")).toEqual(split("column", [a, b, c, d]));
  });

  it("builds a 2x2 grid, with the third pane full width when there are three", () => {
    expect(applyPreset(four, "grid", "A")).toEqual(split("column", [split("row", [a, b]), split("row", [c, d])]));
    expect(applyPreset(split("row", [a, b, c]), "grid", "A")).toEqual(split("column", [split("row", [a, b]), c]));
    expect(applyPreset(split("column", [a, b]), "grid", "A")).toEqual(split("row", [a, b]));
  });

  it("puts the focused pane at 60% on the left and stacks the others", () => {
    expect(applyPreset(four, "main_stack", "C")).toEqual(split("row", [c, split("column", [a, b, d])], [0.6, 0.4]));
    expect(applyPreset(split("column", [a, b]), "main_stack", "B")).toEqual(split("row", [b, a], [0.6, 0.4]));
  });

  it("leaves a single pane unchanged", () => {
    expect(applyPreset(a, "grid", "A")).toBe(a);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --dir app exec vitest run src/lib/layout/tree.test.ts`
Expected: FAIL with "Failed to resolve import "./tree"".

- [ ] **Step 3: Write the implementation**

`app/src/lib/layout/tree.ts`:
```ts
// Pure operations on a tab's split tree (spec sections 9.3 and 9.4). No
// function mutates its input. A valid tree holds 1 to MAX_PANES panes; every
// split has at least two children and one size per child, and its sizes are
// positive fractions that sum to 1. A "row" split places its children side
// by side, left to right; a "column" split stacks them, top to bottom.
import type { LayoutNode } from "../backend/LayoutNode";
import type { SplitDirection } from "../backend/SplitDirection";

export type { LayoutNode, SplitDirection };
export type SplitNode = Extract<LayoutNode, { type: "split" }>;
export type PaneNode = Extract<LayoutNode, { type: "pane" }>;
export type LayoutPreset = "columns" | "rows" | "grid" | "main_stack";

/** Spec section 9.4, "Limits": a tab holds at most 4 panes. */
export const MAX_PANES = 4;

/** Smallest share of its split that a divider drag leaves a child. */
export const MIN_SHARE = 0.1;

export function pane(sessionId: string): PaneNode {
  return { type: "pane", session_id: sessionId };
}

export function equalSizes(n: number): number[] {
  return Array.from({ length: n }, () => 1 / n);
}

export function split(direction: SplitDirection, children: LayoutNode[], sizes?: number[]): SplitNode {
  return { type: "split", direction, children, sizes: sizes ?? equalSizes(children.length) };
}

export function paneIds(node: LayoutNode): string[] {
  return node.type === "pane" ? [node.session_id] : node.children.flatMap(paneIds);
}

export function countPanes(node: LayoutNode): number {
  return paneIds(node).length;
}

export function containsPane(node: LayoutNode, sessionId: string): boolean {
  return paneIds(node).includes(sessionId);
}

export function isFull(node: LayoutNode): boolean {
  return countPanes(node) >= MAX_PANES;
}

function normalizeSizes(sizes: number[]): number[] {
  const valid = sizes.length > 0 && sizes.every((s) => Number.isFinite(s) && s > 0);
  if (!valid) return equalSizes(sizes.length);
  const total = sizes.reduce((sum, s) => sum + s, 0);
  return sizes.map((s) => s / total);
}

/**
 * Restores the tree invariants. Drops splits that have no children, replaces
 * a split that has one child with that child, merges a child split into its
 * parent when both run in the same direction (the child's sizes are scaled
 * by its share of the parent), and rescales sizes to sum to 1. Sizes that
 * are missing, of the wrong length, or not positive finite numbers become
 * equal shares. Returns null for a tree with no panes.
 */
export function normalize(node: LayoutNode): LayoutNode | null {
  if (node.type === "pane") return node;
  const shares = node.sizes.length === node.children.length ? node.sizes : equalSizes(node.children.length);
  const children: LayoutNode[] = [];
  const sizes: number[] = [];
  node.children.forEach((child, index) => {
    const normalized = normalize(child);
    if (!normalized) return;
    const share = shares[index];
    if (normalized.type === "split" && normalized.direction === node.direction) {
      normalized.children.forEach((grandchild, j) => {
        children.push(grandchild);
        sizes.push(share * normalized.sizes[j]);
      });
    } else {
      children.push(normalized);
      sizes.push(share);
    }
  });
  if (children.length === 0) return null;
  if (children.length === 1) return children[0];
  return { type: "split", direction: node.direction, children, sizes: normalizeSizes(sizes) };
}

/**
 * Keeps the panes for which `keep` returns true and normalizes the result,
 * so a removed pane's share goes to its siblings in proportion to their
 * sizes. Returns null when no pane is kept.
 */
export function filterPanes(node: LayoutNode, keep: (sessionId: string) => boolean): LayoutNode | null {
  if (node.type === "pane") return keep(node.session_id) ? node : null;
  const children: LayoutNode[] = [];
  const sizes: number[] = [];
  node.children.forEach((child, index) => {
    const kept = filterPanes(child, keep);
    if (kept) {
      children.push(kept);
      sizes.push(node.sizes[index]);
    }
  });
  if (children.length === 0) return null;
  return normalize({ ...node, children, sizes });
}

function insertBeside(node: LayoutNode, targetId: string, newId: string, direction: SplitDirection): LayoutNode {
  if (node.type === "pane") {
    return node.session_id === targetId ? split(direction, [node, pane(newId)]) : node;
  }
  const index = node.children.findIndex((child) => child.type === "pane" && child.session_id === targetId);
  if (index >= 0 && node.direction === direction) {
    const half = node.sizes[index] / 2;
    const children = [...node.children];
    children.splice(index + 1, 0, pane(newId));
    const sizes = [...node.sizes];
    sizes.splice(index, 1, half, half);
    return { ...node, children, sizes };
  }
  return { ...node, children: node.children.map((child) => insertBeside(child, targetId, newId, direction)) };
}

/**
 * Adds `newId` next to `targetId`: to its right for "row", below it for
 * "column". Inside a split of the same direction the new pane is inserted
 * after the target and the two share the target's former size; otherwise
 * the target is replaced by a new two-pane split. Returns null when the
 * tree is full, the target is missing, or `newId` is already in the tree.
 */
export function splitPane(root: LayoutNode, targetId: string, newId: string, direction: SplitDirection): LayoutNode | null {
  if (isFull(root) || !containsPane(root, targetId) || containsPane(root, newId)) return null;
  return normalize(insertBeside(root, targetId, newId, direction));
}

export function removePane(root: LayoutNode, sessionId: string): LayoutNode | null {
  return filterPanes(root, (id) => id !== sessionId);
}

function mapPanes(node: LayoutNode, rename: (sessionId: string) => string): LayoutNode {
  if (node.type === "pane") return pane(rename(node.session_id));
  return { ...node, children: node.children.map((child) => mapPanes(child, rename)) };
}

/** Exchanges the positions of panes `a` and `b`; unchanged if either is missing. */
export function swapPanes(root: LayoutNode, a: string, b: string): LayoutNode {
  if (a === b || !containsPane(root, a) || !containsPane(root, b)) return root;
  return mapPanes(root, (id) => (id === a ? b : id === b ? a : id));
}

/** Puts `newId` where `oldId` was; unchanged if `oldId` is missing or `newId` is present. */
export function replacePane(root: LayoutNode, oldId: string, newId: string): LayoutNode {
  if (!containsPane(root, oldId) || containsPane(root, newId)) return root;
  return mapPanes(root, (id) => (id === oldId ? newId : id));
}

export function equalize(node: LayoutNode): LayoutNode {
  if (node.type === "pane") return node;
  return { ...node, children: node.children.map(equalize), sizes: equalSizes(node.children.length) };
}

function updateAt(node: LayoutNode, path: number[], update: (node: LayoutNode) => LayoutNode): LayoutNode {
  if (path.length === 0) return update(node);
  if (node.type !== "split" || path[0] < 0 || path[0] >= node.children.length) return node;
  const [head, ...rest] = path;
  const children = node.children.map((child, index) => (index === head ? updateAt(child, rest, update) : child));
  return { ...node, children };
}

/**
 * Moves the boundary between child `index` and child `index + 1` of the
 * split at `path` (child indexes from the root) by `delta`, a fraction of
 * that split's extent; positive moves right or down. Both children keep at
 * least MIN_SHARE. Unchanged when the path does not reach a split or the
 * index does not name a boundary.
 */
export function resizeSplit(root: LayoutNode, path: number[], index: number, delta: number): LayoutNode {
  return updateAt(root, path, (node) => {
    if (node.type !== "split" || index < 0 || index + 1 >= node.children.length) return node;
    const first = node.sizes[index];
    const second = node.sizes[index + 1];
    const lowest = MIN_SHARE - first;
    const highest = second - MIN_SHARE;
    if (lowest > highest) return node;
    const moved = Math.min(highest, Math.max(lowest, delta));
    const sizes = [...node.sizes];
    sizes[index] = first + moved;
    sizes[index + 1] = second - moved;
    return { ...node, sizes };
  });
}

/**
 * Rearranges every pane of a tab (spec 9.4, Layout ▸), keeping the panes'
 * current order. "grid" is 2 columns × 2 rows (with 3 panes the third spans
 * the bottom row; with 2 it is two columns). "main_stack" puts the focused
 * pane on the left at 60% width and stacks the others on the right.
 */
export function applyPreset(root: LayoutNode, preset: LayoutPreset, focusedId: string): LayoutNode {
  const ids = paneIds(root);
  if (ids.length < 2) return root;
  const leaves = ids.map(pane);
  switch (preset) {
    case "columns":
      return split("row", leaves);
    case "rows":
      return split("column", leaves);
    case "grid": {
      if (leaves.length === 2) return split("row", leaves);
      const bottom = leaves.length === 3 ? leaves[2] : split("row", leaves.slice(2, 4));
      return split("column", [split("row", leaves.slice(0, 2)), bottom]);
    }
    case "main_stack": {
      const main = ids.includes(focusedId) ? focusedId : ids[0];
      const others = ids.filter((id) => id !== main).map(pane);
      const stack = others.length === 1 ? others[0] : split("column", others);
      return split("row", [pane(main), stack], [0.6, 0.4]);
    }
  }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm --dir app exec vitest run src/lib/layout/tree.test.ts`
Expected: PASS, all tests in the file.

- [ ] **Step 5: Type-check**

Run: `pnpm --dir app check`
Expected: 0 errors.

- [ ] **Step 6: Commit**

```bash
git add app/src/lib/layout/tree.ts app/src/lib/layout/tree.test.ts
git commit -m "feat(app): split tree operations for pane layout"
```

---

### Task 3: Pane geometry and directional focus

**Files:**
- Create: `app/src/lib/layout/geometry.ts`
- Test: `app/src/lib/layout/geometry.test.ts`

**Interfaces:**
- Consumes: `LayoutNode`, `SplitDirection`, `containsPane` from `./tree` (Task 2).
- Produces (used by Tasks 9 and 11):
  - `interface Rect { x: number; y: number; w: number; h: number }` (fractions 0-1 of the tab's pane area).
  - `interface PaneRect extends Rect { sessionId: string }`.
  - `interface DividerRect { path: number[]; index: number; direction: SplitDirection; x: number; y: number; length: number; extent: number }`.
  - `interface TabRects { panes: PaneRect[]; dividers: DividerRect[] }`.
  - `type FocusDirection = "left" | "right" | "up" | "down"`.
  - `layoutRects(root: LayoutNode, zoomed: string | null): TabRects`.
  - `focusNeighbor(panes: PaneRect[], fromId: string, direction: FocusDirection): string | null`.

The UI renders panes as absolutely positioned boxes from `layoutRects`, keyed by session id, instead of rendering the tree recursively. Restructuring a tab (split, close, swap, preset) then changes only the boxes' positions, and Svelte never unmounts the xterm.js instance of a pane that stays in the tab.

- [ ] **Step 1: Write the failing tests**

`app/src/lib/layout/geometry.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { focusNeighbor, layoutRects } from "./geometry";
import { pane, split } from "./tree";

describe("layoutRects", () => {
  it("gives a single pane the whole area and no dividers", () => {
    expect(layoutRects(pane("A"), null)).toEqual({ panes: [{ sessionId: "A", x: 0, y: 0, w: 1, h: 1 }], dividers: [] });
  });

  it("places row children left to right with a vertical divider between them", () => {
    const { panes, dividers } = layoutRects(split("row", [pane("A"), pane("B")], [0.6, 0.4]), null);
    expect(panes[0]).toMatchObject({ sessionId: "A", x: 0, y: 0, h: 1 });
    expect(panes[0].w).toBeCloseTo(0.6);
    expect(panes[1].x).toBeCloseTo(0.6);
    expect(panes[1].w).toBeCloseTo(0.4);
    expect(dividers).toHaveLength(1);
    expect(dividers[0]).toMatchObject({ path: [], index: 0, direction: "row", y: 0, length: 1, extent: 1 });
    expect(dividers[0].x).toBeCloseTo(0.6);
  });

  it("nests a column split inside a row split", () => {
    const { panes, dividers } = layoutRects(split("row", [pane("A"), split("column", [pane("B"), pane("C")])]), null);
    const c = panes.find((p) => p.sessionId === "C")!;
    expect([c.x, c.y, c.w, c.h].map((v) => Math.round(v * 100))).toEqual([50, 50, 50, 50]);
    const inner = dividers.find((d) => d.direction === "column")!;
    expect(inner).toMatchObject({ path: [1], index: 0 });
    expect([inner.x, inner.y, inner.length, inner.extent].map((v) => Math.round(v * 100))).toEqual([50, 50, 50, 100]);
  });

  it("shows only the zoomed pane, at full size", () => {
    const root = split("row", [pane("A"), pane("B")]);
    expect(layoutRects(root, "B")).toEqual({ panes: [{ sessionId: "B", x: 0, y: 0, w: 1, h: 1 }], dividers: [] });
  });

  it("ignores a zoomed id that is not in the tree", () => {
    expect(layoutRects(split("row", [pane("A"), pane("B")]), "Z").panes).toHaveLength(2);
  });
});

describe("focusNeighbor", () => {
  const grid = layoutRects(split("column", [split("row", [pane("A"), pane("B")]), split("row", [pane("C"), pane("D")])]), null).panes;

  it("moves to the adjacent pane in each direction", () => {
    expect(focusNeighbor(grid, "A", "right")).toBe("B");
    expect(focusNeighbor(grid, "A", "down")).toBe("C");
    expect(focusNeighbor(grid, "D", "left")).toBe("C");
    expect(focusNeighbor(grid, "D", "up")).toBe("B");
  });

  it("returns null at an edge and for an unknown pane", () => {
    expect(focusNeighbor(grid, "A", "left")).toBeNull();
    expect(focusNeighbor(grid, "A", "up")).toBeNull();
    expect(focusNeighbor(grid, "Z", "left")).toBeNull();
  });

  it("picks the pane with the most overlap, then the first in pane order", () => {
    const stack = layoutRects(split("row", [pane("M"), split("column", [pane("S1"), pane("S2")])], [0.6, 0.4]), null).panes;
    expect(focusNeighbor(stack, "M", "right")).toBe("S1");
    expect(focusNeighbor(stack, "S2", "left")).toBe("M");
    const wide = layoutRects(split("column", [split("row", [pane("A"), pane("B")], [0.3, 0.7]), pane("C")]), null).panes;
    expect(focusNeighbor(wide, "C", "up")).toBe("B");
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --dir app exec vitest run src/lib/layout/geometry.test.ts`
Expected: FAIL with "Failed to resolve import "./geometry"".

- [ ] **Step 3: Write the implementation**

`app/src/lib/layout/geometry.ts`:
```ts
// Rectangles for a tab's panes and dividers, as fractions (0 to 1) of the
// tab's pane area, and directional focus movement (spec 9.6, Alt+Arrow).
import { containsPane, type LayoutNode, type SplitDirection } from "./tree";

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface PaneRect extends Rect {
  sessionId: string;
}

/**
 * The boundary between child `index` and child `index + 1` of the split at
 * `path`. For a "row" split it is a vertical line at `x`, from `y` to
 * `y + length`; for a "column" split it is a horizontal line at `y`, from
 * `x` to `x + length`. `extent` is the split's own width ("row") or height
 * ("column") as a fraction of the pane area; dividing a pointer movement in
 * pixels by `extent × area size in pixels` gives the `resizeSplit` delta.
 */
export interface DividerRect {
  path: number[];
  index: number;
  direction: SplitDirection;
  x: number;
  y: number;
  length: number;
  extent: number;
}

export interface TabRects {
  panes: PaneRect[];
  dividers: DividerRect[];
}

export type FocusDirection = "left" | "right" | "up" | "down";

/**
 * Lays out the tree in the unit square. A zoomed pane that is in the tree
 * is returned alone at full size with no dividers; the other panes are not
 * returned, so the UI unmounts them and they detach (spec 5.5).
 */
export function layoutRects(root: LayoutNode, zoomed: string | null): TabRects {
  if (zoomed && containsPane(root, zoomed)) {
    return { panes: [{ sessionId: zoomed, x: 0, y: 0, w: 1, h: 1 }], dividers: [] };
  }
  const panes: PaneRect[] = [];
  const dividers: DividerRect[] = [];
  const walk = (node: LayoutNode, r: Rect, path: number[]): void => {
    if (node.type === "pane") {
      panes.push({ sessionId: node.session_id, ...r });
      return;
    }
    let offset = 0;
    node.children.forEach((child, index) => {
      const share = node.sizes[index];
      const childRect =
        node.direction === "row"
          ? { x: r.x + offset * r.w, y: r.y, w: share * r.w, h: r.h }
          : { x: r.x, y: r.y + offset * r.h, w: r.w, h: share * r.h };
      walk(child, childRect, [...path, index]);
      offset += share;
      if (index < node.children.length - 1) {
        dividers.push(
          node.direction === "row"
            ? { path, index, direction: "row", x: r.x + offset * r.w, y: r.y, length: r.h, extent: r.w }
            : { path, index, direction: "column", x: r.x, y: r.y + offset * r.h, length: r.w, extent: r.h },
        );
      }
    });
  };
  walk(root, { x: 0, y: 0, w: 1, h: 1 }, []);
  return { panes, dividers };
}

const EPSILON = 1e-6;

/**
 * The pane to focus when moving from `fromId` in `direction`. Candidates lie
 * entirely on that side of the current pane and overlap it on the other
 * axis. The nearest candidate wins; among equally near ones, the one with
 * the largest overlap; among those, the first in `panes` order (depth-first
 * tree order). Returns null when there is no candidate or `fromId` is not
 * in `panes`.
 */
export function focusNeighbor(panes: PaneRect[], fromId: string, direction: FocusDirection): string | null {
  const from = panes.find((p) => p.sessionId === fromId);
  if (!from) return null;
  let best: { id: string; distance: number; overlap: number } | null = null;
  for (const p of panes) {
    if (p.sessionId === fromId) continue;
    const horizontal = direction === "left" || direction === "right";
    const distance =
      direction === "left"
        ? from.x - (p.x + p.w)
        : direction === "right"
          ? p.x - (from.x + from.w)
          : direction === "up"
            ? from.y - (p.y + p.h)
            : p.y - (from.y + from.h);
    const overlap = horizontal
      ? Math.min(from.y + from.h, p.y + p.h) - Math.max(from.y, p.y)
      : Math.min(from.x + from.w, p.x + p.w) - Math.max(from.x, p.x);
    if (distance < -EPSILON || overlap <= EPSILON) continue;
    const nearer = best === null || distance < best.distance - EPSILON;
    const asNearWithMoreOverlap =
      best !== null && Math.abs(distance - best.distance) <= EPSILON && overlap > best.overlap + EPSILON;
    if (nearer || asNearWithMoreOverlap) best = { id: p.sessionId, distance, overlap };
  }
  return best?.id ?? null;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm --dir app exec vitest run src/lib/layout/geometry.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src/lib/layout/geometry.ts app/src/lib/layout/geometry.test.ts
git commit -m "feat(app): pane and divider rectangles and directional focus"
```

---

### Task 4: Tab operations and reconciliation with the daemon

**Files:**
- Create: `app/src/lib/layout/tabs.ts`, `app/src/lib/layout/reconcile.ts`
- Test: `app/src/lib/layout/tabs.test.ts`, `app/src/lib/layout/reconcile.test.ts`

**Interfaces:**
- Consumes: Task 2's tree functions; generated `ProjectLayout`, `TabLayout`, `LayoutFile` (Task 1); `Model` from `../state/reducer` and `sessionsForProject` from `../state/views` (Plan 2b).
- Produces (used by Tasks 5, 8, 9, 10):
  - `tabs.ts`: `emptyProject(): ProjectLayout`; `newTab(id, sessionId): TabLayout`; `activeTab(project): TabLayout | null`; `tabOf(project, sessionId): TabLayout | null`; `addTab(project, tab, activate: boolean): ProjectLayout`; `selectTab(project, tabId)`; `cycleTab(project, delta: 1 | -1)`; `closeTab(project, tabId)`; `renameTab(project, tabId, name: string)`; `focusSession(project, sessionId)`; `removeSession(project, sessionId)`; `splitBeside(project, tabId, besideId, newId, direction): ProjectLayout | null`; `toggleZoom(project, sessionId)`; `moveBeside(project, sessionId, targetTabId, besideId, direction): ProjectLayout | null`; `moveToTab(project, sessionId, targetTabId): ProjectLayout | null`; `moveToNewTab(project, sessionId, newTabId): ProjectLayout | null`; `swapInTab(project, a, b)`; `presetTab(project, tabId, preset)`; `equalizeTab(project, tabId)`; `setTabRoot(project, tabId, root: LayoutNode)`; `replaceSession(project, oldId, newId): ProjectLayout | null`; `tabLabel(tab, labelOf: (sessionId: string) => string | undefined): string`. Functions without `| null` return `ProjectLayout` and return the **same object** when nothing changes.
  - `reconcile.ts`: `LOCAL_MACHINE_ID = "local"`; `layoutKey(machineId, projectId): string` (`"local:<project id>"`); `reconcileProject(stored: ProjectLayout | undefined, sessionIds: string[], newTabId: () => string): ProjectLayout`; `reconcileAll(file: LayoutFile, model: Model, newTabId: () => string): LayoutFile`.

Returning the same object when nothing changes lets the app skip writing `layout.json` by comparing identities (Task 9's `commit`).

- [ ] **Step 1: Write the failing tab tests**

`app/src/lib/layout/tabs.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import type { TabLayout } from "../backend/TabLayout";
import {
  activeTab,
  addTab,
  cycleTab,
  emptyProject,
  equalizeTab,
  focusSession,
  moveBeside,
  moveToNewTab,
  moveToTab,
  newTab,
  presetTab,
  removeSession,
  renameTab,
  replaceSession,
  selectTab,
  setTabRoot,
  splitBeside,
  swapInTab,
  tabLabel,
  tabOf,
  toggleZoom,
  type ProjectLayout,
} from "./tabs";
import { pane, paneIds, split } from "./tree";

function tab(id: string, root: TabLayout["root"], focused: string, extra: Partial<TabLayout> = {}): TabLayout {
  return { id, name: null, root, focused, zoomed: null, ...extra };
}

function project(...tabs: TabLayout[]): ProjectLayout {
  return { tabs, active_tab: tabs[0]?.id ?? null };
}

const pair = () => tab("T1", split("row", [pane("A"), pane("B")]), "A");

describe("adding, selecting and cycling tabs", () => {
  it("activates a new tab only when asked, unless nothing is active", () => {
    const one = addTab(emptyProject(), newTab("T1", "A"), false);
    expect(one.active_tab).toBe("T1");
    const two = addTab(one, newTab("T2", "B"), false);
    expect(two.active_tab).toBe("T1");
    expect(addTab(one, newTab("T2", "B"), true).active_tab).toBe("T2");
  });

  it("falls back to the first tab when the active id is missing", () => {
    const p = { tabs: [newTab("T1", "A")], active_tab: "GONE" };
    expect(activeTab(p)?.id).toBe("T1");
    expect(activeTab(emptyProject())).toBeNull();
  });

  it("returns the same object when selecting the active or an unknown tab", () => {
    const p = project(newTab("T1", "A"), newTab("T2", "B"));
    expect(selectTab(p, "T1")).toBe(p);
    expect(selectTab(p, "NOPE")).toBe(p);
    expect(selectTab(p, "T2").active_tab).toBe("T2");
  });

  it("cycles through tabs in both directions, wrapping around", () => {
    const p = project(newTab("T1", "A"), newTab("T2", "B"), newTab("T3", "C"));
    expect(cycleTab(p, 1).active_tab).toBe("T2");
    expect(cycleTab(p, -1).active_tab).toBe("T3");
    const single = project(newTab("T1", "A"));
    expect(cycleTab(single, 1)).toBe(single);
  });
});

describe("renameTab and tabLabel", () => {
  it("trims the name and stores a blank name as null", () => {
    const p = project(newTab("T1", "A"));
    expect(renameTab(p, "T1", "  build  ").tabs[0].name).toBe("build");
    const named = renameTab(p, "T1", "build");
    expect(renameTab(named, "T1", "   ").tabs[0].name).toBeNull();
    expect(renameTab(named, "T1", "build")).toBe(named);
  });

  it("labels a tab by its name, else its focused session's label, else Tab", () => {
    const labels: Record<string, string> = { A: "claude" };
    expect(tabLabel(tab("T1", pane("A"), "A", { name: "work" }), (id) => labels[id])).toBe("work");
    expect(tabLabel(tab("T1", pane("A"), "A"), (id) => labels[id])).toBe("claude");
    expect(tabLabel(tab("T1", pane("X"), "X"), (id) => labels[id])).toBe("Tab");
  });
});

describe("focusSession", () => {
  it("activates the session's tab and focuses its pane", () => {
    const p = project(newTab("T1", "X"), { ...pair(), id: "T2" });
    const next = focusSession(p, "B");
    expect(next.active_tab).toBe("T2");
    expect(tabOf(next, "B")?.focused).toBe("B");
  });

  it("clears a zoom on another pane so the focused pane is visible", () => {
    const p = project({ ...pair(), zoomed: "A" });
    expect(focusSession(p, "B").tabs[0].zoomed).toBeNull();
    expect(focusSession(p, "A")).toBe(p);
  });
});

describe("removeSession", () => {
  it("moves focus to the next pane in order, else the previous one", () => {
    const p = project(tab("T1", split("row", [pane("A"), pane("B"), pane("C")]), "B"));
    expect(removeSession(p, "B").tabs[0].focused).toBe("C");
    const last = project(tab("T1", split("row", [pane("A"), pane("B")]), "B"));
    expect(removeSession(last, "B").tabs[0].focused).toBe("A");
  });

  it("closes a tab whose last pane is removed and activates its right neighbor, else its left", () => {
    const p = { tabs: [newTab("T1", "A"), newTab("T2", "B"), newTab("T3", "C")], active_tab: "T2" };
    expect(removeSession(p, "B")).toEqual({ tabs: [newTab("T1", "A"), newTab("T3", "C")], active_tab: "T3" });
    const end = { tabs: [newTab("T1", "A"), newTab("T2", "B")], active_tab: "T2" };
    expect(removeSession(end, "B").active_tab).toBe("T1");
  });

  it("clears the zoom of a removed pane and of a tab left with one pane", () => {
    const p = project({ ...pair(), zoomed: "B" });
    expect(removeSession(p, "B").tabs[0].zoomed).toBeNull();
    const other = project({ ...pair(), zoomed: "A" });
    expect(removeSession(other, "B").tabs[0].zoomed).toBeNull();
  });

  it("returns the same object for a session in no tab", () => {
    const p = project(pair());
    expect(removeSession(p, "Z")).toBe(p);
  });
});

describe("splitBeside and toggleZoom", () => {
  it("adds the pane, focuses it, activates the tab and clears the zoom", () => {
    const p = { tabs: [newTab("T0", "X"), { ...pair(), zoomed: "A" }], active_tab: "T0" };
    const next = splitBeside(p, "T1", "A", "N", "column")!;
    expect(next.active_tab).toBe("T1");
    const t = tabOf(next, "N")!;
    expect(paneIds(t.root)).toEqual(["A", "N", "B"]);
    expect(t.focused).toBe("N");
    expect(t.zoomed).toBeNull();
  });

  it("returns null for a full tab or an unknown tab", () => {
    const full = project(tab("T1", split("row", [pane("A"), pane("B"), pane("C"), pane("D")]), "A"));
    expect(splitBeside(full, "T1", "A", "N", "row")).toBeNull();
    expect(splitBeside(project(pair()), "NOPE", "A", "N", "row")).toBeNull();
  });

  it("toggles zoom on a pane and focuses it; a single pane never zooms", () => {
    const zoomed = toggleZoom(project(pair()), "B");
    expect(zoomed.tabs[0]).toMatchObject({ zoomed: "B", focused: "B" });
    expect(toggleZoom(zoomed, "B").tabs[0].zoomed).toBeNull();
    const single = project(newTab("T1", "A"));
    expect(toggleZoom(single, "A")).toBe(single);
  });
});

describe("moving panes between tabs", () => {
  it("moves a pane to a new active tab, and refuses when it is alone in its tab", () => {
    const next = moveToNewTab(project(pair()), "B", "T9")!;
    expect(next.tabs.map((t) => paneIds(t.root))).toEqual([["A"], ["B"]]);
    expect(next.active_tab).toBe("T9");
    expect(moveToNewTab(project(newTab("T1", "A")), "A", "T9")).toBeNull();
  });

  it("moves a pane beside the target tab's focused pane and closes the emptied source tab", () => {
    const p = project(newTab("T1", "A"), tab("T2", split("column", [pane("B"), pane("C")]), "C"));
    const next = moveToTab(p, "A", "T2")!;
    expect(next.tabs).toHaveLength(1);
    expect(next.tabs[0].root).toEqual(split("column", [pane("B"), split("row", [pane("C"), pane("A")])]));
    expect(next.tabs[0].focused).toBe("A");
    expect(next.active_tab).toBe("T2");
  });

  it("refuses a full target, the same tab and an unknown target", () => {
    const full = tab("T2", split("row", [pane("B"), pane("C"), pane("D"), pane("E")]), "B");
    expect(moveToTab(project(newTab("T1", "A"), full), "A", "T2")).toBeNull();
    expect(moveToTab(project(pair()), "A", "T1")).toBeNull();
    expect(moveToTab(project(pair()), "A", "NOPE")).toBeNull();
  });

  it("moves beside a chosen pane in a chosen direction", () => {
    const p = project(newTab("T1", "A"), tab("T2", split("row", [pane("B"), pane("C")]), "C"));
    const next = moveBeside(p, "A", "T2", "B", "column")!;
    expect(next.tabs[0].root).toEqual(split("row", [split("column", [pane("B"), pane("A")]), pane("C")]));
  });
});

describe("swap, presets, equalize, roots and replacement", () => {
  it("swaps two panes of the same tab only", () => {
    const p = project(pair(), newTab("T2", "C"));
    expect(paneIds(swapInTab(p, "A", "B").tabs[0].root)).toEqual(["B", "A"]);
    expect(swapInTab(p, "A", "C")).toBe(p);
  });

  it("applies a preset around the focused pane and clears the zoom", () => {
    const p = project(tab("T1", split("row", [pane("A"), pane("B"), pane("C")]), "B", { zoomed: "B" }));
    const next = presetTab(p, "T1", "main_stack");
    expect(next.tabs[0].root).toEqual(split("row", [pane("B"), split("column", [pane("A"), pane("C")])], [0.6, 0.4]));
    expect(next.tabs[0].zoomed).toBeNull();
  });

  it("equalizes a tab", () => {
    const p = project(tab("T1", split("row", [pane("A"), pane("B")], [0.9, 0.1]), "A"));
    expect(equalizeTab(p, "T1").tabs[0].root).toEqual(split("row", [pane("A"), pane("B")]));
  });

  it("sets a new root only when it holds the same panes", () => {
    const p = project(pair());
    const resized = split("row", [pane("A"), pane("B")], [0.7, 0.3]);
    expect(setTabRoot(p, "T1", resized).tabs[0].root).toEqual(resized);
    expect(setTabRoot(p, "T1", split("row", [pane("A"), pane("Z")]))).toBe(p);
  });

  it("replaces a session in place and focuses the new one", () => {
    const p = { tabs: [newTab("T0", "X"), pair()], active_tab: "T0" };
    const next = replaceSession(p, "A", "N")!;
    expect(paneIds(next.tabs[1].root)).toEqual(["N", "B"]);
    expect(next.tabs[1].focused).toBe("N");
    expect(next.active_tab).toBe("T1");
    expect(replaceSession(p, "Z", "N")).toBeNull();
    expect(replaceSession(p, "A", "B")).toBeNull();
  });
});
```

- [ ] **Step 2: Write the failing reconciliation tests**

`app/src/lib/layout/reconcile.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import type { LayoutFile } from "../backend/LayoutFile";
import type { ProjectLayout } from "../backend/ProjectLayout";
import type { SessionInfo } from "../protocol/SessionInfo";
import { applyControl, emptyModel } from "../state/reducer";
import { layoutKey, LOCAL_MACHINE_ID, reconcileAll, reconcileProject } from "./reconcile";
import { newTab } from "./tabs";
import { pane, paneIds, split } from "./tree";

function ids(): () => string {
  let n = 0;
  return () => `NEW${++n}`;
}

describe("reconcileProject", () => {
  it("opens one tab per session, in the given order, when nothing is stored", () => {
    expect(reconcileProject(undefined, ["A", "B"], ids())).toEqual({
      tabs: [newTab("NEW1", "A"), newTab("NEW2", "B")],
      active_tab: "NEW1",
    });
  });

  it("drops session ids the daemon no longer has and removes emptied tabs", () => {
    const stored: ProjectLayout = {
      tabs: [
        { id: "T1", name: null, root: split("row", [pane("A"), pane("GONE")]), focused: "GONE", zoomed: "GONE" },
        newTab("T2", "GONE2"),
      ],
      active_tab: "T2",
    };
    expect(reconcileProject(stored, ["A"], ids())).toEqual({
      tabs: [{ id: "T1", name: null, root: pane("A"), focused: "A", zoomed: null }],
      active_tab: "T1",
    });
  });

  it("places daemon sessions missing from the layout in new tabs after the stored ones", () => {
    const stored: ProjectLayout = { tabs: [newTab("T1", "A")], active_tab: "T1" };
    const result = reconcileProject(stored, ["A", "B", "C"], ids());
    expect(result.tabs.map((t) => [t.id, paneIds(t.root)])).toEqual([
      ["T1", ["A"]],
      ["NEW1", ["B"]],
      ["NEW2", ["C"]],
    ]);
    expect(result.active_tab).toBe("T1");
  });

  it("keeps a session that appears in two tabs only in the first", () => {
    const stored: ProjectLayout = {
      tabs: [newTab("T1", "A"), { id: "T2", name: null, root: split("row", [pane("A"), pane("B")]), focused: "A", zoomed: null }],
      active_tab: "T2",
    };
    const result = reconcileProject(stored, ["A", "B"], ids());
    expect(result.tabs.map((t) => paneIds(t.root))).toEqual([["A"], ["B"]]);
    expect(result.tabs[1].focused).toBe("B");
  });

  it("moves panes beyond the fourth into their own tabs", () => {
    const five = split("row", ["A", "B", "C", "D", "E"].map(pane));
    const stored: ProjectLayout = { tabs: [{ id: "T1", name: null, root: five, focused: "E", zoomed: null }], active_tab: "T1" };
    const result = reconcileProject(stored, ["A", "B", "C", "D", "E"], ids());
    expect(result.tabs.map((t) => paneIds(t.root))).toEqual([["A", "B", "C", "D"], ["E"]]);
    expect(result.tabs[0].focused).toBe("A");
  });

  it("stores a blank name as null and renames duplicate tab ids", () => {
    const stored: ProjectLayout = {
      tabs: [
        { ...newTab("T1", "A"), name: "  " },
        { ...newTab("T1", "B"), name: "logs" },
      ],
      active_tab: "T1",
    };
    const result = reconcileProject(stored, ["A", "B"], ids());
    expect(result.tabs.map((t) => [t.id, t.name])).toEqual([
      ["T1", null],
      ["NEW1", "logs"],
    ]);
  });

  it("gives a project without sessions no tabs and no active tab", () => {
    expect(reconcileProject({ tabs: [newTab("T1", "GONE")], active_tab: "T1" }, [], ids())).toEqual({ tabs: [], active_tab: null });
  });
});

describe("reconcileAll", () => {
  const base = {
    kind: "shell",
    command: "pwsh.exe",
    args: [] as string[],
    label: "pwsh",
    state: "running",
    state_changed_at: 0,
    exit_code: null,
    claude_session_id: null,
    attachable: true,
  } as const;
  const session = (id: string, project_id: string, created_at: number): SessionInfo => ({ ...base, id, project_id, created_at });

  it("reconciles every local project, prunes removed projects and keeps other machines' entries", () => {
    let model = applyControl(emptyModel(), { type: "Projects", projects: [{ id: "P1", path: "C:\\a", name: "a" }] });
    model = applyControl(model, { type: "Sessions", sessions: [session("S2", "P1", 20), session("S1", "P1", 10)] });
    const remote: ProjectLayout = { tabs: [newTab("R1", "X")], active_tab: "R1" };
    const file: LayoutFile = {
      version: 1,
      projects: {
        [layoutKey(LOCAL_MACHINE_ID, "OLD")]: { tabs: [newTab("T1", "Z")], active_tab: "T1" },
        "server1:P9": remote,
      },
    };
    const result = reconcileAll(file, model, ids());
    expect(Object.keys(result.projects).sort()).toEqual(["local:P1", "server1:P9"]);
    expect(result.projects["server1:P9"]).toBe(remote);
    expect(result.projects["local:P1"]!.tabs.map((t) => paneIds(t.root))).toEqual([["S1"], ["S2"]]);
    expect(result.version).toBe(1);
  });
});
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `pnpm --dir app exec vitest run src/lib/layout/tabs.test.ts src/lib/layout/reconcile.test.ts`
Expected: FAIL with "Failed to resolve import "./tabs"" and "Failed to resolve import "./reconcile"".

- [ ] **Step 4: Write the tab operations**

`app/src/lib/layout/tabs.ts`:
```ts
// Operations on one project's tabs (spec sections 9.3 and 9.4). Pure:
// every function returns a new ProjectLayout, or the same object when
// nothing changed, so the caller can skip writing layout.json by comparing
// identities. Functions returning `| null` return null when the operation is
// not possible (the caller then disables or ignores the action).
import type { ProjectLayout } from "../backend/ProjectLayout";
import type { TabLayout } from "../backend/TabLayout";
import {
  applyPreset,
  containsPane,
  countPanes,
  equalize,
  MAX_PANES,
  pane,
  paneIds,
  removePane,
  replacePane,
  splitPane,
  swapPanes,
  type LayoutNode,
  type LayoutPreset,
  type SplitDirection,
} from "./tree";

export type { ProjectLayout, TabLayout };

export function emptyProject(): ProjectLayout {
  return { tabs: [], active_tab: null };
}

export function newTab(id: string, sessionId: string): TabLayout {
  return { id, name: null, root: pane(sessionId), focused: sessionId, zoomed: null };
}

/** The active tab; the first tab when `active_tab` is null or names no tab. */
export function activeTab(project: ProjectLayout): TabLayout | null {
  return project.tabs.find((t) => t.id === project.active_tab) ?? project.tabs[0] ?? null;
}

export function tabOf(project: ProjectLayout, sessionId: string): TabLayout | null {
  return project.tabs.find((t) => containsPane(t.root, sessionId)) ?? null;
}

function findTab(project: ProjectLayout, tabId: string): TabLayout | null {
  return project.tabs.find((t) => t.id === tabId) ?? null;
}

function putTab(project: ProjectLayout, tab: TabLayout): ProjectLayout {
  return { ...project, tabs: project.tabs.map((t) => (t.id === tab.id ? tab : t)) };
}

export function addTab(project: ProjectLayout, tab: TabLayout, activate: boolean): ProjectLayout {
  const active_tab = activate || project.active_tab === null ? tab.id : project.active_tab;
  return { tabs: [...project.tabs, tab], active_tab };
}

export function selectTab(project: ProjectLayout, tabId: string): ProjectLayout {
  if (project.active_tab === tabId || !findTab(project, tabId)) return project;
  return { ...project, active_tab: tabId };
}

export function cycleTab(project: ProjectLayout, delta: 1 | -1): ProjectLayout {
  const current = activeTab(project);
  if (!current || project.tabs.length < 2) return project;
  const index = project.tabs.indexOf(current);
  const next = project.tabs[(index + delta + project.tabs.length) % project.tabs.length];
  return selectTab(project, next.id);
}

/** Removes a tab; when it was active, its right neighbor (else its left) becomes active. */
export function closeTab(project: ProjectLayout, tabId: string): ProjectLayout {
  const index = project.tabs.findIndex((t) => t.id === tabId);
  if (index < 0) return project;
  const wasActive = activeTab(project)?.id === tabId;
  const tabs = project.tabs.filter((t) => t.id !== tabId);
  const active_tab = wasActive ? ((tabs[index] ?? tabs[index - 1])?.id ?? null) : project.active_tab;
  return { tabs, active_tab };
}

/** Trims `name`; a blank name is stored as null, which shows the focused session's label. */
export function renameTab(project: ProjectLayout, tabId: string, name: string): ProjectLayout {
  const tab = findTab(project, tabId);
  if (!tab) return project;
  const trimmed = name.trim();
  const next = trimmed === "" ? null : trimmed;
  return tab.name === next ? project : putTab(project, { ...tab, name: next });
}

/**
 * Activates the session's tab and focuses its pane (spec 9.3 rule 5). A zoom
 * on another pane is cleared, because the zoomed pane hides every other one.
 */
export function focusSession(project: ProjectLayout, sessionId: string): ProjectLayout {
  const tab = tabOf(project, sessionId);
  if (!tab) return project;
  const zoomed = tab.zoomed !== null && tab.zoomed !== sessionId ? null : tab.zoomed;
  if (project.active_tab === tab.id && tab.focused === sessionId && zoomed === tab.zoomed) return project;
  return { ...putTab(project, { ...tab, focused: sessionId, zoomed }), active_tab: tab.id };
}

/**
 * Removes a session's pane. Focus moves to the next pane in tree order, or
 * the previous one when it was last. A tab left with no panes is closed.
 */
export function removeSession(project: ProjectLayout, sessionId: string): ProjectLayout {
  const tab = tabOf(project, sessionId);
  if (!tab) return project;
  const root = removePane(tab.root, sessionId);
  if (!root) return closeTab(project, tab.id);
  const remaining = paneIds(root);
  let focused = tab.focused;
  if (focused === sessionId) {
    const order = paneIds(tab.root);
    const index = order.indexOf(sessionId);
    const after = order.slice(index + 1).find((id) => remaining.includes(id));
    const before = order
      .slice(0, index)
      .reverse()
      .find((id) => remaining.includes(id));
    focused = after ?? before ?? remaining[0];
  }
  const zoomed = tab.zoomed === sessionId || remaining.length < 2 ? null : tab.zoomed;
  return putTab(project, { ...tab, root, focused, zoomed });
}

/** Adds `newId` beside `besideId` in tab `tabId`, focuses it and activates the tab. */
export function splitBeside(
  project: ProjectLayout,
  tabId: string,
  besideId: string,
  newId: string,
  direction: SplitDirection,
): ProjectLayout | null {
  const tab = findTab(project, tabId);
  if (!tab) return null;
  const root = splitPane(tab.root, besideId, newId, direction);
  if (!root) return null;
  return { ...putTab(project, { ...tab, root, focused: newId, zoomed: null }), active_tab: tab.id };
}

/** Zooms the session's pane, or unzooms it if it is already zoomed (spec 9.4, Zoom pane). */
export function toggleZoom(project: ProjectLayout, sessionId: string): ProjectLayout {
  const tab = tabOf(project, sessionId);
  if (!tab || countPanes(tab.root) < 2) return project;
  return putTab(project, { ...tab, focused: sessionId, zoomed: tab.zoomed === sessionId ? null : sessionId });
}

/**
 * Moves a session out of its tab and beside `besideId` in another tab of the
 * same project. Returns null when the target is the same tab, is full, or
 * does not hold `besideId`. An emptied source tab is closed.
 */
export function moveBeside(
  project: ProjectLayout,
  sessionId: string,
  targetTabId: string,
  besideId: string,
  direction: SplitDirection,
): ProjectLayout | null {
  const source = tabOf(project, sessionId);
  const target = findTab(project, targetTabId);
  if (!source || !target || source.id === target.id) return null;
  if (countPanes(target.root) >= MAX_PANES || !containsPane(target.root, besideId)) return null;
  return splitBeside(removeSession(project, sessionId), targetTabId, besideId, sessionId, direction);
}

/** Spec 9.4, Move to ▸ other tab: the pane goes to the right of the target's focused pane. */
export function moveToTab(project: ProjectLayout, sessionId: string, targetTabId: string): ProjectLayout | null {
  const target = findTab(project, targetTabId);
  if (!target) return null;
  return moveBeside(project, sessionId, targetTabId, target.focused, "row");
}

/** Spec 9.4, Move to ▸ New tab. Null when the session is the only pane in its tab. */
export function moveToNewTab(project: ProjectLayout, sessionId: string, newTabId: string): ProjectLayout | null {
  const source = tabOf(project, sessionId);
  if (!source || countPanes(source.root) < 2) return null;
  return addTab(removeSession(project, sessionId), newTab(newTabId, sessionId), true);
}

export function swapInTab(project: ProjectLayout, a: string, b: string): ProjectLayout {
  const tab = tabOf(project, a);
  if (!tab || a === b || !containsPane(tab.root, b)) return project;
  return putTab(project, { ...tab, root: swapPanes(tab.root, a, b) });
}

export function presetTab(project: ProjectLayout, tabId: string, preset: LayoutPreset): ProjectLayout {
  const tab = findTab(project, tabId);
  if (!tab || countPanes(tab.root) < 2) return project;
  return putTab(project, { ...tab, root: applyPreset(tab.root, preset, tab.focused), zoomed: null });
}

export function equalizeTab(project: ProjectLayout, tabId: string): ProjectLayout {
  const tab = findTab(project, tabId);
  if (!tab || tab.root.type === "pane") return project;
  return putTab(project, { ...tab, root: equalize(tab.root) });
}

/** Replaces a tab's tree (used by divider drags); refused when the panes differ. */
export function setTabRoot(project: ProjectLayout, tabId: string, root: LayoutNode): ProjectLayout {
  const tab = findTab(project, tabId);
  if (!tab) return project;
  const before = [...paneIds(tab.root)].sort().join(",");
  const after = [...paneIds(root)].sort().join(",");
  return before === after ? putTab(project, { ...tab, root }) : project;
}

/**
 * Puts `newId` in `oldId`'s position (Restart: the new session replaces the
 * old row), focuses it and activates its tab. Null when `oldId` is in no tab
 * or `newId` is already placed.
 */
export function replaceSession(project: ProjectLayout, oldId: string, newId: string): ProjectLayout | null {
  const tab = tabOf(project, oldId);
  if (!tab || tabOf(project, newId)) return null;
  const zoomed = tab.zoomed === oldId ? newId : tab.zoomed;
  const replaced = { ...tab, root: replacePane(tab.root, oldId, newId), focused: newId, zoomed };
  return { ...putTab(project, replaced), active_tab: tab.id };
}

export function tabLabel(tab: TabLayout, labelOf: (sessionId: string) => string | undefined): string {
  return tab.name ?? labelOf(tab.focused) ?? "Tab";
}
```

- [ ] **Step 5: Write the reconciliation**

`app/src/lib/layout/reconcile.ts`:
```ts
// Brings layout.json in line with the daemon's sessions (spec section 4,
// "Layout" row): session ids that no longer exist are dropped when the
// layout is loaded, and daemon sessions missing from the layout are each
// placed in a new tab.
import type { LayoutFile } from "../backend/LayoutFile";
import type { ProjectLayout } from "../backend/ProjectLayout";
import type { TabLayout } from "../backend/TabLayout";
import type { Model } from "../state/reducer";
import { sessionsForProject } from "../state/views";
import { newTab } from "./tabs";
import { filterPanes, MAX_PANES, paneIds, removePane } from "./tree";

/** Machine id of This PC in layout keys. Plan 5 adds remote machines with their own ids. */
export const LOCAL_MACHINE_ID = "local";

/** A project id is unique only within one daemon, so layout keys include the machine. */
export function layoutKey(machineId: string, projectId: string): string {
  return `${machineId}:${projectId}`;
}

/**
 * Reconciles one project's stored layout with `sessionIds`, the daemon's
 * sessions for that project in creation order:
 *
 * 1. Panes whose session is not in `sessionIds` are removed; a session that
 *    appears more than once keeps only its first pane (tabs in order, tree
 *    order within a tab). Splits collapse and sizes are renormalized.
 * 2. A tab with more than MAX_PANES panes (only possible in a hand-edited
 *    file) keeps its first four; the rest are placed by rule 5.
 * 3. A tab left with no panes is removed. A duplicate tab id gets a new id.
 * 4. A `focused` that names a removed pane moves to the tab's first pane; a
 *    `zoomed` that names a removed pane, or a zoom in a tab with one pane,
 *    is cleared. A blank name becomes null.
 * 5. Every session not placed by rules 1-2 gets its own new tab, appended in
 *    creation order.
 * 6. `active_tab` is kept when it names a remaining tab; otherwise the first
 *    tab becomes active (null when there are no tabs).
 */
export function reconcileProject(stored: ProjectLayout | undefined, sessionIds: string[], newTabId: () => string): ProjectLayout {
  const live = new Set(sessionIds);
  const placed = new Set<string>();
  const tabIds = new Set<string>();
  const tabs: TabLayout[] = [];
  for (const tab of stored?.tabs ?? []) {
    let root = filterPanes(tab.root, (id) => {
      if (!live.has(id) || placed.has(id)) return false;
      placed.add(id);
      return true;
    });
    if (!root) continue;
    for (const extra of paneIds(root).slice(MAX_PANES)) {
      root = removePane(root, extra) ?? root;
      placed.delete(extra);
    }
    const kept = paneIds(root);
    const id = tabIds.has(tab.id) ? newTabId() : tab.id;
    tabIds.add(id);
    tabs.push({
      id,
      name: tab.name !== null && tab.name.trim() !== "" ? tab.name : null,
      root,
      focused: kept.includes(tab.focused) ? tab.focused : kept[0],
      zoomed: tab.zoomed !== null && kept.length > 1 && kept.includes(tab.zoomed) ? tab.zoomed : null,
    });
  }
  for (const sessionId of sessionIds) {
    if (placed.has(sessionId)) continue;
    const id = newTabId();
    tabIds.add(id);
    tabs.push(newTab(id, sessionId));
    placed.add(sessionId);
  }
  const storedActive = stored?.active_tab ?? null;
  const active_tab = storedActive !== null && tabs.some((t) => t.id === storedActive) ? storedActive : (tabs[0]?.id ?? null);
  return { tabs, active_tab };
}

/**
 * Reconciles every project of the local daemon. Entries for local projects
 * the daemon no longer has are dropped; entries for other machines are kept
 * unchanged (Plan 5 reconciles them against their own daemons).
 */
export function reconcileAll(file: LayoutFile, model: Model, newTabId: () => string): LayoutFile {
  const prefix = `${LOCAL_MACHINE_ID}:`;
  const projects: Record<string, ProjectLayout> = {};
  for (const [key, value] of Object.entries(file.projects)) {
    if (value && !key.startsWith(prefix)) projects[key] = value;
  }
  for (const project of Object.values(model.projects)) {
    const key = layoutKey(LOCAL_MACHINE_ID, project.id);
    const sessionIds = sessionsForProject(model, project.id).map((s) => s.id);
    projects[key] = reconcileProject(file.projects[key], sessionIds, newTabId);
  }
  return { version: file.version, projects };
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `pnpm --dir app exec vitest run src/lib/layout/tabs.test.ts src/lib/layout/reconcile.test.ts`
Expected: PASS.

- [ ] **Step 7: Type-check and commit**

Run: `pnpm --dir app check`
Expected: 0 errors.
```bash
git add app/src/lib/layout/tabs.ts app/src/lib/layout/tabs.test.ts app/src/lib/layout/reconcile.ts app/src/lib/layout/reconcile.test.ts
git commit -m "feat(app): tab operations and layout reconciliation with the daemon"
```

---

### Task 5: Placing created sessions, and session rules

**Files:**
- Create: `app/src/lib/state/workspace.ts`, `app/src/lib/state/sessionRules.ts`
- Test: `app/src/lib/state/workspace.test.ts`, `app/src/lib/state/sessionRules.test.ts`

**Interfaces:**
- Consumes: Task 4 (`tabs.ts`, `reconcile.ts`), generated `LayoutFile`, `ProjectLayout`, `SessionInfo`, `AgentState`; `SplitDirection` from `../layout/tree`.
- Produces (used by Tasks 8, 9, 10):
  - `workspace.ts`: `LAYOUT_VERSION = 1` (equals Rust `jojo_app::layout::LAYOUT_VERSION`); `type Placement = { kind: "tab" } | { kind: "split"; tabId: string; besideId: string; direction: SplitDirection } | { kind: "replace"; oldId: string }`; `interface PendingCreate { projectId: string; placement: Placement }`; `interface Workspace { layout: LayoutFile; pending: Record<string, PendingCreate> }`; `emptyWorkspace(): Workspace`; `projectOf(layout, projectId): ProjectLayout`; `withProject(layout, projectId, update: (p: ProjectLayout) => ProjectLayout | null): LayoutFile | null`; `requestCreate(ws, sessionId, request: PendingCreate): Workspace`; `createFailed(ws, sessionId): Workspace`; `interface CreatedResult { workspace: Workspace; requested: boolean; closeSessionId: string | null }`; `sessionCreated(ws, session: SessionInfo, newTabId: () => string): CreatedResult`; `sessionRemoved(ws, sessionId): Workspace`.
  - `sessionRules.ts`: `isBusy(session): boolean`; `busyStateText(state: AgentState): string`; `interface CloseTabSummary { count: number; busy: number; labels: string[] }`; `closeTabSummary(sessions: SessionInfo[]): CloseTabSummary`; `closeTabMessage(summary): string`; `closeTabNeedsConfirmation(summary): boolean`; `canRestart(session): boolean`; `restartShell(session): string | null`.

Plan 2b tracked a single `pendingSessionId`, so a second `CreateSession` sent before the first `SessionCreated` arrived overwrote it, and only the second session was activated (carry-over 3.2 item 3). `Workspace.pending` holds one entry per client-chosen session id, so every requested session is placed where it was requested, in whatever order the replies arrive.

- [ ] **Step 1: Write the failing tests**

`app/src/lib/state/workspace.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import type { TabLayout } from "../backend/TabLayout";
import { layoutKey, LOCAL_MACHINE_ID } from "../layout/reconcile";
import { newTab } from "../layout/tabs";
import { pane, paneIds, split } from "../layout/tree";
import type { SessionInfo } from "../protocol/SessionInfo";
import { createFailed, emptyWorkspace, projectOf, requestCreate, sessionCreated, sessionRemoved, withProject, type Workspace } from "./workspace";

function session(id: string, project_id = "P1"): SessionInfo {
  return {
    id,
    project_id,
    kind: "shell",
    command: "pwsh.exe",
    args: [],
    label: id.toLowerCase(),
    created_at: 1,
    state: "running",
    state_changed_at: 1,
    exit_code: null,
    claude_session_id: null,
    attachable: true,
  };
}

function ids(): () => string {
  let n = 0;
  return () => `T${++n}`;
}

const KEY = layoutKey(LOCAL_MACHINE_ID, "P1");

function withTabs(ws: Workspace, tabs: TabLayout[]): Workspace {
  return { ...ws, layout: { ...ws.layout, projects: { [KEY]: { tabs, active_tab: tabs[0].id } } } };
}

describe("sessionCreated", () => {
  it("places a requested session in a new active tab and clears the pending entry", () => {
    const ws = requestCreate(emptyWorkspace(), "A", { projectId: "P1", placement: { kind: "tab" } });
    const result = sessionCreated(ws, session("A"), ids());
    expect(result.requested).toBe(true);
    expect(result.closeSessionId).toBeNull();
    expect(result.workspace.pending).toEqual({});
    expect(projectOf(result.workspace.layout, "P1")).toEqual({ tabs: [newTab("T1", "A")], active_tab: "T1" });
  });

  it("places both of two quick creates, whichever reply arrives first", () => {
    let ws = requestCreate(emptyWorkspace(), "A", { projectId: "P1", placement: { kind: "tab" } });
    ws = requestCreate(ws, "B", { projectId: "P1", placement: { kind: "tab" } });
    const newIds = ids();
    const first = sessionCreated(ws, session("B"), newIds);
    const second = sessionCreated(first.workspace, session("A"), newIds);
    expect(first.requested && second.requested).toBe(true);
    const project = projectOf(second.workspace.layout, "P1");
    expect(project.tabs.map((t) => paneIds(t.root))).toEqual([["B"], ["A"]]);
    expect(project.active_tab).toBe("T2");
    expect(second.workspace.pending).toEqual({});
  });

  it("splits beside the requested pane, and falls back to a new tab when the tab filled up meanwhile", () => {
    const base = withTabs(emptyWorkspace(), [newTab("T0", "X")]);
    const ws = requestCreate(base, "N", { projectId: "P1", placement: { kind: "split", tabId: "T0", besideId: "X", direction: "row" } });
    const placed = sessionCreated(ws, session("N"), ids());
    expect(projectOf(placed.workspace.layout, "P1").tabs[0].root).toEqual(split("row", [pane("X"), pane("N")]));

    const full = { ...newTab("T0", "X"), root: split("row", ["X", "Y", "Z", "W"].map(pane)) };
    const fullWs = requestCreate(withTabs(emptyWorkspace(), [full]), "N", {
      projectId: "P1",
      placement: { kind: "split", tabId: "T0", besideId: "X", direction: "row" },
    });
    const fallback = sessionCreated(fullWs, session("N"), ids());
    const project = projectOf(fallback.workspace.layout, "P1");
    expect(project.tabs.map((t) => paneIds(t.root))).toEqual([["X", "Y", "Z", "W"], ["N"]]);
    expect(project.active_tab).toBe("T1");
  });

  it("replaces the old session in place for a restart and reports the old id to close", () => {
    const base = withTabs(emptyWorkspace(), [{ ...newTab("T0", "X"), root: split("row", [pane("OLD"), pane("X")]), focused: "X" }]);
    const ws = requestCreate(base, "NEW", { projectId: "P1", placement: { kind: "replace", oldId: "OLD" } });
    const result = sessionCreated(ws, session("NEW"), ids());
    expect(result.closeSessionId).toBe("OLD");
    expect(paneIds(projectOf(result.workspace.layout, "P1").tabs[0].root)).toEqual(["NEW", "X"]);
  });

  it("adds a session nobody requested in a new tab without activating it", () => {
    const base = withTabs(emptyWorkspace(), [newTab("T0", "X")]);
    const result = sessionCreated(base, session("S"), ids());
    expect(result.requested).toBe(false);
    const project = projectOf(result.workspace.layout, "P1");
    expect(project.tabs.map((t) => t.id)).toEqual(["T0", "T1"]);
    expect(project.active_tab).toBe("T0");
  });

  it("leaves the layout unchanged for a session that is already placed", () => {
    const base = withTabs(emptyWorkspace(), [newTab("T0", "X")]);
    expect(sessionCreated(base, session("X"), ids()).workspace.layout).toBe(base.layout);
  });
});

describe("pending creates and removal", () => {
  it("forgets a pending create that failed", () => {
    const ws = requestCreate(emptyWorkspace(), "A", { projectId: "P1", placement: { kind: "tab" } });
    expect(createFailed(ws, "A").pending).toEqual({});
    const untouched = emptyWorkspace();
    expect(createFailed(untouched, "Z")).toBe(untouched);
  });

  it("removes a session's pane from whichever project holds it", () => {
    const base = withTabs(emptyWorkspace(), [newTab("T0", "X"), newTab("T1", "Y")]);
    expect(projectOf(sessionRemoved(base, "Y").layout, "P1").tabs.map((t) => t.id)).toEqual(["T0"]);
    expect(sessionRemoved(base, "Z")).toBe(base);
  });

  it("returns the same layout when an update changes nothing, and null when it refuses", () => {
    const base = withTabs(emptyWorkspace(), [newTab("T0", "X")]);
    expect(withProject(base.layout, "P1", (p) => p)).toBe(base.layout);
    expect(withProject(base.layout, "P1", () => null)).toBeNull();
    expect(withProject(base.layout, "P2", (p) => p)).toBe(base.layout);
  });
});
```

`app/src/lib/state/sessionRules.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import type { SessionInfo } from "../protocol/SessionInfo";
import { busyStateText, canRestart, closeTabMessage, closeTabNeedsConfirmation, closeTabSummary, isBusy, restartShell } from "./sessionRules";

function session(over: Partial<SessionInfo>): SessionInfo {
  return {
    id: "S1",
    project_id: "P1",
    kind: "claude",
    command: "claude.exe",
    args: [],
    label: "claude",
    created_at: 1,
    state: "idle",
    state_changed_at: 1,
    exit_code: null,
    claude_session_id: null,
    attachable: true,
    ...over,
  };
}

describe("sessionRules", () => {
  it("treats working and needs_you as busy and nothing else", () => {
    expect(isBusy(session({ state: "working" }))).toBe(true);
    expect(isBusy(session({ state: "needs_you" }))).toBe(true);
    for (const state of ["starting", "idle", "done", "error", "running", "exited"] as const) {
      expect(isBusy(session({ state })), state).toBe(false);
    }
    expect(busyStateText("needs_you")).toBe("waiting for you");
    expect(busyStateText("working")).toBe("working");
  });

  it("summarizes a tab and asks only for several sessions or a busy one", () => {
    const one = closeTabSummary([session({ label: "a" })]);
    expect(one).toEqual({ count: 1, busy: 0, labels: ["a"] });
    expect(closeTabNeedsConfirmation(one)).toBe(false);
    const two = closeTabSummary([session({ label: "a", state: "needs_you" }), session({ label: "b" })]);
    expect(two).toEqual({ count: 2, busy: 1, labels: ["a (waiting for you)", "b"] });
    expect(closeTabNeedsConfirmation(two)).toBe(true);
    expect(closeTabMessage(two)).toBe("Closing this tab ends 2 sessions. 1 of them is working or waiting for you:");
    expect(closeTabMessage(closeTabSummary([session({}), session({})]))).toBe("Closing this tab ends 2 sessions:");
    expect(closeTabNeedsConfirmation(closeTabSummary([session({ state: "working" })]))).toBe(true);
  });

  it("restarts only exited sessions, with the same shell program for shells", () => {
    expect(canRestart(session({ state: "exited" }))).toBe(true);
    expect(canRestart(session({ state: "running" }))).toBe(false);
    expect(restartShell(session({ kind: "shell", command: "pwsh" }))).toBe("pwsh");
    expect(restartShell(session({ kind: "claude", command: "cmd.exe" }))).toBeNull();
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --dir app exec vitest run src/lib/state/workspace.test.ts src/lib/state/sessionRules.test.ts`
Expected: FAIL with "Failed to resolve import "./workspace"" and "Failed to resolve import "./sessionRules"".

- [ ] **Step 3: Write the workspace module**

`app/src/lib/state/workspace.ts`:
```ts
// The layout of every project plus the session creates still waiting for
// their SessionCreated reply. Pure; app.svelte.ts holds the current value.
import type { LayoutFile } from "../backend/LayoutFile";
import type { ProjectLayout } from "../backend/ProjectLayout";
import { layoutKey, LOCAL_MACHINE_ID } from "../layout/reconcile";
import { addTab, emptyProject, newTab, removeSession, replaceSession, splitBeside, tabOf } from "../layout/tabs";
import type { SplitDirection } from "../layout/tree";
import type { SessionInfo } from "../protocol/SessionInfo";

/** Format version of layout.json; equals `LAYOUT_VERSION` in app/src-tauri/src/layout.rs. */
export const LAYOUT_VERSION = 1;

/** Where a session the user asked for goes when its SessionCreated arrives. */
export type Placement =
  | { kind: "tab" }
  | { kind: "split"; tabId: string; besideId: string; direction: SplitDirection }
  | { kind: "replace"; oldId: string };

export interface PendingCreate {
  projectId: string;
  placement: Placement;
}

export interface Workspace {
  layout: LayoutFile;
  /** Keyed by the client-chosen session id sent in CreateSession. */
  pending: Record<string, PendingCreate>;
}

export function emptyWorkspace(): Workspace {
  return { layout: { version: LAYOUT_VERSION, projects: {} }, pending: {} };
}

export function projectOf(layout: LayoutFile, projectId: string): ProjectLayout {
  return layout.projects[layoutKey(LOCAL_MACHINE_ID, projectId)] ?? emptyProject();
}

/**
 * Applies `update` to one local project's layout. Returns the same
 * LayoutFile when `update` returns its input unchanged, and null when
 * `update` returns null (the operation was refused).
 */
export function withProject(
  layout: LayoutFile,
  projectId: string,
  update: (project: ProjectLayout) => ProjectLayout | null,
): LayoutFile | null {
  const key = layoutKey(LOCAL_MACHINE_ID, projectId);
  const current = layout.projects[key] ?? emptyProject();
  const next = update(current);
  if (next === null) return null;
  if (next === current) return layout;
  return { ...layout, projects: { ...layout.projects, [key]: next } };
}

export function requestCreate(ws: Workspace, sessionId: string, request: PendingCreate): Workspace {
  return { ...ws, pending: { ...ws.pending, [sessionId]: request } };
}

/** Forgets a pending create (the request was refused, or it has been placed). */
export function createFailed(ws: Workspace, sessionId: string): Workspace {
  if (!(sessionId in ws.pending)) return ws;
  const { [sessionId]: _placed, ...pending } = ws.pending;
  return { ...ws, pending };
}

export interface CreatedResult {
  workspace: Workspace;
  /** True when this app asked for the session; the caller then selects its project. */
  requested: boolean;
  /** For a restart: the session the new one replaced, which the caller closes. */
  closeSessionId: string | null;
}

/**
 * Places a session reported by SessionCreated. A requested session goes
 * where it was requested and its tab becomes active. A split whose tab is
 * now full or gone, or a replacement whose old session is gone, falls back
 * to a new tab. A session nobody requested gets a new tab that is not
 * activated. A session that is already placed is left where it is.
 */
export function sessionCreated(ws: Workspace, session: SessionInfo, newTabId: () => string): CreatedResult {
  const request = ws.pending[session.id];
  const requested = request !== undefined;
  const base = createFailed(ws, session.id);
  if (tabOf(projectOf(base.layout, session.project_id), session.id)) {
    return { workspace: base, requested, closeSessionId: null };
  }
  const placement: Placement = request?.placement ?? { kind: "tab" };
  const asNewTab = (project: ProjectLayout): ProjectLayout => addTab(project, newTab(newTabId(), session.id), requested);
  const closeSessionId = placement.kind === "replace" ? placement.oldId : null;
  const layout = withProject(base.layout, session.project_id, (project) => {
    switch (placement.kind) {
      case "split":
        return splitBeside(project, placement.tabId, placement.besideId, session.id, placement.direction) ?? asNewTab(project);
      case "replace":
        return replaceSession(project, placement.oldId, session.id) ?? asNewTab(project);
      case "tab":
        return asNewTab(project);
    }
  });
  return { workspace: { ...base, layout: layout ?? base.layout }, requested, closeSessionId };
}

/** Removes a closed session's pane from whichever local project holds it. */
export function sessionRemoved(ws: Workspace, sessionId: string): Workspace {
  let changed = false;
  const projects: LayoutFile["projects"] = { ...ws.layout.projects };
  for (const [key, project] of Object.entries(ws.layout.projects)) {
    if (!project || !tabOf(project, sessionId)) continue;
    projects[key] = removeSession(project, sessionId);
    changed = true;
  }
  return changed ? { ...ws, layout: { ...ws.layout, projects } } : ws;
}
```

- [ ] **Step 4: Write the session rules**

`app/src/lib/state/sessionRules.ts`:
```ts
// Rules about sessions that the menus and close actions share.
import type { AgentState } from "../protocol/AgentState";
import type { SessionInfo } from "../protocol/SessionInfo";

/**
 * States in which closing a session asks for confirmation first (spec 9.3
 * rule 6, spec 9.4 Close tab). Only Claude Code hooks put a session in
 * these states, and Plan 4 adds the hooks; until then no session is in
 * either state and closing never asks. The rule reads `SessionInfo.state`,
 * so Plan 4 needs no change here.
 */
const BUSY_STATES: ReadonlySet<AgentState> = new Set<AgentState>(["working", "needs_you"]);

export function isBusy(session: Pick<SessionInfo, "state">): boolean {
  return BUSY_STATES.has(session.state);
}

export function busyStateText(state: AgentState): string {
  return state === "needs_you" ? "waiting for you" : state;
}

export interface CloseTabSummary {
  count: number;
  busy: number;
  /** One line per session; busy sessions name their state. */
  labels: string[];
}

export function closeTabSummary(sessions: SessionInfo[]): CloseTabSummary {
  return {
    count: sessions.length,
    busy: sessions.filter(isBusy).length,
    labels: sessions.map((s) => (isBusy(s) ? `${s.label} (${busyStateText(s.state)})` : s.label)),
  };
}

export function closeTabMessage(summary: CloseTabSummary): string {
  const sessions = summary.count === 1 ? "1 session" : `${summary.count} sessions`;
  if (summary.busy === 0) return `Closing this tab ends ${sessions}:`;
  const verb = summary.busy === 1 ? "is" : "are";
  return `Closing this tab ends ${sessions}. ${summary.busy} of them ${verb} working or waiting for you:`;
}

/**
 * Close tab asks first when it would end more than one session or any busy
 * session. Closing a tab that holds one idle session needs no confirmation,
 * for the same reason closing one idle pane needs none (spec 9.3 rule 6).
 */
export function closeTabNeedsConfirmation(summary: CloseTabSummary): boolean {
  return summary.count > 1 || summary.busy > 0;
}

/** Spec 5.4: Restart is offered for sessions in `exited`. */
export function canRestart(session: Pick<SessionInfo, "state">): boolean {
  return session.state === "exited";
}

/**
 * The `shell` field of a restart's CreateSession. A shell session passes its
 * own program, which jojod uses verbatim (`command_for` in
 * crates/jojod/src/command.rs). A claude session passes null, because jojod
 * resolves `claude.exe` or `cmd.exe /d /c claude` itself.
 */
export function restartShell(session: Pick<SessionInfo, "kind" | "command">): string | null {
  return session.kind === "shell" ? session.command : null;
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `pnpm --dir app exec vitest run src/lib/state/workspace.test.ts src/lib/state/sessionRules.test.ts`
Expected: PASS.

- [ ] **Step 6: Type-check and commit**

Run: `pnpm --dir app check`
Expected: 0 errors.
```bash
git add app/src/lib/state/workspace.ts app/src/lib/state/workspace.test.ts app/src/lib/state/sessionRules.ts app/src/lib/state/sessionRules.test.ts
git commit -m "feat(app): place created sessions by request; close and restart rules"
```

---

### Task 6: One outstanding Attach per session, and end-to-end flow control

**Files:**
- Modify: `app/src/lib/terminal/attachGate.ts`, `app/src/lib/terminal/attachGate.test.ts`, `app/src/lib/terminal/router.ts`, `app/src/lib/terminal/router.test.ts`, `app/src/ui/TerminalPane.svelte:11,59-81`, `app/src/lib/state/app.svelte.ts:61-65`
- Create: `app/src/lib/terminal/flowControl.ts`, `app/src/lib/terminal/paneStream.ts`
- Test: `app/src/lib/terminal/flowControl.test.ts`, `app/src/lib/terminal/paneStream.test.ts`

**Interfaces:**
- Consumes: Plan 2b's router and `TerminalPane.svelte`.
- Produces (used by Task 9):
  - `AttachGate { begin(): boolean; abandon(): void; accept(kind: "output" | "snapshot", bytes: Uint8Array): Uint8Array | null; readonly outstanding: boolean }`.
  - `HIGH_WATERMARK = 24 * 1024 * 1024`, `LOW_WATERMARK = 2 * 1024 * 1024`; `WriteFlow { wrote(n: number): boolean; parsed(n: number): boolean; readonly isBehind: boolean; readonly pendingBytes: number }`.
  - `interface PaneStreamDeps { write(bytes: Uint8Array, done: () => void): void; sendAttach(): Promise<void> }`; `interface PaneStream extends PaneHandle { start(): void }`; `createPaneStream(deps): PaneStream`.
  - `interface PaneHandle { deliver(kind, bytes): void; reattach(afterReconnect: boolean): void; attachFailed(): void }`; router gains `attachFailed(sessionId: string): void`; `fellBehind` calls `reattach(false)`, `reattachAll` calls `reattach(true)`.

Two problems are fixed here (carry-over 3.2 items 1 and 2):

1. `AttachGate` was a boolean, and both `router.fellBehind` and `router.reattachAll` could send a second Attach while one was outstanding. jojod's queue accounting (`crates/jojod/src/client_queue.rs`, which counts a Snapshot as 256 bytes toward the 64 MiB total cap) assumes at most one outstanding Attach per session. The gate now counts Attaches sent and Snapshots received and refuses to start another while they differ.
2. The backend drains jojod's pipe as fast as it can, so jojod's own "fell behind" detection never triggers for the app. xterm.js throws `write data discarded, use flow control to avoid losing data` once more than 50,000,000 bytes are waiting in its write queue (`WriteBuffer.write` in `@xterm/xterm/lib/xterm.js`), and the bridge swallowed that exception, so output was lost with no resync. `WriteFlow` counts bytes handed to `term.write` and not yet acknowledged by its callback. Above `HIGH_WATERMARK` (24 MiB) the pane drops Output; once the queue drains to `LOW_WATERMARK` (2 MiB) it sends an Attach, and the Snapshot replaces everything that was dropped. 24 MiB is above the largest Snapshot (`MAX_SNAPSHOT_BYTES`, 16 MiB minus 17 bytes), so a Snapshot alone never crosses it, and 24 MiB plus one more 16 MiB Snapshot stays below xterm.js's 50,000,000-byte limit.

Why dropped Output is always covered by the resync: a Snapshot reflects every byte jojod fed into the grid before it was taken, and it arrives before any Output produced after it. Output dropped before the outstanding Attach's Snapshot is therefore inside that Snapshot. Output dropped after a Snapshot but while still behind is covered by the Attach sent on catching up, which `begin()` permits because no Attach is outstanding at that point.

- [ ] **Step 1: Write the failing tests**

Replace `app/src/lib/terminal/attachGate.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { AttachGate } from "./attachGate";

const b = (n: number) => new Uint8Array([n]);

describe("AttachGate", () => {
  it("drops output until the snapshot of the outstanding attach arrives", () => {
    const gate = new AttachGate();
    expect(gate.begin()).toBe(true);
    expect(gate.accept("output", b(1))).toBeNull();
    expect(gate.accept("snapshot", b(2))).toEqual(b(2));
    expect(gate.accept("output", b(3))).toEqual(b(3));
  });

  it("passes output when no attach is outstanding", () => {
    expect(new AttachGate().accept("output", b(4))).toEqual(b(4));
  });

  it("refuses a second attach while one is outstanding", () => {
    const gate = new AttachGate();
    expect(gate.begin()).toBe(true);
    expect(gate.begin()).toBe(false);
    expect(gate.outstanding).toBe(true);
    gate.accept("snapshot", b(1));
    expect(gate.outstanding).toBe(false);
    expect(gate.begin()).toBe(true);
  });

  it("releases an attach that will never be answered", () => {
    const gate = new AttachGate();
    gate.begin();
    gate.abandon();
    expect(gate.outstanding).toBe(false);
    expect(gate.accept("output", b(5))).toEqual(b(5));
    expect(gate.begin()).toBe(true);
  });

  it("writes a snapshot nobody is waiting for without counting it", () => {
    const gate = new AttachGate();
    expect(gate.accept("snapshot", b(6))).toEqual(b(6));
    expect(gate.begin()).toBe(true);
    expect(gate.accept("output", b(7))).toBeNull();
  });
});
```

`app/src/lib/terminal/flowControl.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { HIGH_WATERMARK, LOW_WATERMARK, WriteFlow } from "./flowControl";

describe("WriteFlow", () => {
  it("reports crossing the high watermark once", () => {
    const flow = new WriteFlow();
    expect(flow.wrote(HIGH_WATERMARK)).toBe(false);
    expect(flow.isBehind).toBe(false);
    expect(flow.wrote(1)).toBe(true);
    expect(flow.isBehind).toBe(true);
    expect(flow.wrote(1)).toBe(false);
    expect(flow.pendingBytes).toBe(HIGH_WATERMARK + 2);
  });

  it("reports catching up once the queue drains to the low watermark", () => {
    const flow = new WriteFlow();
    flow.wrote(HIGH_WATERMARK + 1);
    expect(flow.parsed(HIGH_WATERMARK - LOW_WATERMARK)).toBe(false);
    expect(flow.parsed(1)).toBe(true);
    expect(flow.isBehind).toBe(false);
    expect(flow.parsed(1)).toBe(false);
  });

  it("never reports catching up when it was not behind, and never goes below zero", () => {
    const flow = new WriteFlow();
    flow.wrote(10);
    expect(flow.parsed(10)).toBe(false);
    expect(flow.parsed(10)).toBe(false);
    expect(flow.pendingBytes).toBe(0);
  });

  it("keeps a full-size snapshot below the high watermark", () => {
    expect(HIGH_WATERMARK).toBeGreaterThan(16 * 1024 * 1024);
    expect(HIGH_WATERMARK + 16 * 1024 * 1024).toBeLessThan(50_000_000);
  });
});
```

`app/src/lib/terminal/paneStream.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { HIGH_WATERMARK } from "./flowControl";
import { createPaneStream } from "./paneStream";

function harness() {
  const writes: { bytes: Uint8Array; done: () => void }[] = [];
  const attaches: { resolve: () => void; reject: (err: unknown) => void }[] = [];
  const stream = createPaneStream({
    write: (bytes, done) => writes.push({ bytes, done }),
    sendAttach: () => new Promise<void>((resolve, reject) => attaches.push({ resolve, reject })),
  });
  return { stream, writes, attaches };
}

const b = (...bytes: number[]) => new Uint8Array(bytes);
const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("createPaneStream", () => {
  it("sends one Attach and writes nothing until its snapshot", () => {
    const { stream, writes, attaches } = harness();
    stream.start();
    expect(attaches).toHaveLength(1);
    stream.deliver("output", b(1));
    expect(writes).toHaveLength(0);
    stream.deliver("snapshot", b(2));
    stream.deliver("output", b(3));
    expect(writes.map((w) => [...w.bytes])).toEqual([[2], [3]]);
  });

  it("does not send a second Attach while one is outstanding", () => {
    const { stream, attaches } = harness();
    stream.start();
    stream.reattach(false);
    stream.reattach(false);
    expect(attaches).toHaveLength(1);
    stream.deliver("snapshot", b(1));
    stream.reattach(false);
    expect(attaches).toHaveLength(2);
  });

  it("sends a new Attach after a reconnect even if one was outstanding", () => {
    const { stream, attaches } = harness();
    stream.start();
    stream.reattach(true);
    expect(attaches).toHaveLength(2);
  });

  it("can retry an Attach the bridge refused", async () => {
    const { stream, attaches } = harness();
    stream.start();
    attaches[0].reject(new Error("not connected to jojod"));
    await tick();
    stream.reattach(false);
    expect(attaches).toHaveLength(2);
  });

  it("can retry after jojod refuses the Attach", () => {
    const { stream, attaches } = harness();
    stream.start();
    stream.attachFailed();
    stream.reattach(false);
    expect(attaches).toHaveLength(2);
  });

  it("drops output above the high watermark and resyncs once the terminal catches up", () => {
    const { stream, writes, attaches } = harness();
    stream.start();
    stream.deliver("snapshot", b(0));
    writes[0].done();
    stream.deliver("output", new Uint8Array(HIGH_WATERMARK));
    stream.deliver("output", b(1));
    expect(writes).toHaveLength(3);
    stream.deliver("output", b(2));
    expect(writes).toHaveLength(3);
    writes[1].done();
    expect(attaches).toHaveLength(2);
    stream.deliver("output", b(3));
    expect(writes).toHaveLength(3);
    stream.deliver("snapshot", b(4));
    stream.deliver("output", b(5));
    expect(writes.slice(3).map((w) => [...w.bytes])).toEqual([[4], [5]]);
  });
});
```

Replace `app/src/lib/terminal/router.test.ts`:
```ts
import { describe, expect, it, vi } from "vitest";
import { createRouter } from "./router";

describe("router", () => {
  it("delivers data frames, re-attaches on fell-behind and reconnect, reports attach failures, and unregisters", () => {
    const router = createRouter();
    const pane = { deliver: vi.fn(), reattach: vi.fn(), attachFailed: vi.fn() };
    const unregister = router.register("S1", pane);
    router.deliver({ kind: "output", sessionId: "S1", bytes: new Uint8Array([1]) });
    router.deliver({ kind: "output", sessionId: "S2", bytes: new Uint8Array([2]) });
    expect(pane.deliver).toHaveBeenCalledTimes(1);
    router.fellBehind("S1");
    expect(pane.reattach).toHaveBeenLastCalledWith(false);
    router.reattachAll();
    expect(pane.reattach).toHaveBeenLastCalledWith(true);
    router.attachFailed("S1");
    expect(pane.attachFailed).toHaveBeenCalledTimes(1);
    unregister();
    router.deliver({ kind: "output", sessionId: "S1", bytes: new Uint8Array([3]) });
    expect(pane.deliver).toHaveBeenCalledTimes(1);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --dir app exec vitest run src/lib/terminal`
Expected: FAIL: `begin is not a function` in attachGate.test.ts, unresolved `./flowControl` and `./paneStream`, and `attachFailed is not a function` in router.test.ts.

- [ ] **Step 3: Write the gate, the flow counter and the pane stream**

Replace `app/src/lib/terminal/attachGate.ts`:
```ts
// Decides which Snapshot and Output frames a pane writes into its terminal
// (Plan 2a carry-over 2.1 item 2; Plan 2b carry-over 3.2 item 1).
//
// jojod answers every Attach with exactly one Snapshot, and live Output for
// the session follows that Snapshot. Output that arrives while an Attach is
// outstanding belongs to the previous subscription (queued before a Detach,
// or sent before jojod dropped this client for falling behind) and is
// dropped; the Snapshot replaces it, because a snapshot starts with a full
// terminal reset.
//
// jojod's per-client queue accounting (crates/jojod/src/client_queue.rs
// counts a Snapshot as 256 bytes) assumes the app keeps at most one Attach
// outstanding per session. The gate counts Attaches sent and Snapshots
// received, and `begin()` refuses a second Attach while they differ; the
// outstanding Attach's Snapshot brings the pane up to date anyway.
// Snapshots carry no request id, so a Snapshot that arrives when none is
// outstanding (left from a pane instance that was unmounted before its
// Snapshot arrived) is written but not counted.
export class AttachGate {
  private sent = 0;
  private answered = 0;

  get outstanding(): boolean {
    return this.sent > this.answered;
  }

  /** True when the caller should send an Attach now; false while one is outstanding. */
  begin(): boolean {
    if (this.outstanding) return false;
    this.sent += 1;
    return true;
  }

  /**
   * The outstanding Attach will never be answered: the bridge refused to
   * send it, jojod replied with an Attach error, or the connection it was
   * sent on has closed.
   */
  abandon(): void {
    this.answered = this.sent;
  }

  accept(kind: "output" | "snapshot", bytes: Uint8Array): Uint8Array | null {
    if (kind === "snapshot") {
      this.answered = Math.min(this.answered + 1, this.sent);
      return bytes;
    }
    return this.outstanding ? null : bytes;
  }
}
```

`app/src/lib/terminal/flowControl.ts`:
```ts
// Counts bytes handed to term.write() that xterm.js has not yet parsed
// (Plan 2b carry-over 3.2 item 2). xterm.js throws "write data discarded,
// use flow control to avoid losing data" once more than 50,000,000 bytes
// wait in its write queue. Above HIGH_WATERMARK the pane drops Output; at
// LOW_WATERMARK after that it attaches again, and the Snapshot replaces the
// dropped Output.

/**
 * Above the largest Snapshot jojod sends (MAX_SNAPSHOT_BYTES, 16 MiB minus
 * 17 bytes), so a Snapshot alone never counts as falling behind, and low
 * enough that a full Snapshot written on top of it stays below xterm.js's
 * 50,000,000-byte limit.
 */
export const HIGH_WATERMARK = 24 * 1024 * 1024;

/** Queue size at which a pane that fell behind attaches again. */
export const LOW_WATERMARK = 2 * 1024 * 1024;

export class WriteFlow {
  private pending = 0;
  private behind = false;

  get isBehind(): boolean {
    return this.behind;
  }

  get pendingBytes(): number {
    return this.pending;
  }

  /** Records bytes passed to term.write(); true when this write crossed HIGH_WATERMARK. */
  wrote(n: number): boolean {
    this.pending += n;
    if (!this.behind && this.pending > HIGH_WATERMARK) {
      this.behind = true;
      return true;
    }
    return false;
  }

  /** Records a term.write() callback; true when a pane that fell behind has drained to LOW_WATERMARK. */
  parsed(n: number): boolean {
    this.pending = Math.max(0, this.pending - n);
    if (this.behind && this.pending <= LOW_WATERMARK) {
      this.behind = false;
      return true;
    }
    return false;
  }
}
```

`app/src/lib/terminal/paneStream.ts`:
```ts
// The data path of one mounted pane: which frames reach term.write(), and
// when to send Attach. TerminalPane.svelte supplies the terminal write and
// the Attach request; everything else is here so it can be tested without
// xterm.js.
import { AttachGate } from "./attachGate";
import { WriteFlow } from "./flowControl";
import type { PaneHandle } from "./router";

export interface PaneStreamDeps {
  /** term.write(bytes, done): `done` runs once xterm.js has parsed `bytes`. */
  write(bytes: Uint8Array, done: () => void): void;
  /** Sends Attach with the pane's current size; rejects when the bridge refuses (disconnected). */
  sendAttach(): Promise<void>;
}

export interface PaneStream extends PaneHandle {
  /** Sends the first Attach. */
  start(): void;
}

export function createPaneStream(deps: PaneStreamDeps): PaneStream {
  const gate = new AttachGate();
  const flow = new WriteFlow();
  const attach = (): void => {
    if (!gate.begin()) return;
    deps.sendAttach().catch(() => gate.abandon());
  };
  return {
    start: attach,
    deliver(kind, bytes) {
      const accepted = gate.accept(kind, bytes);
      if (!accepted) return;
      // While behind, Output is dropped; the Attach sent on catching up
      // brings a Snapshot that covers it. Snapshots are always written.
      if (kind === "output" && flow.isBehind) return;
      flow.wrote(accepted.length);
      deps.write(accepted, () => {
        if (flow.parsed(accepted.length)) attach();
      });
    },
    reattach(afterReconnect) {
      // An Attach outstanding on a connection that has since closed will
      // never be answered.
      if (afterReconnect) gate.abandon();
      attach();
    },
    attachFailed() {
      gate.abandon();
    },
  };
}
```

Replace `app/src/lib/terminal/router.ts`:
```ts
// Routes Output and Snapshot frames to the mounted pane of their session.
import type { Frame } from "../wire/frame";

export interface PaneHandle {
  deliver(kind: "output" | "snapshot", bytes: Uint8Array): void;
  /** Attach again for a fresh Snapshot; `afterReconnect` discards an Attach outstanding on the old connection. */
  reattach(afterReconnect: boolean): void;
  /** jojod answered this pane's Attach with an error, so no Snapshot is coming. */
  attachFailed(): void;
}

export function createRouter() {
  const panes = new Map<string, PaneHandle>();
  return {
    register(sessionId: string, pane: PaneHandle): () => void {
      panes.set(sessionId, pane);
      return () => {
        if (panes.get(sessionId) === pane) panes.delete(sessionId);
      };
    },
    deliver(frame: Frame): void {
      if (frame.kind === "output" || frame.kind === "snapshot") panes.get(frame.sessionId)?.deliver(frame.kind, frame.bytes);
    },
    /** jojod detached a client that fell behind; attach again for a fresh snapshot. */
    fellBehind(sessionId: string | null): void {
      if (sessionId) panes.get(sessionId)?.reattach(false);
    },
    /** After a reconnect, every mounted pane attaches again. */
    reattachAll(): void {
      for (const pane of panes.values()) pane.reattach(true);
    },
    attachFailed(sessionId: string): void {
      panes.get(sessionId)?.attachFailed();
    },
  };
}

export const router = createRouter();
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm --dir app exec vitest run src/lib/terminal`
Expected: PASS (attachGate, flowControl, paneStream, router, and the unchanged pane, queries, size tests).

- [ ] **Step 5: Use the pane stream in the terminal pane**

In `app/src/ui/TerminalPane.svelte`, replace the import `import { AttachGate } from "../lib/terminal/attachGate";` with `import { createPaneStream } from "../lib/terminal/paneStream";`, and replace the block from `const id = session.id;` through the closing `});` of `router.register(...)` (lines 59-81) with:
```ts
    const id = session.id;
    let size = clampSize(term.cols, term.rows);
    const stream = createPaneStream({
      write: (bytes, done) => term.write(bytes, done),
      sendAttach: () => sendControl({ type: "Attach", session_id: id, cols: size.cols, rows: size.rows }),
    });
    // A session with no screen is never attached, so it is not registered
    // with the router: reattachAll() after a reconnect must not send an
    // Attach that jojod can only refuse.
    const unregister = session.attachable ? router.register(id, stream) : () => {};
    if (session.attachable) stream.start();
    else term.write(`\x1b[2m${noScreenMessage(session)}\x1b[0m\r\n`);
```

In `app/src/lib/state/app.svelte.ts`, in `handleControl`, add directly after the existing `if (msg.type === "Error" && msg.request === "Output") { ... }` block:
```ts
  if (msg.type === "Error" && msg.request === "Attach" && msg.session_id) {
    router.attachFailed(msg.session_id);
  }
```
(The message still falls through to `applyControl`, so the banner shows the error as before.)

- [ ] **Step 6: Type-check and run the app tests**

Run: `pnpm --dir app check` and `pnpm --dir app test`
Expected: 0 errors; all tests pass.

- [ ] **Step 7: Commit**

```bash
git add app/src/lib/terminal app/src/ui/TerminalPane.svelte app/src/lib/state/app.svelte.ts
git commit -m "feat(app): one outstanding Attach per session and write-queue flow control"
```

---

### Task 7: Shortcut resolution and the right-click rule

**Files:**
- Create: `app/src/lib/input/shortcuts.ts`, `app/src/lib/input/mouse.ts`
- Test: `app/src/lib/input/shortcuts.test.ts`, `app/src/lib/input/mouse.test.ts`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces (used by Tasks 8, 10, 11):
  - `type AppAction = "new-claude-tab" | "new-shell-tab" | "split-right" | "split-down" | "zoom" | "focus-left" | "focus-right" | "focus-up" | "focus-down" | "next-tab" | "previous-tab" | "close-pane" | "copy" | "paste"`.
  - `interface KeyInput { key: string; code: string; ctrlKey: boolean; shiftKey: boolean; altKey: boolean; metaKey: boolean }` (a `KeyboardEvent` satisfies it).
  - `interface ShortcutContext { copyOnSelect: boolean; hasSelection: boolean }`.
  - `SHORTCUT_LABELS: Record<AppAction, string>`; `resolveShortcut(event: KeyInput, context: ShortcutContext): AppAction | null`.
  - `type MouseTrackingMode = "none" | "x10" | "vt200" | "drag" | "any"` (the values of xterm.js 6 `term.modes.mouseTrackingMode`); `contextMenuTarget(shiftKey: boolean, mode: MouseTrackingMode): "jojo" | "program"`.

Ctrl+Shift+P (switcher) and Ctrl+Shift+J (jump to `needs_you`/`done`) are not resolved in Plan 3 (see Scope), so those keystrokes reach the terminal until the plans that add them.

- [ ] **Step 1: Write the failing tests**

`app/src/lib/input/shortcuts.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { resolveShortcut, SHORTCUT_LABELS, type AppAction, type KeyInput } from "./shortcuts";

function key(code: string, mods: Partial<KeyInput> = {}, keyValue = code): KeyInput {
  return { key: keyValue, code, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...mods };
}

const copyOn = { copyOnSelect: true, hasSelection: false };

describe("resolveShortcut", () => {
  it("maps every Plan 3 shortcut from spec 9.6", () => {
    const cases: [KeyInput, AppAction][] = [
      [key("KeyT", { ctrlKey: true, shiftKey: true }, "T"), "new-claude-tab"],
      [key("KeyN", { ctrlKey: true, shiftKey: true }, "N"), "new-shell-tab"],
      [key("Equal", { altKey: true, shiftKey: true }, "+"), "split-right"],
      [key("Minus", { altKey: true, shiftKey: true }, "_"), "split-down"],
      [key("KeyZ", { altKey: true, shiftKey: true }, "Z"), "zoom"],
      [key("ArrowLeft", { altKey: true }), "focus-left"],
      [key("ArrowRight", { altKey: true }), "focus-right"],
      [key("ArrowUp", { altKey: true }), "focus-up"],
      [key("ArrowDown", { altKey: true }), "focus-down"],
      [key("Tab", { ctrlKey: true }), "next-tab"],
      [key("Tab", { ctrlKey: true, shiftKey: true }), "previous-tab"],
      [key("KeyW", { ctrlKey: true, shiftKey: true }, "W"), "close-pane"],
      [key("KeyC", { ctrlKey: true, shiftKey: true }, "C"), "copy"],
      [key("KeyV", { ctrlKey: true, shiftKey: true }, "V"), "paste"],
    ];
    for (const [input, action] of cases) expect(resolveShortcut(input, copyOn), `${input.code}`).toBe(action);
  });

  it("always sends Ctrl+C to the terminal when copyOnSelect is on", () => {
    const ctrlC = key("KeyC", { ctrlKey: true }, "c");
    expect(resolveShortcut(ctrlC, { copyOnSelect: true, hasSelection: true })).toBeNull();
    expect(resolveShortcut(ctrlC, { copyOnSelect: true, hasSelection: false })).toBeNull();
  });

  it("copies with Ctrl+C when copyOnSelect is off and text is selected, else sends it", () => {
    const ctrlC = key("KeyC", { ctrlKey: true }, "c");
    expect(resolveShortcut(ctrlC, { copyOnSelect: false, hasSelection: true })).toBe("copy");
    expect(resolveShortcut(ctrlC, { copyOnSelect: false, hasSelection: false })).toBeNull();
  });

  it("leaves plain Ctrl combinations, AltGr, the Windows key and later-plan shortcuts to the terminal", () => {
    expect(resolveShortcut(key("KeyT", { ctrlKey: true }, "t"), copyOn)).toBeNull();
    expect(resolveShortcut(key("KeyK", { ctrlKey: true }, "k"), copyOn)).toBeNull();
    expect(resolveShortcut(key("KeyV", { ctrlKey: true }, "v"), copyOn)).toBeNull();
    expect(resolveShortcut(key("ArrowLeft", { ctrlKey: true, altKey: true }), copyOn)).toBeNull();
    expect(resolveShortcut(key("KeyT", { ctrlKey: true, shiftKey: true, metaKey: true }, "T"), copyOn)).toBeNull();
    expect(resolveShortcut(key("KeyP", { ctrlKey: true, shiftKey: true }, "P"), copyOn)).toBeNull();
    expect(resolveShortcut(key("KeyJ", { ctrlKey: true, shiftKey: true }, "J"), copyOn)).toBeNull();
    expect(resolveShortcut(key("KeyA"), copyOn)).toBeNull();
  });

  it("has a menu label for every action", () => {
    expect(SHORTCUT_LABELS["split-right"]).toBe("Alt+Shift+=");
    expect(SHORTCUT_LABELS.zoom).toBe("Alt+Shift+Z");
    expect(SHORTCUT_LABELS["close-pane"]).toBe("Ctrl+Shift+W");
  });
});
```

`app/src/lib/input/mouse.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { contextMenuTarget } from "./mouse";

describe("contextMenuTarget", () => {
  it("opens Jojo's menu on a plain right-click when mouse reporting is off", () => {
    expect(contextMenuTarget(false, "none")).toBe("jojo");
  });

  it("sends a plain right-click to the program when mouse reporting is on", () => {
    for (const mode of ["x10", "vt200", "drag", "any"] as const) expect(contextMenuTarget(false, mode), mode).toBe("program");
  });

  it("always opens Jojo's menu on Shift+right-click", () => {
    for (const mode of ["none", "x10", "vt200", "drag", "any"] as const) expect(contextMenuTarget(true, mode), mode).toBe("jojo");
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --dir app exec vitest run src/lib/input`
Expected: FAIL with "Failed to resolve import "./shortcuts"" and "Failed to resolve import "./mouse"".

- [ ] **Step 3: Write the implementation**

`app/src/lib/input/shortcuts.ts`:
```ts
// Application keyboard shortcuts (spec 9.6) and the Ctrl+C rules (spec 9.5).
// Application shortcuts use Ctrl+Shift or Alt+Shift, plus Ctrl+Tab,
// Ctrl+Shift+Tab and Alt+Arrow. Plain Ctrl combinations reach the terminal,
// because Claude Code and shells bind them (Claude Code binds Ctrl+T;
// readline binds Ctrl+K), with one exception: Ctrl+C copies when
// copyOnSelect is off and text is selected. Letter and punctuation keys are
// matched by KeyboardEvent.code (the physical key), because with Shift held
// KeyboardEvent.key is "T" or "+" rather than "t" or "=".

export type AppAction =
  | "new-claude-tab"
  | "new-shell-tab"
  | "split-right"
  | "split-down"
  | "zoom"
  | "focus-left"
  | "focus-right"
  | "focus-up"
  | "focus-down"
  | "next-tab"
  | "previous-tab"
  | "close-pane"
  | "copy"
  | "paste";

export interface KeyInput {
  key: string;
  code: string;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
}

export interface ShortcutContext {
  copyOnSelect: boolean;
  /** Whether the focused pane has selected text. */
  hasSelection: boolean;
}

/** Shortcut text shown next to menu items (spec 9.4). */
export const SHORTCUT_LABELS: Record<AppAction, string> = {
  "new-claude-tab": "Ctrl+Shift+T",
  "new-shell-tab": "Ctrl+Shift+N",
  "split-right": "Alt+Shift+=",
  "split-down": "Alt+Shift+-",
  zoom: "Alt+Shift+Z",
  "focus-left": "Alt+Left",
  "focus-right": "Alt+Right",
  "focus-up": "Alt+Up",
  "focus-down": "Alt+Down",
  "next-tab": "Ctrl+Tab",
  "previous-tab": "Ctrl+Shift+Tab",
  "close-pane": "Ctrl+Shift+W",
  copy: "Ctrl+Shift+C",
  paste: "Ctrl+Shift+V",
};

const CTRL_SHIFT: Record<string, AppAction> = {
  KeyT: "new-claude-tab",
  KeyN: "new-shell-tab",
  KeyW: "close-pane",
  KeyC: "copy",
  KeyV: "paste",
  Tab: "previous-tab",
};

const ALT_SHIFT: Record<string, AppAction> = {
  Equal: "split-right",
  Minus: "split-down",
  KeyZ: "zoom",
};

const ALT_ARROWS: Record<string, AppAction> = {
  ArrowLeft: "focus-left",
  ArrowRight: "focus-right",
  ArrowUp: "focus-up",
  ArrowDown: "focus-down",
};

/**
 * The application action for a key event, or null when the key belongs to
 * the terminal. Ctrl+Alt is AltGr on many keyboard layouts and is never an
 * application shortcut; neither is anything with the Windows key.
 */
export function resolveShortcut(event: KeyInput, context: ShortcutContext): AppAction | null {
  const { ctrlKey: ctrl, shiftKey: shift, altKey: alt, metaKey: meta } = event;
  if (meta) return null;
  if (ctrl && shift && !alt) return CTRL_SHIFT[event.code] ?? null;
  if (alt && shift && !ctrl) return ALT_SHIFT[event.code] ?? null;
  if (alt && !shift && !ctrl) return ALT_ARROWS[event.key] ?? null;
  if (ctrl && !shift && !alt) {
    if (event.code === "Tab") return "next-tab";
    if (event.code === "KeyC" && !context.copyOnSelect && context.hasSelection) return "copy";
  }
  return null;
}
```

`app/src/lib/input/mouse.ts`:
```ts
// Spec 9.4, mouse reporting rule. When the program in a pane has enabled
// mouse reporting (vim, htop, any TUI in mouse mode), a plain right-click
// goes to the program and Shift+right-click opens Jojo's menu; when
// reporting is off, a plain right-click opens Jojo's menu.
//
// Spec 9.5 rule 4 (Shift+drag forces a selection while mouse reporting is
// on) needs no code here: on Windows, xterm.js's
// SelectionService.shouldForceSelection returns event.shiftKey, and its
// mouse service then does not report the press to the program.

/** The values of xterm.js 6 `term.modes.mouseTrackingMode`. */
export type MouseTrackingMode = "none" | "x10" | "vt200" | "drag" | "any";

export function contextMenuTarget(shiftKey: boolean, mode: MouseTrackingMode): "jojo" | "program" {
  return shiftKey || mode === "none" ? "jojo" : "program";
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm --dir app exec vitest run src/lib/input`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src/lib/input
git commit -m "feat(app): shortcut resolution with the Ctrl+C rules, and the right-click rule"
```

---

### Task 8: Menu model, navigation and builders

**Files:**
- Create: `app/src/lib/menu/types.ts`, `app/src/lib/menu/navigation.ts`, `app/src/lib/menu/menus.ts`
- Test: `app/src/lib/menu/navigation.test.ts`, `app/src/lib/menu/menus.test.ts`

**Interfaces:**
- Consumes: `MAX_PANES`, `countPanes`, `paneIds`, `LayoutPreset`, `SplitDirection` (Task 2); `SHORTCUT_LABELS` (Task 7); `canRestart` (Task 5); generated `TabLayout`, `SessionInfo`, `SessionKind`.
- Produces (used by Task 10):
  - `types.ts`: `interface MenuAction { kind: "action"; label: string; shortcut?: string; disabledReason?: string; run: () => void }`; `interface MenuSubmenu { kind: "submenu"; label: string; disabledReason?: string; items: MenuEntry[] }`; `interface MenuSeparator { kind: "separator" }`; `type MenuEntry`; `SEPARATOR: MenuSeparator`; `isEnabled(entry: MenuEntry): boolean`.
  - `navigation.ts`: `interface Point { x: number; y: number }`; `interface Size { w: number; h: number }`; `interface Box { x: number; y: number; w: number; h: number }`; `moveHighlight(entries, current: number, delta: 1 | -1): number`; `firstEnabled(entries): number`; `placeMenu(anchor: Point, size: Size, viewport: Size, margin?: number): Point`; `placeSubmenu(row: Box, size: Size, viewport: Size, margin?: number): Point`.
  - `menus.ts`: `TAB_FULL = "Tab is full (4 panes)"`; `ONLY_PANE = "Only pane in this tab"`; `ALONE_IN_TAB = "Already the only pane in its tab"`; `PRESET_LABELS: Record<LayoutPreset, string>`; `interface PaneMenuContext { sessionId: string; hasSelection: boolean; tab: TabLayout; otherTabs: TabLayout[]; labelOf(sessionId: string): string; tabLabelOf(tab: TabLayout): string }`; `interface PaneMenuActions { copy(): void; paste(): void; split(direction: SplitDirection, kind: SessionKind): void; toggleZoom(): void; swapWith(sessionId: string): void; moveToNewTab(): void; moveToTab(tabId: string): void; applyPreset(preset: LayoutPreset): void; equalize(): void; rename(): void; close(): void }`; `buildPaneMenu(ctx, actions): MenuEntry[]`; `interface TabMenuActions { rename(): void; applyPreset(preset: LayoutPreset): void; close(): void }`; `buildTabMenu(tab: TabLayout, actions): MenuEntry[]`; `interface SessionMenuContext { session: SessionInfo; currentTab: TabLayout | null; currentProjectId: string | null; sessionTab: TabLayout | null }`; `interface SessionMenuActions { openInSplit(direction: SplitDirection): void; moveToNewTab(): void; rename(): void; restart(): void; kill(): void }`; `buildSessionMenu(ctx, actions): MenuEntry[]`.

A disabled entry carries `disabledReason`, which the component shows as the row's tooltip (spec 9.4, "Limits"). A submenu whose items are all disabled is itself disabled.

- [ ] **Step 1: Write the failing tests**

`app/src/lib/menu/navigation.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { firstEnabled, moveHighlight, placeMenu, placeSubmenu } from "./navigation";
import { isEnabled, SEPARATOR, type MenuEntry } from "./types";

const run = () => {};
const entries: MenuEntry[] = [
  { kind: "action", label: "Copy", disabledReason: "No text selected", run },
  { kind: "action", label: "Paste", run },
  SEPARATOR,
  { kind: "submenu", label: "Swap with", items: [{ kind: "action", label: "x", disabledReason: "no", run }] },
  { kind: "action", label: "Close pane", run },
];

describe("isEnabled", () => {
  it("disables separators, entries with a reason, and submenus whose items are all disabled", () => {
    expect(entries.map(isEnabled)).toEqual([false, true, false, false, true]);
  });
});

describe("moveHighlight", () => {
  it("skips separators and disabled entries and wraps in both directions", () => {
    expect(moveHighlight(entries, 1, 1)).toBe(4);
    expect(moveHighlight(entries, 4, 1)).toBe(1);
    expect(moveHighlight(entries, 1, -1)).toBe(4);
  });

  it("starts at the first or last enabled entry when nothing is highlighted", () => {
    expect(moveHighlight(entries, -1, 1)).toBe(1);
    expect(moveHighlight(entries, -1, -1)).toBe(4);
    expect(firstEnabled(entries)).toBe(1);
  });

  it("returns -1 when no entry is enabled", () => {
    expect(moveHighlight([SEPARATOR], -1, 1)).toBe(-1);
    expect(moveHighlight([], 0, 1)).toBe(-1);
  });
});

describe("placement", () => {
  const viewport = { w: 800, h: 600 };

  it("opens a menu at the pointer when it fits", () => {
    expect(placeMenu({ x: 10, y: 10 }, { w: 100, h: 50 }, viewport)).toEqual({ x: 10, y: 10 });
  });

  it("flips a menu left of the pointer and moves it up to stay inside the window", () => {
    expect(placeMenu({ x: 750, y: 580 }, { w: 100, h: 50 }, viewport)).toEqual({ x: 650, y: 546 });
  });

  it("opens a submenu to the right of its row, or to the left when there is no room", () => {
    expect(placeSubmenu({ x: 100, y: 100, w: 150, h: 24 }, { w: 120, h: 80 }, viewport)).toEqual({ x: 250, y: 100 });
    expect(placeSubmenu({ x: 600, y: 560, w: 150, h: 24 }, { w: 120, h: 80 }, viewport)).toEqual({ x: 480, y: 516 });
  });
});
```

`app/src/lib/menu/menus.test.ts`:
```ts
import { describe, expect, it, vi } from "vitest";
import type { TabLayout } from "../backend/TabLayout";
import { pane, split } from "../layout/tree";
import type { SessionInfo } from "../protocol/SessionInfo";
import {
  ALONE_IN_TAB,
  buildPaneMenu,
  buildSessionMenu,
  buildTabMenu,
  ONLY_PANE,
  TAB_FULL,
  type PaneMenuActions,
  type PaneMenuContext,
  type SessionMenuActions,
} from "./menus";
import { isEnabled, type MenuEntry, type MenuSubmenu } from "./types";

const noop = () => {};

function paneActions(over: Partial<PaneMenuActions> = {}): PaneMenuActions {
  return {
    copy: noop,
    paste: noop,
    split: noop,
    toggleZoom: noop,
    swapWith: noop,
    moveToNewTab: noop,
    moveToTab: noop,
    applyPreset: noop,
    equalize: noop,
    rename: noop,
    close: noop,
    ...over,
  };
}

function sessionActions(over: Partial<SessionMenuActions> = {}): SessionMenuActions {
  return { openInSplit: noop, moveToNewTab: noop, rename: noop, restart: noop, kill: noop, ...over };
}

function tab(id: string, ids: string[], extra: Partial<TabLayout> = {}): TabLayout {
  const root = ids.length === 1 ? pane(ids[0]) : split("row", ids.map(pane));
  return { id, name: null, root, focused: ids[0], zoomed: null, ...extra };
}

function context(over: Partial<PaneMenuContext> = {}): PaneMenuContext {
  return {
    sessionId: "A",
    hasSelection: false,
    tab: tab("T1", ["A", "B"]),
    otherTabs: [],
    labelOf: (id) => `label-${id}`,
    tabLabelOf: (t) => `tab-${t.id}`,
    ...over,
  };
}

function entry(entries: MenuEntry[], label: string): MenuEntry {
  const found = entries.find((e) => e.kind !== "separator" && e.label === label);
  if (!found) throw new Error(`no entry ${label}`);
  return found;
}

const labels = (entries: MenuEntry[]) => entries.flatMap((e) => (e.kind === "separator" ? [] : [e.label]));

function session(over: Partial<SessionInfo> = {}): SessionInfo {
  return {
    id: "S",
    project_id: "P1",
    kind: "shell",
    command: "pwsh",
    args: [],
    label: "pwsh",
    created_at: 1,
    state: "running",
    state_changed_at: 1,
    exit_code: null,
    claude_session_id: null,
    attachable: true,
    ...over,
  };
}

describe("buildPaneMenu", () => {
  it("lists the spec 9.4 pane menu items in order", () => {
    expect(labels(buildPaneMenu(context(), paneActions()))).toEqual([
      "Copy",
      "Paste",
      "Split right",
      "Split down",
      "Zoom pane",
      "Swap with",
      "Move to",
      "Layout",
      "Equalize sizes",
      "Rename session…",
      "Close pane",
    ]);
  });

  it("disables splits and full Move-to targets in a full tab with the tab-full tooltip", () => {
    const menu = buildPaneMenu(
      context({ tab: tab("T1", ["A", "B", "C", "D"]), otherTabs: [tab("T2", ["E", "F", "G", "H"]), tab("T3", ["I"])] }),
      paneActions(),
    );
    expect(entry(menu, "Split right")).toMatchObject({ disabledReason: TAB_FULL });
    expect(entry(menu, "Split down")).toMatchObject({ disabledReason: TAB_FULL });
    const moveTo = (entry(menu, "Move to") as MenuSubmenu).items;
    expect(entry(moveTo, "tab-T2")).toMatchObject({ disabledReason: TAB_FULL });
    expect(isEnabled(entry(moveTo, "tab-T3"))).toBe(true);
    expect(TAB_FULL).toBe("Tab is full (4 panes)");
  });

  it("enables Copy only with a selection", () => {
    expect(isEnabled(entry(buildPaneMenu(context(), paneActions()), "Copy"))).toBe(false);
    expect(isEnabled(entry(buildPaneMenu(context({ hasSelection: true }), paneActions()), "Copy"))).toBe(true);
  });

  it("runs split with the direction and kind, and shows the split shortcut", () => {
    const split = vi.fn();
    const right = (entry(buildPaneMenu(context(), paneActions({ split })), "Split right") as MenuSubmenu).items;
    expect(entry(right, "New Claude")).toMatchObject({ shortcut: "Alt+Shift+=" });
    const newShell = entry(right, "New shell");
    if (newShell.kind === "action") newShell.run();
    expect(split).toHaveBeenCalledWith("row", "shell");
  });

  it("offers the other panes by label in Swap with", () => {
    const swapWith = vi.fn();
    const items = (entry(buildPaneMenu(context({ tab: tab("T1", ["A", "B", "C"]) }), paneActions({ swapWith })), "Swap with") as MenuSubmenu).items;
    expect(labels(items)).toEqual(["label-B", "label-C"]);
    const c = entry(items, "label-C");
    if (c.kind === "action") c.run();
    expect(swapWith).toHaveBeenCalledWith("C");
  });

  it("disables pane-arrangement items for the only pane in a tab", () => {
    const menu = buildPaneMenu(context({ tab: tab("T1", ["A"]) }), paneActions());
    for (const label of ["Zoom pane", "Swap with", "Layout", "Equalize sizes"]) {
      expect(entry(menu, label), label).toMatchObject({ disabledReason: ONLY_PANE });
    }
    expect(entry((entry(menu, "Move to") as MenuSubmenu).items, "New tab")).toMatchObject({ disabledReason: ALONE_IN_TAB });
  });

  it("labels the zoom item by the current zoom state", () => {
    const zoomed = context({ tab: tab("T1", ["A", "B"], { zoomed: "A" }) });
    expect(labels(buildPaneMenu(zoomed, paneActions()))).toContain("Unzoom pane");
  });
});

describe("buildTabMenu", () => {
  it("offers rename, layout presets and close, with presets disabled for one pane", () => {
    const menu = buildTabMenu(tab("T1", ["A"]), { rename: noop, applyPreset: noop, close: noop });
    expect(labels(menu)).toEqual(["Rename tab…", "Layout", "Close tab"]);
    expect(entry(menu, "Layout")).toMatchObject({ disabledReason: ONLY_PANE });
    const presets = (entry(buildTabMenu(tab("T1", ["A", "B"]), { rename: noop, applyPreset: noop, close: noop }), "Layout") as MenuSubmenu).items;
    expect(labels(presets)).toEqual(["Columns", "Rows", "Grid 2×2", "Main + stack"]);
  });
});

describe("buildSessionMenu", () => {
  const current = tab("T1", ["X"]);

  it("enables Open in split only for another session of the current tab's project in a tab with room", () => {
    const ok = buildSessionMenu({ session: session(), currentTab: current, currentProjectId: "P1", sessionTab: tab("T2", ["S"]) }, sessionActions());
    expect(isEnabled(entry(ok, "Open in split right"))).toBe(true);
    const reasons = (ctx: Parameters<typeof buildSessionMenu>[0]) => {
      const e = entry(buildSessionMenu(ctx, sessionActions()), "Open in split down");
      return e.kind === "action" ? e.disabledReason : undefined;
    };
    expect(reasons({ session: session(), currentTab: null, currentProjectId: null, sessionTab: null })).toBe("No tab is open");
    expect(reasons({ session: session(), currentTab: current, currentProjectId: "P2", sessionTab: null })).toBe("The current tab belongs to another project");
    expect(reasons({ session: session(), currentTab: tab("T1", ["X", "S"]), currentProjectId: "P1", sessionTab: null })).toBe("Already in the current tab");
    expect(reasons({ session: session(), currentTab: tab("T1", ["W", "X", "Y", "Z"]), currentProjectId: "P1", sessionTab: null })).toBe(TAB_FULL);
  });

  it("offers Restart only for exited sessions and disables Kill for them", () => {
    const running = buildSessionMenu({ session: session(), currentTab: current, currentProjectId: "P1", sessionTab: tab("T2", ["S"]) }, sessionActions());
    expect(labels(running)).toEqual(["Open in split right", "Open in split down", "Move to new tab", "Rename…", "Kill"]);
    expect(entry(running, "Move to new tab")).toMatchObject({ disabledReason: ALONE_IN_TAB });
    const exited = buildSessionMenu(
      { session: session({ state: "exited" }), currentTab: current, currentProjectId: "P1", sessionTab: tab("T2", ["S", "Q"]) },
      sessionActions(),
    );
    expect(labels(exited)).toContain("Restart");
    expect(entry(exited, "Kill")).toMatchObject({ disabledReason: "Session has already exited" });
    expect(isEnabled(entry(exited, "Move to new tab"))).toBe(true);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --dir app exec vitest run src/lib/menu`
Expected: FAIL with "Failed to resolve import "./navigation"", "./types" and "./menus".

- [ ] **Step 3: Write the menu types and navigation**

`app/src/lib/menu/types.ts`:
```ts
// Entries of Jojo's right-click menus (spec 9.4). A disabled entry carries
// the reason, which the menu shows as the row's tooltip.

export interface MenuAction {
  kind: "action";
  label: string;
  shortcut?: string;
  disabledReason?: string;
  run: () => void;
}

export interface MenuSubmenu {
  kind: "submenu";
  label: string;
  disabledReason?: string;
  items: MenuEntry[];
}

export interface MenuSeparator {
  kind: "separator";
}

export type MenuEntry = MenuAction | MenuSubmenu | MenuSeparator;

export const SEPARATOR: MenuSeparator = { kind: "separator" };

/** Separators are never enabled; a submenu is enabled only if one of its items is. */
export function isEnabled(entry: MenuEntry): boolean {
  if (entry.kind === "separator") return false;
  if (entry.disabledReason) return false;
  return entry.kind === "action" || entry.items.some(isEnabled);
}
```

`app/src/lib/menu/navigation.ts`:
```ts
// Keyboard highlight movement and on-screen placement for ContextMenu.svelte.
import { isEnabled, type MenuEntry } from "./types";

export interface Point {
  x: number;
  y: number;
}

export interface Size {
  w: number;
  h: number;
}

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

/**
 * The next enabled entry after `current` in direction `delta`, wrapping
 * around. With nothing highlighted (`current` < 0), Down starts at the first
 * enabled entry and Up at the last. Returns -1 when no entry is enabled.
 */
export function moveHighlight(entries: MenuEntry[], current: number, delta: 1 | -1): number {
  const n = entries.length;
  if (n === 0) return -1;
  let index = current < 0 ? (delta === 1 ? -1 : n) : current;
  for (let step = 0; step < n; step++) {
    index = (index + delta + n) % n;
    if (isEnabled(entries[index])) return index;
  }
  return -1;
}

export function firstEnabled(entries: MenuEntry[]): number {
  return moveHighlight(entries, -1, 1);
}

/**
 * The top-left corner of a menu opened at `anchor`. It opens to the right of
 * and below the pointer; it flips left of the pointer when it would cross
 * the right edge, and moves up when it would cross the bottom edge, keeping
 * `margin` pixels from the window edges.
 */
export function placeMenu(anchor: Point, size: Size, viewport: Size, margin = 4): Point {
  const x = anchor.x + size.w + margin <= viewport.w ? anchor.x : Math.max(margin, anchor.x - size.w);
  const y = anchor.y + size.h + margin <= viewport.h ? anchor.y : Math.max(margin, viewport.h - size.h - margin);
  return { x, y };
}

/** A submenu opens to the right of its parent row, or to the left of the parent when there is no room. */
export function placeSubmenu(row: Box, size: Size, viewport: Size, margin = 4): Point {
  const right = row.x + row.w;
  const x = right + size.w + margin <= viewport.w ? right : Math.max(margin, row.x - size.w);
  const y = row.y + size.h + margin <= viewport.h ? row.y : Math.max(margin, viewport.h - size.h - margin);
  return { x, y };
}
```

- [ ] **Step 4: Write the menu builders**

`app/src/lib/menu/menus.ts`:
```ts
// The pane menu, tab menu and sidebar session menu of spec 9.4, built from
// plain data so their enabled states and tooltips are tested without the UI.
import type { TabLayout } from "../backend/TabLayout";
import { SHORTCUT_LABELS } from "../input/shortcuts";
import { countPanes, MAX_PANES, paneIds, type LayoutPreset, type SplitDirection } from "../layout/tree";
import type { SessionInfo } from "../protocol/SessionInfo";
import type { SessionKind } from "../protocol/SessionKind";
import { canRestart } from "../state/sessionRules";
import { SEPARATOR, type MenuEntry } from "./types";

/** Spec 9.4, "Limits": the tooltip of split items and Move-to targets in a full tab. */
export const TAB_FULL = "Tab is full (4 panes)";
export const ONLY_PANE = "Only pane in this tab";
export const ALONE_IN_TAB = "Already the only pane in its tab";

export const PRESET_LABELS: Record<LayoutPreset, string> = {
  columns: "Columns",
  rows: "Rows",
  grid: "Grid 2×2",
  main_stack: "Main + stack",
};

function presetItems(apply: (preset: LayoutPreset) => void): MenuEntry[] {
  return (Object.keys(PRESET_LABELS) as LayoutPreset[]).map(
    (preset): MenuEntry => ({ kind: "action", label: PRESET_LABELS[preset], run: () => apply(preset) }),
  );
}

export interface PaneMenuContext {
  sessionId: string;
  hasSelection: boolean;
  /** The tab that holds the pane. */
  tab: TabLayout;
  /** The project's other tabs, in order (Move to targets). */
  otherTabs: TabLayout[];
  labelOf(sessionId: string): string;
  tabLabelOf(tab: TabLayout): string;
}

export interface PaneMenuActions {
  copy(): void;
  paste(): void;
  split(direction: SplitDirection, kind: SessionKind): void;
  toggleZoom(): void;
  swapWith(sessionId: string): void;
  moveToNewTab(): void;
  moveToTab(tabId: string): void;
  applyPreset(preset: LayoutPreset): void;
  equalize(): void;
  rename(): void;
  close(): void;
}

export function buildPaneMenu(ctx: PaneMenuContext, actions: PaneMenuActions): MenuEntry[] {
  const ids = paneIds(ctx.tab.root);
  const single = ids.length < 2;
  const full = ids.length >= MAX_PANES;
  const others = ids.filter((id) => id !== ctx.sessionId);
  const splitItems = (direction: SplitDirection, shortcut: string): MenuEntry[] => [
    { kind: "action", label: "New Claude", shortcut, run: () => actions.split(direction, "claude") },
    { kind: "action", label: "New shell", run: () => actions.split(direction, "shell") },
  ];
  return [
    {
      kind: "action",
      label: "Copy",
      shortcut: SHORTCUT_LABELS.copy,
      disabledReason: ctx.hasSelection ? undefined : "No text selected",
      run: actions.copy,
    },
    { kind: "action", label: "Paste", shortcut: SHORTCUT_LABELS.paste, run: actions.paste },
    SEPARATOR,
    { kind: "submenu", label: "Split right", disabledReason: full ? TAB_FULL : undefined, items: splitItems("row", SHORTCUT_LABELS["split-right"]) },
    { kind: "submenu", label: "Split down", disabledReason: full ? TAB_FULL : undefined, items: splitItems("column", SHORTCUT_LABELS["split-down"]) },
    {
      kind: "action",
      label: ctx.tab.zoomed === ctx.sessionId ? "Unzoom pane" : "Zoom pane",
      shortcut: SHORTCUT_LABELS.zoom,
      disabledReason: single ? ONLY_PANE : undefined,
      run: actions.toggleZoom,
    },
    {
      kind: "submenu",
      label: "Swap with",
      disabledReason: single ? ONLY_PANE : undefined,
      items: others.map((id): MenuEntry => ({ kind: "action", label: ctx.labelOf(id), run: () => actions.swapWith(id) })),
    },
    {
      kind: "submenu",
      label: "Move to",
      items: [
        { kind: "action", label: "New tab", disabledReason: single ? ALONE_IN_TAB : undefined, run: actions.moveToNewTab },
        ...ctx.otherTabs.map(
          (t): MenuEntry => ({
            kind: "action",
            label: ctx.tabLabelOf(t),
            disabledReason: countPanes(t.root) >= MAX_PANES ? TAB_FULL : undefined,
            run: () => actions.moveToTab(t.id),
          }),
        ),
      ],
    },
    { kind: "submenu", label: "Layout", disabledReason: single ? ONLY_PANE : undefined, items: presetItems(actions.applyPreset) },
    { kind: "action", label: "Equalize sizes", disabledReason: single ? ONLY_PANE : undefined, run: actions.equalize },
    SEPARATOR,
    { kind: "action", label: "Rename session…", run: actions.rename },
    { kind: "action", label: "Close pane", shortcut: SHORTCUT_LABELS["close-pane"], run: actions.close },
  ];
}

export interface TabMenuActions {
  rename(): void;
  applyPreset(preset: LayoutPreset): void;
  close(): void;
}

export function buildTabMenu(tab: TabLayout, actions: TabMenuActions): MenuEntry[] {
  const single = countPanes(tab.root) < 2;
  return [
    { kind: "action", label: "Rename tab…", run: actions.rename },
    { kind: "submenu", label: "Layout", disabledReason: single ? ONLY_PANE : undefined, items: presetItems(actions.applyPreset) },
    SEPARATOR,
    { kind: "action", label: "Close tab", run: actions.close },
  ];
}

export interface SessionMenuContext {
  session: SessionInfo;
  /** The active tab of the selected project, if any. */
  currentTab: TabLayout | null;
  /** The selected project's id, which owns `currentTab`. */
  currentProjectId: string | null;
  /** The tab that holds this session. */
  sessionTab: TabLayout | null;
}

export interface SessionMenuActions {
  openInSplit(direction: SplitDirection): void;
  moveToNewTab(): void;
  rename(): void;
  restart(): void;
  kill(): void;
}

/**
 * Why "Open in split" is disabled, or undefined when it is enabled. Spec
 * 9.4: both items move the session into the current tab next to the
 * focused pane, and are enabled only when the current tab belongs to the
 * session's project.
 */
function openInSplitBlocker(ctx: SessionMenuContext): string | undefined {
  if (!ctx.currentTab) return "No tab is open";
  if (ctx.currentProjectId !== ctx.session.project_id) return "The current tab belongs to another project";
  if (paneIds(ctx.currentTab.root).includes(ctx.session.id)) return "Already in the current tab";
  if (countPanes(ctx.currentTab.root) >= MAX_PANES) return TAB_FULL;
  return undefined;
}

export function buildSessionMenu(ctx: SessionMenuContext, actions: SessionMenuActions): MenuEntry[] {
  const blocked = openInSplitBlocker(ctx);
  const alone = !ctx.sessionTab || countPanes(ctx.sessionTab.root) < 2;
  const entries: MenuEntry[] = [
    { kind: "action", label: "Open in split right", disabledReason: blocked, run: () => actions.openInSplit("row") },
    { kind: "action", label: "Open in split down", disabledReason: blocked, run: () => actions.openInSplit("column") },
    { kind: "action", label: "Move to new tab", disabledReason: alone ? ALONE_IN_TAB : undefined, run: actions.moveToNewTab },
    SEPARATOR,
    { kind: "action", label: "Rename…", run: actions.rename },
  ];
  if (canRestart(ctx.session)) entries.push({ kind: "action", label: "Restart", run: actions.restart });
  entries.push({
    kind: "action",
    label: "Kill",
    disabledReason: ctx.session.state === "exited" ? "Session has already exited" : undefined,
    run: actions.kill,
  });
  return entries;
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `pnpm --dir app exec vitest run src/lib/menu`
Expected: PASS.

- [ ] **Step 6: Type-check and commit**

Run: `pnpm --dir app check`
Expected: 0 errors.
```bash
git add app/src/lib/menu
git commit -m "feat(app): context menu model, keyboard navigation and menu builders"
```

---

### Task 9: Layout-driven workspace state and the pane grid

**Files:**
- Rewrite: `app/src/lib/state/app.svelte.ts`, `app/src/ui/TerminalPane.svelte`, `app/src/ui/TabBar.svelte`, `app/src/App.svelte`
- Modify: `app/src/ui/Sidebar.svelte`, `app/src/lib/state/views.ts`, `app/src/lib/state/views.test.ts`, `app/src/themes/neon-glass.json`, `app/src/lib/theme.test.ts`
- Create: `app/src/lib/errors.ts`, `app/src/lib/terminal/registry.ts`, `app/src/lib/state/dialog.svelte.ts`, `app/src/ui/Badge.svelte`, `app/src/ui/ConfirmDialog.svelte`, `app/src/ui/PaneGrid.svelte`, `app/src/ui/PaneView.svelte`

**Interfaces:**
- Consumes: Tasks 1-6 (`get_layout`/`save_layout`, tree, geometry, tabs, reconcile, workspace, sessionRules, pane stream, router).
- Produces (used by Tasks 10-14):
  - `app.svelte.ts` state: `app.{ model, status, settings, selectedProjectId, workspace, layoutLoaded, sessionsLoaded, now }`.
  - Accessors: `projectLayout(projectId): ProjectLayout`; `currentProjectLayout(): ProjectLayout | null`; `currentTab(): TabLayout | null`; `focusedSessionId(): string | null`.
  - Actions: `startApp(): () => void`; `selectProject(id)`; `selectTab(tabId)`; `cycleTab(delta: 1 | -1)`; `focusPane(sessionId)`; `focusDirection(direction: FocusDirection)`; `newSession(projectId, kind, placement?: Placement, shell?: string | null)`; `splitSession(sessionId, direction, kind)`; `splitFocused(direction, kind)`; `closePane(sessionId): Promise<void>`; `closeTab(tabId): Promise<void>`; `renameTab(tabId, name)`; `renameSession(sessionId, label)`; `killSession(sessionId)`; `restartSession(sessionId)`; `zoomSession(sessionId)`; `swapSessions(a, b)`; `moveSessionToNewTab(sessionId)`; `moveSessionToTab(sessionId, tabId)`; `openSessionInSplit(sessionId, direction)`; `applyTabPreset(tabId, preset)`; `equalizeTabSizes(tabId)`; `setTabRoot(tabId, root)`; `addProject()`; `dismissError()`; `restartDaemon()`.
  - `errors.ts`: `errorText(err: unknown): string`.
  - `registry.ts`: `interface TerminalControls { focus(): void; hasSelection(): boolean; getSelection(): string; paste(text: string): void; mouseTrackingMode(): MouseTrackingMode }`; `registerTerminal(sessionId, controls): () => void`; `terminalControls(sessionId: string | null): TerminalControls | undefined`.
  - `dialog.svelte.ts`: `interface ConfirmRequest { title: string; message: string; details: string[]; confirmLabel: string }`; `dialog.confirm`; `askConfirm(request): Promise<boolean>`; `answerConfirm(confirmed: boolean)`.
  - `Badge.svelte` props `{ agentState: AgentState; size?: number }`; `PaneView.svelte` props `{ session; terminalTheme; focused }`; `TerminalPane.svelte` props `{ session; terminalTheme; focused }`.
  - Theme tokens `scrim`, `menu`, `shadow`.

This task removes Plan 2b's `activeSessionId`, `pendingSessionId`, `activateSession` and `closeSession`, and `views.ts`'s `nextActiveAfterRemoval` (with its tests), because the active tab and focused pane now come from the layout and removal is handled by `tabs.removeSession`. Nothing else uses them after this task.

How the pieces fit:

1. `startApp` reads `layout.json` with `get_layout` **before** subscribing to the bridge, so no `SessionCreated` can be placed into an empty layout that the file then overwrites.
2. `reconcile()` runs whenever the full `Sessions` list or `Projects` list arrives (and after `ProjectRemoved`), once both the layout and the first `Sessions` list are loaded.
3. Every layout change goes through `commit`, which writes `layout.json` 250 ms after the last change (chained, so writes never overlap). Pure functions return the same object when nothing changed, so focusing an already focused pane writes nothing.
4. `PaneGrid` renders the active tab's panes from `layoutRects`, keyed by session id. A pane in a hidden tab, in a non-selected project, or hidden by a zoom has no `TerminalPane`, so it is detached (spec 5.5); showing it mounts a `TerminalPane`, which attaches and receives a Snapshot.

- [ ] **Step 1: Add the theme tokens and their test**

In `app/src/themes/neon-glass.json`, add these keys to `tokens` after `"banner"`:
```json
    "scrim": "rgba(4, 6, 14, 0.62)",
    "menu": "rgba(17, 22, 44, 0.98)",
    "shadow": "0 14px 36px rgba(0, 0, 0, 0.5)",
```
In `app/src/lib/theme.test.ts`, extend the `required` array with `"scrim", "menu", "shadow"`.

Run: `pnpm --dir app exec vitest run src/lib/theme.test.ts`
Expected: PASS.

- [ ] **Step 2: Remove nextActiveAfterRemoval**

In `app/src/lib/state/views.ts`, delete the `nextActiveAfterRemoval` function and its doc comment. In `app/src/lib/state/views.test.ts`, delete the `describe("nextActiveAfterRemoval", ...)` block and remove `nextActiveAfterRemoval` from the import.

- [ ] **Step 3: Write the small shared modules**

`app/src/lib/errors.ts`:
```ts
/**
 * Text for the reason a promise rejected. Tauri commands reject with the
 * command's error string; DOM APIs (clipboard) reject with Error objects.
 */
export function errorText(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
```

`app/src/lib/terminal/registry.ts`:
```ts
// The mounted terminals by session id, so menus and shortcuts can act on a
// pane's terminal (copy, paste, selection, mouse mode, focus). A pane that
// is not mounted (hidden tab, zoomed away) has no entry.
import type { MouseTrackingMode } from "../input/mouse";

export interface TerminalControls {
  focus(): void;
  hasSelection(): boolean;
  getSelection(): string;
  paste(text: string): void;
  mouseTrackingMode(): MouseTrackingMode;
}

const terminals = new Map<string, TerminalControls>();

export function registerTerminal(sessionId: string, controls: TerminalControls): () => void {
  terminals.set(sessionId, controls);
  return () => {
    if (terminals.get(sessionId) === controls) terminals.delete(sessionId);
  };
}

export function terminalControls(sessionId: string | null): TerminalControls | undefined {
  return sessionId ? terminals.get(sessionId) : undefined;
}
```

`app/src/lib/state/dialog.svelte.ts`:
```ts
// State of the confirmation dialog (ConfirmDialog.svelte). One request is
// open at a time; opening another answers the first with "cancel".

export interface ConfirmRequest {
  title: string;
  message: string;
  /** Lines listed under the message (for example the sessions a tab close ends). */
  details: string[];
  confirmLabel: string;
}

interface OpenConfirm extends ConfirmRequest {
  resolve(confirmed: boolean): void;
}

export const dialog = $state({ confirm: null as OpenConfirm | null });

export function askConfirm(request: ConfirmRequest): Promise<boolean> {
  dialog.confirm?.resolve(false);
  return new Promise((resolve) => {
    dialog.confirm = { ...request, resolve };
  });
}

export function answerConfirm(confirmed: boolean): void {
  const open = dialog.confirm;
  dialog.confirm = null;
  open?.resolve(confirmed);
}
```

- [ ] **Step 4: Rewrite the app state module**

Replace `app/src/lib/state/app.svelte.ts`:
```ts
// The app's reactive state and actions. The rules live in pure modules
// (../layout, ./workspace, ./sessionRules); this module holds the current
// values, talks to the backend, and keeps layout.json in sync.
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { ConnectionStatus } from "../backend/ConnectionStatus";
import type { LayoutFile } from "../backend/LayoutFile";
import type { ProjectLayout } from "../backend/ProjectLayout";
import type { Settings } from "../backend/Settings";
import type { TabLayout } from "../backend/TabLayout";
import { errorText } from "../errors";
import { focusNeighbor, layoutRects, type FocusDirection } from "../layout/geometry";
import { reconcileAll } from "../layout/reconcile";
import * as tabs from "../layout/tabs";
import { isFull, paneIds, type LayoutNode, type LayoutPreset, type SplitDirection } from "../layout/tree";
import type { ControlMsg } from "../protocol/ControlMsg";
import type { SessionInfo } from "../protocol/SessionInfo";
import type { SessionKind } from "../protocol/SessionKind";
import { router } from "../terminal/router";
import { sendControl, subscribe } from "../wire/bridge";
import { newUlid } from "../wire/ulid";
import { askConfirm } from "./dialog.svelte";
import { applyControl, clearConnectionErrors, emptyModel, type Model } from "./reducer";
import {
  busyStateText,
  canRestart,
  closeTabMessage,
  closeTabNeedsConfirmation,
  closeTabSummary,
  isBusy,
  restartShell,
} from "./sessionRules";
import { sortedProjects } from "./views";
import {
  createFailed,
  emptyWorkspace,
  projectOf,
  requestCreate,
  sessionCreated,
  sessionRemoved,
  withProject,
  type Placement,
  type Workspace,
} from "./workspace";

export const app = $state({
  model: emptyModel() as Model,
  status: { state: "starting" } as ConnectionStatus,
  settings: null as Settings | null,
  selectedProjectId: null as string | null,
  workspace: emptyWorkspace() as Workspace,
  /** True once layout.json has been read; nothing is written before that. */
  layoutLoaded: false,
  /** True once the first Sessions list has been applied; reconciliation waits for it. */
  sessionsLoaded: false,
  now: Date.now(),
});

/** Delay from the last layout change to the layout.json write. */
const SAVE_DELAY_MS = 250;
let saveTimer: ReturnType<typeof setTimeout> | undefined;
let saveChain: Promise<unknown> = Promise.resolve();

function scheduleSave(): void {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    const layout = $state.snapshot(app.workspace.layout) as LayoutFile;
    // Chained so two writes never overlap. A failed write is not retried
    // by itself; the next layout change writes the whole file again.
    saveChain = saveChain.then(() => invoke("save_layout", { layout })).catch(() => {});
  }, SAVE_DELAY_MS);
}

/** Replaces the workspace and schedules a layout.json write when the layout changed. */
function commit(next: Workspace): void {
  const layoutChanged = next.layout !== app.workspace.layout;
  app.workspace = next;
  if (layoutChanged && app.layoutLoaded) scheduleSave();
}

export function startApp(): () => void {
  void invoke<Settings>("get_settings")
    .then((s) => {
      app.settings = s;
    })
    .catch(() => {
      // Settings stay at their built-in defaults; nothing in the UI depends
      // on this having succeeded.
    });
  // layout.json is read before subscribing, so no SessionCreated can be
  // placed into an empty layout that the file then overwrites.
  void loadLayout().then(connect);
  const tick = setInterval(() => (app.now = Date.now()), 30_000);
  return () => clearInterval(tick);
}

async function loadLayout(): Promise<void> {
  try {
    const layout = await invoke<LayoutFile>("get_layout");
    app.workspace = { ...app.workspace, layout };
  } catch {
    // get_layout returns an empty layout for a missing or unreadable file
    // itself; only a failed IPC call lands here, and the empty layout from
    // emptyWorkspace() stays in place. Reconciliation then opens one tab
    // per session.
  }
  app.layoutLoaded = true;
}

function connect(): void {
  void subscribe({
    onStatus(status) {
      app.status = status;
      if (status.state === "connected") {
        app.model = clearConnectionErrors(app.model);
        void sendControl({ type: "ListProjects" }).catch(() => {});
        void sendControl({ type: "ListSessions" }).catch(() => {});
      }
    },
    onFrame(frame) {
      if (frame.kind === "control") handleControl(frame.msg);
      else router.deliver(frame);
    },
  }).catch((err: unknown) => {
    // The bridge itself failed to register (not merely "disconnected", which
    // arrives as a status update instead): there is no connection to retry
    // from in-app, so show it the same way a failed daemon launch would be.
    app.status = { state: "failed", message: errorText(err) };
  });
}

function reconcile(): void {
  if (!app.layoutLoaded || !app.sessionsLoaded) return;
  commit({ ...app.workspace, layout: reconcileAll(app.workspace.layout, app.model, newUlid) });
}

function handleControl(msg: ControlMsg): void {
  if (msg.type === "Error" && msg.request === "Output") {
    router.fellBehind(msg.session_id);
    return;
  }
  if (msg.type === "Error" && msg.session_id) {
    if (msg.request === "Attach") router.attachFailed(msg.session_id);
    if (msg.request === "CreateSession") commit(createFailed(app.workspace, msg.session_id));
  }

  app.model = applyControl(app.model, msg);

  switch (msg.type) {
    case "Sessions":
      app.sessionsLoaded = true;
      reconcile();
      break;
    case "Projects":
    case "ProjectRemoved":
      reconcile();
      break;
    case "SessionCreated": {
      const result = sessionCreated(app.workspace, msg.session, newUlid);
      commit(result.workspace);
      if (result.requested) app.selectedProjectId = msg.session.project_id;
      if (result.closeSessionId) sendClose(result.closeSessionId);
      break;
    }
    case "SessionRemoved":
      commit(sessionRemoved(app.workspace, msg.session_id));
      break;
  }

  if (!app.selectedProjectId || !app.model.projects[app.selectedProjectId]) {
    app.selectedProjectId = sortedProjects(app.model)[0]?.id ?? null;
  }

  // Sessions carries every session's current attachable value; reattach only
  // once it has been applied, so a reconnect does not race an Attach against
  // stale attachable state from before the daemon restarted.
  if (msg.type === "Sessions") router.reattachAll();
}

export function projectLayout(projectId: string): ProjectLayout {
  return projectOf(app.workspace.layout, projectId);
}

export function currentProjectLayout(): ProjectLayout | null {
  return app.selectedProjectId ? projectLayout(app.selectedProjectId) : null;
}

export function currentTab(): TabLayout | null {
  const project = currentProjectLayout();
  return project ? tabs.activeTab(project) : null;
}

export function focusedSessionId(): string | null {
  return currentTab()?.focused ?? null;
}

/** Applies a tab operation to one project; false when the operation was refused (returned null). */
function updateProject(projectId: string, update: (project: ProjectLayout) => ProjectLayout | null): boolean {
  const layout = withProject(app.workspace.layout, projectId, update);
  if (!layout) return false;
  if (layout !== app.workspace.layout) commit({ ...app.workspace, layout });
  return true;
}

function updateCurrentProject(update: (project: ProjectLayout) => ProjectLayout | null): boolean {
  return app.selectedProjectId ? updateProject(app.selectedProjectId, update) : false;
}

function projectIdOf(sessionId: string): string | null {
  return app.model.sessions[sessionId]?.project_id ?? null;
}

function updateSessionProject(sessionId: string, update: (project: ProjectLayout) => ProjectLayout | null): boolean {
  const projectId = projectIdOf(sessionId);
  return projectId ? updateProject(projectId, update) : false;
}

export function selectProject(id: string): void {
  app.selectedProjectId = id;
}

export function selectTab(tabId: string): void {
  updateCurrentProject((p) => tabs.selectTab(p, tabId));
}

export function cycleTab(delta: 1 | -1): void {
  updateCurrentProject((p) => tabs.cycleTab(p, delta));
}

/** Spec 9.3 rule 5: selects the session's project, activates its tab and focuses its pane. */
export function focusPane(sessionId: string): void {
  const projectId = projectIdOf(sessionId);
  if (!projectId) return;
  app.selectedProjectId = projectId;
  updateProject(projectId, (p) => tabs.focusSession(p, sessionId));
}

export function focusDirection(direction: FocusDirection): void {
  const tab = currentTab();
  if (!tab) return;
  const target = focusNeighbor(layoutRects(tab.root, tab.zoomed).panes, tab.focused, direction);
  if (target) focusPane(target);
}

/**
 * Sends CreateSession with a client-chosen id and records where the session
 * goes when its SessionCreated arrives. `shell` overrides the program of a
 * shell session (Restart passes the old session's program); when omitted, a
 * shell uses the defaultShell setting.
 */
export function newSession(projectId: string, kind: SessionKind, placement: Placement = { kind: "tab" }, shell?: string | null): void {
  const id = newUlid();
  commit(requestCreate(app.workspace, id, { projectId, placement }));
  const program = shell !== undefined ? shell : kind === "shell" ? (app.settings?.defaultShell ?? null) : null;
  void sendControl({ type: "CreateSession", session_id: id, project_id: projectId, kind, cols: 120, rows: 30, shell: program }).catch(() => {
    // Refused (for example, not connected): the reconnecting banner already
    // explains why. Stop waiting for a SessionCreated that is never coming.
    commit(createFailed(app.workspace, id));
  });
}

export function splitSession(sessionId: string, direction: SplitDirection, kind: SessionKind): void {
  const projectId = projectIdOf(sessionId);
  if (!projectId) return;
  const tab = tabs.tabOf(projectLayout(projectId), sessionId);
  if (!tab || isFull(tab.root)) return;
  newSession(projectId, kind, { kind: "split", tabId: tab.id, besideId: sessionId, direction });
}

export function splitFocused(direction: SplitDirection, kind: SessionKind): void {
  const id = focusedSessionId();
  if (id) splitSession(id, direction, kind);
}

function sendClose(sessionId: string): void {
  void sendControl({ type: "CloseSession", session_id: sessionId }).catch(() => {});
}

/** Spec 9.3 rule 6: closing a pane ends its session, after confirmation for a busy session. */
export async function closePane(sessionId: string): Promise<void> {
  const session = app.model.sessions[sessionId];
  if (!session) return;
  if (isBusy(session)) {
    const confirmed = await askConfirm({
      title: "Close pane?",
      message: `${session.label} is ${busyStateText(session.state)}. Closing the pane ends the session.`,
      details: [],
      confirmLabel: "Close pane",
    });
    if (!confirmed) return;
  }
  sendClose(sessionId);
}

/** Spec 9.4, Close tab: lists the sessions it ends and how many are busy before confirming. */
export async function closeTab(tabId: string): Promise<void> {
  const tab = currentProjectLayout()?.tabs.find((t) => t.id === tabId);
  if (!tab) return;
  const sessions = paneIds(tab.root)
    .map((id) => app.model.sessions[id])
    .filter((s): s is SessionInfo => s !== undefined);
  const summary = closeTabSummary(sessions);
  if (closeTabNeedsConfirmation(summary)) {
    const confirmed = await askConfirm({ title: "Close tab?", message: closeTabMessage(summary), details: summary.labels, confirmLabel: "Close tab" });
    if (!confirmed) return;
  }
  // Each SessionRemoved removes one pane; the last one removes the tab.
  for (const session of sessions) sendClose(session.id);
}

export function renameTab(tabId: string, name: string): void {
  updateCurrentProject((p) => {
    const tab = p.tabs.find((t) => t.id === tabId);
    if (!tab) return p;
    // Committing the label the tab already shows keeps it following the
    // focused session instead of pinning that text as a name.
    if (tab.name === null && name.trim() === tabs.tabLabel(tab, (id) => app.model.sessions[id]?.label)) return p;
    return tabs.renameTab(p, tabId, name);
  });
}

/** RenameSession is handled by jojod, which broadcasts SessionUpdated with the new label. */
export function renameSession(sessionId: string, label: string): void {
  const session = app.model.sessions[sessionId];
  const trimmed = label.trim();
  if (!session || trimmed === "" || trimmed === session.label) return;
  void sendControl({ type: "RenameSession", session_id: sessionId, label: trimmed }).catch(() => {});
}

/** Kill ends the process; jojod keeps the row as exited so the final output stays readable. */
export function killSession(sessionId: string): void {
  void sendControl({ type: "Kill", session_id: sessionId }).catch(() => {});
}

/**
 * Spec 5.4 Restart: a new session with the same project, kind and program
 * takes the old pane's place; the old session is closed when the new one's
 * SessionCreated arrives (workspace.sessionCreated reports it).
 */
export function restartSession(sessionId: string): void {
  const session = app.model.sessions[sessionId];
  if (!session || !canRestart(session)) return;
  newSession(session.project_id, session.kind, { kind: "replace", oldId: sessionId }, restartShell(session));
}

export function zoomSession(sessionId: string): void {
  updateSessionProject(sessionId, (p) => tabs.toggleZoom(p, sessionId));
}

export function swapSessions(a: string, b: string): void {
  updateSessionProject(a, (p) => tabs.swapInTab(p, a, b));
}

export function moveSessionToNewTab(sessionId: string): void {
  updateSessionProject(sessionId, (p) => tabs.moveToNewTab(p, sessionId, newUlid()));
}

export function moveSessionToTab(sessionId: string, tabId: string): void {
  updateSessionProject(sessionId, (p) => tabs.moveToTab(p, sessionId, tabId));
}

/** Sidebar menu, Open in split: moves the session into the current tab beside its focused pane. */
export function openSessionInSplit(sessionId: string, direction: SplitDirection): void {
  const tab = currentTab();
  if (!tab || projectIdOf(sessionId) !== app.selectedProjectId) return;
  updateSessionProject(sessionId, (p) => tabs.moveBeside(p, sessionId, tab.id, tab.focused, direction));
}

export function applyTabPreset(tabId: string, preset: LayoutPreset): void {
  updateCurrentProject((p) => tabs.presetTab(p, tabId, preset));
}

export function equalizeTabSizes(tabId: string): void {
  updateCurrentProject((p) => tabs.equalizeTab(p, tabId));
}

export function setTabRoot(tabId: string, root: LayoutNode): void {
  updateCurrentProject((p) => tabs.setTabRoot(p, tabId, root));
}

export async function addProject(): Promise<void> {
  const path = await open({ directory: true, multiple: false });
  if (typeof path !== "string") return;
  const name = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  void sendControl({ type: "AddProject", path, name }).catch(() => {});
}

export function dismissError(): void {
  app.model = { ...app.model, lastError: null };
}

export function restartDaemon(): void {
  void invoke("restart_daemon").catch(() => {});
}
```

- [ ] **Step 5: Write the badge and the confirmation dialog**

`app/src/ui/Badge.svelte`:
```svelte
<script lang="ts">
  import type { AgentState } from "../lib/protocol/AgentState";

  let { agentState, size = 13 }: { agentState: AgentState; size?: number } = $props();
</script>

<span class="badge {agentState}" style:width="{size}px" style:height="{size}px"></span>

<style>
  .badge {
    display: inline-block;
    flex: none;
    border-radius: 50%;
    border: 2px solid var(--jojo-idle);
  }
  .badge.working {
    border-color: var(--jojo-working);
    border-top-color: transparent;
    animation: spin 1s linear infinite;
  }
  .badge.needs_you {
    border-color: var(--jojo-needs-you);
    background: var(--jojo-needs-you-soft);
    box-shadow: 0 0 10px var(--jojo-needs-you-glow);
  }
  .badge.done {
    border-color: var(--jojo-done);
    background: var(--jojo-done);
  }
  .badge.error {
    border-color: var(--jojo-error);
    background: var(--jojo-error);
  }
  .badge.exited {
    opacity: 0.6;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .badge.working {
      animation: none;
    }
  }
</style>
```

`app/src/ui/ConfirmDialog.svelte`:
```svelte
<script lang="ts">
  import { answerConfirm, dialog } from "../lib/state/dialog.svelte";

  let confirmButton = $state<HTMLButtonElement | undefined>();

  // The confirm button takes focus when the dialog opens, so Enter confirms
  // and Escape (handled below) cancels without touching the mouse.
  $effect(() => {
    if (dialog.confirm) confirmButton?.focus();
  });

  function onkeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      answerConfirm(false);
    }
  }
</script>

{#if dialog.confirm}
  <!-- Clicking the scrim outside the dialog cancels; the dialog itself is keyboard operable. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="scrim" onpointerdown={(e) => e.target === e.currentTarget && answerConfirm(false)}>
    <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title" tabindex="-1" {onkeydown}>
      <h2 id="confirm-title">{dialog.confirm.title}</h2>
      <p>{dialog.confirm.message}</p>
      {#if dialog.confirm.details.length > 0}
        <ul>
          {#each dialog.confirm.details as line}
            <li>{line}</li>
          {/each}
        </ul>
      {/if}
      <div class="actions">
        <button onclick={() => answerConfirm(false)}>Cancel</button>
        <button class="primary" bind:this={confirmButton} onclick={() => answerConfirm(true)}>{dialog.confirm.confirmLabel}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: grid;
    place-items: center;
    background: var(--jojo-scrim);
  }
  .dialog {
    width: min(420px, calc(100% - 32px));
    padding: 18px 20px 16px;
    background: var(--jojo-menu);
    border: 1px solid var(--jojo-line);
    border-radius: var(--jojo-radius);
    box-shadow: var(--jojo-shadow);
    outline: none;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 14px;
    font-weight: 600;
    color: var(--jojo-fg);
  }
  p {
    margin: 0 0 8px;
    color: var(--jojo-fg2);
  }
  ul {
    margin: 0 0 8px;
    padding-left: 18px;
    color: var(--jojo-fg2);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
  }
  .actions button {
    padding: 5px 12px;
    border: 1px solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    color: var(--jojo-fg2);
  }
  .actions .primary {
    border-color: var(--jojo-danger);
    background: var(--jojo-danger);
    color: var(--jojo-fg);
  }
</style>
```

- [ ] **Step 6: Rewrite the terminal pane**

Replace `app/src/ui/TerminalPane.svelte`:
```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { Terminal, type ITheme } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { Unicode11Addon } from "@xterm/addon-unicode11";
  import { WebLinksAddon } from "@xterm/addon-web-links";
  import { WebglAddon } from "@xterm/addon-webgl";
  import type { SessionInfo } from "../lib/protocol/SessionInfo";
  import { app } from "../lib/state/app.svelte";
  import { noScreenMessage } from "../lib/terminal/pane";
  import { createPaneStream } from "../lib/terminal/paneStream";
  import { suppressQueryReplies } from "../lib/terminal/queries";
  import { registerTerminal } from "../lib/terminal/registry";
  import { router } from "../lib/terminal/router";
  import { clampSize } from "../lib/terminal/size";
  import { sendControl, sendInput } from "../lib/wire/bridge";

  let { session, terminalTheme, focused }: { session: SessionInfo; terminalTheme: ITheme; focused: boolean } = $props();
  let host: HTMLDivElement;
  /** Set in onMount and cleared on unmount; read by the focus effect below. */
  let term: Terminal | undefined;

  onMount(() => {
    const settings = app.settings;
    const id = session.id;
    const created = new Terminal({
      allowProposedApi: true,
      fontFamily: settings?.terminalFontFamily ?? "Geist Mono",
      fontSize: settings?.terminalFontSize ?? 13,
      lineHeight: settings?.terminalLineHeight ?? 1.25,
      scrollback: Math.max(settings?.scrollbackLines ?? 10_000, 10_000),
      theme: terminalTheme,
      cursorBlink: true,
    });
    term = created;
    const fit = new FitAddon();
    created.loadAddon(fit);
    created.loadAddon(new Unicode11Addon());
    created.unicode.activeVersion = "11";
    // The addon's default handler calls window.open, which the main webview
    // denies (see main.rs). The open_url command validates the URL (http and
    // https only) and opens it in the default browser instead.
    created.loadAddon(new WebLinksAddon((_event, uri) => void invoke("open_url", { url: uri }).catch(() => {})));
    suppressQueryReplies(created);
    created.open(host);
    let webgl: WebglAddon | undefined;
    try {
      webgl = new WebglAddon();
      created.loadAddon(webgl);
      // xterm.js falls back to its DOM renderer on context loss; the addon
      // itself does not dispose on loss, so do it here to release the GL
      // resources instead of leaving a dead context attached.
      webgl.onContextLoss(() => {
        webgl?.dispose();
        webgl = undefined;
      });
    } catch {
      // WebGL unavailable: xterm.js keeps its DOM renderer.
      webgl = undefined;
    }
    fit.fit();

    let size = clampSize(created.cols, created.rows);
    const stream = createPaneStream({
      write: (bytes, done) => created.write(bytes, done),
      sendAttach: () => sendControl({ type: "Attach", session_id: id, cols: size.cols, rows: size.rows }),
    });
    // A session with no screen is never attached, so it is not registered
    // with the router: reattachAll() after a reconnect must not send an
    // Attach that jojod can only refuse.
    const unregister = session.attachable ? router.register(id, stream) : () => {};
    if (session.attachable) stream.start();
    else created.write(`\x1b[2m${noScreenMessage(session)}\x1b[0m\r\n`);

    const unregisterControls = registerTerminal(id, {
      focus: () => created.focus(),
      hasSelection: () => created.hasSelection(),
      getSelection: () => created.getSelection(),
      paste: (text) => created.paste(text),
      mouseTrackingMode: () => created.modes.mouseTrackingMode,
    });

    // A session with no screen has nothing on the daemon side to receive
    // input, so keystrokes are not sent. PaneGrid keys this pane on
    // paneKey(), which includes `attachable`, so a session that loses its
    // screen while mounted is remounted and reaches this check.
    const encoder = new TextEncoder();
    const onData = created.onData((data) => {
      if (!session.attachable) return;
      void sendInput(id, encoder.encode(data)).catch(() => {});
    });
    const onBinary = created.onBinary((data) => {
      if (!session.attachable) return;
      void sendInput(id, Uint8Array.from(data, (c) => c.charCodeAt(0) & 0xff)).catch(() => {});
    });

    let timer: ReturnType<typeof setTimeout> | undefined;
    const observer = new ResizeObserver(() => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        // Always re-fit so a session with no screen still wraps its static
        // message to the host's width; only an attachable session has
        // anything on the daemon side to tell about the new size.
        fit.fit();
        const next = clampSize(created.cols, created.rows);
        if (next.cols !== size.cols || next.rows !== size.rows) {
          size = next;
          if (session.attachable) {
            void sendControl({ type: "Resize", session_id: id, cols: size.cols, rows: size.rows }).catch(() => {});
          }
        }
      }, 50);
    });
    observer.observe(host);

    return () => {
      observer.disconnect();
      clearTimeout(timer);
      unregister();
      unregisterControls();
      onData.dispose();
      onBinary.dispose();
      if (session.attachable) void sendControl({ type: "Detach", session_id: id }).catch(() => {});
      webgl?.dispose();
      term = undefined;
      created.dispose();
    };
  });

  // Moves keyboard focus into this terminal whenever its pane becomes the
  // focused pane: a click on the pane, Alt+Arrow, a sidebar click, or a tab
  // switch. Declared after onMount, so on first mount it runs after `term`
  // is set.
  $effect(() => {
    if (focused) term?.focus();
  });
</script>

<div class="host" bind:this={host}></div>

<style>
  .host {
    width: 100%;
    height: 100%;
  }
</style>
```

- [ ] **Step 7: Write the pane view and the pane grid**

`app/src/ui/PaneView.svelte`:
```svelte
<script lang="ts">
  import type { ITheme } from "@xterm/xterm";
  import type { SessionInfo } from "../lib/protocol/SessionInfo";
  import { focusPane } from "../lib/state/app.svelte";
  import Badge from "./Badge.svelte";
  import TerminalPane from "./TerminalPane.svelte";

  let { session, terminalTheme, focused }: { session: SessionInfo; terminalTheme: ITheme; focused: boolean } = $props();
</script>

<!-- Pointer focus only; keyboard users move focus between panes with Alt+Arrow (spec 9.6). -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="pane" class:focused onpointerdowncapture={() => focusPane(session.id)}>
  <header class="strip">
    <Badge agentState={session.state} size={10} />
    <span class="label">{session.label}</span>
  </header>
  <div class="body">
    <TerminalPane {session} {terminalTheme} {focused} />
  </div>
</div>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    background: var(--jojo-pane);
    border: 1px solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    overflow: hidden;
  }
  /* Spec 9.3 rule 4: the focused pane has a 1 px accent outline. */
  .pane.focused {
    border-color: var(--jojo-accent);
  }
  .strip {
    display: flex;
    align-items: center;
    gap: 7px;
    flex: none;
    height: 24px;
    padding: 0 10px;
    font-size: 11.5px;
    color: var(--jojo-fg2);
    background: var(--jojo-bar);
    border-bottom: 1px solid var(--jojo-line);
  }
  .label {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .body {
    position: relative;
    flex: 1;
    min-height: 0;
    padding: 6px 8px;
  }
</style>
```

`app/src/ui/PaneGrid.svelte`:
```svelte
<script lang="ts">
  import type { ITheme } from "@xterm/xterm";
  import { layoutRects, type DividerRect } from "../lib/layout/geometry";
  import { resizeSplit } from "../lib/layout/tree";
  import { app, currentTab, setTabRoot } from "../lib/state/app.svelte";
  import { paneKey } from "../lib/terminal/pane";
  import PaneView from "./PaneView.svelte";

  let { terminalTheme }: { terminalTheme: ITheme } = $props();
  let area: HTMLDivElement;
  const tab = $derived(currentTab());
  const rects = $derived(tab ? layoutRects(tab.root, tab.zoomed) : { panes: [], dividers: [] });
  const percent = (fraction: number) => `${fraction * 100}%`;

  /**
   * Drags a divider (spec 9.3 rule 3). Every move recomputes the sizes from
   * the tree as it was when the drag started plus the total pointer
   * movement, so clamping at MIN_SHARE never lets the divider drift away
   * from the pointer.
   */
  function startDrag(event: PointerEvent, divider: DividerRect): void {
    if (!tab || event.button !== 0) return;
    event.preventDefault();
    const tabId = tab.id;
    const base = $state.snapshot(tab.root);
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    const box = area.getBoundingClientRect();
    const extentPx = (divider.direction === "row" ? box.width : box.height) * divider.extent;
    const start = divider.direction === "row" ? event.clientX : event.clientY;
    const move = (e: PointerEvent) => {
      if (extentPx <= 0) return;
      const now = divider.direction === "row" ? e.clientX : e.clientY;
      setTabRoot(tabId, resizeSplit(base, divider.path, divider.index, (now - start) / extentPx));
    };
    const end = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", end);
      handle.removeEventListener("pointercancel", end);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);
  }
</script>

<div class="area" bind:this={area}>
  {#each rects.panes as rect (rect.sessionId)}
    {@const session = app.model.sessions[rect.sessionId]}
    {#if session}
      <div class="slot" style:left={percent(rect.x)} style:top={percent(rect.y)} style:width={percent(rect.w)} style:height={percent(rect.h)}>
        {#key paneKey(session)}
          <PaneView {session} {terminalTheme} focused={tab?.focused === session.id} />
        {/key}
      </div>
    {/if}
  {/each}
  {#each rects.dividers as divider (`${divider.path.join(".")}/${divider.index}`)}
    <!-- Dividers are pointer-only; Layout ▸ presets and Equalize sizes in the pane menu cover the keyboard. -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="divider {divider.direction}"
      role="separator"
      aria-orientation={divider.direction === "row" ? "vertical" : "horizontal"}
      style:left={percent(divider.x)}
      style:top={percent(divider.y)}
      style:width={divider.direction === "row" ? null : percent(divider.length)}
      style:height={divider.direction === "row" ? percent(divider.length) : null}
      onpointerdown={(e) => startDrag(e, divider)}
    ></div>
  {/each}
</div>

<style>
  .area {
    position: absolute;
    inset: 0;
    padding: 3px;
  }
  .slot {
    position: absolute;
    padding: 3px;
    min-width: 0;
    min-height: 0;
  }
  .divider {
    position: absolute;
    z-index: 1;
  }
  .divider.row {
    width: 6px;
    margin-left: -3px;
    cursor: col-resize;
  }
  .divider.column {
    height: 6px;
    margin-top: -3px;
    cursor: row-resize;
  }
  .divider:hover {
    background: var(--jojo-line);
  }
</style>
```
(Percentages on absolutely positioned children resolve against the padding box of `.area`, so the 3 px `.area` padding plus the 3 px `.slot` padding gives 6 px gaps between panes and 6 px at the edges; the 6 px divider hit areas sit centered in the gaps.)

- [ ] **Step 8: Rewrite the tab bar**

Replace `app/src/ui/TabBar.svelte`:
```svelte
<script lang="ts">
  import { activeTab, tabLabel } from "../lib/layout/tabs";
  import { countPanes } from "../lib/layout/tree";
  import { app, closeTab, currentProjectLayout, newSession, selectTab } from "../lib/state/app.svelte";
  import Badge from "./Badge.svelte";
  import WindowControls from "./WindowControls.svelte";

  const project = $derived(currentProjectLayout());
  const activeId = $derived(project ? activeTab(project)?.id : undefined);
  const labelOf = (sessionId: string) => app.model.sessions[sessionId]?.label;
</script>

<div class="tabs">
  {#each project?.tabs ?? [] as tab (tab.id)}
    {@const focusedSession = app.model.sessions[tab.focused]}
    {@const panes = countPanes(tab.root)}
    <div class="tab" class:on={activeId === tab.id}>
      <button class="title" onclick={() => selectTab(tab.id)}>
        {#if focusedSession}<Badge agentState={focusedSession.state} size={11} />{/if}
        <span class="name">{tabLabel(tab, labelOf)}</span>
        {#if panes > 1}<span class="count">{panes}</span>{/if}
      </button>
      <button class="close" aria-label="Close tab {tabLabel(tab, labelOf)}" onclick={() => void closeTab(tab.id)}>×</button>
    </div>
  {/each}
  {#if app.selectedProjectId}
    <button class="new" title="New Claude session" aria-label="New Claude session" onclick={() => newSession(app.selectedProjectId!, "claude")}>+</button>
  {/if}
  <div class="drag" data-tauri-drag-region></div>
  <WindowControls />
</div>

<style>
  .tabs {
    display: flex;
    align-items: flex-end;
    gap: 4px;
    height: 38px;
    padding: 6px 0 0 8px;
    background: var(--jojo-bar);
    border-bottom: 1px solid var(--jojo-line);
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 6px 0 12px;
    height: 31px;
    border-radius: var(--jojo-radius) var(--jojo-radius) 0 0;
    color: var(--jojo-muted);
  }
  .tab.on {
    background: var(--jojo-pane);
    color: var(--jojo-fg);
    border: 1px solid var(--jojo-line);
    border-bottom-color: transparent;
    margin-bottom: -1px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 7px;
    max-width: 220px;
    white-space: nowrap;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .count {
    padding: 0 5px;
    border-radius: var(--jojo-radius-sm);
    font-size: 10.5px;
    color: var(--jojo-muted);
    background: var(--jojo-sel);
  }
  .close {
    padding: 0 4px;
    color: var(--jojo-muted);
  }
  .close:hover {
    color: var(--jojo-fg);
  }
  .new {
    height: 31px;
    padding: 0 10px;
    color: var(--jojo-muted);
  }
  .drag {
    flex: 1;
    align-self: stretch;
  }
</style>
```

- [ ] **Step 9: Point the sidebar at the layout**

In `app/src/ui/Sidebar.svelte`:
1. Replace the first import line with:
   ```ts
   import { addProject, app, focusPane, focusedSessionId, newSession, selectProject } from "../lib/state/app.svelte";
   ```
   and add `import Badge from "./Badge.svelte";` after the `views` import.
2. Replace the session button:
   ```svelte
   <button class="session" class:active={focusedSessionId() === session.id} onclick={() => focusPane(session.id)}>
     <Badge agentState={session.state} />
     <span class="label">{session.label}</span>
     <span class="time">{formatElapsed(app.now - session.state_changed_at)}</span>
   </button>
   ```
3. Delete the `.badge`, `.badge.working`, `.badge.needs_you`, `.badge.done`, `.badge.error` and `.badge.exited` rules, the `@keyframes spin` block and the `@media (prefers-reduced-motion: reduce)` block from its `<style>` (they now live in `Badge.svelte`).

- [ ] **Step 10: Rewrite the root component**

Replace `app/src/App.svelte`:
```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import neonGlass from "./themes/neon-glass.json";
  import { app, currentTab, startApp } from "./lib/state/app.svelte";
  import { applyTheme, type Theme } from "./lib/theme";
  import Banner from "./ui/Banner.svelte";
  import ConfirmDialog from "./ui/ConfirmDialog.svelte";
  import PaneGrid from "./ui/PaneGrid.svelte";
  import Sidebar from "./ui/Sidebar.svelte";
  import StatusBar from "./ui/StatusBar.svelte";
  import TabBar from "./ui/TabBar.svelte";

  const theme = neonGlass as Theme;
  onMount(() => {
    applyTheme(theme);
    return startApp();
  });
  const tab = $derived(currentTab());
</script>

<div class="window">
  <Sidebar />
  <section class="main">
    <TabBar />
    <Banner />
    <div class="panes">
      {#if tab}
        <PaneGrid terminalTheme={theme.terminal} />
      {:else if app.selectedProjectId}
        <p class="empty">No session open. Use + to start Claude or a shell.</p>
      {:else}
        <p class="empty">Add a project folder with + next to This PC.</p>
      {/if}
    </div>
    <StatusBar />
  </section>
</div>
<ConfirmDialog />

<style>
  .window {
    display: grid;
    grid-template-columns: 250px 1fr;
    height: 100%;
    background: var(--jojo-bg-glow);
  }
  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .panes {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  .empty {
    margin: 40px auto;
    text-align: center;
    color: var(--jojo-muted);
  }
</style>
```

- [ ] **Step 11: Type-check, lint and test**

Run: `pnpm --dir app check`, `pnpm --dir app lint:colors`, `pnpm --dir app test`
Expected: 0 errors from svelte-check (warnings for intentionally ignored a11y rules are suppressed by the `svelte-ignore` comments); `No color literals in components.`; all tests pass.

- [ ] **Step 12: Run the app and check the layout by hand**

From the Bash tool or a desktop terminal (not Claude Code's PowerShell tool), with a staged daemon (`pnpm --dir app stage`), run `pnpm --dir app tauri dev`. Check:
1. Existing sessions each appear in their own tab (reconciliation of an empty `layout.json`); `%APPDATA%\Jojo\layout.json` exists within a second and lists them under `"local:<project id>"`.
2. Clicking a sidebar session activates its tab and draws the accent outline on its pane; the terminal accepts typing.
3. Close the window. Edit `%APPDATA%\Jojo\layout.json`: in one project, move the second tab's `"root"` pane into the first tab by replacing the first tab's `"root"` with `{"type":"split","direction":"row","children":[<first tab's root>,<second tab's root>],"sizes":[0.5,0.5]}` and deleting the second tab. Start the app again: the first tab shows both panes side by side, both terminals show their screens, and dragging the divider between them resizes both (`$Host.UI.RawUI.WindowSize` in a pwsh pane changes).
4. Typing `exit` in one of the two panes and closing it with × on the tab (confirm dialog lists both sessions) closes both sessions.
If the agent cannot see the window, record which checks were observed and leave the rest for the user.

- [ ] **Step 13: Commit**

```bash
git add app/src
git commit -m "feat(app): layout-driven tabs and split panes with persisted layout"
```

---

### Task 10: Context menus, inline rename, Kill and Restart

**Files:**
- Create: `app/src/lib/state/ui.svelte.ts`, `app/src/lib/state/notices.svelte.ts`, `app/src/lib/state/clipboard.ts`, `app/src/lib/state/contextMenus.ts`, `app/src/ui/ContextMenu.svelte`, `app/src/ui/RenameField.svelte`
- Rewrite: `app/src/ui/PaneView.svelte`, `app/src/ui/TabBar.svelte`, `app/src/ui/Sidebar.svelte`
- Modify: `app/src/ui/TerminalPane.svelte`, `app/src/App.svelte`, `app/src-tauri/src/main.rs`

**Interfaces:**
- Consumes: Task 7 (`contextMenuTarget`), Task 8 (menu builders, navigation), Task 9 (app actions, registry, Badge, dialog).
- Produces (used by Tasks 11 and 12):
  - `ui.svelte.ts`: `type RenameTarget = { kind: "tab"; tabId: string } | { kind: "session"; sessionId: string; where: "pane" | "sidebar" }`; `ui.{ menu: { x: number; y: number; entries: MenuEntry[] } | null; renaming: RenameTarget | null }`; `openMenu(x, y, entries)`; `closeMenu()`; `beginRename(target)`; `endRename()`.
  - `notices.svelte.ts`: `COPIED_MS = 1200`; `ERROR_MS = 4000`; `interface Notice { text: string; tone: "info" | "error"; serial: number }`; `notices: Record<string, Notice>`; `showNotice(sessionId, text, tone?: "info" | "error")`.
  - `clipboard.ts`: `copyFromPane(sessionId): Promise<void>`; `pasteIntoPane(sessionId): Promise<void>`.
  - `contextMenus.ts`: `paneMenu(sessionId): MenuEntry[]`; `tabMenu(tabId): MenuEntry[]`; `sessionMenu(sessionId): MenuEntry[]`.
  - `ContextMenu.svelte` props `{ entries; x?; y?; row?: Box | null; autofocus?: boolean; onclose; onback? }`; `RenameField.svelte` props `{ value: string; label: string; onCommit(value: string): void; onCancel(): void }`.
  - `TerminalPane.svelte` gains the prop `onmenu: (x: number, y: number) => void`.

Clipboard access uses the WebView2 clipboard API. `navigator.clipboard.writeText` works without a permission prompt. `navigator.clipboard.readText` triggers a WebView2 clipboard-read permission request, which wry 0.57 answers with "allow" only when the webview was built with clipboard access enabled (`wry::webview2` registers a `PermissionRequested` handler for `COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ` when `attributes.clipboard` is true). Tauri exposes that as `WebviewWindowBuilder::enable_clipboard_access()`, which this task adds in `main.rs`. No clipboard plugin (a new dependency) is needed.

- [ ] **Step 1: Write the UI state, notices and clipboard modules**

`app/src/lib/state/ui.svelte.ts`:
```ts
// Transient UI state: the open context menu and the inline rename field.
import type { MenuEntry } from "../menu/types";

export type RenameTarget = { kind: "tab"; tabId: string } | { kind: "session"; sessionId: string; where: "pane" | "sidebar" };

export const ui = $state({
  menu: null as { x: number; y: number; entries: MenuEntry[] } | null,
  renaming: null as RenameTarget | null,
});

export function openMenu(x: number, y: number, entries: MenuEntry[]): void {
  if (entries.length === 0) return;
  ui.menu = { x, y, entries };
}

export function closeMenu(): void {
  ui.menu = null;
}

export function beginRename(target: RenameTarget): void {
  ui.menu = null;
  ui.renaming = target;
}

export function endRename(): void {
  ui.renaming = null;
}
```

`app/src/lib/state/notices.svelte.ts`:
```ts
// Short messages in a pane's header strip: "Copied" (spec 9.5 rule 2) and
// errors such as a link that could not be opened.

/** Spec 9.5 rule 2: the header shows "Copied" for 1.2 seconds. */
export const COPIED_MS = 1200;
/** Errors stay longer so they can be read. */
export const ERROR_MS = 4000;

export interface Notice {
  text: string;
  tone: "info" | "error";
  /** Identifies this notice, so an older timer does not remove a newer notice. */
  serial: number;
}

export const notices: Record<string, Notice> = $state({});
let lastSerial = 0;

export function showNotice(sessionId: string, text: string, tone: "info" | "error" = "info"): void {
  const serial = ++lastSerial;
  notices[sessionId] = { text, tone, serial };
  setTimeout(
    () => {
      if (notices[sessionId]?.serial === serial) delete notices[sessionId];
    },
    tone === "info" ? COPIED_MS : ERROR_MS,
  );
}
```

`app/src/lib/state/clipboard.ts`:
```ts
// Copy and paste for one pane, through the WebView2 clipboard API (main.rs
// enables clipboard access on the webview so readText is allowed).
import { errorText } from "../errors";
import { terminalControls } from "../terminal/registry";
import { showNotice } from "./notices.svelte";

/**
 * Copies the pane's selection. xterm.js getSelection() joins wrapped lines
 * and trims trailing whitespace (spec 9.5 rule 1). Does nothing when
 * nothing is selected.
 */
export async function copyFromPane(sessionId: string): Promise<void> {
  const text = terminalControls(sessionId)?.getSelection() ?? "";
  if (text === "") return;
  try {
    await navigator.clipboard.writeText(text);
    showNotice(sessionId, "Copied");
  } catch (err) {
    showNotice(sessionId, `Copy failed: ${errorText(err)}`, "error");
  }
}

/** Pastes the clipboard text into the pane; xterm.js applies bracketed paste when the program enabled it. */
export async function pasteIntoPane(sessionId: string): Promise<void> {
  const controls = terminalControls(sessionId);
  if (!controls) return;
  try {
    const text = await navigator.clipboard.readText();
    if (text !== "") controls.paste(text);
  } catch (err) {
    showNotice(sessionId, `Paste failed: ${errorText(err)}`, "error");
  }
}
```

- [ ] **Step 2: Bind the menus to the app actions**

`app/src/lib/state/contextMenus.ts`:
```ts
// The pane, tab and sidebar session menus (spec 9.4), bound to app actions.
import { tabLabel, tabOf } from "../layout/tabs";
import { buildPaneMenu, buildSessionMenu, buildTabMenu } from "../menu/menus";
import type { MenuEntry } from "../menu/types";
import { terminalControls } from "../terminal/registry";
import {
  app,
  applyTabPreset,
  closePane,
  closeTab,
  currentProjectLayout,
  currentTab,
  equalizeTabSizes,
  killSession,
  moveSessionToNewTab,
  moveSessionToTab,
  openSessionInSplit,
  projectLayout,
  restartSession,
  splitSession,
  swapSessions,
  zoomSession,
} from "./app.svelte";
import { copyFromPane, pasteIntoPane } from "./clipboard";
import { beginRename } from "./ui.svelte";

const labelOf = (sessionId: string): string | undefined => app.model.sessions[sessionId]?.label;

export function paneMenu(sessionId: string): MenuEntry[] {
  const projectId = app.model.sessions[sessionId]?.project_id;
  if (!projectId) return [];
  const project = projectLayout(projectId);
  const tab = tabOf(project, sessionId);
  if (!tab) return [];
  return buildPaneMenu(
    {
      sessionId,
      hasSelection: terminalControls(sessionId)?.hasSelection() ?? false,
      tab,
      otherTabs: project.tabs.filter((t) => t.id !== tab.id),
      labelOf: (id) => labelOf(id) ?? id,
      tabLabelOf: (t) => tabLabel(t, labelOf),
    },
    {
      copy: () => void copyFromPane(sessionId),
      paste: () => void pasteIntoPane(sessionId),
      split: (direction, kind) => splitSession(sessionId, direction, kind),
      toggleZoom: () => zoomSession(sessionId),
      swapWith: (other) => swapSessions(sessionId, other),
      moveToNewTab: () => moveSessionToNewTab(sessionId),
      moveToTab: (tabId) => moveSessionToTab(sessionId, tabId),
      applyPreset: (preset) => applyTabPreset(tab.id, preset),
      equalize: () => equalizeTabSizes(tab.id),
      rename: () => beginRename({ kind: "session", sessionId, where: "pane" }),
      close: () => void closePane(sessionId),
    },
  );
}

export function tabMenu(tabId: string): MenuEntry[] {
  const tab = currentProjectLayout()?.tabs.find((t) => t.id === tabId);
  if (!tab) return [];
  return buildTabMenu(tab, {
    rename: () => beginRename({ kind: "tab", tabId }),
    applyPreset: (preset) => applyTabPreset(tabId, preset),
    close: () => void closeTab(tabId),
  });
}

export function sessionMenu(sessionId: string): MenuEntry[] {
  const session = app.model.sessions[sessionId];
  if (!session) return [];
  return buildSessionMenu(
    {
      session,
      currentTab: currentTab(),
      currentProjectId: app.selectedProjectId,
      sessionTab: tabOf(projectLayout(session.project_id), sessionId),
    },
    {
      openInSplit: (direction) => openSessionInSplit(sessionId, direction),
      moveToNewTab: () => moveSessionToNewTab(sessionId),
      rename: () => beginRename({ kind: "session", sessionId, where: "sidebar" }),
      restart: () => restartSession(sessionId),
      kill: () => killSession(sessionId),
    },
  );
}
```

- [ ] **Step 3: Write the context menu component**

`app/src/ui/ContextMenu.svelte`:
```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { firstEnabled, moveHighlight, placeMenu, placeSubmenu, type Box } from "../lib/menu/navigation";
  import { isEnabled, type MenuEntry } from "../lib/menu/types";
  import Self from "./ContextMenu.svelte";

  let {
    entries,
    x = 0,
    y = 0,
    row = null,
    autofocus = true,
    onclose,
    onback = null,
  }: {
    entries: MenuEntry[];
    /** Pointer position, for the root menu. */
    x?: number;
    y?: number;
    /** The parent row's rectangle, for a submenu; null for the root menu. */
    row?: Box | null;
    /** Take keyboard focus on mount: the root menu, and submenus opened from the keyboard. */
    autofocus?: boolean;
    /** Closes the whole menu. */
    onclose: () => void;
    /** Closes this submenu and returns keyboard focus to its parent. */
    onback?: (() => void) | null;
  } = $props();

  let element: HTMLDivElement;
  let highlighted = $state(-1);
  let open = $state<{ index: number; row: Box; viaKeyboard: boolean } | null>(null);
  let left = $state(0);
  let top = $state(0);
  let placed = $state(false);

  onMount(() => {
    const size = { w: element.offsetWidth, h: element.offsetHeight };
    const viewport = { w: window.innerWidth, h: window.innerHeight };
    const at = row ? placeSubmenu(row, size, viewport) : placeMenu({ x, y }, size, viewport);
    left = at.x;
    top = at.y;
    placed = true;
    if (autofocus) {
      element.focus();
      if (row) highlighted = firstEnabled(entries);
    }
    if (row) return;
    // Only the root menu closes the menu on a click outside every menu
    // level, on window blur and on resize.
    const close = () => onclose();
    const onPointerDown = (event: PointerEvent) => {
      if (!(event.target instanceof Element && event.target.closest("[data-context-menu]"))) close();
    };
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("blur", close);
    window.addEventListener("resize", close);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("blur", close);
      window.removeEventListener("resize", close);
    };
  });

  function itemsAt(index: number): MenuEntry[] {
    const entry = entries[index];
    return entry?.kind === "submenu" ? entry.items : [];
  }

  function openSubmenu(index: number, viaKeyboard: boolean): void {
    const entry = entries[index];
    if (entry?.kind !== "submenu" || !isEnabled(entry)) {
      open = null;
      return;
    }
    const rowElement = element.querySelector<HTMLElement>(`[data-index="${index}"]`);
    if (!rowElement) return;
    const r = rowElement.getBoundingClientRect();
    open = { index, row: { x: r.left, y: r.top, w: r.width, h: r.height }, viaKeyboard };
  }

  function activate(index: number, viaKeyboard: boolean): void {
    const entry = entries[index];
    if (!entry || !isEnabled(entry)) return;
    if (entry.kind === "submenu") {
      openSubmenu(index, viaKeyboard);
    } else if (entry.kind === "action") {
      onclose();
      entry.run();
    }
  }

  function closeSubmenu(): void {
    open = null;
    element.focus();
  }

  // Spec 9.4: arrow keys, Enter and Esc operate every menu.
  function onkeydown(event: KeyboardEvent): void {
    switch (event.key) {
      case "ArrowDown":
        highlighted = moveHighlight(entries, highlighted, 1);
        break;
      case "ArrowUp":
        highlighted = moveHighlight(entries, highlighted, -1);
        break;
      case "ArrowRight":
        if (highlighted >= 0) openSubmenu(highlighted, true);
        break;
      case "ArrowLeft":
        onback?.();
        break;
      case "Enter":
      case " ":
        if (highlighted >= 0) activate(highlighted, true);
        break;
      case "Escape":
        if (onback) onback();
        else onclose();
        break;
      default:
        return;
    }
    event.preventDefault();
    event.stopPropagation();
  }
</script>

<div bind:this={element} class="menu" class:placed data-context-menu role="menu" tabindex="-1" style:left="{left}px" style:top="{top}px" {onkeydown}>
  {#each entries as entry, index (index)}
    {#if entry.kind === "separator"}
      <div class="separator" role="separator"></div>
    {:else}
      {@const enabled = isEnabled(entry)}
      <!-- Rows are operated from the keyboard through the menu element's keydown handler; tabindex -1 keeps them out of the Tab order. -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="row"
        class:highlighted={highlighted === index}
        class:disabled={!enabled}
        role="menuitem"
        tabindex="-1"
        data-index={index}
        aria-disabled={!enabled}
        aria-haspopup={entry.kind === "submenu" ? "menu" : undefined}
        title={entry.disabledReason}
        onpointerenter={() => {
          highlighted = enabled ? index : -1;
          if (entry.kind === "submenu") openSubmenu(index, false);
          else open = null;
        }}
        onclick={() => activate(index, false)}
      >
        <span class="label">{entry.label}</span>
        {#if entry.kind === "action" && entry.shortcut}<span class="shortcut">{entry.shortcut}</span>{/if}
        {#if entry.kind === "submenu"}<span class="arrow" aria-hidden="true">▸</span>{/if}
      </div>
    {/if}
  {/each}
</div>
{#if open}
  {#key open.index}
    <Self entries={itemsAt(open.index)} row={open.row} autofocus={open.viaKeyboard} {onclose} onback={closeSubmenu} />
  {/key}
{/if}

<style>
  .menu {
    position: fixed;
    z-index: 100;
    min-width: 190px;
    padding: 4px;
    visibility: hidden;
    font-size: 12px;
    color: var(--jojo-fg);
    background: var(--jojo-menu);
    border: 1px solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    box-shadow: var(--jojo-shadow);
    outline: none;
  }
  .menu.placed {
    visibility: visible;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 5px 10px;
    border-radius: 5px;
    white-space: nowrap;
    cursor: default;
  }
  .row.highlighted {
    background: var(--jojo-sel);
  }
  .row.disabled {
    color: var(--jojo-muted);
  }
  .label {
    flex: 1;
  }
  .shortcut,
  .arrow {
    font-size: 11px;
    color: var(--jojo-muted);
  }
  .separator {
    height: 1px;
    margin: 4px 6px;
    background: var(--jojo-line);
  }
</style>
```

- [ ] **Step 4: Write the rename field**

`app/src/ui/RenameField.svelte`:
```svelte
<script lang="ts">
  let { value, label, onCommit, onCancel }: { value: string; label: string; onCommit: (value: string) => void; onCancel: () => void } = $props();
  let input: HTMLInputElement;
  let finished = false;

  $effect(() => {
    input.focus();
    input.select();
  });

  /** Enter and blur commit; Escape cancels. `finished` stops the blur that follows Enter or Escape from acting twice. */
  function finish(commit: boolean): void {
    if (finished) return;
    finished = true;
    if (commit) onCommit(input.value);
    else onCancel();
  }

  function onkeydown(event: KeyboardEvent): void {
    // Keys typed here never reach the terminal or the app shortcuts.
    event.stopPropagation();
    if (event.key === "Enter") finish(true);
    else if (event.key === "Escape") finish(false);
  }
</script>

<input bind:this={input} class="rename" aria-label={label} {value} {onkeydown} onblur={() => finish(true)} />

<style>
  .rename {
    flex: 1;
    min-width: 0;
    padding: 1px 5px;
    font: inherit;
    color: var(--jojo-fg);
    background: var(--jojo-pane);
    border: 1px solid var(--jojo-accent);
    border-radius: 4px;
    outline: none;
  }
</style>
```

- [ ] **Step 5: Open the pane menu from the terminal, following the mouse reporting rule**

In `app/src/ui/TerminalPane.svelte`:
1. Add `import { contextMenuTarget } from "../lib/input/mouse";` to the imports.
2. Replace the props line with:
   ```ts
   let {
     session,
     terminalTheme,
     focused,
     onmenu,
   }: { session: SessionInfo; terminalTheme: ITheme; focused: boolean; onmenu: (x: number, y: number) => void } = $props();
   ```
3. Directly after `created.open(host);` add:
   ```ts
    // Spec 9.4 mouse reporting rule. This listener runs in the capture phase
    // on the host, before xterm.js's handlers on its child elements. The
    // browser's own context menu is always suppressed. When the program has
    // mouse reporting on and Shift is not held, xterm.js has already
    // reported the right-button press to the program and nothing else
    // happens here.
    const onContextMenu = (event: MouseEvent) => {
      event.preventDefault();
      if (contextMenuTarget(event.shiftKey, created.modes.mouseTrackingMode) === "jojo") onmenu(event.clientX, event.clientY);
    };
    host.addEventListener("contextmenu", onContextMenu, true);
   ```
4. In the cleanup function, add `host.removeEventListener("contextmenu", onContextMenu, true);` as its first line.

- [ ] **Step 6: Rewrite the pane view with the header menu, notices and rename**

Replace `app/src/ui/PaneView.svelte`:
```svelte
<script lang="ts">
  import type { ITheme } from "@xterm/xterm";
  import type { SessionInfo } from "../lib/protocol/SessionInfo";
  import { focusPane, renameSession } from "../lib/state/app.svelte";
  import { paneMenu } from "../lib/state/contextMenus";
  import { notices } from "../lib/state/notices.svelte";
  import { endRename, openMenu, ui } from "../lib/state/ui.svelte";
  import Badge from "./Badge.svelte";
  import RenameField from "./RenameField.svelte";
  import TerminalPane from "./TerminalPane.svelte";

  let { session, terminalTheme, focused }: { session: SessionInfo; terminalTheme: ITheme; focused: boolean } = $props();

  const renaming = $derived(ui.renaming?.kind === "session" && ui.renaming.where === "pane" && ui.renaming.sessionId === session.id);
  const notice = $derived(notices[session.id]);

  function showMenu(x: number, y: number): void {
    focusPane(session.id);
    openMenu(x, y, paneMenu(session.id));
  }

  function onHeaderMenu(event: MouseEvent): void {
    event.preventDefault();
    showMenu(event.clientX, event.clientY);
  }
</script>

<!-- Pointer focus only; keyboard users move focus between panes with Alt+Arrow (spec 9.6). -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="pane" class:focused onpointerdowncapture={() => focusPane(session.id)}>
  <!-- Spec 9.4: right-click on the header strip opens the pane menu, whatever the mouse mode. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header class="strip" oncontextmenu={onHeaderMenu}>
    <Badge agentState={session.state} size={10} />
    {#if renaming}
      <RenameField
        value={session.label}
        label="Session name"
        onCommit={(value) => {
          endRename();
          renameSession(session.id, value);
        }}
        onCancel={endRename}
      />
    {:else}
      <span class="label">{session.label}</span>
    {/if}
    {#if notice}
      <span class="notice" class:error={notice.tone === "error"} role="status">{notice.text}</span>
    {/if}
  </header>
  <div class="body">
    <TerminalPane {session} {terminalTheme} {focused} onmenu={showMenu} />
  </div>
</div>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    background: var(--jojo-pane);
    border: 1px solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    overflow: hidden;
  }
  /* Spec 9.3 rule 4: the focused pane has a 1 px accent outline. */
  .pane.focused {
    border-color: var(--jojo-accent);
  }
  .strip {
    display: flex;
    align-items: center;
    gap: 7px;
    flex: none;
    height: 24px;
    padding: 0 10px;
    font-size: 11.5px;
    color: var(--jojo-fg2);
    background: var(--jojo-bar);
    border-bottom: 1px solid var(--jojo-line);
  }
  .label {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .notice {
    flex: none;
    max-width: 60%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--jojo-done);
  }
  .notice.error {
    color: var(--jojo-error);
  }
  .body {
    position: relative;
    flex: 1;
    min-height: 0;
    padding: 6px 8px;
  }
</style>
```

- [ ] **Step 7: Rewrite the tab bar with the tab menu and rename**

Replace `app/src/ui/TabBar.svelte`:
```svelte
<script lang="ts">
  import { activeTab, tabLabel } from "../lib/layout/tabs";
  import { countPanes } from "../lib/layout/tree";
  import { app, closeTab, currentProjectLayout, newSession, renameTab, selectTab } from "../lib/state/app.svelte";
  import { tabMenu } from "../lib/state/contextMenus";
  import { beginRename, endRename, openMenu, ui } from "../lib/state/ui.svelte";
  import Badge from "./Badge.svelte";
  import RenameField from "./RenameField.svelte";
  import WindowControls from "./WindowControls.svelte";

  const project = $derived(currentProjectLayout());
  const activeId = $derived(project ? activeTab(project)?.id : undefined);
  const labelOf = (sessionId: string) => app.model.sessions[sessionId]?.label;

  function onTabMenu(event: MouseEvent, tabId: string): void {
    event.preventDefault();
    selectTab(tabId);
    openMenu(event.clientX, event.clientY, tabMenu(tabId));
  }
</script>

<div class="tabs">
  {#each project?.tabs ?? [] as tab (tab.id)}
    {@const focusedSession = app.model.sessions[tab.focused]}
    {@const panes = countPanes(tab.root)}
    <!-- The tab's buttons are its keyboard controls; the container only adds the right-click menu. -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="tab" class:on={activeId === tab.id} oncontextmenu={(e) => onTabMenu(e, tab.id)}>
      {#if ui.renaming?.kind === "tab" && ui.renaming.tabId === tab.id}
        <RenameField
          value={tabLabel(tab, labelOf)}
          label="Tab name"
          onCommit={(value) => {
            endRename();
            renameTab(tab.id, value);
          }}
          onCancel={endRename}
        />
      {:else}
        <button class="title" onclick={() => selectTab(tab.id)} ondblclick={() => beginRename({ kind: "tab", tabId: tab.id })}>
          {#if focusedSession}<Badge agentState={focusedSession.state} size={11} />{/if}
          <span class="name">{tabLabel(tab, labelOf)}</span>
          {#if panes > 1}<span class="count">{panes}</span>{/if}
        </button>
      {/if}
      <button class="close" aria-label="Close tab {tabLabel(tab, labelOf)}" onclick={() => void closeTab(tab.id)}>×</button>
    </div>
  {/each}
  {#if app.selectedProjectId}
    <button class="new" title="New Claude session" aria-label="New Claude session" onclick={() => newSession(app.selectedProjectId!, "claude")}>+</button>
  {/if}
  <div class="drag" data-tauri-drag-region></div>
  <WindowControls />
</div>

<style>
  .tabs {
    display: flex;
    align-items: flex-end;
    gap: 4px;
    height: 38px;
    padding: 6px 0 0 8px;
    background: var(--jojo-bar);
    border-bottom: 1px solid var(--jojo-line);
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 6px 0 12px;
    height: 31px;
    max-width: 260px;
    border-radius: var(--jojo-radius) var(--jojo-radius) 0 0;
    color: var(--jojo-muted);
  }
  .tab.on {
    background: var(--jojo-pane);
    color: var(--jojo-fg);
    border: 1px solid var(--jojo-line);
    border-bottom-color: transparent;
    margin-bottom: -1px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    max-width: 220px;
    white-space: nowrap;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .count {
    padding: 0 5px;
    border-radius: var(--jojo-radius-sm);
    font-size: 10.5px;
    color: var(--jojo-muted);
    background: var(--jojo-sel);
  }
  .close {
    padding: 0 4px;
    color: var(--jojo-muted);
  }
  .close:hover {
    color: var(--jojo-fg);
  }
  .new {
    height: 31px;
    padding: 0 10px;
    color: var(--jojo-muted);
  }
  .drag {
    flex: 1;
    align-self: stretch;
  }
</style>
```

- [ ] **Step 8: Rewrite the sidebar with the session menu and rename**

Replace `app/src/ui/Sidebar.svelte`:
```svelte
<script lang="ts">
  import { addProject, app, focusPane, focusedSessionId, newSession, renameSession, selectProject } from "../lib/state/app.svelte";
  import { sessionMenu } from "../lib/state/contextMenus";
  import { formatElapsed } from "../lib/state/elapsed";
  import { beginRename, endRename, openMenu, ui } from "../lib/state/ui.svelte";
  import { sessionsForProject, sortedProjects } from "../lib/state/views";
  import Badge from "./Badge.svelte";
  import RenameField from "./RenameField.svelte";

  let menuFor = $state<string | null>(null);
  const projects = $derived(sortedProjects(app.model));

  function isInsideOpenMenu(target: EventTarget | null): boolean {
    return target instanceof Element && target.closest(`[data-session-menu="${menuFor}"]`) !== null;
  }

  function isRenaming(sessionId: string): boolean {
    return ui.renaming?.kind === "session" && ui.renaming.where === "sidebar" && ui.renaming.sessionId === sessionId;
  }

  function onSessionMenu(event: MouseEvent, sessionId: string): void {
    event.preventDefault();
    openMenu(event.clientX, event.clientY, sessionMenu(sessionId));
  }

  // The Claude/Shell menu is a single piece of state (menuFor), so only one
  // project's menu is ever open; these listeners are only attached while
  // that is true, and torn down as soon as it closes.
  $effect(() => {
    if (menuFor === null) return;
    const onKeydown = (e: KeyboardEvent) => {
      if (e.key === "Escape") menuFor = null;
    };
    const onClick = (e: MouseEvent) => {
      // Reads menuFor live, not a stale closed-over value: when this click
      // is the same one that toggled the menu via the "+" button's own
      // onclick, that handler already ran (target-phase listeners fire
      // before this bubble-phase one), so menuFor already reflects the
      // toggle's result and isInsideOpenMenu is checked against it.
      if (!isInsideOpenMenu(e.target)) menuFor = null;
    };
    window.addEventListener("keydown", onKeydown);
    window.addEventListener("click", onClick);
    return () => {
      window.removeEventListener("keydown", onKeydown);
      window.removeEventListener("click", onClick);
    };
  });
</script>

<aside class="side">
  <div class="brand" data-tauri-drag-region>jojo</div>
  <div class="machine">
    <span class="dot" class:ok={app.status.state === "connected"}></span>
    <span class="grow">This PC</span>
    <button class="icon" title="Add project" aria-label="Add project" onclick={addProject}>+</button>
  </div>
  {#each projects as project (project.id)}
    <div class="project" class:selected={app.selectedProjectId === project.id}>
      <button class="name" onclick={() => selectProject(project.id)}>{project.name}</button>
      <button
        class="icon add"
        title="New session"
        aria-label={`New session in ${project.name}`}
        data-session-menu={project.id}
        onclick={() => (menuFor = menuFor === project.id ? null : project.id)}
      >+</button>
    </div>
    {#if menuFor === project.id}
      <div class="menu" data-session-menu={project.id}>
        <button onclick={() => { menuFor = null; newSession(project.id, "claude"); }}>Claude</button>
        <button onclick={() => { menuFor = null; newSession(project.id, "shell"); }}>Shell</button>
      </div>
    {/if}
    {#each sessionsForProject(app.model, project.id) as session (session.id)}
      {#if isRenaming(session.id)}
        <div class="session renaming">
          <Badge agentState={session.state} />
          <RenameField
            value={session.label}
            label="Session name"
            onCommit={(value) => {
              endRename();
              renameSession(session.id, value);
            }}
            onCancel={endRename}
          />
        </div>
      {:else}
        <button
          class="session"
          class:active={focusedSessionId() === session.id}
          onclick={() => focusPane(session.id)}
          ondblclick={() => beginRename({ kind: "session", sessionId: session.id, where: "sidebar" })}
          oncontextmenu={(e) => onSessionMenu(e, session.id)}
        >
          <Badge agentState={session.state} />
          <span class="label">{session.label}</span>
          <span class="time">{formatElapsed(app.now - session.state_changed_at)}</span>
        </button>
      {/if}
    {/each}
  {/each}
</aside>

<style>
  .side {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 10px 8px;
    background: var(--jojo-side);
    border-right: 1px solid var(--jojo-line);
    overflow-y: auto;
    min-height: 0;
  }
  .brand {
    padding: 4px 8px 10px;
    font-size: 15px;
    font-weight: 600;
    background: var(--jojo-brand-gradient);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
  .machine {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 10px 8px 4px;
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--jojo-muted);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--jojo-needs-you);
  }
  .dot.ok {
    background: var(--jojo-done);
  }
  .grow {
    flex: 1;
  }
  .icon {
    color: var(--jojo-muted);
    padding: 0 4px;
  }
  .icon:hover {
    color: var(--jojo-fg);
  }
  .project {
    display: flex;
    align-items: center;
    padding: 6px 8px 2px;
  }
  .project .name {
    flex: 1;
    text-align: left;
    color: var(--jojo-fg2);
    font-weight: 500;
  }
  .project.selected .name {
    color: var(--jojo-fg);
  }
  .project .add {
    opacity: 0;
  }
  .project:hover .add,
  .project:focus-within .add {
    opacity: 1;
  }
  .menu {
    display: flex;
    gap: 4px;
    padding: 2px 8px 4px 14px;
  }
  .menu button {
    padding: 3px 8px;
    border: 1px solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    color: var(--jojo-fg2);
  }
  .menu button:hover {
    background: var(--jojo-sel);
  }
  .session {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px 5px 14px;
    border-radius: var(--jojo-radius);
    color: var(--jojo-fg2);
    text-align: left;
  }
  .session.active {
    background: var(--jojo-sel);
    color: var(--jojo-fg);
    box-shadow: inset 2px 0 0 var(--jojo-accent);
  }
  .label {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .time {
    color: var(--jojo-muted);
    font-size: 10.5px;
  }
</style>
```

- [ ] **Step 9: Mount the menu and enable clipboard access**

In `app/src/App.svelte`:
1. Add imports:
   ```ts
   import { focusedSessionId } from "./lib/state/app.svelte";
   import { closeMenu, ui } from "./lib/state/ui.svelte";
   import { terminalControls } from "./lib/terminal/registry";
   import ContextMenu from "./ui/ContextMenu.svelte";
   ```
   (merge `focusedSessionId` into the existing `app.svelte` import line).
2. Add to the script:
   ```ts
   /** Closing a menu returns keyboard focus to the focused pane's terminal. */
   function closeMenuAndRefocus(): void {
     closeMenu();
     terminalControls(focusedSessionId())?.focus();
   }
   ```
3. Add after `<ConfirmDialog />`:
   ```svelte
   {#if ui.menu}
     {#key ui.menu}
       <ContextMenu entries={ui.menu.entries} x={ui.menu.x} y={ui.menu.y} onclose={closeMenuAndRefocus} />
     {/key}
   {/if}
   ```

In `app/src-tauri/src/main.rs`, change the window builder line in `setup` to:
```rust
            // enable_clipboard_access makes wry allow WebView2's clipboard-read
            // permission, which navigator.clipboard.readText (Paste) needs.
            WebviewWindowBuilder::from_config(app.handle(), &config)?
                .on_new_window(|_, _| NewWindowResponse::Deny)
                .enable_clipboard_access()
                .build()?;
```

- [ ] **Step 10: Type-check, lint, test and build**

Run: `pnpm --dir app check`, `pnpm --dir app lint:colors`, `pnpm --dir app test`, `cargo clippy -p jojo-app --all-targets --jobs 4`
Expected: 0 errors; no color literals; all tests pass; no clippy warnings.

- [ ] **Step 11: Check the menus by hand**

Run `pnpm --dir app tauri dev` from the Bash tool or a desktop terminal. Check:
1. Right-click in a pwsh pane opens the pane menu at the pointer; Split right ▸ New shell adds a pane to the right; with 4 panes, Split right and Split down are disabled and hovering shows "Tab is full (4 panes)".
2. Arrow keys move the highlight, Right opens a submenu, Left closes it, Enter runs an item, Esc closes the menu.
3. Run `vim` (or `htop` in WSL, or `less` with mouse mode) in a pane: a plain right-click goes to the program; Shift+right-click opens the pane menu.
4. Zoom pane fills the tab and the other panes detach; Unzoom brings them back with their screens. Swap with, Move to ▸ New tab, Move to ▸ another tab, Layout presets and Equalize sizes rearrange panes without the moved terminals losing their content.
5. Rename session… edits the label in the header; the sidebar shows the new label (jojod broadcast `SessionUpdated`). Double-clicking a sidebar session renames it there. Right-click on a tab ▸ Rename tab… and double-click on a tab title rename the tab; the name survives an app restart.
6. Sidebar session menu: Open in split right/down is disabled for a session of another project; Kill on a running shell marks it exited and keeps its row and screen; Restart on that exited row starts a new shell in the same pane position and the old row disappears.
7. Paste from the pane menu inserts the clipboard text; Copy with a selection puts it on the clipboard and the header shows "Copied".

- [ ] **Step 12: Commit**

```bash
git add app/src app/src-tauri/src/main.rs
git commit -m "feat(app): pane, tab and session context menus with rename, kill and restart"
```

---

### Task 11: Keyboard shortcuts and copy on select

**Files:**
- Create: `app/src/lib/state/actions.ts`
- Modify: `app/src/App.svelte`, `app/src/ui/TerminalPane.svelte`

**Interfaces:**
- Consumes: `resolveShortcut`, `AppAction` (Task 7); app actions and `focusedSessionId` (Task 9); `copyFromPane`, `pasteIntoPane` (Task 10); `terminalControls` (Task 9); `dialog` (Task 9); `ui` (Task 10).
- Produces: `runAction(action: AppAction): void` in `app/src/lib/state/actions.ts`.

How a key travels:

1. `App.svelte` listens for `keydown` on `window` in the **capture** phase, so it runs before xterm.js's own handler on its hidden textarea. It resolves the key with `resolveShortcut`; for an application shortcut it calls `preventDefault()` and `stopPropagation()` (xterm.js never sees the keydown, so nothing is sent to the program) and runs the action. Any other key continues to xterm.js unchanged.
2. The listener does nothing while the confirmation dialog or a context menu is open, or when the key goes to an `<input>` (the rename field), because those handle their own keys.
3. `TerminalPane` also installs `attachCustomKeyEventHandler` returning `false` for application shortcuts, so the `keypress` and `keyup` events of a shortcut, which still reach the textarea, are ignored by xterm.js too.
4. Ctrl+C: with `copyOnSelect` on, `resolveShortcut` returns null and xterm.js sends `\x03` whether or not text is selected (spec 9.5 rule 5); xterm.js 6 itself never copies on Ctrl+C on Windows. With `copyOnSelect` off and a selection, the shortcut resolves to `copy`.

Copy on select (spec 9.5 rules 1-2): `TerminalPane` listens for `mouseup` (left button) on its host and, on the next task tick (after xterm.js's own mouseup handling has finished the selection), copies `getSelection()` if there is one. A double-click selects a word and a triple-click a line on `mousedown`, and their `mouseup` then copies them the same way. The header shows "Copied" for `COPIED_MS`. Rule 3 (the selection stays highlighted until a click elsewhere in the pane or a keystroke) is xterm.js's default behavior.

`actions.ts` imports `app.svelte.ts`, which uses Svelte runes, and vitest is configured without the Svelte compiler (`app/vitest.config.ts`), so this glue is verified by svelte-check and the manual checks below; the decisions it relies on (`resolveShortcut`, the tab operations, focus geometry) are unit-tested in Tasks 3, 4 and 7.

- [ ] **Step 1: Write the action dispatcher**

`app/src/lib/state/actions.ts`:
```ts
// Runs an application shortcut (spec 9.6) against the current state.
import type { AppAction } from "../input/shortcuts";
import { app, closePane, cycleTab, focusDirection, focusedSessionId, newSession, splitFocused, zoomSession } from "./app.svelte";
import { copyFromPane, pasteIntoPane } from "./clipboard";

export function runAction(action: AppAction): void {
  const focused = focusedSessionId();
  switch (action) {
    case "new-claude-tab":
      if (app.selectedProjectId) newSession(app.selectedProjectId, "claude");
      return;
    case "new-shell-tab":
      if (app.selectedProjectId) newSession(app.selectedProjectId, "shell");
      return;
    case "split-right":
      splitFocused("row", "claude");
      return;
    case "split-down":
      splitFocused("column", "claude");
      return;
    case "zoom":
      if (focused) zoomSession(focused);
      return;
    case "focus-left":
      focusDirection("left");
      return;
    case "focus-right":
      focusDirection("right");
      return;
    case "focus-up":
      focusDirection("up");
      return;
    case "focus-down":
      focusDirection("down");
      return;
    case "next-tab":
      cycleTab(1);
      return;
    case "previous-tab":
      cycleTab(-1);
      return;
    case "close-pane":
      if (focused) void closePane(focused);
      return;
    case "copy":
      if (focused) void copyFromPane(focused);
      return;
    case "paste":
      if (focused) void pasteIntoPane(focused);
      return;
  }
}
```

- [ ] **Step 2: Listen for shortcuts in the root component**

In `app/src/App.svelte`:
1. Add imports:
   ```ts
   import { resolveShortcut } from "./lib/input/shortcuts";
   import { runAction } from "./lib/state/actions";
   import { dialog } from "./lib/state/dialog.svelte";
   ```
2. Replace the `onMount` call with:
   ```ts
   onMount(() => {
     applyTheme(theme);
     window.addEventListener("keydown", onKeydown, true);
     const stopApp = startApp();
     return () => {
       window.removeEventListener("keydown", onKeydown, true);
       stopApp();
     };
   });

   /**
    * Application shortcuts (spec 9.6), handled in the capture phase on window
    * so they run before xterm.js's keydown handler on its textarea. A matched
    * shortcut is stopped here, so xterm.js never sends it to the program;
    * every other key reaches the terminal unchanged. The dialog, menus and
    * rename fields handle their own keys.
    */
   function onKeydown(event: KeyboardEvent): void {
     if (dialog.confirm || ui.menu || event.target instanceof HTMLInputElement) return;
     const action = resolveShortcut(event, {
       copyOnSelect: app.settings?.copyOnSelect ?? true,
       hasSelection: terminalControls(focusedSessionId())?.hasSelection() ?? false,
     });
     if (!action) return;
     event.preventDefault();
     event.stopPropagation();
     runAction(action);
   }
   ```

- [ ] **Step 3: Ignore shortcuts in xterm.js and copy on select**

In `app/src/ui/TerminalPane.svelte`:
1. Add imports:
   ```ts
   import { resolveShortcut } from "../lib/input/shortcuts";
   import { copyFromPane } from "../lib/state/clipboard";
   ```
2. After the `host.addEventListener("contextmenu", onContextMenu, true);` line (Task 10), add:
   ```ts
    // App.svelte handles application shortcuts on keydown before xterm.js
    // sees them; returning false here makes xterm.js also ignore the
    // keypress and keyup events of those keys.
    created.attachCustomKeyEventHandler(
      (event) => resolveShortcut(event, { copyOnSelect: app.settings?.copyOnSelect ?? true, hasSelection: created.hasSelection() }) === null,
    );

    // Spec 9.5 rules 1-2: releasing the left button after a drag, double-click
    // or triple-click selection copies it. The check waits one task so that
    // xterm.js's own mouseup handling has finished the selection.
    const onMouseUp = (event: MouseEvent) => {
      if (event.button !== 0 || !(app.settings?.copyOnSelect ?? true)) return;
      setTimeout(() => {
        if (created.hasSelection()) void copyFromPane(id);
      }, 0);
    };
    host.addEventListener("mouseup", onMouseUp);
   ```
3. In the cleanup function, add `host.removeEventListener("mouseup", onMouseUp);` after the contextmenu removal.

- [ ] **Step 4: Type-check, lint and test**

Run: `pnpm --dir app check`, `pnpm --dir app lint:colors`, `pnpm --dir app test`
Expected: 0 errors; no color literals; all tests pass.

- [ ] **Step 5: Check the shortcuts by hand**

Run `pnpm --dir app tauri dev` from the Bash tool or a desktop terminal. Check:
1. Ctrl+Shift+T opens a Claude tab and Ctrl+Shift+N a shell tab in the selected project; Ctrl+Tab and Ctrl+Shift+Tab cycle tabs.
2. Alt+Shift+= and Alt+Shift+- split the focused pane with a new Claude session; Alt+Arrow moves the accent outline and keyboard focus between panes; Alt+Shift+Z zooms and unzooms; Ctrl+Shift+W closes the focused pane.
3. In a shell running `ping -t localhost`, select text with the mouse: the header shows "Copied" for about a second and the clipboard holds the text; pressing Ctrl+C (selection still visible) interrupts ping.
4. Set `"copyOnSelect": false` in `%APPDATA%\Jojo\settings.json`, restart the app: dragging no longer copies; Ctrl+C with a selection copies (ping keeps running); Ctrl+C without a selection interrupts.
5. Ctrl+Shift+C copies the selection and Ctrl+Shift+V pastes; plain Ctrl+V still pastes; plain Ctrl+T in a Claude Code pane opens its todo list.
6. With the rename field open (double-click a tab), Ctrl+Shift+W does not close a pane.

- [ ] **Step 6: Commit**

```bash
git add app/src/lib/state/actions.ts app/src/App.svelte app/src/ui/TerminalPane.svelte
git commit -m "feat(app): keyboard shortcuts and copy on select"
```

---

### Task 12: Links: OSC 8, failure feedback, and open_url off the main thread

**Files:**
- Create: `app/src/lib/terminal/links.ts`
- Modify: `app/src/ui/TerminalPane.svelte`, `app/src-tauri/src/links.rs`, `app/src-tauri/src/main.rs`, `app/src-tauri/Cargo.toml`
- Test: `app/src-tauri/src/links.rs` (tests module)

**Interfaces:**
- Consumes: `askConfirm` (Task 9), `showNotice` (Task 10), `errorText` (Task 9), the existing `jojo_app::links::open_url`.
- Produces: `openLink(sessionId: string, url: string, confirmTarget: boolean): Promise<void>`; the `open_url` command becomes async and runs `ShellExecuteW` on a blocking worker thread inside a COM apartment.

Carry-over 3.1 items 2-4:

1. **OSC 8.** xterm.js 6's default OSC 8 handler shows `confirm()` and then calls `window.open()`, which `main.rs`'s `on_new_window` handler denies, so nothing opened. The `linkHandler` terminal option now calls `openLink(..., confirmTarget = true)`: OSC 8 link text is chosen by the program and can differ from the URL it opens, so Jojo shows the URL in its own confirmation dialog first. `allowNonHttpProtocols` stays at its default (`false`), so xterm.js ignores OSC 8 links whose scheme is not http or https. Plain URLs recognized by `WebLinksAddon` open without a confirmation, because their text is the URL.
2. **Failure feedback.** A URL `open_url` rejects, or a failed `ShellExecuteW`, shows `Link not opened: <reason>` in the pane's header for `ERROR_MS` (4 s), instead of being swallowed.
3. **Main thread.** `open_url` was a synchronous command, which Tauri runs on the main thread, and `ShellExecuteW` can block there (for example while Windows shows its "How do you want to open this?" dialog), freezing the window. The command is now `async` and calls `links::open_url` through `spawn_blocking`. `ShellExecuteW`'s documentation requires COM to be initialized on the calling thread, because the handlers it may invoke use COM; worker threads do not have it, so `shell_open` now holds a `ComApartment` (`CoInitializeEx(COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE)`, with `CoUninitialize` on drop) around the call.

- [ ] **Step 1: Write the failing Rust test**

Add to the tests module in `app/src-tauri/src/links.rs`:
```rust
    #[cfg(windows)]
    #[test]
    fn a_worker_thread_gets_a_com_apartment_and_releases_it() {
        std::thread::spawn(|| {
            let first = ComApartment::enter();
            assert!(first.initialized, "CoInitializeEx failed on a fresh thread");
            // A second enter on the same thread returns S_FALSE, which also
            // needs its own CoUninitialize, so it also counts as initialized.
            let second = ComApartment::enter();
            assert!(second.initialized);
            drop(second);
            drop(first);
        })
        .join()
        .unwrap();
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p jojo-app --jobs 4 --lib links::`
Expected: FAIL to compile with "failed to resolve: use of undeclared type `ComApartment`".

- [ ] **Step 3: Add the COM apartment**

In `app/src-tauri/Cargo.toml`, change the Windows dependency line to:
```toml
windows-sys = { workspace = true, features = ["Win32_UI_Shell", "Win32_UI_WindowsAndMessaging", "Win32_System_Com"] }
```

In `app/src-tauri/src/links.rs`, add `let _com = ComApartment::enter();` as the first statement of the Windows `shell_open` (before the `wide` closure), and add after that function:
```rust
/// Initializes COM on the calling thread for as long as the value lives.
/// ShellExecuteW's documentation requires COM to be initialized before the
/// call, because the protocol handlers and shell extensions it may run use
/// COM. `open_url` runs on a blocking worker thread (see main.rs), which has
/// no COM apartment of its own.
#[cfg(windows)]
struct ComApartment {
    initialized: bool,
}

#[cfg(windows)]
impl ComApartment {
    fn enter() -> ComApartment {
        use windows_sys::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
        // SAFETY: the reserved pointer must be null, and the flags are the
        // combination ShellExecuteW's documentation recommends.
        let hr = unsafe { CoInitializeEx(std::ptr::null(), (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32) };
        // S_OK (0) and S_FALSE (1, already initialized on this thread) both
        // need a matching CoUninitialize; a failure such as
        // RPC_E_CHANGED_MODE (negative) does not, and ShellExecuteW then runs
        // in the apartment the thread already has.
        ComApartment { initialized: hr >= 0 }
    }
}

#[cfg(windows)]
impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: balances the successful CoInitializeEx in `enter`; the
            // value is created and dropped inside `shell_open` on one thread.
            unsafe { windows_sys::Win32::System::Com::CoUninitialize() };
        }
    }
}
```

- [ ] **Step 4: Run the Rust tests to verify they pass**

Run: `cargo test -p jojo-app --jobs 4 --lib links::`
Expected: PASS (the new test and the four existing URL validation tests).

- [ ] **Step 5: Make the command asynchronous**

In `app/src-tauri/src/main.rs`, replace the `open_url` command with:
```rust
/// Opens a link clicked in a terminal in the default browser. Only
/// `http://` and `https://` URLs are accepted (see `jojo_app::links`).
/// ShellExecuteW can block, for example while Windows asks which app should
/// open the link; a synchronous command runs on the main thread, which runs
/// the window's event loop, so the call is made on a blocking worker thread.
#[tauri::command]
async fn open_url(url: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || jojo_app::links::open_url(&url))
        .await
        .map_err(|e| format!("the link opener thread failed: {e}"))?
}
```

- [ ] **Step 6: Open links from the terminal through openLink**

`app/src/lib/terminal/links.ts`:
```ts
// Opens links clicked in a terminal (Plan 2b carry-over 3.1 items 2 and 3).
import { invoke } from "@tauri-apps/api/core";
import { errorText } from "../errors";
import { askConfirm } from "../state/dialog.svelte";
import { showNotice } from "../state/notices.svelte";

/**
 * Opens `url` in the default browser through the open_url command, which
 * accepts only http and https URLs with a host (app/src-tauri/src/links.rs).
 * With `confirmTarget`, the URL is shown in a confirmation dialog first;
 * OSC 8 hyperlinks use it because their visible text is chosen by the
 * program and can differ from the URL. A rejected URL or a failed
 * ShellExecuteW is shown in the pane's header strip.
 */
export async function openLink(sessionId: string, url: string, confirmTarget: boolean): Promise<void> {
  if (confirmTarget) {
    const confirmed = await askConfirm({
      title: "Open link?",
      message: "Link text in a terminal can differ from where the link goes. This link opens:",
      details: [url],
      confirmLabel: "Open link",
    });
    if (!confirmed) return;
  }
  try {
    await invoke("open_url", { url });
  } catch (err) {
    showNotice(sessionId, `Link not opened: ${errorText(err)}`, "error");
  }
}
```

In `app/src/ui/TerminalPane.svelte`:
1. Remove `import { invoke } from "@tauri-apps/api/core";` and add `import { openLink } from "../lib/terminal/links";`.
2. Add to the `new Terminal({ ... })` options, after `cursorBlink: true,`:
   ```ts
      // OSC 8 hyperlinks. xterm.js's default handler calls confirm() and then
      // window.open(), which the main window denies, so nothing opened.
      // allowNonHttpProtocols stays false, so links with other schemes are
      // ignored by xterm.js.
      linkHandler: { activate: (_event, uri) => void openLink(id, uri, true) },
   ```
3. Replace the `WebLinksAddon` line and its comment with:
   ```ts
    // Plain URLs found in the output. Their text is the URL itself, so they
    // open without a confirmation; open_url validates them.
    created.loadAddon(new WebLinksAddon((_event, uri) => void openLink(id, uri, false)));
   ```

- [ ] **Step 7: Type-check, lint, test and build**

Run: `pnpm --dir app check`, `pnpm --dir app test`, `cargo clippy -p jojo-app --all-targets --jobs 4`
Expected: 0 errors; all tests pass; no clippy warnings.

- [ ] **Step 8: Check links by hand**

Run `pnpm --dir app tauri dev` from the Bash tool or a desktop terminal, and in a pwsh pane:
1. `$e = [char]27; Write-Host "$e]8;;https://example.com$e\click here$e]8;;$e\"` prints "click here"; clicking it shows "Open link?" with `https://example.com`; Open opens the default browser; Cancel does nothing.
2. `Write-Host "see https://example.org/docs"`: clicking the URL opens it directly.
3. `Write-Host "$e]8;;http://$e\broken$e]8;;$e\"`: clicking "broken" and confirming shows `Link not opened: URL has no host` in the header for about 4 seconds.
4. While a browser is starting, the Jojo window stays responsive (drag it).

- [ ] **Step 9: Commit**

```bash
git add app/src/lib/terminal/links.ts app/src/ui/TerminalPane.svelte app/src-tauri/src/links.rs app/src-tauri/src/main.rs app/src-tauri/Cargo.toml
git commit -m "feat(app): OSC 8 links with confirmation, link failure notices, open_url off the main thread"
```

---

### Task 13: Windows 11 snap layouts on the maximize button

**Files:**
- Create: `app/src-tauri/src/snap.rs`, `app/src-tauri/windows-app-manifest.xml`, `app/src/lib/window/snap.ts`
- Modify: `app/src-tauri/src/lib.rs`, `app/src-tauri/src/main.rs`, `app/src-tauri/build.rs`, `app/src-tauri/Cargo.toml`, `app/src/ui/WindowControls.svelte`
- Test: `app/src-tauri/src/snap.rs` (tests module), `app/src/lib/window/snap.test.ts`

**Interfaces:**
- Consumes: the main window built in `main.rs`'s `setup` (Task 10 version).
- Produces: `jojo_app::snap::{HOVER_EVENT = "maximize-hover", install(app: &AppHandle, parent: isize) -> Result<(), String>, set_button_rect(x: i32, y: i32, width: i32, height: i32)}` (Windows only); Tauri command `set_maximize_button_rect(x, y, width, height)`; TypeScript `toPhysicalRect(rect: { left; top; width; height }, scale: number): { x: number; y: number; width: number; height: number }`.

Spec 9.1 requires the maximize button's area to be reported to Windows as the maximize button (`HTMAXBUTTON`) so that Windows 11 shows the snap layout flyout when hovering it (carry-over 3.1 item 1). The title bar is HTML inside WebView2, and WebView2 receives mouse input in a child window that belongs to the WebView2 browser process, which Jojo cannot subclass. `snap.rs` therefore creates an input-sink child window of the main window, the technique Windows Terminal uses for its custom title bar:

1. The sink is created with `WS_EX_LAYERED | WS_EX_NOREDIRECTIONBITMAP` and `WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS`, and `SetLayeredWindowAttributes(alpha 255)`. It has no redirection bitmap and never paints, so nothing of it is visible, but because its alpha is 255 Windows hit-tests it. Windows accepts `WS_EX_LAYERED` on child windows only for applications whose manifest declares Windows 8 or later; Tauri's default manifest declares no `supportedOS`, so `build.rs` embeds `windows-app-manifest.xml`, which keeps Tauri's Common Controls 6 dependency and adds the Windows 10/11 `supportedOS` id.
2. The frontend reports the HTML maximize button's rectangle in physical pixels (CSS pixels × `devicePixelRatio`) on mount, on window resize and on button resize; `set_button_rect` moves the sink there with `SetWindowPos(HWND_TOP, SWP_NOACTIVATE | SWP_SHOWWINDOW)`, which also keeps it above the WebView2 child window.
3. The sink answers `WM_NCHITTEST` with `HTMAXBUTTON`, which is what makes Windows show the flyout. Because the sink receives the button's clicks, it maximizes or restores the main window itself on `WM_NCLBUTTONUP` (`ShowWindow(SW_MAXIMIZE)` or `ShowWindow(SW_RESTORE)` depending on `IsZoomed`), swallows `WM_NCLBUTTONDOWN` and `WM_NCLBUTTONDBLCLK` (otherwise `DefWindowProcW` runs caption-button tracking for a window that has no caption), and tracks non-client hover with `TrackMouseEvent(TME_LEAVE | TME_NONCLIENT)` to emit `maximize-hover` `true`/`false`, which the HTML button uses to draw its hover background.
4. Keyboard activation of the HTML button (Tab, then Enter) still reaches the button, whose `onclick` calls `toggleMaximize()`.
5. If the sink cannot be created, `install` returns an error that `main.rs` ignores: the HTML button then keeps working through the window API, only without the flyout.

- [ ] **Step 1: Write the failing tests**

`app/src/lib/window/snap.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { toPhysicalRect } from "./snap";

describe("toPhysicalRect", () => {
  it("keeps CSS pixels at scale 1", () => {
    expect(toPhysicalRect({ left: 1188, top: 0, width: 46, height: 38 }, 1)).toEqual({ x: 1188, y: 0, width: 46, height: 38 });
  });

  it("scales and rounds each edge separately", () => {
    expect(toPhysicalRect({ left: 1188, top: 0, width: 46, height: 38 }, 1.25)).toEqual({ x: 1485, y: 0, width: 58, height: 48 });
  });
});
```

Create `app/src-tauri/src/snap.rs` with only a module doc line and the tests module:
```rust
//! Windows 11 snap layouts for the custom maximize button.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sink_answers_hit_tests_as_the_maximize_button() {
        // SAFETY: WM_NCHITTEST is answered without using the window handle.
        let result = unsafe { sink_proc(std::ptr::null_mut(), WM_NCHITTEST, 0, 0) };
        assert_eq!(result, HTMAXBUTTON as LRESULT);
    }

    #[test]
    fn moving_the_sink_before_it_exists_does_nothing() {
        set_button_rect(0, 0, 46, 38);
    }
}
```
and add to `app/src-tauri/src/lib.rs`:
```rust
#[cfg(windows)]
pub mod snap;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --dir app exec vitest run src/lib/window/snap.test.ts` and `cargo test -p jojo-app --jobs 4 --lib snap::`
Expected: vitest FAIL with "Failed to resolve import "./snap""; cargo FAIL to compile with "cannot find function `sink_proc`".

- [ ] **Step 3: Write the conversion helper**

`app/src/lib/window/snap.ts`:
```ts
// The maximize button's rectangle in physical pixels, for the snap-layout
// input sink (app/src-tauri/src/snap.rs). Each edge is scaled and rounded
// separately, so the sink covers the same pixels the button is drawn on.

export interface PhysicalRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function toPhysicalRect(rect: { left: number; top: number; width: number; height: number }, scale: number): PhysicalRect {
  const x = Math.round(rect.left * scale);
  const y = Math.round(rect.top * scale);
  return {
    x,
    y,
    width: Math.round((rect.left + rect.width) * scale) - x,
    height: Math.round((rect.top + rect.height) * scale) - y,
  };
}
```

- [ ] **Step 4: Write the input sink**

In `app/src-tauri/Cargo.toml`, change the Windows dependency line to:
```toml
windows-sys = { workspace = true, features = [
  "Win32_UI_Shell",
  "Win32_UI_WindowsAndMessaging",
  "Win32_System_Com",
  "Win32_Graphics_Gdi",
  "Win32_UI_Input_KeyboardAndMouse",
  "Win32_System_LibraryLoader",
] }
```
(`Win32_Graphics_Gdi` is required by `WNDCLASSEXW`'s `hbrBackground` field.)

Replace the top of `app/src-tauri/src/snap.rs` (everything above `#[cfg(test)]`) with:
```rust
//! Windows 11 snap layouts for the custom maximize button (spec section
//! 9.1; Plan 2b carry-over 3.1 item 1).
//!
//! Windows shows the snap layout flyout when the pointer rests on an area
//! whose WM_NCHITTEST answer is HTMAXBUTTON. Jojo's title bar is HTML inside
//! WebView2, and WebView2 takes mouse input in a child window owned by the
//! WebView2 browser process, which this process cannot subclass. This
//! module creates a small child window of the main window (an input sink,
//! the technique Windows Terminal uses for its title bar), keeps it above
//! the WebView2 window exactly over the HTML maximize button, and answers
//! HTMAXBUTTON for it. Because the sink receives the button's mouse input,
//! it also maximizes or restores the window on click, and it reports pointer
//! enter and leave to the frontend (`HOVER_EVENT`) so the HTML button can
//! still draw its hover state.
//!
//! The sink is a layered window (WS_EX_LAYERED) without a redirection bitmap
//! (WS_EX_NOREDIRECTIONBITMAP): it never paints, so nothing of it is
//! visible, and because its layered alpha is 255 Windows still hit-tests it.
//! Layered child windows require an application manifest that declares
//! Windows 8 or later; app/src-tauri/windows-app-manifest.xml declares
//! Windows 10 and 11.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetParent, IsZoomed, LoadCursorW, RegisterClassExW, SetLayeredWindowAttributes,
    SetWindowPos, ShowWindow, HTMAXBUTTON, HWND_TOP, IDC_ARROW, LWA_ALPHA, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SW_MAXIMIZE, SW_RESTORE, WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE,
    WM_NCMOUSEMOVE, WNDCLASSEXW, WS_CHILD, WS_CLIPSIBLINGS, WS_EX_LAYERED, WS_EX_NOREDIRECTIONBITMAP, WS_VISIBLE,
};

/// Event sent to the frontend: `true` when the pointer enters the sink,
/// `false` when it leaves.
pub const HOVER_EVENT: &str = "maximize-hover";

/// The sink's HWND, stored as an integer because HWND is a raw pointer
/// (not Send); 0 until `install` succeeds.
static SINK: AtomicIsize = AtomicIsize::new(0);
/// True between the first WM_NCMOUSEMOVE and the WM_NCMOUSELEAVE that
/// TrackMouseEvent requested.
static TRACKING: AtomicBool = AtomicBool::new(false);
static APP: OnceLock<AppHandle> = OnceLock::new();

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Creates the sink as a child of the main window, whose HWND is passed as
/// an integer (`parent`): HWND is a raw pointer, and a safe public function
/// must not take a raw pointer that it hands to the OS (clippy's
/// `not_unsafe_ptr_arg_deref`). Call once, on the main thread, after the
/// webview exists, so the sink is created above the WebView2 child window.
pub fn install(app: &AppHandle, parent: isize) -> Result<(), String> {
    let parent = parent as HWND;
    let _ = APP.set(app.clone());
    let class = wide("JojoMaximizeButtonSink");
    // SAFETY: every pointer passed below is either null where the API allows
    // null, or points to a NUL-terminated UTF-16 buffer (`class`) or a fully
    // initialized struct (`wc`) that outlives the call. `parent` is the live
    // main window handle from Tauri.
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(sink_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: std::ptr::null_mut(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
            hIconSm: std::ptr::null_mut(),
        };
        if RegisterClassExW(&wc) == 0 {
            return Err(format!("RegisterClassExW failed: {}", std::io::Error::last_os_error()));
        }
        let sink = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_NOREDIRECTIONBITMAP,
            class.as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            0,
            0,
            0,
            0,
            parent,
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if sink.is_null() {
            return Err(format!("CreateWindowExW failed: {}", std::io::Error::last_os_error()));
        }
        SetLayeredWindowAttributes(sink, 0, 255, LWA_ALPHA);
        SINK.store(sink as isize, Ordering::Release);
    }
    Ok(())
}

/// Moves the sink over the HTML maximize button. Coordinates are physical
/// pixels in the main window's client area. Does nothing before `install`
/// has succeeded.
pub fn set_button_rect(x: i32, y: i32, width: i32, height: i32) {
    let sink = SINK.load(Ordering::Acquire) as HWND;
    if sink.is_null() {
        return;
    }
    // SAFETY: `sink` is the window `install` created; it lives as long as
    // its parent, the main window. HWND_TOP puts it above its siblings,
    // including the WebView2 child window.
    unsafe {
        SetWindowPos(sink, HWND_TOP, x, y, width.max(0), height.max(0), SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }
}

fn emit_hover(hovered: bool) {
    if let Some(app) = APP.get() {
        let _ = app.emit(HOVER_EVENT, hovered);
    }
}

/// Window procedure of the sink.
unsafe extern "system" fn sink_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        // The whole sink is the maximize button. This answer is what makes
        // Windows 11 show the snap layout flyout on hover.
        WM_NCHITTEST => HTMAXBUTTON as LRESULT,
        WM_NCMOUSEMOVE => {
            if !TRACKING.swap(true, Ordering::AcqRel) {
                let mut track = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE | TME_NONCLIENT,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                TrackMouseEvent(&mut track);
                emit_hover(true);
            }
            0
        }
        WM_NCMOUSELEAVE => {
            TRACKING.store(false, Ordering::Release);
            emit_hover(false);
            0
        }
        // Swallowed: DefWindowProcW would run its own caption-button
        // tracking for a window that has no caption.
        WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK => 0,
        WM_NCLBUTTONUP => {
            if wparam as u32 == HTMAXBUTTON {
                let parent = GetParent(hwnd);
                ShowWindow(parent, if IsZoomed(parent) != 0 { SW_RESTORE } else { SW_MAXIMIZE });
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `pnpm --dir app exec vitest run src/lib/window/snap.test.ts` and `cargo test -p jojo-app --jobs 4 --lib snap::`
Expected: PASS for both.

- [ ] **Step 6: Embed the manifest**

`app/src-tauri/windows-app-manifest.xml`:
```xml
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <!-- Tauri's default manifest content: Common Controls version 6. -->
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
  <!-- Windows 10 and 11. Windows creates WS_EX_LAYERED child windows (the
       snap-layout input sink in src/snap.rs) only for applications that
       declare Windows 8 or later here. -->
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
</assembly>
```

Replace `app/src-tauri/build.rs`:
```rust
fn main() {
    // A custom manifest replaces Tauri's default one; it keeps the default's
    // Common Controls 6 dependency and adds supportedOS (see the file).
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("windows-app-manifest.xml"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows)).expect("failed to run tauri-build");
}
```

- [ ] **Step 7: Install the sink and add the command**

In `app/src-tauri/src/main.rs`:
1. Add the command:
   ```rust
   /// Places the snap-layout input sink over the HTML maximize button (see
   /// `jojo_app::snap`). The frontend sends the button's rectangle in
   /// physical pixels whenever the window or the button changes size.
   #[tauri::command]
   fn set_maximize_button_rect(x: i32, y: i32, width: i32, height: i32) {
       #[cfg(windows)]
       jojo_app::snap::set_button_rect(x, y, width, height);
       #[cfg(not(windows))]
       let _ = (x, y, width, height);
   }
   ```
   and add `set_maximize_button_rect` to the `generate_handler!` list.
2. In `setup`, bind the built window and install the sink:
   ```rust
            let window = WebviewWindowBuilder::from_config(app.handle(), &config)?
                .on_new_window(|_, _| NewWindowResponse::Deny)
                .enable_clipboard_access()
                .build()?;
            // Snap layouts add a flyout to the maximize button; they are not
            // needed for the button to work. If the sink cannot be created,
            // a click on the HTML button still maximizes through the window
            // API, only without the flyout.
            #[cfg(windows)]
            if let Ok(hwnd) = window.hwnd() {
                let _ = jojo_app::snap::install(app.handle(), hwnd.0 as isize);
            }
            #[cfg(not(windows))]
            drop(window);
   ```

- [ ] **Step 8: Report the button's rectangle and draw the hover state**

Replace `app/src/ui/WindowControls.svelte`:
```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { toPhysicalRect } from "../lib/window/snap";

  const win = getCurrentWindow();
  let maximize: HTMLButtonElement;
  let hovered = $state(false);

  onMount(() => {
    // The snap-layout input sink (app/src-tauri/src/snap.rs) sits over this
    // button and receives its mouse input. Its position is sent whenever it
    // can change, and its hover state comes back as an event.
    const report = () => {
      const rect = toPhysicalRect(maximize.getBoundingClientRect(), window.devicePixelRatio);
      void invoke("set_maximize_button_rect", { ...rect }).catch(() => {});
    };
    const observer = new ResizeObserver(report);
    observer.observe(maximize);
    window.addEventListener("resize", report);
    report();
    const unlisten = listen<boolean>("maximize-hover", (event) => {
      hovered = event.payload;
    });
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", report);
      void unlisten.then((stop) => stop());
    };
  });
</script>

<div class="controls">
  <button aria-label="Minimize" onclick={() => win.minimize()}>&#xE921;</button>
  <!-- Mouse input goes to the input sink above this button; keyboard activation (Tab, then Enter) reaches the button. -->
  <button bind:this={maximize} class:hover={hovered} aria-label="Maximize" onclick={() => win.toggleMaximize()}>&#xE922;</button>
  <button aria-label="Close" class="close" onclick={() => win.close()}>&#xE8BB;</button>
</div>

<style>
  .controls {
    display: flex;
    align-self: stretch;
  }
  button {
    width: 46px;
    font-family: "Segoe Fluent Icons", "Segoe MDL2 Assets";
    font-size: 10px;
    color: var(--jojo-fg2);
  }
  button:hover,
  button.hover {
    background: var(--jojo-sel);
  }
  .close:hover {
    background: var(--jojo-danger);
    color: var(--jojo-fg);
  }
</style>
```

- [ ] **Step 9: Type-check, lint, test and build**

Run: `pnpm --dir app check`, `pnpm --dir app lint:colors`, `pnpm --dir app test`, `cargo clippy -p jojo-app --all-targets --jobs 4`, `cargo test -p jojo-app --jobs 4`
Expected: 0 errors; no color literals; all tests pass; no clippy warnings. (`cargo test -p jojo-app` also runs `tests/connection.rs`, which needs `cargo build -p jojod --jobs 4` first.)

- [ ] **Step 10: Check snap layouts by hand**

Run `pnpm --dir app tauri dev` from the Bash tool or a desktop terminal. Check:
1. Resting the pointer on the maximize button shows the Windows 11 snap layout flyout; choosing a zone snaps the window.
2. The button's background highlights while the pointer is over it and clears when it leaves.
3. Clicking it maximizes; clicking again restores. Win+Up / Win+Down still work.
4. After moving the window to a monitor with a different scale factor, the flyout still appears over the button.
5. Tab to the maximize button and press Enter: the window maximizes.
If the flyout does not appear, check Windows Settings ▸ System ▸ Multitasking ▸ "Show snap layouts when I hover over a window's maximize button" is on before investigating the sink.

- [ ] **Step 11: Commit**

```bash
git add app/src-tauri/src/snap.rs app/src-tauri/src/lib.rs app/src-tauri/src/main.rs app/src-tauri/build.rs app/src-tauri/windows-app-manifest.xml app/src-tauri/Cargo.toml app/src/lib/window app/src/ui/WindowControls.svelte
git commit -m "feat(app): Windows 11 snap layouts on the custom maximize button"
```

---

### Task 14: Retry during connect, and a distinct first-launch banner

**Files:**
- Modify: `app/src-tauri/src/connection.rs`, `app/src/lib/state/app.svelte.ts`, `app/src/ui/Banner.svelte`
- Create: `app/src/lib/state/banner.ts`
- Test: `app/src-tauri/src/connection.rs` (tests module), `app/src/lib/state/banner.test.ts`

**Interfaces:**
- Consumes: `Bridge::{restart_generation, wait_for_restart_after, request_restart, emit_status}`, `connect_or_start`, `ConnectError` (Plan 2b).
- Produces: `jojo_app::connection::run_with<F, Fut>(connect: F, bridge: Arc<Bridge>)` where `F: FnMut() -> Fut` and `Fut: Future<Output = Result<Client, ConnectError>>`; `run` delegates to it. TypeScript `interface BannerView { text: string; action: string | null }`, `connectionBanner(status: ConnectionStatus, everConnected: boolean): BannerView | null`; `app.everConnected: boolean`.

Carry-over 3.3 item 1: `connection::run` read the restart generation (`since`) **after** `connect_or_start` returned, and `connect_or_start` can take up to its start deadline (about 10 s) when it starts a daemon. A Retry click during that time bumped the generation before `since` was read, so the backoff wait that followed a failure did not see it. `since` is now read at the top of each loop iteration, before connecting, and the Elevated, Mismatch and Io branches wait against it. The branch after a short-lived connection keeps reading its own `since` after `serve()` returns, as before: `serve()` handles restarts requested while it runs (carry-over 3.3 item 2 about that branch's backoff stays open).

Carry-over 3.3 item 7: before any connection has succeeded, "Disconnected from jojod … Sessions keep running" is wrong (nothing was ever connected). The frontend records `everConnected` when the first `connected` status arrives; until then a `reconnecting` status reads "Starting jojod (attempt N): <reason>." and a `failed` status reads "jojod did not start: <reason>. Logs are in …". The backend's `ConnectionStatus` is unchanged, so no generated type changes.

- [ ] **Step 1: Write the failing tests**

Add to the tests module in `app/src-tauri/src/connection.rs`:
```rust
    #[tokio::test(flavor = "multi_thread")]
    async fn a_retry_clicked_while_connecting_skips_the_backoff() {
        let bridge = Arc::new(Bridge::default());
        let starts = Arc::new(std::sync::Mutex::new(Vec::<Instant>::new()));
        let connect = {
            let starts = starts.clone();
            let bridge = bridge.clone();
            move || {
                let starts = starts.clone();
                let bridge = bridge.clone();
                async move {
                    let first = {
                        let mut starts = starts.lock().unwrap();
                        starts.push(Instant::now());
                        starts.len() == 1
                    };
                    if first {
                        // The user clicks Retry while this attempt is still running.
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        bridge.request_restart();
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    Err::<Client, _>(ConnectError::Io(std::io::Error::other("no daemon")))
                }
            }
        };
        let task = tokio::spawn(run_with(connect, bridge.clone()));
        let deadline = Instant::now() + Duration::from_secs(5);
        while starts.lock().unwrap().len() < 2 && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        task.abort();
        let starts = starts.lock().unwrap();
        assert!(starts.len() >= 2, "the loop never retried");
        // With the click lost, the loop first sleeps backoff(1) = 1 s.
        let gap = starts[1] - starts[0];
        assert!(gap < Duration::from_millis(700), "the second attempt started {gap:?} after the first");
    }
```

`app/src/lib/state/banner.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { connectionBanner } from "./banner";

const LOG = String.raw`%LOCALAPPDATA%\Jojo\logs\jojod.<date>.log`;

describe("connectionBanner", () => {
  it("shows nothing while connected", () => {
    expect(connectionBanner({ state: "connected", jojod_version: "0.1.0" }, true)).toBeNull();
  });

  it("says jojod is starting until the first connection has succeeded", () => {
    expect(connectionBanner({ state: "starting" }, false)).toEqual({ text: "Starting jojod…", action: null });
    expect(connectionBanner({ state: "reconnecting", attempt: 3, message: "pipe not found" }, false)).toEqual({
      text: "Starting jojod (attempt 3): pipe not found.",
      action: null,
    });
    expect(connectionBanner({ state: "failed", message: "start timed out" }, false)).toEqual({
      text: `jojod did not start: start timed out. Logs are in ${LOG}.`,
      action: "Retry",
    });
  });

  it("says sessions keep running only after a connection was lost", () => {
    expect(connectionBanner({ state: "reconnecting", attempt: 2, message: "pipe closed" }, true)).toEqual({
      text: "Disconnected from jojod: pipe closed. Reconnecting (attempt 2). Sessions keep running.",
      action: null,
    });
    expect(connectionBanner({ state: "failed", message: "pipe closed" }, true)).toEqual({
      text: `Cannot reach jojod: pipe closed. Logs are in ${LOG}.`,
      action: "Retry",
    });
  });

  it("keeps the elevated and version mismatch banners", () => {
    expect(connectionBanner({ state: "elevated" }, false)?.text).toContain("running as administrator");
    expect(connectionBanner({ state: "version_mismatch", message: "jojod speaks protocol 1" }, true)).toEqual({
      text: "jojod speaks protocol 1.",
      action: "Restart jojod (ends all running sessions)",
    });
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jojo-app --jobs 4 --lib connection::` and `pnpm --dir app exec vitest run src/lib/state/banner.test.ts`
Expected: cargo FAIL to compile with "cannot find function `run_with`"; vitest FAIL with "Failed to resolve import "./banner"".

- [ ] **Step 3: Inject the connect step and read `since` first**

In `app/src-tauri/src/connection.rs`, replace `pub async fn run(...)` and its body with:
```rust
/// Runs until the task is aborted.
pub async fn run(paths: DaemonPaths, bridge: Arc<Bridge>) {
    let paths = &paths;
    run_with(move || connect_or_start(paths), bridge).await
}

/// The connection loop, with the connect step passed in so tests can drive
/// the loop without starting a daemon.
pub async fn run_with<F, Fut>(mut connect: F, bridge: Arc<Bridge>)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<Client, ConnectError>>,
{
    let mut attempt: u32 = 0;
    loop {
        // Read before connecting: connect_or_start can take up to its start
        // deadline (about 10 s) while it starts a daemon, and a Retry or
        // Restart clicked during that time must cut short the wait that
        // follows a failure instead of being lost (Plan 2b carry-over 3.3
        // item 1).
        let since = bridge.restart_generation();
        match connect().await {
            Ok(client) => {
                let started = Instant::now();
                let reason = serve(client, &bridge).await;
                bridge.set_outbound(None);
                if started.elapsed() < SHORT_CONNECTION {
                    attempt += 1;
                    let message = reason;
                    bridge.emit_status(if attempt >= FAILED_AFTER {
                        ConnectionStatus::Failed { message }
                    } else {
                        ConnectionStatus::Reconnecting { attempt, message }
                    });
                    // serve() itself reacts to restarts requested while it
                    // ran, so this wait counts only restarts requested from
                    // here on.
                    let since = bridge.restart_generation();
                    tokio::select! {
                        _ = tokio::time::sleep(backoff(attempt)) => {}
                        _ = bridge.wait_for_restart_after(since) => { attempt = 0; }
                    }
                } else {
                    attempt = 1;
                    bridge.emit_status(ConnectionStatus::Reconnecting { attempt, message: reason });
                }
            }
            Err(ConnectError::Elevated) => {
                bridge.emit_status(ConnectionStatus::Elevated);
                let _ = tokio::time::timeout(Duration::from_secs(10), bridge.wait_for_restart_after(since)).await;
            }
            Err(e @ ConnectError::Mismatch { .. }) => {
                bridge.emit_status(ConnectionStatus::VersionMismatch { message: e.to_string() });
                bridge.wait_for_restart_after(since).await;
                kill_daemons_async().await;
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Err(ConnectError::Io(e)) => {
                attempt += 1;
                let message = e.to_string();
                bridge.emit_status(if attempt >= FAILED_AFTER {
                    ConnectionStatus::Failed { message }
                } else {
                    ConnectionStatus::Reconnecting { attempt, message }
                });
                tokio::select! {
                    _ = tokio::time::sleep(backoff(attempt)) => {}
                    _ = bridge.wait_for_restart_after(since) => { attempt = 0; }
                }
            }
        }
    }
}
```

- [ ] **Step 4: Write the banner text function**

`app/src/lib/state/banner.ts`:
```ts
// The connection banner's text and button (Banner.svelte).
import type { ConnectionStatus } from "../backend/ConnectionStatus";

const LOG_PATH = String.raw`%LOCALAPPDATA%\Jojo\logs\jojod.<date>.log`;

export interface BannerView {
  text: string;
  /** Label of the button, which calls restart_daemon (it doubles as Retry); null for no button. */
  action: string | null;
}

/**
 * Banner for a connection status. Until the first connection has succeeded
 * (`everConnected` false) the app has never had sessions on screen, so the
 * texts say jojod is starting or did not start, rather than "Disconnected
 * … Sessions keep running" (Plan 2b carry-over 3.3 item 7).
 */
export function connectionBanner(status: ConnectionStatus, everConnected: boolean): BannerView | null {
  switch (status.state) {
    case "connected":
      return null;
    case "starting":
      return { text: "Starting jojod…", action: null };
    case "reconnecting":
      return everConnected
        ? { text: `Disconnected from jojod: ${status.message}. Reconnecting (attempt ${status.attempt}). Sessions keep running.`, action: null }
        : { text: `Starting jojod (attempt ${status.attempt}): ${status.message}.`, action: null };
    case "elevated":
      return {
        text: "Jojo is running as administrator, and jojod will not run elevated. Close Jojo and start it without administrator rights.",
        action: null,
      };
    case "version_mismatch":
      return { text: `${status.message}.`, action: "Restart jojod (ends all running sessions)" };
    case "failed":
      return everConnected
        ? { text: `Cannot reach jojod: ${status.message}. Logs are in ${LOG_PATH}.`, action: "Retry" }
        : { text: `jojod did not start: ${status.message}. Logs are in ${LOG_PATH}.`, action: "Retry" };
  }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p jojo-app --jobs 4 --lib connection::` and `pnpm --dir app exec vitest run src/lib/state/banner.test.ts`
Expected: PASS for both.

- [ ] **Step 6: Record the first connection and use the banner function**

In `app/src/lib/state/app.svelte.ts`:
1. Add to the `app` state, after `status`:
   ```ts
     /** True once any connection to jojod has succeeded in this app run. */
     everConnected: false,
   ```
2. In `connect()`'s `onStatus`, add `app.everConnected = true;` as the first line inside `if (status.state === "connected") {`.

Replace `app/src/ui/Banner.svelte`:
```svelte
<script lang="ts">
  import { app, dismissError, restartDaemon } from "../lib/state/app.svelte";
  import { connectionBanner } from "../lib/state/banner";

  const banner = $derived(connectionBanner(app.status, app.everConnected));
  const error = $derived(app.model.lastError);
</script>

{#if banner}
  <div class="banner">
    {banner.text}
    {#if banner.action}
      <button class="action" onclick={restartDaemon}>{banner.action}</button>
    {/if}
  </div>
{/if}
{#if error}
  <div class="banner error">
    {error.request}: {error.message}
    <button class="action" aria-label="Dismiss" onclick={dismissError}>×</button>
  </div>
{/if}

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    background: var(--jojo-banner);
    border-bottom: 1px solid var(--jojo-line);
    color: var(--jojo-fg2);
  }
  .banner.error {
    color: var(--jojo-error);
  }
  .action {
    margin-left: auto;
    padding: 2px 8px;
    border: 1px solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
  }
</style>
```

- [ ] **Step 7: Type-check, lint, test and build**

Run: `pnpm --dir app check`, `pnpm --dir app test`, `cargo clippy -p jojo-app --all-targets --jobs 4`, `cargo build -p jojod --jobs 4`, `cargo test -p jojo-app --jobs 4 -- --test-threads=1`
Expected: 0 errors; all tests pass (including `tests/connection.rs`, which starts the debug jojod); no clippy warnings.

- [ ] **Step 8: Commit**

```bash
git add app/src-tauri/src/connection.rs app/src/lib/state/banner.ts app/src/lib/state/banner.test.ts app/src/lib/state/app.svelte.ts app/src/ui/Banner.svelte
git commit -m "fix(app): honor Retry during connect; distinct banner before the first connection"
```

---

### Task 15: Manual checklist, installer and final verification

**Files:**
- Modify: `docs/manual-test-checklist.md`

**Interfaces:**
- Consumes: everything above.
- Produces: the Plan 3 section of the manual checklist and an NSIS installer built from this branch.

- [ ] **Step 1: Extend the manual checklist**

Append to `docs/manual-test-checklist.md`:
```markdown

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
```

- [ ] **Step 2: Final verification**

Run (each with a timeout of at least 10 minutes):
```bash
cargo build -p jojod --jobs 4
cargo test --workspace --jobs 4 -- --test-threads=1
cargo clippy --workspace --all-targets --jobs 4
pnpm --dir app test
pnpm --dir app check
pnpm --dir app lint:colors
CARGO_BUILD_JOBS=4 pwsh -NoProfile -File scripts/check-protocol.ps1
```
Expected: all pass; the last command prints `Generated types are up to date.`

- [ ] **Step 3: Build the installer**

From the Bash tool (not Claude Code's PowerShell tool), run:
```bash
CARGO_BUILD_JOBS=4 pnpm --dir app tauri build
```
`beforeBuildCommand` stages a release jojod first. Expected: an NSIS installer at `target/release/bundle/nsis/Jojo_0.1.0_x64-setup.exe`. Record its path and size (`ls -l target/release/bundle/nsis/`).

- [ ] **Step 4: Run the checklist**

Install the build, run the Plan 3 section of `docs/manual-test-checklist.md`, and record which items were observed. Items the agent cannot observe (it cannot see the window) are left for the user and listed as such.

- [ ] **Step 5: Commit**

```bash
git add docs/manual-test-checklist.md
git commit -m "docs: Plan 3 manual checklist"
```

---

## Self-Review Notes

- **Spec coverage.**
  - Spec 4, Layout row (app-side `layout.json`, keyed by machine id and project id, ordered tabs with name, split tree and focused pane, stale ids dropped, missing sessions in new tabs): Tasks 1 and 4; loaded before subscribing and reconciled in Task 9.
  - Spec 5.5 (attach visible panes, detach hidden ones): Task 9 (`PaneGrid` mounts only the active tab's panes; zoom hides the others through `layoutRects`), with one outstanding Attach per session from Task 6.
  - Spec 9.1 (`HTMAXBUTTON` snap layouts): Task 13.
  - Spec 9.3 rules 1-6: one pane per session (reconciliation, Task 4); per-project tabs (Task 9); split trees of 1-4 panes with draggable dividers (Tasks 2, 3, 9); header strip with badge and label and the 1 px accent outline (Tasks 9-10); sidebar click focuses (Task 9); close confirmation for `working`/`needs_you` (Tasks 5, 9).
  - Spec 9.4: menu component with submenus, keyboard and shortcuts (Tasks 8, 10); mouse reporting rule (Tasks 7, 10); pane menu (all items), tab menu, sidebar session menu (Tasks 8, 10); the 4-pane limit and its tooltip (Tasks 2, 8). Sidebar project and machine menus are deferred (Scope).
  - Spec 9.5 rules 1-5 and the `copyOnSelect` off rule: Tasks 7, 10, 11.
  - Spec 9.6: every shortcut except Ctrl+Shift+P and Ctrl+Shift+J (Scope): Tasks 7, 11.
  - Spec 9.10: `copyOnSelect` and the terminal font settings are read from `settings.json`; the settings screen is deferred (Scope).
  - Spec 11 item 6 (vitest for split-tree operations, presets, equalize, the 4-pane limit, shortcut resolution and the Ctrl+C rules): Tasks 2, 4, 7.
  - Carry-over 3.1 items 1-4: Tasks 13, 12, 12, 12. Item 5 was already done on `master` (`5084b84`). 3.2 items 1-3: Tasks 6, 6, 5. 3.3 items 1 and 7: Task 14.
- **Type consistency.** `LayoutNode`, `TabLayout`, `ProjectLayout`, `LayoutFile` and `SplitDirection` are generated in Task 1 and imported from `app/src/lib/backend/` everywhere; field names are snake_case (`session_id`, `active_tab`) in Rust, JSON and TypeScript. `splitPane` (tree) and `splitSession` (app action) are different functions on purpose: the tree function edits a tree, the action creates a session. `Placement` kinds (`tab`, `split`, `replace`) match between `workspace.ts`, `app.svelte.ts` and the tests. `PaneHandle.reattach(afterReconnect: boolean)` and `attachFailed()` match between `router.ts`, `paneStream.ts` and the router test. `TerminalControls` (Task 9) has exactly the methods `clipboard.ts`, `contextMenus.ts` and `App.svelte` call. `RenameTarget.where` is `"pane" | "sidebar"` in `ui.svelte.ts`, `PaneView.svelte`, `Sidebar.svelte` and `contextMenus.ts`. `LAYOUT_VERSION` is 1 in both `layout.rs` and `workspace.ts`.
- **Placeholder scan.** No step defers code; the only free-form values are the session ids a tester substitutes into `layout.json` during manual checks.
- **Verification limits.** Modules that import Svelte rune modules (`app.svelte.ts`, `actions.ts`, `contextMenus.ts`, `clipboard.ts`, `links.ts`) are not unit-tested, because `app/vitest.config.ts` runs without the Svelte compiler and spec 1.2 item 8 excludes automated UI tests; every decision they make is delegated to a tested pure function, and each UI task ends with manual checks.
- **Pre-check of the plan's code.** Before this plan was committed, its code blocks were applied to a scratch worktree of `5084b84` (Tasks 1-14 in order) and checked there: `cargo clippy -p jojo-app --all-targets` reported no warnings, `cargo test -p jojo-app --lib --test bindings` passed (29 + 1 tests, including the generated TypeScript shown in Task 1), `pnpm check` (svelte-check) reported 0 errors and 0 warnings, `lint:colors` passed, `vitest run` passed (27 files, 154 tests) and `vite build` succeeded. That check found and fixed two issues now reflected above: `snap::install` takes the HWND as `isize` (clippy `not_unsafe_ptr_arg_deref` rejects a raw pointer argument on a safe public function), and the generated `LayoutFile.projects` type is `{ [key in string]: ProjectLayout }`. The manual checks were not run.
