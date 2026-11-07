# Jojo Plan 2b: App Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `jojo.exe`, the Tauri 2 window: it starts and connects to the local `jojod`, shows This PC's projects and sessions in a Neon Glass sidebar, opens each session in a tab with an xterm.js pane that reattaches with a snapshot, and lets you add projects and start Claude or shell sessions.

**Architecture:** The Tauri app lives in `app/` (Svelte 5 + TypeScript frontend built with Vite) and `app/src-tauri/` (Rust backend, workspace member `jojo-app`). The Rust backend owns the single connection to `jojod` (it reuses `jojod::client` and `jojod::transport`), starts the daemon when it is not running, and relays traffic. Daemon frames travel to the frontend over one Tauri `Channel` as raw bytes (a one-byte envelope kind, then the frame body); the frontend sends frame bodies back with a raw-body `invoke`. The frontend decodes frames itself with the ts-rs generated types, keeps an event-sourced model, and mounts one xterm.js instance for the active tab only.

**Tech Stack:** Rust 1.99, Tauri 2 (`tauri` 2.x, `tauri-build` 2.x, `tauri-plugin-single-instance` 2.x, `tauri-plugin-dialog` 2.x), ts-rs 12; Node 22, pnpm 10, Svelte 5.57, `@sveltejs/vite-plugin-svelte` 7.3, Vite 8.3, TypeScript 5 (not 7), svelte-check 4.7, vitest 5.1, `@tauri-apps/api` 2.12, `@tauri-apps/plugin-dialog` 2, `@tauri-apps/cli` 2.12, `@xterm/xterm` 6.0.0 with `addon-fit` 0.11.0, `addon-webgl` 0.19.0, `addon-unicode11` 0.9.0, `addon-web-links` 0.12.0, `@xterm/headless` 6.0.0 (tests), `@fontsource/geist-sans` 5.3.0, `@fontsource/geist-mono` 5.3.0.

**Inputs:** Spec `docs/superpowers/specs/2026-11-06-jojo-design.md` (sections 8.1, 9.1, 9.2, 9.7–9.10, 10). Carry-overs `docs/superpowers/plans/2026-11-06-jojo-plan-1-carryover.md` and `docs/superpowers/plans/2026-11-07-jojo-plan-2a-carryover.md`. Approved mockup (layout A, Neon Glass): `.superpowers/brainstorm/36603-1793981075/content/layout.html` (git-ignored; values are copied into the theme file in Task 6).

## Scope

In scope: daemon adjustments the app needs; the Tauri scaffold; daemon auto-start and connection; the frontend protocol layer; the theme system; the terminal pane; the window UI (layout A with sidebar, tab row, window controls, status bar, banner); staging the daemon binaries for `tauri dev` and `tauri build`.

Deferred, with reasons:

| Item | Moves to | Reason |
|---|---|---|
| Windows 11 snap layouts on the custom maximize button (`HTMAXBUTTON` hit-testing, spec 9.1) | Plan 3 | Needs a native window subclass; Plan 3 already owns window chrome interaction (menus, shortcuts) |
| Split panes, `layout.json`, right-click menus, switcher, copy on select | Plan 3 | Roadmap |
| Confirm-before-close for `working` / `needs_you` sessions | Plan 4 | Those states do not exist until hooks arrive |
| `spawn_blocking` / async `Daemon::handle`, cap on exited grids | Plan 3 | With one pane per tab, at most one attach is in flight; Plan 3's splits make several concurrent attaches normal |
| A Claude Code recording for the snapshot tests | Plan 3 | Needs a "save raw output" action in the app |

In Plan 2b, each tab shows exactly one session, and a project's tabs are its sessions in creation order (no layout file).

## Global Constraints

- All Plan 1 and Plan 2a constraints still apply (names, paths, frame format, `PROTOCOL_VERSION = 2`, session size bounds 2–1000 × 1–1000, US spelling, the commit trailer naming the authoring model).
- The window is frameless (`decorations: false`), 1280 × 800 by default, minimum 720 × 420, label `main`, product name `Jojo`, binary `jojo.exe`, identifier `com.mcgstrategic.jojo`.
- Daemon install folder: `%LOCALAPPDATA%\Jojo\bin` holding `jojod.exe`, `conpty.dll`, `OpenConsole.exe`. Daemon start flags: `DETACHED_PROCESS | CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB` (retry without `CREATE_BREAKAWAY_FROM_JOB` if Windows refuses it).
- Settings file `%APPDATA%\Jojo\settings.json`, keys and defaults exactly as spec 9.10: `theme "neon-glass"`, `terminalFontFamily "Geist Mono"`, `terminalFontSize 13`, `terminalLineHeight 1.25`, `scrollbackLines 10000`, `copyOnSelect true`, `toasts true`, `defaultShell "pwsh"`.
- xterm.js: Unicode 11 widths active, `scrollback` at least 10,000, query replies suppressed (DSR, DA1, DA2, DECRQM ANSI and private).
- Every color, font, radius and glow in `.svelte` files comes from `var(--jojo-*)`; a lint check fails on color literals in `.svelte` files outside `app/src/themes/`.
- Envelope from backend to frontend: byte 0 is the kind (`0` = daemon frame body `[u8 type][payload]`, `1` = UTF-8 JSON `ConnectionStatus`), then the body.
- Keystrokes sent while disconnected are refused, not queued (spec 8.3 rule 4).
- Never start jojod from Claude Code's PowerShell tool (it inherits `NO_COLOR=1`).
- Generated TypeScript lives only in `app/src/lib/protocol/` and `app/src/lib/backend/`; nothing hand-written goes in those folders.

## File Structure

```
.gitattributes                         # + app/src/lib/backend/*.ts eol=lf
Cargo.toml                             # + member app/src-tauri; tauri deps
crates/jojod/src/{client,server,session,daemon,client_queue,main}.rs   # Task 1
scripts/check-protocol.ps1             # covers both generated folders, detects stale files
scripts/stage-daemon.ps1               # builds jojod, verifies ConPTY, copies into app/src-tauri/resources/daemon
scripts/make-icon.ps1                  # draws app-icon.png
app/
  package.json  pnpm-lock.yaml  tsconfig.json  vite.config.ts  vitest.config.ts
  svelte.config.js  index.html  .prettierignore  app-icon.png
  scripts/lint-colors.mjs  scripts/lint-colors.test.ts
  src/
    main.ts  app.css  App.svelte
    themes/neon-glass.json
    lib/protocol/*.ts                  # generated (Plan 2a)
    lib/backend/{ConnectionStatus,Settings}.ts   # generated (Task 4)
    lib/wire/{ulid,frame,bridge}.ts (+ .test.ts)
    lib/state/{reducer,views,elapsed}.ts (+ .test.ts)  app.svelte.ts
    lib/terminal/{queries,attachGate,size,router}.ts (+ .test.ts)
    lib/theme.ts (+ theme.test.ts)
    ui/{Sidebar,TabBar,WindowControls,StatusBar,Banner,TerminalPane}.svelte
  src-tauri/
    Cargo.toml  build.rs  tauri.conf.json  capabilities/default.json  icons/*
    resources/daemon/.gitkeep
    src/{main,bridge,connection,daemon_manager,settings}.rs
    tests/{bindings,connection}.rs
docs/manual-test-checklist.md          # + app section
```

---

### Task 1: Daemon adjustments for the app

**Files:**
- Modify: `crates/jojod/src/client.rs`, `crates/jojod/src/server.rs`, `crates/jojod/src/client_queue.rs`, `crates/jojod/src/session.rs`, `crates/jojod/src/daemon.rs`, `crates/jojod/src/main.rs`
- Create: `crates/jojod/tests/env_isolation.rs`
- Test: `crates/jojod/src/client.rs` (tests module), `crates/jojod/src/client_queue.rs` (tests module), `crates/jojod/tests/daemon_roundtrip.rs`

**Interfaces:**
- Consumes: Plan 2a code on `master`.
- Produces (used by Tasks 3–4):
  - `jojod::client::HELLO_TIMEOUT: Duration` (5 s).
  - `jojod::client::ProtocolMismatch { pub daemon: u32 }` (an `std::error::Error`) and `jojod::client::is_protocol_mismatch(&io::Error) -> Option<u32>`.
  - `Client::daemon_version(&self) -> &str`; `Client::into_parts(self) -> (BoxRead, BoxWrite)`.
  - `client_queue::MAX_CLIENT_TOTAL_BYTES` (64 MiB) and `ClientTx::overflowed(&self) -> bool`.
  - Sessions never inherit `CLAUDECODE`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/jojod/src/client.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use jojo_proto::{write_frame, ControlMsg, Frame};

    #[tokio::test]
    async fn handshake_times_out_when_the_daemon_never_says_hello() {
        let (client_side, _daemon_side) = tokio::io::duplex(1024);
        let (r, w) = tokio::io::split(client_side);
        let started = std::time::Instant::now();
        let err = Client::handshake(Box::new(r), Box::new(w), Duration::from_millis(200)).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn handshake_reports_a_protocol_mismatch() {
        let (client_side, daemon_side) = tokio::io::duplex(1024);
        let (r, w) = tokio::io::split(client_side);
        let (_dr, mut dw) = tokio::io::split(daemon_side);
        write_frame(&mut dw, &Frame::Control(ControlMsg::Hello { protocol_version: 1, jojod_version: "0.0.1".into() }))
            .await
            .unwrap();
        let err = Client::handshake(Box::new(r), Box::new(w), HELLO_TIMEOUT).await.unwrap_err();
        assert_eq!(is_protocol_mismatch(&err), Some(1));
    }

    #[tokio::test]
    async fn handshake_keeps_the_daemon_version() {
        let (client_side, daemon_side) = tokio::io::duplex(1024);
        let (r, w) = tokio::io::split(client_side);
        let (_dr, mut dw) = tokio::io::split(daemon_side);
        write_frame(
            &mut dw,
            &Frame::Control(ControlMsg::Hello { protocol_version: jojo_proto::PROTOCOL_VERSION, jojod_version: "9.9.9".into() }),
        )
        .await
        .unwrap();
        let client = Client::handshake(Box::new(r), Box::new(w), HELLO_TIMEOUT).await.unwrap();
        assert_eq!(client.daemon_version(), "9.9.9");
    }
}
```

Add to the tests module of `crates/jojod/src/client_queue.rs`:
```rust
    #[tokio::test]
    async fn total_cap_counts_every_frame_and_marks_overflow() {
        let (tx, _rx) = client_queue_with_total(1_000_000, 300);
        assert!(tx.send(Frame::Snapshot { session_id: SessionId::new(), bytes: vec![0; 200] }));
        assert!(!tx.overflowed());
        assert!(!tx.send(Frame::Snapshot { session_id: SessionId::new(), bytes: vec![0; 200] }));
        assert!(tx.overflowed(), "a refused frame past the total cap marks the client overflowed");
    }
```

Create `crates/jojod/tests/env_isolation.rs` (its own binary, because it changes the process environment):
```rust
//! Sessions must not inherit CLAUDECODE: a daemon started from inside a
//! Claude Code shell would otherwise mark every session as nested inside
//! Claude Code.

use jojo_grid::TermGrid;
use jojo_proto::{Frame, SessionId};
use jojod::client_queue::{client_queue, MAX_CLIENT_QUEUE_BYTES};
use jojod::session::{Session, SpawnSpec};
use std::time::{Duration, Instant};

#[test]
fn sessions_do_not_inherit_claudecode() {
    // SAFETY: this test binary is single-threaded at this point and owns its environment.
    unsafe { std::env::set_var("CLAUDECODE", "1") };
    #[cfg(windows)]
    let (program, args) = ("cmd.exe", vec!["/d".to_string(), "/k".to_string()]);
    #[cfg(unix)]
    let (program, args) = ("sh", vec![]);
    let session = Session::spawn(
        SessionId::new(),
        SpawnSpec {
            program: program.into(),
            args,
            cwd: std::env::temp_dir(),
            env: vec![],
            cols: 80,
            rows: 24,
            scrollback: 100,
        },
        |_| {},
    )
    .unwrap();
    let (tx, mut rx) = client_queue(MAX_CLIENT_QUEUE_BYTES);
    session.attach(1, tx, 80, 24);
    #[cfg(windows)]
    session.write(b"echo [%CLAUDECODE%]\r".to_vec());
    #[cfg(unix)]
    session.write(b"echo \"[${CLAUDECODE-unset}]\"\n".to_vec());
    let mut grid = TermGrid::new(80, 24, 100);
    let deadline = Instant::now() + Duration::from_secs(15);
    #[cfg(windows)]
    let unset = "[%CLAUDECODE%]";
    #[cfg(unix)]
    let unset = "[unset]";
    loop {
        while let Some(frame) = rx.try_recv() {
            if let Frame::Output { bytes, .. } | Frame::Snapshot { bytes, .. } = frame {
                grid.feed(&bytes);
            }
        }
        let text = grid.visible_text();
        assert!(!text.lines().any(|l| l.trim() == "[1]"), "CLAUDECODE leaked into the session:\n{text}");
        if text.lines().any(|l| l.trim() == unset) {
            break;
        }
        assert!(Instant::now() < deadline, "no echo output:\n{text}");
        std::thread::sleep(Duration::from_millis(50));
    }
    session.kill();
}
```

Add to `crates/jojod/tests/daemon_roundtrip.rs`:
```rust
#[tokio::test(flavor = "multi_thread")]
async fn a_connection_that_never_says_hello_is_closed() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, server) = start(dir.path()).await;
    let (mut reader, _writer) = jojod::transport::connect(&endpoint).await.unwrap();
    // The daemon sends its Hello, then waits for ours; it must give up.
    let first = jojo_proto::read_frame(&mut reader).await.unwrap();
    assert!(matches!(first, Some(Frame::Control(ControlMsg::Hello { .. }))));
    let next = tokio::time::timeout(Duration::from_secs(10), jojo_proto::read_frame(&mut reader)).await;
    assert!(matches!(next, Ok(Ok(None)) | Ok(Err(_))), "daemon kept a silent connection open");
    let mut c = Client::connect(&endpoint).await.unwrap();
    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p jojod -- --test-threads=1`
Expected: compile errors (`Client::handshake`, `HELLO_TIMEOUT`, `is_protocol_mismatch`, `daemon_version`, `client_queue_with_total`, `overflowed` not found).

- [ ] **Step 3: Implement**

`crates/jojod/src/client.rs`:
- Add `jojod_version: String` to `Client`.
- Add:
```rust
/// How long either side waits for the other's Hello before giving up.
pub const HELLO_TIMEOUT: Duration = Duration::from_secs(5);

/// The daemon speaks a different protocol version than this client.
#[derive(Debug)]
pub struct ProtocolMismatch {
    pub daemon: u32,
}

impl std::fmt::Display for ProtocolMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "jojod speaks protocol {}, this client speaks {}", self.daemon, PROTOCOL_VERSION)
    }
}

impl std::error::Error for ProtocolMismatch {}

/// The daemon's protocol version when `err` is a version mismatch.
pub fn is_protocol_mismatch(err: &io::Error) -> Option<u32> {
    err.get_ref().and_then(|e| e.downcast_ref::<ProtocolMismatch>()).map(|m| m.daemon)
}
```
- Split `connect` into `connect` (opens the transport, then `Self::handshake(reader, writer, HELLO_TIMEOUT)`) and:
```rust
    /// Sends our Hello and waits up to `timeout` for the daemon's.
    pub async fn handshake(reader: BoxRead, writer: BoxWrite, timeout: Duration) -> io::Result<Client> {
        let mut client = Client { reader, writer, jojod_version: String::new() };
        client
            .send(Frame::Control(ControlMsg::Hello {
                protocol_version: PROTOCOL_VERSION,
                jojod_version: crate::VERSION.to_string(),
            }))
            .await?;
        let first = tokio::time::timeout(timeout, client.recv())
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "jojod did not say hello"))??;
        match first {
            Frame::Control(ControlMsg::Hello { protocol_version, jojod_version }) => {
                if protocol_version != PROTOCOL_VERSION {
                    return Err(io::Error::other(ProtocolMismatch { daemon: protocol_version }));
                }
                client.jojod_version = jojod_version;
                Ok(client)
            }
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unexpected first frame from jojod: {other:?}"),
            )),
        }
    }

    pub fn daemon_version(&self) -> &str {
        &self.jojod_version
    }

    /// The raw halves, for callers that read and write concurrently.
    pub fn into_parts(self) -> (BoxRead, BoxWrite) {
        (self.reader, self.writer)
    }
```
(Add `use std::time::Duration;` and `use crate::transport::{BoxRead, BoxWrite};` as needed.)

`crates/jojod/src/server.rs`: wrap the Hello `read_frame` in `tokio::time::timeout(crate::client::HELLO_TIMEOUT, ...)`; on timeout, log `tracing::warn!(client, "client did not say hello")`, drop `tx`, await the writer task and return.

`crates/jojod/src/client_queue.rs`:
- Add `pub const MAX_CLIENT_TOTAL_BYTES: usize = 64 * 1024 * 1024;`, a second counter `total: Arc<AtomicUsize>`, `total_limit: usize` and `overflow: Arc<AtomicBool>` on `ClientTx` (`total` shared with `ClientRx`).
- `client_queue(limit)` becomes `client_queue_with_total(limit, MAX_CLIENT_TOTAL_BYTES)`; add `pub fn client_queue_with_total(output_limit: usize, total_limit: usize) -> (ClientTx, ClientRx)`.
- In `send`, after the Output check, reserve `frame_len` (Output and Snapshot byte lengths; control frames count 256 bytes each) in `total` with the same exact compare-and-swap pattern; when it would exceed `total_limit`, set `overflow` and return false (undo the Output reservation if one was made). `recv`/`try_recv` subtract the same amount from `total`.
- Add `pub fn overflowed(&self) -> bool`.
- Module doc: the output limit detaches one session's stream; the total limit means the client has stopped reading altogether and the connection is closed.

`crates/jojod/src/server.rs`, in `serve_connection`:
- Wrap each `write_frame` in the writer task with `tokio::time::timeout(WRITE_TIMEOUT, ...)` (`const WRITE_TIMEOUT: Duration = Duration::from_secs(30);`); on timeout or error, break.
- Give the writer task a `Notify` (`writer_done`) it signals when its loop ends, and add `_ = writer_done.notified() => break,` to the request-loop `select!`.
- After `daemon.handle(...)`, when `tx.overflowed()` is true, log `tracing::warn!(client, "client stopped reading; closing")` and break.

`crates/jojod/src/session.rs`, in `Session::spawn` after building `cmd`: `cmd.env_remove("CLAUDECODE");` with a comment (the session is a fresh terminal, not a child of whatever Claude Code process may have started the daemon). If portable-pty 0.9.0's `CommandBuilder` names the method differently, find it in `~/.cargo/registry/src/*/portable-pty-0.9.0/src/cmdbuilder.rs` and record it.

`crates/jojod/src/daemon.rs`, `CloseSession`: take the session out of the live map into a local `Option<Arc<Session>>` while holding the locks (store, then sessions, as now), broadcast, release both locks, and only then drop the taken session (whose `Drop` kills the process) — so no lock is held across `kill()`.

`crates/jojod/src/main.rs`, at the start of the `Serve` arm on Windows, before anything else:
```rust
            #[cfg(windows)]
            {
                use windows_sys::Win32::System::LibraryLoader::{
                    SetDefaultDllDirectories, LOAD_LIBRARY_SEARCH_APPLICATION_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
                };
                // portable-pty loads "conpty.dll" by bare name. Restrict the search to
                // jojod's own folder and System32, so a missing sideloaded copy falls
                // back to the system ConPTY instead of a conpty.dll found on PATH or in
                // the current directory.
                // SAFETY: plain Win32 call with constant flags.
                let ok = unsafe { SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_APPLICATION_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32) };
                if ok == 0 {
                    eprintln!("jojod: SetDefaultDllDirectories failed: {}", std::io::Error::last_os_error());
                }
            }
```
(Add `windows-sys` to `crates/jojod`'s Windows dependencies if `main.rs` cannot see it; it is already a dependency.)

- [ ] **Step 4: Run the tests**

Run: `cargo test --workspace -- --test-threads=1` and `cargo clippy --workspace --all-targets`
Expected: all pass, no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat(jojod): hello timeouts, total queue cap, write timeout, CLAUDECODE isolation, safe DLL search"
```

---

### Task 2: Tauri app scaffold

**Files:**
- Create: everything under `app/` listed in File Structure except the `lib/` modules and `ui/` components (those come in later tasks), `scripts/make-icon.ps1`, `app/src-tauri/resources/daemon/.gitkeep`
- Modify: `Cargo.toml` (members), `.gitignore`, `.gitattributes`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `pnpm --dir app build`, `pnpm --dir app test`, `pnpm --dir app check` and `cargo build -p jojo-app` all succeed; `pnpm --dir app tauri dev` opens a frameless 1280 × 800 window showing "jojo". Package `jojo-app`, binary `jojo`.

- [ ] **Step 1: Create the frontend scaffold**

`app/package.json`:
```json
{
  "name": "jojo-app",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "check": "svelte-check --tsconfig ./tsconfig.json",
    "test": "vitest run",
    "lint:colors": "node scripts/lint-colors.mjs",
    "stage": "pwsh -NoProfile -File ../scripts/stage-daemon.ps1",
    "tauri": "tauri"
  },
  "dependencies": {
    "@fontsource/geist-mono": "5.3.0",
    "@fontsource/geist-sans": "5.3.0",
    "@tauri-apps/api": "^2.12.4",
    "@tauri-apps/plugin-dialog": "^2",
    "@xterm/addon-fit": "0.11.0",
    "@xterm/addon-unicode11": "0.9.0",
    "@xterm/addon-web-links": "0.12.0",
    "@xterm/addon-webgl": "0.19.0",
    "@xterm/xterm": "6.0.0"
  },
  "devDependencies": {
    "@sveltejs/vite-plugin-svelte": "^7.3.1",
    "@tauri-apps/cli": "^2.12.4",
    "@xterm/headless": "6.0.0",
    "svelte": "^5.57.3",
    "svelte-check": "^4.7.8",
    "typescript": "^5",
    "vite": "^8.3.6",
    "vitest": "^5.1.0"
  }
}
```
`app/tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "strict": true,
    "isolatedModules": true,
    "verbatimModuleSyntax": true,
    "resolveJsonModule": true,
    "skipLibCheck": true,
    "noEmit": true,
    "types": ["vite/client"]
  },
  "include": ["src/**/*.ts", "src/**/*.svelte", "scripts/**/*.ts"]
}
```
`app/vite.config.ts`:
```ts
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Tauri expects a fixed dev-server port and its own console output.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
});
```
`app/vitest.config.ts`:
```ts
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: { environment: "node", include: ["src/**/*.test.ts", "scripts/**/*.test.ts"] },
});
```
`app/svelte.config.js`:
```js
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

export default { preprocess: vitePreprocess() };
```
`app/.prettierignore`:
```
# Generated by ts-rs (one type per line); never reformat.
src/lib/protocol/
src/lib/backend/
```
`app/index.html`:
```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Jojo</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```
`app/src/main.ts`:
```ts
import { mount } from "svelte";
import App from "./App.svelte";

mount(App, { target: document.getElementById("app")! });
```
`app/src/App.svelte` (placeholder until Task 8):
```svelte
<main data-tauri-drag-region>jojo</main>
```
Add a trivial `app/src/lib/smoke.test.ts` so `vitest run` has a test until real ones exist:
```ts
import { expect, it } from "vitest";

it("runs", () => expect(1 + 1).toBe(2));
```
(Delete it in Task 5 once real tests exist.)

- [ ] **Step 2: Create the Rust backend scaffold**

Root `Cargo.toml`: add `"app/src-tauri"` to `members`, and to `[workspace.dependencies]`:
```toml
tauri = { version = "2", features = [] }
tauri-build = { version = "2", features = [] }
tauri-plugin-dialog = "2"
tauri-plugin-single-instance = "2"
jojod = { path = "crates/jojod" }
```
`app/src-tauri/Cargo.toml`:
```toml
[package]
name = "jojo-app"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "jojo"
path = "src/main.rs"

[build-dependencies]
tauri-build.workspace = true

[dependencies]
anyhow.workspace = true
jojo-proto.workspace = true
jojod.workspace = true
serde.workspace = true
serde_json.workspace = true
tauri.workspace = true
tauri-plugin-dialog.workspace = true
tauri-plugin-single-instance.workspace = true
tokio.workspace = true
ts-rs.workspace = true

[target.'cfg(windows)'.dependencies]
windows-sys.workspace = true

[dev-dependencies]
tempfile.workspace = true
```
`app/src-tauri/build.rs`:
```rust
fn main() {
    tauri_build::build()
}
```
`app/src-tauri/src/main.rs` (Task 4 replaces the body):
```rust
// Hide the console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("run Jojo");
}
```
`app/src-tauri/tauri.conf.json`:
```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Jojo",
  "mainBinaryName": "jojo",
  "version": "0.1.0",
  "identifier": "com.mcgstrategic.jojo",
  "build": {
    "beforeDevCommand": "pnpm dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "pnpm build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "Jojo",
        "width": 1280,
        "height": 800,
        "minWidth": 720,
        "minHeight": 420,
        "decorations": false,
        "backgroundColor": "#070a16"
      }
    ],
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": ["nsis"],
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.ico"],
    "resources": { "resources/daemon/*": "daemon/" }
  }
}
```
(If the Tauri CLI rejects `mainBinaryName` or `backgroundColor` in the installed version, remove the field and record it; the `[[bin]] name = "jojo"` already names the executable.)

`app/src-tauri/capabilities/default.json`:
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Permissions for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:window:allow-minimize",
    "core:window:allow-toggle-maximize",
    "core:window:allow-close",
    "core:window:allow-start-dragging",
    "dialog:allow-open"
  ]
}
```
Create the empty file `app/src-tauri/resources/daemon/.gitkeep`.

Append to `.gitignore`:
```
/app/node_modules/
/app/dist/
/app/src-tauri/gen/
/app/src-tauri/resources/daemon/*
!/app/src-tauri/resources/daemon/.gitkeep
```
Append to `.gitattributes`:
```
app/src/lib/backend/*.ts text eol=lf
```

- [ ] **Step 3: Generate the icon**

Create `scripts/make-icon.ps1`:
```powershell
# Draws a 1024x1024 app icon (violet-to-cyan disc with a lowercase j) into
# app/app-icon.png. Run from the repository root, then `pnpm --dir app tauri icon app-icon.png`.
Add-Type -AssemblyName System.Drawing
$size = 1024
$bmp = New-Object System.Drawing.Bitmap $size, $size
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = 'AntiAlias'
$g.TextRenderingHint = 'AntiAliasGridFit'
$g.Clear([System.Drawing.Color]::Transparent)
$rect = New-Object System.Drawing.Rectangle 32, 32, ($size - 64), ($size - 64)
$brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, ([System.Drawing.Color]::FromArgb(255, 155, 140, 255)), ([System.Drawing.Color]::FromArgb(255, 127, 212, 255)), 45
$g.FillEllipse($brush, $rect)
$font = New-Object System.Drawing.Font 'Segoe UI', 560, ([System.Drawing.FontStyle]::Bold), ([System.Drawing.GraphicsUnit]::Pixel)
$format = New-Object System.Drawing.StringFormat
$format.Alignment = 'Center'
$format.LineAlignment = 'Center'
$text = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 7, 10, 22))
$g.DrawString('j', $font, $text, (New-Object System.Drawing.RectangleF 0, -30, $size, $size), $format)
$bmp.Save((Join-Path (Get-Location) 'app/app-icon.png'), [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Output 'Wrote app/app-icon.png'
```
Run:
```powershell
pwsh -NoProfile -File scripts/make-icon.ps1
pnpm --dir app install
pnpm --dir app tauri icon app-icon.png
```
Expected: `app/src-tauri/icons/` contains `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.ico` (plus other sizes; commit them all).

- [ ] **Step 4: Verify the scaffold builds**

Run (each with a timeout):
```powershell
pnpm --dir app build
pnpm --dir app check
pnpm --dir app test
cargo build -p jojo-app
cargo clippy --workspace --all-targets
```
Expected: all succeed with no warnings. Debug builds do not embed `dist/`, so `cargo build` does not depend on `pnpm build`; if `tauri::generate_context!()` reports a missing `frontendDist`, run `pnpm --dir app build` first and record it.

Then, if a display is available, run `pnpm --dir app tauri dev` once: a frameless, dark 1280 × 800 window titled Jojo shows "jojo". Close it. If the agent cannot observe the window, record that this check is left to the manual checklist (Task 9).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock .gitignore .gitattributes scripts/make-icon.ps1 app
git commit -m "build: scaffold the Tauri 2 + Svelte 5 app"
```

---

### Task 3: Daemon manager (stage, start, connect)

**Files:**
- Create: `app/src-tauri/src/daemon_manager.rs`, `app/src-tauri/tests/connection.rs` (first test only; Task 4 adds more)
- Modify: `app/src-tauri/src/main.rs` (module declaration only)

**Interfaces:**
- Consumes: `jojod::client::{Client, is_protocol_mismatch}`, `jojod::paths::{Endpoint, default_endpoint}`, `jojod::pipe_security_is_elevated`.
- Produces (used by Task 4):
  - `pub struct DaemonPaths { pub source_dir: PathBuf, pub bin_dir: PathBuf, pub endpoint: Endpoint, pub serve_args: Vec<String> }` with `DaemonPaths::for_app(source_dir: PathBuf) -> DaemonPaths` (bin dir `%LOCALAPPDATA%\Jojo\bin`, default endpoint, args `["serve"]`).
  - `pub const DAEMON_FILES: [&str; 3]`.
  - `pub fn stage_binaries(source: &Path, dest: &Path) -> io::Result<bool>`.
  - `pub fn start_detached(paths: &DaemonPaths) -> io::Result<()>`.
  - `pub enum ConnectError { Elevated, Mismatch { daemon: u32 }, Io(io::Error) }` (with `Display`).
  - `pub async fn connect_or_start(paths: &DaemonPaths) -> Result<Client, ConnectError>`.
  - `pub fn kill_daemons() -> io::Result<()>`.

The module is a library module of the binary crate. To make it reachable from integration tests, add `app/src-tauri/src/lib.rs` declaring `pub mod daemon_manager;` (and later `bridge`, `connection`, `settings`), and have `main.rs` use `jojo_app::...`. In `Cargo.toml` add:
```toml
[lib]
name = "jojo_app"
path = "src/lib.rs"
```

- [ ] **Step 1: Write the failing tests**

Unit tests at the bottom of `app/src-tauri/src/daemon_manager.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, bytes: &[u8]) {
        std::fs::write(dir.join(name), bytes).unwrap();
    }

    #[test]
    fn stages_missing_and_changed_files_only() {
        let source = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        write(source.path(), "jojod.exe", b"v1");
        write(source.path(), "conpty.dll", b"dll");
        assert!(stage_binaries(source.path(), dest.path()).unwrap(), "first run copies");
        assert!(!stage_binaries(source.path(), dest.path()).unwrap(), "unchanged files are not copied");
        write(source.path(), "jojod.exe", b"v2");
        assert!(stage_binaries(source.path(), dest.path()).unwrap(), "a changed file is copied");
        assert_eq!(std::fs::read(dest.path().join("jojod.exe")).unwrap(), b"v2");
        assert!(!dest.path().join("OpenConsole.exe").exists(), "absent optional files are skipped");
    }

    #[test]
    fn staging_requires_jojod() {
        let source = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        let err = stage_binaries(source.path(), dest.path()).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }
}
```

`app/src-tauri/tests/connection.rs`:
```rust
//! Starts a real jojod (target/debug/jojod.exe) through the app's daemon
//! manager. Build it first: `cargo build -p jojod`.

use jojo_app::daemon_manager::{connect_or_start, DaemonPaths};
use jojo_proto::{ControlMsg, Frame};
use jojod::paths::Endpoint;
use std::path::PathBuf;

pub fn built_jojod_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug");
    dir.join("jojod.exe").exists().then_some(dir)
}

pub fn test_paths(source: PathBuf, scratch: &std::path::Path) -> DaemonPaths {
    let endpoint = Endpoint(format!(r"\\.\pipe\jojo-app-test-{}", jojo_proto::SessionId::new()));
    let state = scratch.join("state");
    DaemonPaths {
        source_dir: source,
        bin_dir: scratch.join("bin"),
        endpoint: endpoint.clone(),
        serve_args: vec![
            "serve".into(),
            "--endpoint".into(),
            endpoint.0.clone(),
            "--state-dir".into(),
            state.to_string_lossy().into_owned(),
        ],
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn starts_the_daemon_when_none_is_running() {
    let Some(source) = built_jojod_dir() else {
        eprintln!("skipped: run `cargo build -p jojod` first");
        return;
    };
    if jojod::pipe_security_is_elevated().unwrap_or(false) {
        eprintln!("skipped: jojod refuses to run elevated");
        return;
    }
    let scratch = tempfile::tempdir().unwrap();
    let paths = test_paths(source, scratch.path());
    let mut client = connect_or_start(&paths).await.expect("daemon started and connected");
    assert!(!client.daemon_version().is_empty());
    assert!(paths.bin_dir.join("jojod.exe").exists(), "binaries were staged");
    client.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo build -p jojod; cargo test -p jojo-app`
Expected: compile errors (module `daemon_manager` not found).

- [ ] **Step 3: Implement**

`app/src-tauri/src/lib.rs`:
```rust
//! Jojo app backend: daemon management, the connection to jojod, and the
//! bridge to the frontend.

pub mod daemon_manager;
```
`app/src-tauri/src/daemon_manager.rs`:
```rust
//! Finds, installs, starts and connects to the local jojod (spec section 8.1).

use jojod::client::{is_protocol_mismatch, Client};
use jojod::paths::{default_endpoint, Endpoint};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Files installed next to each other in the daemon folder. jojod.exe is
/// required; the ConPTY pair is optional (without it the system ConPTY is used).
pub const DAEMON_FILES: [&str; 3] = ["jojod.exe", "conpty.dll", "OpenConsole.exe"];

/// How long to wait for a freshly started daemon to accept connections.
const START_DEADLINE: Duration = Duration::from_secs(5);

const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

pub struct DaemonPaths {
    /// Where the app's bundled daemon files are (the `daemon` resource folder).
    pub source_dir: PathBuf,
    /// Where the daemon runs from (`%LOCALAPPDATA%\Jojo\bin`).
    pub bin_dir: PathBuf,
    pub endpoint: Endpoint,
    /// Arguments for jojod.exe (`["serve"]` in the app).
    pub serve_args: Vec<String>,
}

impl DaemonPaths {
    pub fn for_app(source_dir: PathBuf) -> DaemonPaths {
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        DaemonPaths {
            source_dir,
            bin_dir: local.join("Jojo").join("bin"),
            endpoint: default_endpoint(),
            serve_args: vec!["serve".into()],
        }
    }
}

#[derive(Debug)]
pub enum ConnectError {
    /// The app is elevated, and jojod refuses to run elevated.
    Elevated,
    /// A jojod with another protocol version is running.
    Mismatch { daemon: u32 },
    Io(io::Error),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectError::Elevated => f.write_str("Jojo is running as administrator, and jojod will not run elevated"),
            ConnectError::Mismatch { daemon } => write!(
                f,
                "the running jojod speaks protocol {daemon}; this Jojo needs protocol {}",
                jojo_proto::PROTOCOL_VERSION
            ),
            ConnectError::Io(e) => write!(f, "{e}"),
        }
    }
}

/// Copies each daemon file that is missing in `dest` or differs from
/// `source`. Returns whether anything was copied. A file that cannot be
/// replaced because the running daemon holds it open is skipped: the running
/// daemon keeps its version, and a protocol mismatch is reported on connect.
pub fn stage_binaries(source: &Path, dest: &Path) -> io::Result<bool> {
    if !source.join("jojod.exe").is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("jojod.exe not found in {}", source.display()),
        ));
    }
    std::fs::create_dir_all(dest)?;
    let mut copied = false;
    for name in DAEMON_FILES {
        let from = source.join(name);
        if !from.is_file() {
            continue;
        }
        let to = dest.join(name);
        if to.is_file() && std::fs::read(&to)? == std::fs::read(&from)? {
            continue;
        }
        match std::fs::copy(&from, &to) {
            Ok(_) => copied = true,
            // ERROR_SHARING_VIOLATION: the daemon is running from this file.
            Err(e) if e.raw_os_error() == Some(32) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(copied)
}

/// Starts jojod detached from the app, so closing the window never stops it.
pub fn start_detached(paths: &DaemonPaths) -> io::Result<()> {
    let exe = paths.bin_dir.join("jojod.exe");
    let spawn = |flags: u32| {
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(&paths.serve_args)
            .current_dir(&paths.bin_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(flags);
        }
        cmd.spawn().map(|_| ())
    };
    match spawn(DETACHED_PROCESS | CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB) {
        // ERROR_ACCESS_DENIED: the job the app runs in forbids breakaway.
        Err(e) if e.raw_os_error() == Some(5) => spawn(DETACHED_PROCESS | CREATE_NO_WINDOW),
        other => other,
    }
}

/// Connects to the running daemon, or installs and starts one and connects.
pub async fn connect_or_start(paths: &DaemonPaths) -> Result<Client, ConnectError> {
    match Client::connect(&paths.endpoint).await {
        Ok(client) => return Ok(client),
        Err(e) => {
            if let Some(daemon) = is_protocol_mismatch(&e) {
                return Err(ConnectError::Mismatch { daemon });
            }
        }
    }
    if jojod::pipe_security_is_elevated().unwrap_or(false) {
        return Err(ConnectError::Elevated);
    }
    stage_binaries(&paths.source_dir, &paths.bin_dir).map_err(ConnectError::Io)?;
    start_detached(paths).map_err(ConnectError::Io)?;
    let deadline = Instant::now() + START_DEADLINE;
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        match Client::connect(&paths.endpoint).await {
            Ok(client) => return Ok(client),
            Err(e) => {
                if let Some(daemon) = is_protocol_mismatch(&e) {
                    return Err(ConnectError::Mismatch { daemon });
                }
                if Instant::now() >= deadline {
                    return Err(ConnectError::Io(e));
                }
            }
        }
    }
}

/// Ends every jojod process of this user (used to replace a daemon that
/// speaks an older protocol; its sessions end).
pub fn kill_daemons() -> io::Result<()> {
    let mut cmd = std::process::Command::new("taskkill");
    cmd.args(["/IM", "jojod.exe", "/F"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.output().map(|_| ())
}
```
In `main.rs` keep the current body; it does not use the module yet.

- [ ] **Step 4: Run the tests**

Run: `cargo build -p jojod; cargo test -p jojo-app -- --nocapture` (timeout)
Expected: 2 unit tests and `starts_the_daemon_when_none_is_running` pass, with no "skipped" message. Then `cargo clippy --workspace --all-targets` is clean.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri
git commit -m "feat(app): stage, start and connect to the local jojod"
```

---

### Task 4: Connection loop and frontend bridge

**Files:**
- Create: `app/src-tauri/src/bridge.rs`, `app/src-tauri/src/connection.rs`, `app/src-tauri/src/settings.rs`, `app/src-tauri/tests/bindings.rs`, `app/src/lib/backend/*.ts` (generated)
- Modify: `app/src-tauri/src/lib.rs`, `app/src-tauri/src/main.rs`, `app/src-tauri/tests/connection.rs`, `scripts/check-protocol.ps1`

**Interfaces:**
- Consumes: Task 3 (`DaemonPaths`, `connect_or_start`, `kill_daemons`, `ConnectError`), Task 1 (`Client::into_parts`, `daemon_version`).
- Produces (used by Tasks 5 and 8):
  - `ConnectionStatus` (Rust + TS): `{ state: "starting" } | { state: "connected", jojod_version: string } | { state: "reconnecting", attempt: number, message: string } | { state: "elevated" } | { state: "version_mismatch", message: string } | { state: "failed", message: string }`.
  - `Settings` (Rust + TS, camelCase keys per Global Constraints).
  - Tauri commands: `subscribe(events: Channel)`, `send_frame(<raw body>)`, `restart_daemon()`, `get_settings() -> Settings`.
  - `bridge::envelope(kind: u8, body: &[u8]) -> Vec<u8>`; `ENVELOPE_FRAME = 0`, `ENVELOPE_STATUS = 1`.

- [ ] **Step 1: Write the failing tests**

Unit tests in `app/src-tauri/src/bridge.rs` (write the tests module first):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn collecting_bridge() -> (Bridge, Arc<Mutex<Vec<Vec<u8>>>>) {
        let bridge = Bridge::default();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        bridge.set_sink(Box::new(move |bytes| sink.lock().unwrap().push(bytes)));
        (bridge, seen)
    }

    #[test]
    fn envelope_prefixes_the_kind() {
        assert_eq!(envelope(ENVELOPE_FRAME, &[3, 9]), vec![0, 3, 9]);
    }

    #[test]
    fn subscribing_replays_the_latest_status() {
        let bridge = Bridge::default();
        bridge.emit_status(ConnectionStatus::Connected { jojod_version: "1.2.3".into() });
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        bridge.set_sink(Box::new(move |bytes| sink.lock().unwrap().push(bytes)));
        let first = seen.lock().unwrap()[0].clone();
        assert_eq!(first[0], ENVELOPE_STATUS);
        let json: serde_json::Value = serde_json::from_slice(&first[1..]).unwrap();
        assert_eq!(json["state"], "connected");
        assert_eq!(json["jojod_version"], "1.2.3");
    }

    #[test]
    fn sending_while_disconnected_is_refused() {
        let (bridge, _) = collecting_bridge();
        assert!(bridge.send(vec![1, b'{', b'}']).is_err());
    }
}
```
Unit tests in `app/src-tauri/src/settings.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(&dir.path().join("settings.json")), Settings::default());
    }

    #[test]
    fn partial_file_keeps_defaults_for_missing_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{ "terminalFontSize": 15 }"#).unwrap();
        let s = load(&path);
        assert_eq!(s.terminal_font_size, 15);
        assert_eq!(s.scrollback_lines, 10_000);
        assert_eq!(s.default_shell, "pwsh");
    }

    #[test]
    fn invalid_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ nope").unwrap();
        assert_eq!(load(&path), Settings::default());
    }
}
```
Unit test in `app/src-tauri/src/connection.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_to_thirty_seconds() {
        let secs: Vec<u64> = (1..=7).map(|a| backoff(a).as_secs()).collect();
        assert_eq!(secs, vec![1, 2, 4, 8, 16, 30, 30]);
    }
}
```
`app/src-tauri/tests/bindings.rs`:
```rust
//! Generates the TypeScript types for the app backend's own messages
//! (app/src/lib/backend). scripts/check-protocol.ps1 fails when they drift.

use jojo_app::bridge::ConnectionStatus;
use jojo_app::settings::Settings;
use std::path::PathBuf;
use ts_rs::{Config, TS};

#[test]
fn export_backend_bindings() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/lib/backend");
    let cfg = Config::new().with_out_dir(&dir);
    ConnectionStatus::export_all(&cfg).unwrap();
    Settings::export_all(&cfg).unwrap();
    let status = std::fs::read_to_string(dir.join("ConnectionStatus.ts")).unwrap();
    assert!(status.contains("\"state\": \"version_mismatch\""), "{status}");
    let settings = std::fs::read_to_string(dir.join("Settings.ts")).unwrap();
    assert!(settings.contains("terminalFontSize: number"), "{settings}");
    assert!(!settings.contains("bigint"), "{settings}");
}
```
Add to `app/src-tauri/tests/connection.rs`:
```rust
use jojo_app::bridge::{Bridge, ENVELOPE_FRAME, ENVELOPE_STATUS};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
async fn connection_loop_relays_frames_both_ways() {
    let Some(source) = built_jojod_dir() else {
        eprintln!("skipped: run `cargo build -p jojod` first");
        return;
    };
    if jojod::pipe_security_is_elevated().unwrap_or(false) {
        eprintln!("skipped: jojod refuses to run elevated");
        return;
    }
    let scratch = tempfile::tempdir().unwrap();
    let paths = test_paths(source, scratch.path());
    let endpoint = paths.endpoint.clone();
    let bridge = Arc::new(Bridge::default());
    let seen = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let sink = seen.clone();
    bridge.set_sink(Box::new(move |bytes| sink.lock().unwrap().push(bytes)));
    let task = tokio::spawn(jojo_app::connection::run(paths, bridge.clone()));

    let connected = wait_for(&seen, |e| e[0] == ENVELOPE_STATUS && e[1..].starts_with(br#"{"state":"connected""#)).await;
    assert!(connected, "never connected: {:?}", printable(&seen));

    let body = &Frame::Control(ControlMsg::ListProjects).encode()[4..];
    bridge.send(body.to_vec()).expect("connected bridge accepts frames");
    let projects = wait_for(&seen, |e| e[0] == ENVELOPE_FRAME && String::from_utf8_lossy(&e[2..]).contains(r#""type":"Projects""#)).await;
    assert!(projects, "no Projects reply: {:?}", printable(&seen));

    task.abort();
    let mut c = jojod::client::Client::connect(&endpoint).await.unwrap();
    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
}

async fn wait_for(seen: &Arc<Mutex<Vec<Vec<u8>>>>, pred: impl Fn(&Vec<u8>) -> bool) -> bool {
    for _ in 0..300 {
        if seen.lock().unwrap().iter().any(&pred) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

fn printable(seen: &Arc<Mutex<Vec<Vec<u8>>>>) -> Vec<String> {
    seen.lock().unwrap().iter().map(|e| format!("{}:{}", e[0], String::from_utf8_lossy(&e[1..]))).collect()
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p jojo-app`
Expected: compile errors (modules `bridge`, `connection`, `settings` not found).

- [ ] **Step 3: Implement the bridge and settings**

`app/src-tauri/src/bridge.rs`, above the tests:
```rust
//! Carries daemon frames and connection status to the frontend, and frame
//! bodies from the frontend to the daemon.
//!
//! Envelope sent to the frontend (one Tauri Channel message each): byte 0 is
//! the kind, the rest is the body. `ENVELOPE_FRAME`: a daemon frame without
//! its length prefix (`[u8 type][payload]`). `ENVELOPE_STATUS`: UTF-8 JSON
//! `ConnectionStatus`.

use serde::Serialize;
use std::sync::Mutex;
use tokio::sync::{mpsc, Notify};
use ts_rs::TS;

pub const ENVELOPE_FRAME: u8 = 0;
pub const ENVELOPE_STATUS: u8 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ConnectionStatus {
    Starting,
    Connected { jojod_version: String },
    Reconnecting { attempt: u32, message: String },
    Elevated,
    VersionMismatch { message: String },
    Failed { message: String },
}

pub fn envelope(kind: u8, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + body.len());
    out.push(kind);
    out.extend_from_slice(body);
    out
}

type Sink = Box<dyn Fn(Vec<u8>) + Send + Sync>;

pub struct Bridge {
    sink: Mutex<Option<Sink>>,
    last_status: Mutex<ConnectionStatus>,
    outbound: Mutex<Option<mpsc::UnboundedSender<Vec<u8>>>>,
    /// Signaled by the `restart_daemon` command.
    pub restart: Notify,
}

impl Default for Bridge {
    fn default() -> Self {
        Bridge {
            sink: Mutex::new(None),
            last_status: Mutex::new(ConnectionStatus::Starting),
            outbound: Mutex::new(None),
            restart: Notify::new(),
        }
    }
}

impl Bridge {
    /// Installs the frontend's channel and sends it the latest status, so a
    /// frontend that subscribes late still learns the connection state.
    pub fn set_sink(&self, sink: Sink) {
        let status = self.last_status.lock().unwrap().clone();
        sink(envelope(ENVELOPE_STATUS, &serde_json::to_vec(&status).expect("status serializes")));
        *self.sink.lock().unwrap() = Some(sink);
    }

    pub fn emit_status(&self, status: ConnectionStatus) {
        let bytes = envelope(ENVELOPE_STATUS, &serde_json::to_vec(&status).expect("status serializes"));
        *self.last_status.lock().unwrap() = status;
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink(bytes);
        }
    }

    pub fn emit_frame(&self, body: &[u8]) {
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink(envelope(ENVELOPE_FRAME, body));
        }
    }

    pub fn set_outbound(&self, tx: Option<mpsc::UnboundedSender<Vec<u8>>>) {
        *self.outbound.lock().unwrap() = tx;
    }

    /// Queues a frame body for the daemon. Refused while disconnected:
    /// keystrokes typed into a terminal the user cannot see must not be
    /// replayed later (spec section 8.3, rule 4).
    pub fn send(&self, body: Vec<u8>) -> Result<(), &'static str> {
        match self.outbound.lock().unwrap().as_ref() {
            Some(tx) if tx.send(body).is_ok() => Ok(()),
            _ => Err("not connected to jojod"),
        }
    }
}
```
`app/src-tauri/src/settings.rs`, above the tests:
```rust
//! %APPDATA%\Jojo\settings.json (spec section 9.10).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: String,
    pub terminal_font_family: String,
    pub terminal_font_size: u16,
    pub terminal_line_height: f32,
    pub scrollback_lines: u32,
    pub copy_on_select: bool,
    pub toasts: bool,
    pub default_shell: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "neon-glass".into(),
            terminal_font_family: "Geist Mono".into(),
            terminal_font_size: 13,
            terminal_line_height: 1.25,
            scrollback_lines: 10_000,
            copy_on_select: true,
            toasts: true,
            default_shell: "pwsh".into(),
        }
    }
}

pub fn default_path() -> PathBuf {
    let roaming = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    roaming.join("Jojo").join("settings.json")
}

/// Reads settings. A missing or unreadable file gives the defaults; keys
/// absent from the file keep their defaults.
pub fn load(path: &Path) -> Settings {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}
```

- [ ] **Step 4: Implement the connection loop**

`app/src-tauri/src/connection.rs`, above the tests:
```rust
//! Keeps the app connected to the local jojod and relays frames
//! (spec sections 8.1 and 10).

use crate::bridge::{Bridge, ConnectionStatus};
use crate::daemon_manager::{connect_or_start, kill_daemons, ConnectError, DaemonPaths};
use jojo_proto::read_frame;
use jojod::client::Client;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

/// Reconnect attempts before the status changes from reconnecting to failed
/// (retries continue either way).
const FAILED_AFTER: u32 = 10;

pub fn backoff(attempt: u32) -> Duration {
    let secs = 1u64 << attempt.saturating_sub(1).min(5);
    Duration::from_secs(secs.min(30))
}

/// Runs until the task is aborted.
pub async fn run(paths: DaemonPaths, bridge: Arc<Bridge>) {
    let mut attempt: u32 = 0;
    loop {
        match connect_or_start(&paths).await {
            Ok(client) => {
                attempt = 0;
                let reason = serve(client, &bridge).await;
                bridge.set_outbound(None);
                attempt += 1;
                bridge.emit_status(ConnectionStatus::Reconnecting { attempt, message: reason });
            }
            Err(ConnectError::Elevated) => {
                bridge.emit_status(ConnectionStatus::Elevated);
                let _ = tokio::time::timeout(Duration::from_secs(10), bridge.restart.notified()).await;
            }
            Err(e @ ConnectError::Mismatch { .. }) => {
                bridge.emit_status(ConnectionStatus::VersionMismatch { message: e.to_string() });
                bridge.restart.notified().await;
                let _ = kill_daemons();
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
                    _ = bridge.restart.notified() => { attempt = 0; }
                }
            }
        }
    }
}

/// Relays frames until the connection ends; returns why it ended.
async fn serve(client: Client, bridge: &Bridge) -> String {
    bridge.emit_status(ConnectionStatus::Connected { jojod_version: client.daemon_version().to_string() });
    let (mut reader, mut writer) = client.into_parts();
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    bridge.set_outbound(Some(tx));
    loop {
        tokio::select! {
            frame = read_frame(&mut reader) => match frame {
                Ok(Some(frame)) => {
                    let encoded = frame.encode();
                    bridge.emit_frame(&encoded[4..]);
                }
                Ok(None) => return "jojod closed the connection".into(),
                Err(e) => return format!("lost the connection to jojod: {e}"),
            },
            body = rx.recv() => {
                let Some(body) = body else { return "outbound queue closed".into() };
                let len = (body.len() as u32).to_le_bytes();
                let write = async {
                    writer.write_all(&len).await?;
                    writer.write_all(&body).await?;
                    writer.flush().await
                };
                if let Err(e) = write.await {
                    return format!("could not write to jojod: {e}");
                }
            }
            _ = bridge.restart.notified() => {
                let _ = kill_daemons();
                return "restarting jojod".into();
            }
        }
    }
}
```
`app/src-tauri/src/lib.rs`: declare `pub mod bridge; pub mod connection; pub mod daemon_manager; pub mod settings;`.

`app/src-tauri/src/main.rs`:
```rust
// Hide the console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use jojo_app::bridge::Bridge;
use jojo_app::daemon_manager::DaemonPaths;
use jojo_app::settings::{self, Settings};
use std::sync::Arc;
use tauri::ipc::{Channel, InvokeBody, InvokeResponseBody, Request};
use tauri::{Manager, State};

#[tauri::command]
fn subscribe(events: Channel<InvokeResponseBody>, bridge: State<'_, Arc<Bridge>>) {
    bridge.set_sink(Box::new(move |bytes| {
        let _ = events.send(InvokeResponseBody::Raw(bytes));
    }));
}

#[tauri::command]
fn send_frame(request: Request<'_>, bridge: State<'_, Arc<Bridge>>) -> Result<(), String> {
    let InvokeBody::Raw(body) = request.body() else {
        return Err("send_frame expects raw frame bytes".into());
    };
    bridge.send(body.clone()).map_err(str::to_owned)
}

#[tauri::command]
fn restart_daemon(bridge: State<'_, Arc<Bridge>>) {
    bridge.restart.notify_one();
}

#[tauri::command]
fn get_settings() -> Settings {
    settings::load(&settings::default_path())
}

fn main() {
    let bridge = Arc::new(Bridge::default());
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(bridge.clone())
        .invoke_handler(tauri::generate_handler![subscribe, send_frame, restart_daemon, get_settings])
        .setup(move |app| {
            let source = app.path().resource_dir()?.join("daemon");
            let paths = DaemonPaths::for_app(source);
            tauri::async_runtime::spawn(jojo_app::connection::run(paths, bridge.clone()));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("run Jojo");
}
```
If `Channel<InvokeResponseBody>` or `InvokeBody::Raw` differs in the resolved Tauri 2 version, check `https://docs.rs/tauri/<version>/tauri/ipc/` and adjust; record the change.

- [ ] **Step 5: Extend the drift check**

Replace `scripts/check-protocol.ps1` with:
```powershell
# Regenerates every ts-rs output folder and fails when the result differs
# from the committed files (spec section 6.1). Generated folders are emptied
# first, so a type that was removed in Rust shows up as a deleted file.
# Run from the repository root with cargo on PATH.
$ErrorActionPreference = 'Stop'
$folders = 'app/src/lib/protocol', 'app/src/lib/backend'
foreach ($folder in $folders) {
    Get-ChildItem $folder -Filter *.ts -ErrorAction SilentlyContinue | Remove-Item
}
cargo test -q -p jojo-proto --test bindings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo test -q -p jojo-app --test bindings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$changes = git status --porcelain -- $folders
if ($changes) {
    Write-Error "Generated types are out of date. Commit these changes:`n$changes"
    exit 1
}
Write-Output "Generated types are up to date."
```

- [ ] **Step 6: Run everything**

Run: `cargo build -p jojod; cargo test -p jojo-app -- --nocapture` (timeout), `cargo test --workspace -- --test-threads=1`, `cargo clippy --workspace --all-targets`, then commit the generated `app/src/lib/backend/*.ts` and run `pwsh -NoProfile -File scripts/check-protocol.ps1` (with `~/.cargo/bin` on `PATH`).
Expected: all pass with no "skipped" messages; the script prints `Generated types are up to date.`

- [ ] **Step 7: Commit**

```bash
git add app/src-tauri app/src/lib/backend scripts/check-protocol.ps1
git commit -m "feat(app): connection loop, frontend bridge, settings and backend bindings"
```

---

### Task 5: Frontend protocol layer and model

**Files:**
- Create: `app/src/lib/wire/{ulid,frame,bridge}.ts` and `.test.ts` for `ulid`, `frame`, `bridge`; `app/src/lib/state/{reducer,views,elapsed}.ts` and `.test.ts` for each
- Delete: `app/src/lib/smoke.test.ts`

**Interfaces:**
- Consumes: generated `ControlMsg`, `ProjectInfo`, `SessionInfo` (`app/src/lib/protocol`), `ConnectionStatus` (`app/src/lib/backend`).
- Produces (used by Tasks 7–8):
  - `ulid.ts`: `ulidToBytes(id: string): Uint8Array`, `bytesToUlid(bytes: Uint8Array): string`, `newUlid(): string`.
  - `frame.ts`: `type Frame = { kind: "control"; msg: ControlMsg } | { kind: "output" | "snapshot" | "input"; sessionId: string; bytes: Uint8Array }`; `decodeFrameBody(body: Uint8Array): Frame`; `encodeControl(msg: ControlMsg): Uint8Array`; `encodeInput(sessionId: string, bytes: Uint8Array): Uint8Array`.
  - `bridge.ts`: `interface BridgeHandlers { onStatus(s: ConnectionStatus): void; onFrame(f: Frame): void }`; `dispatchEnvelope(bytes: Uint8Array, h: BridgeHandlers): void`; `subscribe(h: BridgeHandlers): Promise<void>`; `sendControl(msg: ControlMsg): Promise<void>`; `sendInput(sessionId: string, bytes: Uint8Array): Promise<void>`.
  - `reducer.ts`: `interface Model { projects: Record<string, ProjectInfo>; sessions: Record<string, SessionInfo>; lastError: { request: string; message: string; session_id: string | null } | null }`; `emptyModel(): Model`; `applyControl(model: Model, msg: ControlMsg): Model`.
  - `views.ts`: `sessionsForProject(model: Model, projectId: string): SessionInfo[]` (creation order); `sortedProjects(model: Model): ProjectInfo[]` (by name).
  - `elapsed.ts`: `formatElapsed(ms: number): string`.

- [ ] **Step 1: Write the failing tests**

`app/src/lib/wire/ulid.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { bytesToUlid, newUlid, ulidToBytes } from "./ulid";

describe("ulid", () => {
  it("encodes the zero and maximum values", () => {
    expect(bytesToUlid(new Uint8Array(16))).toBe("00000000000000000000000000");
    expect(bytesToUlid(new Uint8Array(16).fill(0xff))).toBe("7ZZZZZZZZZZZZZZZZZZZZZZZZZ");
  });

  it("round-trips random values", () => {
    for (let i = 0; i < 100; i++) {
      const id = newUlid();
      expect(id).toMatch(/^[0-9A-HJKMNP-TV-Z]{26}$/);
      expect(bytesToUlid(ulidToBytes(id))).toBe(id);
    }
  });

  it("puts the timestamp in the first 48 bits", () => {
    const before = Date.now();
    const bytes = ulidToBytes(newUlid());
    const ms = bytes.slice(0, 6).reduce((acc, b) => acc * 256 + b, 0);
    expect(ms).toBeGreaterThanOrEqual(before);
    expect(ms).toBeLessThanOrEqual(Date.now());
  });

  it("rejects malformed ids", () => {
    expect(() => ulidToBytes("short")).toThrow();
    expect(() => ulidToBytes("0000000000000000000000000U")).toThrow();
  });
});
```
`app/src/lib/wire/frame.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { decodeFrameBody, encodeControl, encodeInput, FRAME_CONTROL, FRAME_INPUT, FRAME_OUTPUT } from "./frame";
import { ulidToBytes } from "./ulid";

const ID = "01J00000000000000000000000";

describe("frame", () => {
  it("encodes control frames as a type byte and JSON", () => {
    const body = encodeControl({ type: "ListSessions" });
    expect(body[0]).toBe(FRAME_CONTROL);
    expect(new TextDecoder().decode(body.subarray(1))).toBe('{"type":"ListSessions"}');
  });

  it("decodes control frames", () => {
    const body = encodeControl({ type: "SessionRemoved", session_id: ID });
    expect(decodeFrameBody(body)).toEqual({ kind: "control", msg: { type: "SessionRemoved", session_id: ID } });
  });

  it("encodes input as type, 16-byte id, bytes", () => {
    const body = encodeInput(ID, new Uint8Array([104, 105]));
    expect(body[0]).toBe(FRAME_INPUT);
    expect(body.subarray(1, 17)).toEqual(ulidToBytes(ID));
    expect([...body.subarray(17)]).toEqual([104, 105]);
  });

  it("decodes output frames", () => {
    const body = new Uint8Array([FRAME_OUTPUT, ...ulidToBytes(ID), 7, 8]);
    const frame = decodeFrameBody(body);
    expect(frame).toEqual({ kind: "output", sessionId: ID, bytes: new Uint8Array([7, 8]) });
  });

  it("rejects unknown and short frames", () => {
    expect(() => decodeFrameBody(new Uint8Array([99]))).toThrow();
    expect(() => decodeFrameBody(new Uint8Array([FRAME_OUTPUT, 1, 2]))).toThrow();
  });
});
```
`app/src/lib/wire/bridge.test.ts`:
```ts
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: class {} }));

import { dispatchEnvelope } from "./bridge";
import { encodeControl } from "./frame";

describe("dispatchEnvelope", () => {
  it("routes frames and statuses", () => {
    const onFrame = vi.fn();
    const onStatus = vi.fn();
    dispatchEnvelope(new Uint8Array([0, ...encodeControl({ type: "ListProjects" })]), { onFrame, onStatus });
    expect(onFrame).toHaveBeenCalledWith({ kind: "control", msg: { type: "ListProjects" } });
    const status = new TextEncoder().encode('{"state":"elevated"}');
    dispatchEnvelope(new Uint8Array([1, ...status]), { onFrame, onStatus });
    expect(onStatus).toHaveBeenCalledWith({ state: "elevated" });
  });
});
```
`app/src/lib/state/reducer.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import type { SessionInfo } from "../protocol/SessionInfo";
import { applyControl, emptyModel } from "./reducer";

const project = { id: "P1", path: "C:\\work\\app", name: "app" };
function session(id: string, over: Partial<SessionInfo> = {}): SessionInfo {
  return {
    id,
    project_id: "P1",
    kind: "shell",
    command: "pwsh.exe",
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

describe("applyControl", () => {
  it("replaces lists and upserts events idempotently", () => {
    let m = applyControl(emptyModel(), { type: "Projects", projects: [project] });
    m = applyControl(m, { type: "Sessions", sessions: [session("S1")] });
    m = applyControl(m, { type: "SessionCreated", session: session("S1") });
    expect(Object.keys(m.sessions)).toEqual(["S1"]);
    m = applyControl(m, { type: "SessionUpdated", session: session("S1", { label: "renamed" }) });
    expect(m.sessions.S1.label).toBe("renamed");
  });

  it("marks exits and ignores events for unknown ids", () => {
    let m = applyControl(emptyModel(), { type: "Sessions", sessions: [session("S1")] });
    m = applyControl(m, { type: "SessionExited", session_id: "S1", exit_code: 0 });
    expect(m.sessions.S1.state).toBe("exited");
    expect(m.sessions.S1.exit_code).toBe(0);
    const unchanged = applyControl(m, { type: "SessionExited", session_id: "NOPE", exit_code: 1 });
    expect(unchanged).toEqual(m);
    m = applyControl(m, { type: "SessionRemoved", session_id: "S1" });
    expect(m.sessions).toEqual({});
  });

  it("tracks projects and the last error", () => {
    let m = applyControl(emptyModel(), { type: "ProjectAdded", project });
    expect(m.projects.P1.name).toBe("app");
    m = applyControl(m, { type: "Error", request: "AddProject", message: "nope", session_id: null });
    expect(m.lastError).toEqual({ request: "AddProject", message: "nope", session_id: null });
    m = applyControl(m, { type: "ProjectRemoved", id: "P1" });
    expect(m.projects).toEqual({});
  });
});
```
`app/src/lib/state/views.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { applyControl, emptyModel } from "./reducer";
import { sessionsForProject, sortedProjects } from "./views";

describe("views", () => {
  it("orders sessions by creation and projects by name", () => {
    const base = { kind: "shell", command: "c", args: [], label: "l", state: "running", state_changed_at: 0, exit_code: null, claude_session_id: null, attachable: true } as const;
    let m = applyControl(emptyModel(), {
      type: "Projects",
      projects: [
        { id: "B", path: "b", name: "beta" },
        { id: "A", path: "a", name: "alpha" },
      ],
    });
    m = applyControl(m, {
      type: "Sessions",
      sessions: [
        { ...base, id: "S2", project_id: "A", created_at: 20 },
        { ...base, id: "S1", project_id: "A", created_at: 10 },
        { ...base, id: "S3", project_id: "B", created_at: 5 },
      ],
    });
    expect(sessionsForProject(m, "A").map((s) => s.id)).toEqual(["S1", "S2"]);
    expect(sortedProjects(m).map((p) => p.name)).toEqual(["alpha", "beta"]);
  });
});
```
`app/src/lib/state/elapsed.test.ts`:
```ts
import { expect, it } from "vitest";
import { formatElapsed } from "./elapsed";

it("formats elapsed time like the sidebar mockup", () => {
  expect(formatElapsed(-5)).toBe("now");
  expect(formatElapsed(59_000)).toBe("now");
  expect(formatElapsed(2 * 60_000)).toBe("2m");
  expect(formatElapsed(3 * 3_600_000)).toBe("3h");
  expect(formatElapsed(2 * 86_400_000)).toBe("2d");
});
```
Delete `app/src/lib/smoke.test.ts`.

- [ ] **Step 2: Run them to verify they fail**

Run: `pnpm --dir app test`
Expected: failures for the missing modules.

- [ ] **Step 3: Implement**

`app/src/lib/wire/ulid.ts`:
```ts
// ULIDs as 26-character Crockford base32 strings (the daemon's JSON form)
// and 16 raw bytes (the data-frame form).
const ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

export function ulidToBytes(id: string): Uint8Array {
  if (id.length !== 26) throw new Error(`ULID must have 26 characters: ${id}`);
  let value = 0n;
  for (const ch of id.toUpperCase()) {
    const digit = ALPHABET.indexOf(ch);
    if (digit < 0) throw new Error(`invalid ULID character ${ch} in ${id}`);
    value = (value << 5n) | BigInt(digit);
  }
  const out = new Uint8Array(16);
  for (let i = 15; i >= 0; i--) {
    out[i] = Number(value & 0xffn);
    value >>= 8n;
  }
  return out;
}

export function bytesToUlid(bytes: Uint8Array): string {
  let value = 0n;
  for (const b of bytes) value = (value << 8n) | BigInt(b);
  let out = "";
  for (let i = 0; i < 26; i++) {
    out = ALPHABET[Number(value & 31n)] + out;
    value >>= 5n;
  }
  return out;
}

export function newUlid(): string {
  const bytes = new Uint8Array(16);
  let ms = Date.now();
  for (let i = 5; i >= 0; i--) {
    bytes[i] = ms % 256;
    ms = Math.floor(ms / 256);
  }
  crypto.getRandomValues(bytes.subarray(6));
  return bytesToUlid(bytes);
}
```
`app/src/lib/wire/frame.ts`:
```ts
// Frame bodies as the backend relays them: [u8 type][payload], without the
// u32 length prefix used on the daemon pipe (spec section 6.1).
import type { ControlMsg } from "../protocol/ControlMsg";
import { bytesToUlid, ulidToBytes } from "./ulid";

export const FRAME_CONTROL = 1;
export const FRAME_INPUT = 2;
export const FRAME_OUTPUT = 3;
export const FRAME_SNAPSHOT = 4;

export type Frame =
  | { kind: "control"; msg: ControlMsg }
  | { kind: "output" | "snapshot" | "input"; sessionId: string; bytes: Uint8Array };

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function decodeFrameBody(body: Uint8Array): Frame {
  const type = body[0];
  if (type === FRAME_CONTROL) {
    return { kind: "control", msg: JSON.parse(decoder.decode(body.subarray(1))) as ControlMsg };
  }
  const kind = type === FRAME_OUTPUT ? "output" : type === FRAME_SNAPSHOT ? "snapshot" : type === FRAME_INPUT ? "input" : null;
  if (kind === null) throw new Error(`unknown frame type ${type}`);
  if (body.length < 17) throw new Error("data frame shorter than its 16-byte session id");
  return { kind, sessionId: bytesToUlid(body.subarray(1, 17)), bytes: body.slice(17) };
}

export function encodeControl(msg: ControlMsg): Uint8Array {
  const json = encoder.encode(JSON.stringify(msg));
  const out = new Uint8Array(1 + json.length);
  out[0] = FRAME_CONTROL;
  out.set(json, 1);
  return out;
}

export function encodeInput(sessionId: string, bytes: Uint8Array): Uint8Array {
  const out = new Uint8Array(17 + bytes.length);
  out[0] = FRAME_INPUT;
  out.set(ulidToBytes(sessionId), 1);
  out.set(bytes, 17);
  return out;
}
```
`app/src/lib/wire/bridge.ts`:
```ts
// The link to the Rust backend: one Channel for everything coming in
// (envelope byte 0 = daemon frame body, 1 = ConnectionStatus JSON) and a
// raw-body invoke for frames going out.
import { Channel, invoke } from "@tauri-apps/api/core";
import type { ConnectionStatus } from "../backend/ConnectionStatus";
import type { ControlMsg } from "../protocol/ControlMsg";
import { decodeFrameBody, encodeControl, encodeInput, type Frame } from "./frame";

export interface BridgeHandlers {
  onStatus(status: ConnectionStatus): void;
  onFrame(frame: Frame): void;
}

const decoder = new TextDecoder();

export function dispatchEnvelope(bytes: Uint8Array, handlers: BridgeHandlers): void {
  const body = bytes.subarray(1);
  if (bytes[0] === 0) handlers.onFrame(decodeFrameBody(body));
  else if (bytes[0] === 1) handlers.onStatus(JSON.parse(decoder.decode(body)) as ConnectionStatus);
}

function toBytes(data: unknown): Uint8Array {
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  if (data instanceof Uint8Array) return data;
  return Uint8Array.from(data as number[]);
}

export async function subscribe(handlers: BridgeHandlers): Promise<void> {
  const channel = new Channel<unknown>();
  channel.onmessage = (data) => dispatchEnvelope(toBytes(data), handlers);
  await invoke("subscribe", { events: channel });
}

export function sendControl(msg: ControlMsg): Promise<void> {
  return invoke("send_frame", encodeControl(msg));
}

export function sendInput(sessionId: string, bytes: Uint8Array): Promise<void> {
  return invoke("send_frame", encodeInput(sessionId, bytes));
}
```
`app/src/lib/state/reducer.ts`:
```ts
// The app's view of the daemon, built only from control messages.
// Events are applied as idempotent upserts; events about unknown ids are
// ignored (Plan 2a carry-over 2.1, item 1).
import type { ControlMsg } from "../protocol/ControlMsg";
import type { ProjectInfo } from "../protocol/ProjectInfo";
import type { SessionInfo } from "../protocol/SessionInfo";

export interface Model {
  projects: Record<string, ProjectInfo>;
  sessions: Record<string, SessionInfo>;
  lastError: { request: string; message: string; session_id: string | null } | null;
}

export function emptyModel(): Model {
  return { projects: {}, sessions: {}, lastError: null };
}

function without<T>(record: Record<string, T>, key: string): Record<string, T> {
  const { [key]: _removed, ...rest } = record;
  return rest;
}

export function applyControl(model: Model, msg: ControlMsg): Model {
  switch (msg.type) {
    case "Projects":
      return { ...model, projects: Object.fromEntries(msg.projects.map((p) => [p.id, p])) };
    case "ProjectAdded":
      return { ...model, projects: { ...model.projects, [msg.project.id]: msg.project } };
    case "ProjectRemoved":
      return { ...model, projects: without(model.projects, msg.id) };
    case "Sessions":
      return { ...model, sessions: Object.fromEntries(msg.sessions.map((s) => [s.id, s])) };
    case "SessionCreated":
    case "SessionUpdated":
      return { ...model, sessions: { ...model.sessions, [msg.session.id]: msg.session } };
    case "SessionRemoved":
      return { ...model, sessions: without(model.sessions, msg.session_id) };
    case "SessionExited": {
      const current = model.sessions[msg.session_id];
      if (!current) return model;
      return {
        ...model,
        sessions: { ...model.sessions, [current.id]: { ...current, state: "exited", exit_code: msg.exit_code } },
      };
    }
    case "Error":
      return { ...model, lastError: { request: msg.request, message: msg.message, session_id: msg.session_id } };
    default:
      return model;
  }
}
```
`app/src/lib/state/views.ts`:
```ts
import type { ProjectInfo } from "../protocol/ProjectInfo";
import type { SessionInfo } from "../protocol/SessionInfo";
import type { Model } from "./reducer";

export function sessionsForProject(model: Model, projectId: string): SessionInfo[] {
  return Object.values(model.sessions)
    .filter((s) => s.project_id === projectId)
    .sort((a, b) => a.created_at - b.created_at);
}

export function sortedProjects(model: Model): ProjectInfo[] {
  return Object.values(model.projects).sort((a, b) => a.name.localeCompare(b.name));
}
```
`app/src/lib/state/elapsed.ts`:
```ts
/** Time since a session's last state change, as shown in the sidebar. */
export function formatElapsed(ms: number): string {
  if (ms < 60_000) return "now";
  if (ms < 3_600_000) return `${Math.floor(ms / 60_000)}m`;
  if (ms < 86_400_000) return `${Math.floor(ms / 3_600_000)}h`;
  return `${Math.floor(ms / 86_400_000)}d`;
}
```

- [ ] **Step 4: Run the tests**

Run: `pnpm --dir app test` and `pnpm --dir app check`
Expected: all pass, no type errors. If the generated `ControlMsg` type rejects a test literal (for example a required `null` field), fix the test literal to match the generated type, not the type.

- [ ] **Step 5: Commit**

```bash
git add app/src/lib
git commit -m "feat(app): frontend frame codec, bridge client and event model"
```

---

### Task 6: Theme system, fonts and color lint

**Files:**
- Create: `app/src/themes/neon-glass.json`, `app/src/lib/theme.ts`, `app/src/lib/theme.test.ts`, `app/src/app.css`, `app/scripts/lint-colors.mjs`, `app/scripts/lint-colors.test.ts`
- Modify: `app/src/main.ts`

**Interfaces:**
- Consumes: nothing.
- Produces (used by Tasks 7–8): `interface Theme { name: string; tokens: Record<string, string>; terminal: ITheme }`; `applyTheme(theme: Theme, root?: { style: { setProperty(name: string, value: string): void } }): void`; token names below (used as `var(--jojo-<name>)`); `findColorLiterals(source: string): string[]` and the `lint:colors` script.

- [ ] **Step 1: Write the failing tests**

`app/src/lib/theme.test.ts`:
```ts
import { describe, expect, it, vi } from "vitest";
import neonGlass from "../themes/neon-glass.json";
import { applyTheme, type Theme } from "./theme";

describe("theme", () => {
  it("writes every token as a --jojo- custom property", () => {
    const setProperty = vi.fn();
    applyTheme(neonGlass as Theme, { style: { setProperty } });
    expect(setProperty).toHaveBeenCalledWith("--jojo-bg", "#070a16");
    expect(setProperty).toHaveBeenCalledTimes(Object.keys(neonGlass.tokens).length);
  });

  it("defines the tokens and terminal colors the UI uses", () => {
    const required = ["bg", "bg-glow", "side", "bar", "pane", "line", "sel", "fg", "fg2", "muted", "accent", "working", "needs-you", "done", "error", "idle", "font-ui", "font-mono", "brand-gradient", "radius", "radius-sm", "danger"];
    for (const name of required) expect(neonGlass.tokens, name).toHaveProperty(name);
    for (const key of ["background", "foreground", "cursor", "selectionBackground", "black", "brightWhite"]) {
      expect(neonGlass.terminal, key).toHaveProperty(key);
    }
  });
});
```
`app/scripts/lint-colors.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { findColorLiterals } from "./lint-colors.mjs";

describe("findColorLiterals", () => {
  it("finds hex, rgb and hsl literals and named colors in styles", () => {
    const source = `<div style="color: red"></div><style>.a { background: #fff; border-color: rgba(1,2,3,.5); fill: hsl(1 2% 3%); color: var(--jojo-fg); }</style>`;
    expect(findColorLiterals(source).sort()).toEqual(["#fff", "hsl(", "red", "rgba("].sort());
  });

  it("ignores token references and plain text", () => {
    expect(findColorLiterals(`<p>The red button</p><style>.a { color: var(--jojo-error); }</style>`)).toEqual([]);
  });
});
```

- [ ] **Step 2: Run them to verify they fail**

Run: `pnpm --dir app test`
Expected: failures (missing theme file and modules).

- [ ] **Step 3: Implement**

`app/src/themes/neon-glass.json` (values from the approved mockup):
```json
{
  "name": "Neon Glass",
  "tokens": {
    "bg": "#070a16",
    "bg-glow": "radial-gradient(800px 400px at 85% -10%, rgba(155, 140, 255, 0.35), transparent 60%), radial-gradient(600px 400px at -10% 110%, rgba(62, 230, 176, 0.18), transparent 60%), #070a16",
    "side": "rgba(16, 22, 44, 0.55)",
    "bar": "rgba(14, 19, 38, 0.6)",
    "pane": "rgba(8, 11, 22, 0.86)",
    "line": "rgba(140, 160, 255, 0.14)",
    "sel": "rgba(120, 140, 255, 0.16)",
    "fg": "#eef1ff",
    "fg2": "#bcc4e8",
    "muted": "#7d87b3",
    "accent": "#9b8cff",
    "accent-glow": "rgba(155, 140, 255, 0.55)",
    "working": "#7fd4ff",
    "needs-you": "#ffb347",
    "needs-you-glow": "rgba(255, 179, 71, 0.7)",
    "needs-you-soft": "rgba(255, 179, 71, 0.25)",
    "done": "#3ee6b0",
    "error": "#ff5c7a",
    "idle": "#3a4370",
    "danger": "#c42b1c",
    "banner": "rgba(255, 179, 71, 0.12)",
    "brand-gradient": "linear-gradient(90deg, #c7bfff, #7fd4ff)",
    "radius": "9px",
    "radius-sm": "7px",
    "font-ui": "\"Geist Sans\", \"Segoe UI\", sans-serif",
    "font-mono": "\"Geist Mono\", \"Cascadia Code\", monospace"
  },
  "terminal": {
    "background": "#080b16",
    "foreground": "#d6dcff",
    "cursor": "#9b8cff",
    "cursorAccent": "#080b16",
    "selectionBackground": "rgba(155, 140, 255, 0.35)",
    "black": "#1b2040",
    "red": "#ff5c7a",
    "green": "#3ee6b0",
    "yellow": "#ffb347",
    "blue": "#7fa8ff",
    "magenta": "#c49bff",
    "cyan": "#7fd4ff",
    "white": "#d6dcff",
    "brightBlack": "#5a6390",
    "brightRed": "#ff8aa0",
    "brightGreen": "#7ff0c8",
    "brightYellow": "#ffcc80",
    "brightBlue": "#a8c4ff",
    "brightMagenta": "#dcc4ff",
    "brightCyan": "#b0e6ff",
    "brightWhite": "#ffffff"
  }
}
```
`app/src/lib/theme.ts`:
```ts
// A theme is one JSON file: `tokens` become --jojo-* custom properties on
// :root, `terminal` is the xterm.js palette (spec section 9.9).
import type { ITheme } from "@xterm/xterm";

export interface Theme {
  name: string;
  tokens: Record<string, string>;
  terminal: ITheme;
}

interface StyleTarget {
  style: { setProperty(name: string, value: string): void };
}

export function applyTheme(theme: Theme, root: StyleTarget = document.documentElement): void {
  for (const [name, value] of Object.entries(theme.tokens)) {
    root.style.setProperty(`--jojo-${name}`, value);
  }
}
```
`app/src/app.css` (tokens only, no literals):
```css
html,
body,
#app {
  height: 100%;
  margin: 0;
}
body {
  background: var(--jojo-bg);
  color: var(--jojo-fg);
  font-family: var(--jojo-font-ui);
  font-size: 12.5px;
  overflow: hidden;
  user-select: none;
}
* {
  box-sizing: border-box;
}
button {
  font: inherit;
  color: inherit;
  background: none;
  border: 0;
  padding: 0;
  cursor: pointer;
}
```
`app/src/main.ts`:
```ts
import "@fontsource/geist-sans/400.css";
import "@fontsource/geist-sans/500.css";
import "@fontsource/geist-sans/600.css";
import "@fontsource/geist-mono/400.css";
import "@xterm/xterm/css/xterm.css";
import "./app.css";
import { mount } from "svelte";
import App from "./App.svelte";

mount(App, { target: document.getElementById("app")! });
```
Check the family names the fontsource packages declare (`node_modules/@fontsource/geist-sans/400.css` → `font-family`). If they differ from `Geist Sans` / `Geist Mono`, update `font-ui`, `font-mono` and the settings default, and record it.

`app/scripts/lint-colors.mjs`:
```js
// Fails when a .svelte file outside src/themes contains a color literal in
// a style block or style attribute (spec section 9.9: components use only
// var(--jojo-*) values).
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const NAMED = /:\s*(white|black|red|green|blue|yellow|orange|purple|pink|gray|grey|cyan|magenta)\b/gi;
const LITERAL = /#[0-9a-fA-F]{3,8}\b|rgba?\(|hsla?\(/g;

/** @param {string} source @returns {string[]} */
export function findColorLiterals(source) {
  const styles = [
    ...[...source.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].map((m) => m[1]),
    ...[...source.matchAll(/style="([^"]*)"/g)].map((m) => `:${m[1]}`),
  ];
  const found = [];
  for (const css of styles) {
    for (const m of css.matchAll(LITERAL)) found.push(m[0]);
    for (const m of css.matchAll(NAMED)) found.push(m[1].toLowerCase());
  }
  return found;
}

function svelteFiles(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return name === "themes" ? [] : svelteFiles(path);
    return name.endsWith(".svelte") ? [path] : [];
  });
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const src = join(fileURLToPath(new URL(".", import.meta.url)), "..", "src");
  let failed = false;
  for (const file of svelteFiles(src)) {
    const hits = findColorLiterals(readFileSync(file, "utf8"));
    if (hits.length > 0) {
      failed = true;
      console.error(`${relative(process.cwd(), file).split(sep).join("/")}: ${hits.join(", ")}`);
    }
  }
  if (failed) process.exit(1);
  console.log("No color literals in components.");
}
```

- [ ] **Step 4: Run the tests**

Run: `pnpm --dir app test`, `pnpm --dir app lint:colors`, `pnpm --dir app check`, `pnpm --dir app build`
Expected: all pass (`No color literals in components.`).

- [ ] **Step 5: Commit**

```bash
git add app
git commit -m "feat(app): Neon Glass theme tokens, bundled fonts and the color lint"
```

---

### Task 7: Terminal pane

**Files:**
- Create: `app/src/lib/terminal/{queries,attachGate,size,router}.ts` with `.test.ts` for `queries`, `attachGate`, `size`, `router`; `app/src/ui/TerminalPane.svelte`

**Interfaces:**
- Consumes: Task 5 (`sendControl`, `sendInput`, `Frame`), Task 6 (`Theme.terminal`), the `app` store from Task 8 (`app.settings`; this task imports it, and Task 8 creates it — create a minimal `app/src/lib/state/app.svelte.ts` here exporting `export const app = $state({ settings: null as Settings | null })` and let Task 8 extend it).
- Produces (used by Task 8):
  - `suppressQueryReplies(term: { parser: { registerCsiHandler(id: { prefix?: string; intermediates?: string; final: string }, cb: () => boolean): { dispose(): void } } }): { dispose(): void }[]`.
  - `class AttachGate { requestAttach(): void; accept(kind: "output" | "snapshot", bytes: Uint8Array): Uint8Array | null }`.
  - `clampSize(cols: number, rows: number): { cols: number; rows: number }`.
  - `router`: `register(sessionId: string, pane: PaneHandle): () => void`; `deliver(frame)`; `fellBehind(sessionId: string | null)`; `reattachAll()`; `interface PaneHandle { deliver(kind: "output" | "snapshot", bytes: Uint8Array): void; reattach(): void }`.
  - `TerminalPane.svelte` with props `{ session: SessionInfo; terminalTheme: ITheme }`.

- [ ] **Step 1: Write the failing tests**

`app/src/lib/terminal/queries.test.ts`:
```ts
import { Terminal } from "@xterm/headless";
import { describe, expect, it } from "vitest";
import { suppressQueryReplies } from "./queries";

const QUERIES = "\x1b[6n\x1b[5n\x1b[c\x1b[>c\x1b[4$p\x1b[?25$p";

async function repliesTo(suppress: boolean): Promise<string> {
  const term = new Terminal({ cols: 80, rows: 24, allowProposedApi: true });
  if (suppress) suppressQueryReplies(term);
  let replies = "";
  term.onData((d) => (replies += d));
  await new Promise<void>((resolve) => term.write(QUERIES, resolve));
  term.dispose();
  return replies;
}

describe("suppressQueryReplies", () => {
  it("xterm.js answers these queries by default (control)", async () => {
    expect(await repliesTo(false)).not.toBe("");
  });

  it("stops xterm.js from answering DSR, DA1, DA2 and DECRQM", async () => {
    expect(await repliesTo(true)).toBe("");
  });
});
```
`app/src/lib/terminal/attachGate.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { AttachGate } from "./attachGate";

const b = (n: number) => new Uint8Array([n]);

describe("AttachGate", () => {
  it("drops output until the snapshot of the latest attach arrives", () => {
    const gate = new AttachGate();
    gate.requestAttach();
    expect(gate.accept("output", b(1))).toBeNull();
    expect(gate.accept("snapshot", b(2))).toEqual(b(2));
    expect(gate.accept("output", b(3))).toEqual(b(3));
  });

  it("passes output when no attach is pending", () => {
    expect(new AttachGate().accept("output", b(4))).toEqual(b(4));
  });
});
```
`app/src/lib/terminal/size.test.ts`:
```ts
import { expect, it } from "vitest";
import { clampSize } from "./size";

it("keeps sizes within what jojod accepts", () => {
  expect(clampSize(0, 0)).toEqual({ cols: 2, rows: 1 });
  expect(clampSize(Number.NaN, Number.NaN)).toEqual({ cols: 2, rows: 1 });
  expect(clampSize(120.7, 30.2)).toEqual({ cols: 120, rows: 30 });
  expect(clampSize(5000, 5000)).toEqual({ cols: 1000, rows: 1000 });
});
```
`app/src/lib/terminal/router.test.ts`:
```ts
import { describe, expect, it, vi } from "vitest";
import { createRouter } from "./router";

describe("router", () => {
  it("delivers data frames, re-attaches on fell-behind and reconnect, and unregisters", () => {
    const router = createRouter();
    const pane = { deliver: vi.fn(), reattach: vi.fn() };
    const unregister = router.register("S1", pane);
    router.deliver({ kind: "output", sessionId: "S1", bytes: new Uint8Array([1]) });
    router.deliver({ kind: "output", sessionId: "S2", bytes: new Uint8Array([2]) });
    expect(pane.deliver).toHaveBeenCalledTimes(1);
    router.fellBehind("S1");
    router.reattachAll();
    expect(pane.reattach).toHaveBeenCalledTimes(2);
    unregister();
    router.deliver({ kind: "output", sessionId: "S1", bytes: new Uint8Array([3]) });
    expect(pane.deliver).toHaveBeenCalledTimes(1);
  });
});
```

- [ ] **Step 2: Run them to verify they fail**

Run: `pnpm --dir app test`
Expected: failures (missing modules).

- [ ] **Step 3: Implement**

`app/src/lib/terminal/queries.ts`:
```ts
// jojod answers terminal queries itself (Plan 2a carry-over, decision 3
// of Plan 1). If xterm.js also answered, programs would receive two
// replies. Returning true from a CSI handler marks the sequence handled, so
// xterm.js sends nothing.
interface CsiParser {
  registerCsiHandler(
    id: { prefix?: string; intermediates?: string; final: string },
    callback: () => boolean,
  ): { dispose(): void };
}

const QUERIES = [
  { final: "n" }, // DSR, including cursor position report (CSI 6 n)
  { final: "c" }, // DA1
  { prefix: ">", final: "c" }, // DA2
  { intermediates: "$", final: "p" }, // DECRQM, ANSI modes
  { prefix: "?", intermediates: "$", final: "p" }, // DECRQM, DEC private modes
];

export function suppressQueryReplies(term: { parser: CsiParser }): { dispose(): void }[] {
  return QUERIES.map((id) => term.parser.registerCsiHandler(id, () => true));
}
```
`app/src/lib/terminal/attachGate.ts`:
```ts
// Output queued before a Detach can still arrive after a new Attach. Until
// the Snapshot answering the latest Attach arrives, Output is stale and is
// dropped (Plan 2a carry-over 2.1, item 2). A snapshot starts with a full
// reset, so it can be written into the same terminal.
export class AttachGate {
  private awaitingSnapshot = false;

  requestAttach(): void {
    this.awaitingSnapshot = true;
  }

  accept(kind: "output" | "snapshot", bytes: Uint8Array): Uint8Array | null {
    if (kind === "snapshot") {
      this.awaitingSnapshot = false;
      return bytes;
    }
    return this.awaitingSnapshot ? null : bytes;
  }
}
```
`app/src/lib/terminal/size.ts`:
```ts
// jojod rejects sizes outside 2-1000 columns and 1-1000 rows; a hidden pane
// reports no size at all.
export function clampSize(cols: number, rows: number): { cols: number; rows: number } {
  const clamp = (v: number, min: number) => (Number.isFinite(v) ? Math.min(1000, Math.max(min, Math.floor(v))) : min);
  return { cols: clamp(cols, 2), rows: clamp(rows, 1) };
}
```
`app/src/lib/terminal/router.ts`:
```ts
// Routes Output and Snapshot frames to the mounted pane of their session.
import type { Frame } from "../wire/frame";

export interface PaneHandle {
  deliver(kind: "output" | "snapshot", bytes: Uint8Array): void;
  reattach(): void;
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
      if (sessionId) panes.get(sessionId)?.reattach();
    },
    /** After a reconnect, every mounted pane attaches again. */
    reattachAll(): void {
      for (const pane of panes.values()) pane.reattach();
    },
  };
}

export const router = createRouter();
```
Minimal `app/src/lib/state/app.svelte.ts` (Task 8 extends it):
```ts
import type { Settings } from "../backend/Settings";

export const app = $state({ settings: null as Settings | null });
```
`app/src/ui/TerminalPane.svelte`:
```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { Terminal, type ITheme } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { Unicode11Addon } from "@xterm/addon-unicode11";
  import { WebLinksAddon } from "@xterm/addon-web-links";
  import { WebglAddon } from "@xterm/addon-webgl";
  import type { SessionInfo } from "../lib/protocol/SessionInfo";
  import { app } from "../lib/state/app.svelte";
  import { AttachGate } from "../lib/terminal/attachGate";
  import { suppressQueryReplies } from "../lib/terminal/queries";
  import { router } from "../lib/terminal/router";
  import { clampSize } from "../lib/terminal/size";
  import { sendControl, sendInput } from "../lib/wire/bridge";

  let { session, terminalTheme }: { session: SessionInfo; terminalTheme: ITheme } = $props();
  let host: HTMLDivElement;

  onMount(() => {
    const settings = app.settings;
    const term = new Terminal({
      allowProposedApi: true,
      fontFamily: settings?.terminalFontFamily ?? "Geist Mono",
      fontSize: settings?.terminalFontSize ?? 13,
      lineHeight: settings?.terminalLineHeight ?? 1.25,
      scrollback: Math.max(settings?.scrollbackLines ?? 10_000, 10_000),
      theme: terminalTheme,
      cursorBlink: true,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new Unicode11Addon());
    term.unicode.activeVersion = "11";
    term.loadAddon(new WebLinksAddon());
    suppressQueryReplies(term);
    term.open(host);
    try {
      term.loadAddon(new WebglAddon());
    } catch {
      // WebGL unavailable: xterm.js keeps its DOM renderer.
    }
    fit.fit();

    const id = session.id;
    const gate = new AttachGate();
    let size = clampSize(term.cols, term.rows);
    const attach = () => {
      if (!session.attachable) return;
      gate.requestAttach();
      void sendControl({ type: "Attach", session_id: id, cols: size.cols, rows: size.rows });
    };
    if (session.attachable) attach();
    else term.write("\x1b[2mThis session has no screen: it ran in a previous jojod. Close it and start a new one.\x1b[0m\r\n");

    const unregister = router.register(id, {
      deliver(kind, bytes) {
        const accepted = gate.accept(kind, bytes);
        if (accepted) term.write(accepted);
      },
      reattach: attach,
    });

    const encoder = new TextEncoder();
    const onData = term.onData((data) => void sendInput(id, encoder.encode(data)));
    const onBinary = term.onBinary((data) => void sendInput(id, Uint8Array.from(data, (c) => c.charCodeAt(0) & 0xff)));

    let timer: ReturnType<typeof setTimeout> | undefined;
    const observer = new ResizeObserver(() => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        fit.fit();
        const next = clampSize(term.cols, term.rows);
        if (next.cols !== size.cols || next.rows !== size.rows) {
          size = next;
          void sendControl({ type: "Resize", session_id: id, cols: size.cols, rows: size.rows });
        }
      }, 50);
    });
    observer.observe(host);
    term.focus();

    return () => {
      observer.disconnect();
      clearTimeout(timer);
      unregister();
      onData.dispose();
      onBinary.dispose();
      if (session.attachable) void sendControl({ type: "Detach", session_id: id });
      term.dispose();
    };
  });
</script>

<div class="frame">
  <div class="host" bind:this={host}></div>
</div>

<style>
  .frame {
    position: absolute;
    inset: 0;
    padding: 10px 12px;
    background: var(--jojo-pane);
  }
  .host {
    width: 100%;
    height: 100%;
  }
</style>
```

- [ ] **Step 4: Run the tests**

Run: `pnpm --dir app test`, `pnpm --dir app check`, `pnpm --dir app lint:colors`, `pnpm --dir app build`
Expected: all pass. The control test in `queries.test.ts` must pass: it proves the suppression test is meaningful. If `@xterm/headless` does not answer one of the queries by default, keep that query in `QUERIES` (the browser build may) and note it in the report.

- [ ] **Step 5: Commit**

```bash
git add app/src
git commit -m "feat(app): terminal pane with snapshot attach, query suppression and resize"
```

---

### Task 8: Window UI (layout A)

**Files:**
- Create: `app/src/ui/{Sidebar,TabBar,WindowControls,StatusBar,Banner}.svelte`
- Modify: `app/src/lib/state/app.svelte.ts`, `app/src/App.svelte`

**Interfaces:**
- Consumes: Tasks 4–7.
- Produces: the finished window. `app.svelte.ts` exports `app` (state: `model`, `status`, `settings`, `selectedProjectId`, `activeSessionId`, `pendingSessionId`, `now`) and the actions `startApp(): () => void`, `selectProject(id)`, `activateSession(session)`, `newSession(projectId, kind)`, `closeSession(id)`, `addProject()`, `dismissError()`, `restartDaemon()`.

Behavior (spec 9.2, 9.3, 9.7 for Plan 2b's scope):
1. On `connected`, the app sends `ListProjects` and `ListSessions`, then `router.reattachAll()`.
2. The first project (by name) is selected when none is; selecting a project activates its first session, or none.
3. `newSession` generates the session id with `newUlid()`, sends `CreateSession` with `cols: 120, rows: 30` (the pane resizes it on attach) and `shell: settings.defaultShell` for shells, `null` for Claude, and records the id in `pendingSessionId`; when `SessionCreated` with that id arrives, it becomes the active session.
4. An `Error` with `request: "Output"` goes to `router.fellBehind`, not the banner. Other errors show in the banner until dismissed.
5. Removing the active session clears `activeSessionId` and activates the next session of the project.

- [ ] **Step 1: Extend the app store**

Replace `app/src/lib/state/app.svelte.ts`:
```ts
// The app's reactive state and actions.
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { ConnectionStatus } from "../backend/ConnectionStatus";
import type { Settings } from "../backend/Settings";
import type { ControlMsg } from "../protocol/ControlMsg";
import type { SessionInfo } from "../protocol/SessionInfo";
import type { SessionKind } from "../protocol/SessionKind";
import { router } from "../terminal/router";
import { sendControl, subscribe } from "../wire/bridge";
import { newUlid } from "../wire/ulid";
import { applyControl, emptyModel, type Model } from "./reducer";
import { sessionsForProject, sortedProjects } from "./views";

export const app = $state({
  model: emptyModel() as Model,
  status: { state: "starting" } as ConnectionStatus,
  settings: null as Settings | null,
  selectedProjectId: null as string | null,
  activeSessionId: null as string | null,
  pendingSessionId: null as string | null,
  now: Date.now(),
});

export function startApp(): () => void {
  void invoke<Settings>("get_settings").then((s) => (app.settings = s));
  void subscribe({
    onStatus(status) {
      app.status = status;
      if (status.state === "connected") {
        void sendControl({ type: "ListProjects" });
        void sendControl({ type: "ListSessions" });
        router.reattachAll();
      }
    },
    onFrame(frame) {
      if (frame.kind === "control") handleControl(frame.msg);
      else router.deliver(frame);
    },
  });
  const tick = setInterval(() => (app.now = Date.now()), 30_000);
  return () => clearInterval(tick);
}

function handleControl(msg: ControlMsg): void {
  if (msg.type === "Error" && msg.request === "Output") {
    router.fellBehind(msg.session_id);
    return;
  }
  app.model = applyControl(app.model, msg);
  if (msg.type === "SessionCreated" && msg.session.id === app.pendingSessionId) {
    app.pendingSessionId = null;
    activateSession(msg.session);
  }
  if (!app.selectedProjectId || !app.model.projects[app.selectedProjectId]) {
    const first = sortedProjects(app.model)[0];
    if (first) selectProject(first.id);
  }
  if (app.activeSessionId && !app.model.sessions[app.activeSessionId]) {
    app.activeSessionId = app.selectedProjectId ? (sessionsForProject(app.model, app.selectedProjectId)[0]?.id ?? null) : null;
  }
}

export function selectProject(id: string): void {
  app.selectedProjectId = id;
  app.activeSessionId = sessionsForProject(app.model, id)[0]?.id ?? null;
}

export function activateSession(session: SessionInfo): void {
  app.selectedProjectId = session.project_id;
  app.activeSessionId = session.id;
}

export function newSession(projectId: string, kind: SessionKind): void {
  const id = newUlid();
  app.pendingSessionId = id;
  void sendControl({
    type: "CreateSession",
    session_id: id,
    project_id: projectId,
    kind,
    cols: 120,
    rows: 30,
    shell: kind === "shell" ? (app.settings?.defaultShell ?? null) : null,
  });
}

export function closeSession(id: string): void {
  void sendControl({ type: "CloseSession", session_id: id });
}

export async function addProject(): Promise<void> {
  const path = await open({ directory: true, multiple: false });
  if (typeof path !== "string") return;
  const name = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  void sendControl({ type: "AddProject", path, name });
}

export function dismissError(): void {
  app.model = { ...app.model, lastError: null };
}

export function restartDaemon(): void {
  void invoke("restart_daemon");
}
```
(If the generated `CreateSession` field order or names differ, follow the generated type.)

- [ ] **Step 2: Write the components**

`app/src/ui/WindowControls.svelte`:
```svelte
<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  const win = getCurrentWindow();
</script>

<div class="controls">
  <button aria-label="Minimize" onclick={() => win.minimize()}>&#xE921;</button>
  <button aria-label="Maximize" onclick={() => win.toggleMaximize()}>&#xE922;</button>
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
  button:hover {
    background: var(--jojo-sel);
  }
  .close:hover {
    background: var(--jojo-danger);
    color: var(--jojo-fg);
  }
</style>
```
`app/src/ui/Sidebar.svelte`:
```svelte
<script lang="ts">
  import { activateSession, addProject, app, newSession, selectProject } from "../lib/state/app.svelte";
  import { formatElapsed } from "../lib/state/elapsed";
  import { sessionsForProject, sortedProjects } from "../lib/state/views";

  let menuFor = $state<string | null>(null);
  const projects = $derived(sortedProjects(app.model));
</script>

<aside class="side">
  <div class="brand" data-tauri-drag-region>jojo</div>
  <div class="machine">
    <span class="dot" class:ok={app.status.state === "connected"}></span>
    <span class="grow">This PC</span>
    <button class="icon" title="Add project" onclick={addProject}>+</button>
  </div>
  {#each projects as project (project.id)}
    <div class="project" class:selected={app.selectedProjectId === project.id}>
      <button class="name" onclick={() => selectProject(project.id)}>{project.name}</button>
      <button class="icon add" title="New session" onclick={() => (menuFor = menuFor === project.id ? null : project.id)}>+</button>
    </div>
    {#if menuFor === project.id}
      <div class="menu">
        <button onclick={() => { menuFor = null; newSession(project.id, "claude"); }}>Claude</button>
        <button onclick={() => { menuFor = null; newSession(project.id, "shell"); }}>Shell</button>
      </div>
    {/if}
    {#each sessionsForProject(app.model, project.id) as session (session.id)}
      <button class="session" class:active={app.activeSessionId === session.id} onclick={() => activateSession(session)}>
        <span class="badge {session.state}"></span>
        <span class="label">{session.label}</span>
        <span class="time">{formatElapsed(app.now - session.state_changed_at)}</span>
      </button>
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
    visibility: hidden;
  }
  .project:hover .add {
    visibility: visible;
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
  .badge {
    width: 13px;
    height: 13px;
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
`app/src/ui/TabBar.svelte`:
```svelte
<script lang="ts">
  import { activateSession, app, closeSession, newSession } from "../lib/state/app.svelte";
  import { sessionsForProject } from "../lib/state/views";
  import WindowControls from "./WindowControls.svelte";

  const tabs = $derived(app.selectedProjectId ? sessionsForProject(app.model, app.selectedProjectId) : []);
</script>

<div class="tabs">
  {#each tabs as session (session.id)}
    <div class="tab" class:on={app.activeSessionId === session.id}>
      <button class="title" onclick={() => activateSession(session)}>
        <span class="badge {session.state}"></span>{session.label}
      </button>
      <button class="close" aria-label="Close {session.label}" onclick={() => closeSession(session.id)}>×</button>
    </div>
  {/each}
  {#if app.selectedProjectId}
    <button class="new" title="New Claude session" onclick={() => newSession(app.selectedProjectId!, "claude")}>+</button>
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
    overflow: hidden;
    text-overflow: ellipsis;
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
  .badge {
    width: 11px;
    height: 11px;
    flex: none;
    border-radius: 50%;
    border: 2px solid var(--jojo-idle);
  }
  .badge.done {
    border-color: var(--jojo-done);
    background: var(--jojo-done);
  }
  .badge.error {
    border-color: var(--jojo-error);
    background: var(--jojo-error);
  }
</style>
```
`app/src/ui/StatusBar.svelte`:
```svelte
<script lang="ts">
  import { app } from "../lib/state/app.svelte";

  const project = $derived(app.selectedProjectId ? app.model.projects[app.selectedProjectId] : undefined);
</script>

<footer class="status">
  <span>This PC</span>
  {#if project}<span>{project.path}</span>{/if}
  <span class="grow"></span>
  {#if app.status.state === "connected"}<span>jojod {app.status.jojod_version}</span>{/if}
</footer>

<style>
  .status {
    display: flex;
    gap: 14px;
    padding: 5px 12px;
    font-size: 11px;
    color: var(--jojo-muted);
    background: var(--jojo-bar);
    border-top: 1px solid var(--jojo-line);
  }
  .grow {
    flex: 1;
  }
</style>
```
`app/src/ui/Banner.svelte`:
```svelte
<script lang="ts">
  import { app, dismissError, restartDaemon } from "../lib/state/app.svelte";

  const status = $derived(app.status);
  const error = $derived(app.model.lastError);
</script>

{#if status.state === "starting"}
  <div class="banner">Starting jojod…</div>
{:else if status.state === "reconnecting"}
  <div class="banner">Disconnected from jojod: {status.message}. Reconnecting (attempt {status.attempt}). Sessions keep running.</div>
{:else if status.state === "elevated"}
  <div class="banner">Jojo is running as administrator, and jojod will not run elevated. Close Jojo and start it without administrator rights.</div>
{:else if status.state === "version_mismatch"}
  <div class="banner">
    {status.message}.
    <button class="action" onclick={restartDaemon}>Restart jojod (ends all running sessions)</button>
  </div>
{:else if status.state === "failed"}
  <div class="banner">
    Cannot reach jojod: {status.message}. Logs are in %LOCALAPPDATA%\Jojo\logs\jojod.&lt;date&gt;.log.
    <button class="action" onclick={restartDaemon}>Retry</button>
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
`app/src/App.svelte`:
```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import neonGlass from "./themes/neon-glass.json";
  import { app, startApp } from "./lib/state/app.svelte";
  import { applyTheme, type Theme } from "./lib/theme";
  import Banner from "./ui/Banner.svelte";
  import Sidebar from "./ui/Sidebar.svelte";
  import StatusBar from "./ui/StatusBar.svelte";
  import TabBar from "./ui/TabBar.svelte";
  import TerminalPane from "./ui/TerminalPane.svelte";

  const theme = neonGlass as Theme;
  onMount(() => {
    applyTheme(theme);
    return startApp();
  });
  const active = $derived(app.activeSessionId ? app.model.sessions[app.activeSessionId] : undefined);
</script>

<div class="window">
  <Sidebar />
  <section class="main">
    <TabBar />
    <Banner />
    <div class="panes">
      {#if active}
        {#key active.id}
          <TerminalPane session={active} terminalTheme={theme.terminal} />
        {/key}
      {:else if app.selectedProjectId}
        <p class="empty">No session open. Use + to start Claude or a shell.</p>
      {:else}
        <p class="empty">Add a project folder with + next to This PC.</p>
      {/if}
    </div>
    <StatusBar />
  </section>
</div>

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
Note: `WindowControls.svelte` names a font family in CSS ("Segoe Fluent Icons"); that is not a color and passes the lint.

- [ ] **Step 3: Verify**

Run: `pnpm --dir app test`, `pnpm --dir app check`, `pnpm --dir app lint:colors`, `pnpm --dir app build`, `cargo build -p jojo-app`
Expected: all pass with no warnings (svelte-check reports 0 errors and 0 warnings; fix accessibility warnings it raises rather than suppressing them).

- [ ] **Step 4: Commit**

```bash
git add app/src
git commit -m "feat(app): layout A window with sidebar, tabs, window controls, status bar and banner"
```

---

### Task 9: Daemon staging and end-to-end run

**Files:**
- Create: `scripts/stage-daemon.ps1`
- Modify: `docs/manual-test-checklist.md`, `app/package.json` (script already present)

**Interfaces:**
- Consumes: everything.
- Produces: `pnpm --dir app stage` fills `app/src-tauri/resources/daemon` with a fresh `jojod.exe` and the verified ConPTY pair; `pnpm --dir app tauri dev` runs the full app; `pnpm --dir app tauri build` produces an installer.

- [ ] **Step 1: Write the staging script**

`scripts/stage-daemon.ps1`:
```powershell
# Builds jojod and copies it, with the verified ConPTY pair, into
# app/src-tauri/resources/daemon, which Tauri bundles as the `daemon`
# resource folder. Run before `tauri dev` or `tauri build`.
param([ValidateSet('debug', 'release')][string]$Profile = 'debug')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    if ($Profile -eq 'release') { cargo build -p jojod --release } else { cargo build -p jojod }
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    pwsh -NoProfile -File scripts/fetch-conpty.ps1
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $dest = 'app/src-tauri/resources/daemon'
    New-Item -ItemType Directory -Force $dest | Out-Null
    Copy-Item "target/$Profile/jojod.exe" $dest -Force
    Copy-Item 'resources/conpty/x64/conpty.dll', 'resources/conpty/x64/OpenConsole.exe' $dest -Force
    Get-ChildItem $dest | Format-Table Name, Length
} finally {
    Pop-Location
}
```
(`fetch-conpty.ps1` in verify mode downloads the pinned package and refuses mismatched hashes; Plan 2a made it verify before copying.)

- [ ] **Step 2: Stage and run the app**

Run from a normal (non-administrator) terminal that is **not** Claude Code's PowerShell tool (use the Bash tool or a desktop terminal), with any jojod already running stopped first (`jojod.exe shutdown`, or `taskkill /IM jojod.exe /F` if it speaks protocol 1):
```powershell
pnpm --dir app stage
pnpm --dir app tauri dev
```
Expected: the window opens; the banner shows "Starting jojod…" briefly and disappears; `%LOCALAPPDATA%\Jojo\bin` contains `jojod.exe`, `conpty.dll`, `OpenConsole.exe`; the status bar shows `jojod 0.1.0`. If the agent cannot see the window, verify the daemon side instead: `Get-Process jojod` shows one process whose path is `%LOCALAPPDATA%\Jojo\bin\jojod.exe`, and `%LOCALAPPDATA%\Jojo\logs` has today's log with a "client connected" line. Record which checks were observed.

- [ ] **Step 3: Build the installer**

Run: `pnpm --dir app stage -- -Profile release` (or `pwsh -NoProfile -File scripts/stage-daemon.ps1 -Profile release`), then `pnpm --dir app tauri build`.
Expected: an NSIS installer under `target/release/bundle/nsis/`. Record its path and size.

- [ ] **Step 4: Extend the manual checklist**

Append to `docs/manual-test-checklist.md`:
```markdown
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
```

- [ ] **Step 5: Final verification**

Run (timeouts on each):
```powershell
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets
pnpm --dir app test
pnpm --dir app check
pnpm --dir app lint:colors
pwsh -NoProfile -File scripts/check-protocol.ps1
```
Expected: all pass; the script prints `Generated types are up to date.`

- [ ] **Step 6: Commit**

```bash
git add scripts/stage-daemon.ps1 docs/manual-test-checklist.md
git commit -m "build: stage the daemon for tauri dev and build; app manual checklist"
```

---

## Self-Review Notes

- **Spec and carry-over coverage.** Spec 8.1 (auto-start with the process flags, user-only pipe via `jojod::transport`) — Tasks 3–4. Spec 9.1 frame and drag region — Tasks 2 and 8 (snap layouts deferred to Plan 3, stated in Scope); fonts bundled — Task 6. Spec 9.2 sidebar (hierarchy, badge, label, elapsed, connection dot, + for projects and sessions) — Task 8 (double-click rename deferred: no rename UI requested for 2b; listed for Plan 3 with the right-click menus). Spec 9.7 status bar — Task 8 (switcher hint omitted until the switcher exists in Plan 3). Spec 9.8 xterm add-ons — Task 7. Spec 9.9 theming and lint — Task 6. Spec 9.10 settings — Task 4. Spec 10 rows for daemon crash (reconnect and restart) and protocol mismatch — Tasks 4 and 8. Plan 2a carry-over 2.1 items 1–8 — Tasks 5, 7, 8; 2.2 items 1–5 — Tasks 1, 3, 4, 8, 9; 2.3 items 3–5 — Tasks 1 and 4 (items 1–2 deferred, stated in Scope); 2.4 items 1–3 — Tasks 2 and 4. Plan 1 carry-over 2.2 items 1–7 — Tasks 1, 3, 5, 7.
- **Type consistency.** `ConnectionStatus` variants and field names match between `bridge.rs`, the generated TS and `Banner.svelte`/`app.svelte.ts`. `Settings` keys are camelCase in TS (`terminalFontSize`, `scrollbackLines`, `defaultShell`) and snake_case in Rust. `router.register` takes a `PaneHandle` with `deliver(kind, bytes)` and `reattach()` in both `router.ts` and `TerminalPane.svelte`. `Client::handshake`, `daemon_version`, `into_parts` and `is_protocol_mismatch` are defined in Task 1 and used in Tasks 3–4.
