# Jojo Plan 1: Daemon Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `jojod`, the Jojo session daemon for This PC. It owns ConPTY sessions, keeps an `alacritty_terminal` grid per session, persists metadata to `state.json`, and serves the Jojo wire protocol over a user-only Windows named pipe, including attach with a VT snapshot.

**Architecture:** This is a Cargo workspace with three crates. `jojo-proto` defines ids, messages and the frame codec. `jojo-grid` wraps `alacritty_terminal` and converts a grid back into VT sequences. `jojod` holds the PTY sessions, the store, the transport, the server and the CLI. Each session runs on three OS threads: a PTY reader, a PTY writer and a child waiter. The IPC server runs on tokio. Output for one session is fed into its grid and fanned out to subscribers while holding a single mutex. Attach takes its snapshot under that same mutex, so there is no gap and no duplicated output between the snapshot and the live stream.

**Tech Stack:** Rust stable (edition 2021), tokio 1, serde/serde_json 1, ulid 1, alacritty_terminal 0.25, portable-pty 0.9, clap 4, anyhow 1, windows-sys 0.59, tempfile 3.

**Spec:** `docs/superpowers/specs/2026-11-06-jojo-design.md` (approved 2026-11-06).

## Roadmap (Plans 2–5)

Each later plan is written when the previous plan is complete. Each produces working, tested software on its own.

| Plan | Scope | Spec sections |
|---|---|---|
| 1 (this plan) | Daemon core on This PC: protocol, grid and snapshot, PTY sessions, store, named-pipe server, `jojod serve` / `shutdown` / `--version` | 3, 4, 5, 6, 8.1 (pipe and security only), 10 (daemon rows) |
| 2 | Tauri app shell: Svelte 5 + TS, `ts-rs` type generation, app backend connection to the local daemon (auto-start with the process flags in 8.1), sidebar (machine → project → session), tabs, single pane per tab with xterm.js, Neon Glass theme tokens and lint, custom frame (layout A), settings file, bundled `conpty.dll` / `OpenConsole.exe`, real recorded fixtures for the snapshot tests | 8.1, 9.1, 9.2, 9.7–9.10, 11.1 (recordings), 14.1 |
| 3 | Pane layout: split tree (1–4 panes), right-click menus, zoom, swap, move, presets, equalize, keyboard shortcuts, switcher, copy on select and the Ctrl+C rules, `layout.json` | 9.3–9.6, 11.6 |
| 4 | Agent state: `jojo-agent` state machine, `jojod hook`, `install-hooks` / `uninstall-hooks`, `StateChanged`, silence fallback, label from first prompt, Resume via `claude_session_id`, badges, toasts | 7, 5.4 (Resume) |
| 5 | Remote machines: Linux musl builds via `cargo-zigbuild`, `serve --daemonize` with lock file, `attach --stdio`, Add machine flow, live SSH connection and reconnect, version upgrade, Docker sshd test | 8.2–8.4, 11.5 |

## Global Constraints

- Product name is **Jojo**. Binaries: `jojo.exe` (app), `jojod` / `jojod.exe` (daemon).
- Environment variable set in every session: `JOJO_SESSION_ID=<session id>`. Also `TERM=xterm-256color` and `COLORTERM=truecolor`.
- Windows pipe name: `\\.\pipe\jojo-<username>`. Only the current user's SID may access it.
- Daemon state file: `%LOCALAPPDATA%\Jojo\state.json` on Windows, `~/.jojo/state.json` on Linux. Written atomically (write `state.json.tmp`, then rename).
- Unreadable `state.json` is renamed to `state.json.corrupt-<unix timestamp>` and the daemon starts empty.
- On daemon start, every session not already `exited` is marked `exited` with `exit_code = null`.
- Frame format: `[u32 little-endian: length of type + payload][u8: frame type][payload]`. Control payloads are UTF-8 JSON. Data payloads (`Input`, `Output`, `Snapshot`) are a 16-byte binary ULID followed by raw bytes.
- `Hello { protocol_version, jojod_version }` is the first frame on every connection; `protocol_version` must match exactly. `PROTOCOL_VERSION = 1`.
- Default scrollback: 10,000 lines per session.
- Windows `claude` resolution: `claude.exe` on `PATH`; otherwise `claude.cmd` launched as `cmd.exe /d /c claude`.
- Windows default shell: `pwsh` (falls back to `powershell.exe` when `pwsh.exe` is not on `PATH`). Linux: `$SHELL`, falling back to `/bin/bash`.
- Logs never contain terminal input or output bytes.
- US spelling in code, comments, docs and commit messages.
- Commit messages end with:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  ```

## Protocol Additions Beyond Spec Section 6.2

Spec 6.2 lists request messages and some replies. This plan adds the event and reply messages the daemon needs. Mutations are broadcast to every connected client as events. Only `Error` goes solely to the requesting client.

| Added message | Direction | Purpose |
|---|---|---|
| `ProjectAdded { project }` | daemon → all | Reply to `AddProject` |
| `ProjectRemoved { id }` | daemon → all | Reply to `RemoveProject` |
| `SessionCreated { session }` | daemon → all | Reply to `CreateSession` |
| `SessionUpdated { session }` | daemon → all | After `RenameSession` (and state changes in Plan 4) |
| `CloseSession { session_id }` / `SessionRemoved { session_id }` | app → daemon / daemon → all | Spec 5.3 and 9.3 rule 6: closing a pane kills the process if it is running and removes the session. `Kill` keeps the row |
| `Shutdown` | app/CLI → daemon | Used by `jojod shutdown` (spec 3.2) |
| `CreateSession.shell: Option<String>` | field | Lets the app pass its `defaultShell` setting (spec 9.10) |

## Known Gaps Carried Forward

These are deliberate limits of the Plan 1 snapshot. Each is documented in code at the point where it applies.

1. While a session is on the alternate screen (vim, htop), the snapshot contains only the alternate screen. `alacritty_terminal` doesn't expose the inactive primary grid publicly, so the primary scrollback isn't sent on reattach until the program leaves the alternate screen. Claude Code renders on the primary screen, so it isn't affected.
2. The scroll region (DECSTBM), origin mode and the window title aren't restored.
3. Tab characters are restored as spaces. `alacritty_terminal` stores `\t` only in the first cell a tab covers, so the clipboard is the only place this is visible.

## File Structure

```
terminal-app/
├─ Cargo.toml                         # workspace: members, shared dependency versions
├─ rust-toolchain.toml                # pins stable channel
├─ crates/
│  ├─ jojo-proto/
│  │  ├─ Cargo.toml
│  │  └─ src/
│  │     ├─ lib.rs                    # re-exports, PROTOCOL_VERSION
│  │     ├─ ids.rs                    # SessionId, ProjectId
│  │     ├─ msg.rs                    # ControlMsg, SessionInfo, ProjectInfo, SessionKind, AgentState
│  │     └─ frame.rs                  # Frame, encode/decode, read_frame, write_frame
│  ├─ jojo-grid/
│  │  ├─ Cargo.toml
│  │  ├─ src/
│  │  │  ├─ lib.rs                    # TermGrid: new, feed, resize, take_replies, visible_text
│  │  │  ├─ snapshot.rs               # TermGrid::snapshot and SGR encoding
│  │  │  └─ dump.rs                   # TermGrid::debug_dump (canonical text form for tests)
│  │  └─ tests/roundtrip.rs           # snapshot golden round-trip tests
│  └─ jojod/
│     ├─ Cargo.toml
│     ├─ src/
│     │  ├─ main.rs                   # CLI: serve, shutdown, --version
│     │  ├─ lib.rs                    # module declarations, VERSION, DEFAULT_SCROLLBACK
│     │  ├─ paths.rs                  # default state dir and endpoint
│     │  ├─ command.rs                # program resolution for claude / shell
│     │  ├─ store.rs                  # state.json load/save
│     │  ├─ session.rs                # PTY session runtime
│     │  ├─ transport.rs              # named pipe (Windows) / Unix socket listener and connect
│     │  ├─ pipe_security.rs          # Windows user-only security descriptor
│     │  ├─ daemon.rs                 # request handling, broadcast, exit handling
│     │  ├─ server.rs                 # accept loop, per-connection loop, run()
│     │  └─ client.rs                 # Client used by the CLI and tests
│     └─ tests/daemon_roundtrip.rs    # end-to-end tests over the real transport
└─ docs/
```

---

### Task 1: Toolchain and workspace skeleton

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`
- Create: `crates/jojo-proto/Cargo.toml`, `crates/jojo-proto/src/lib.rs`
- Create: `crates/jojo-grid/Cargo.toml`, `crates/jojo-grid/src/lib.rs`
- Create: `crates/jojod/Cargo.toml`, `crates/jojod/src/lib.rs`, `crates/jojod/src/main.rs`
- Modify: `.gitignore`

**Interfaces:**
- Consumes: nothing.
- Produces: a workspace where `cargo test --workspace` runs. Crate names `jojo-proto`, `jojo-grid`, `jojod` (library name `jojod`, binary name `jojod`).

- [ ] **Step 1: Check for an existing Rust and MSVC toolchain**

Run (PowerShell):
```powershell
Get-Command cargo -ErrorAction SilentlyContinue; & "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
```
Expected on this PC as of 2026-11-06: `cargo` is not found. `vswhere` prints an installation path if the C++ build tools are present.

- [ ] **Step 2: Install what is missing (requires user approval; installs system software)**

Ask the user before running these. Run only the lines whose tool was missing in Step 1:
```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install --id Rustlang.Rustup
```
Then open a new terminal so `PATH` includes `%USERPROFILE%\.cargo\bin`, and run:
```powershell
rustup default stable
cargo --version
```
Expected: `cargo 1.x.y (...)`.

- [ ] **Step 3: Create the workspace files**

`Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/jojo-proto", "crates/jojo-grid", "crates/jojod"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "UNLICENSED"

[workspace.dependencies]
anyhow = "1"
alacritty_terminal = "0.25"
clap = { version = "4", features = ["derive"] }
portable-pty = "0.9"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tempfile = "3"
tokio = { version = "1", features = ["full"] }
ulid = { version = "1", features = ["serde"] }
windows-sys = { version = "0.59", features = [
  "Win32_Foundation",
  "Win32_Security",
  "Win32_Security_Authorization",
  "Win32_System_Threading",
] }
jojo-proto = { path = "crates/jojo-proto" }
jojo-grid = { path = "crates/jojo-grid" }
```

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "stable"
```

`crates/jojo-proto/Cargo.toml`:
```toml
[package]
name = "jojo-proto"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
ulid.workspace = true
```

`crates/jojo-proto/src/lib.rs`:
```rust
//! Jojo wire protocol: ids, control messages and the frame codec.

/// Protocol version exchanged in `Hello`. Both sides require an exact match.
pub const PROTOCOL_VERSION: u32 = 1;
```

`crates/jojo-grid/Cargo.toml`:
```toml
[package]
name = "jojo-grid"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
alacritty_terminal.workspace = true
```

`crates/jojo-grid/src/lib.rs`:
```rust
//! Terminal grid for one session, backed by `alacritty_terminal`.
```

`crates/jojod/Cargo.toml`:
```toml
[package]
name = "jojod"
version.workspace = true
edition.workspace = true
license.workspace = true

[lib]
name = "jojod"
path = "src/lib.rs"

[[bin]]
name = "jojod"
path = "src/main.rs"

[dependencies]
anyhow.workspace = true
clap.workspace = true
jojo-grid.workspace = true
jojo-proto.workspace = true
portable-pty.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true

[target.'cfg(windows)'.dependencies]
windows-sys.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

`crates/jojod/src/lib.rs`:
```rust
//! jojod: the Jojo session daemon.

/// Daemon version reported in `Hello` and by `jojod --version`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Scrollback lines kept per session (spec section 5.2).
pub const DEFAULT_SCROLLBACK: usize = 10_000;
```

`crates/jojod/src/main.rs`:
```rust
fn main() {
    println!("jojod {}", jojod::VERSION);
}
```

Append to `.gitignore`:
```
/target/
```

- [ ] **Step 4: Build and run the empty test suite**

Run: `cargo test --workspace`
Expected: compiles all three crates (first build downloads dependencies; if `alacritty_terminal = "0.25"` fails to resolve, change it to `"0.24"` in `Cargo.toml` and record that in the commit message), ends with `test result: ok. 0 passed`.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock rust-toolchain.toml .gitignore crates
git commit -m "build: add Cargo workspace for jojo-proto, jojo-grid and jojod

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Protocol crate (ids, messages, frame codec)

**Files:**
- Create: `crates/jojo-proto/src/ids.rs`, `crates/jojo-proto/src/msg.rs`, `crates/jojo-proto/src/frame.rs`
- Modify: `crates/jojo-proto/src/lib.rs`

**Interfaces:**
- Consumes: workspace from Task 1.
- Produces (used by Tasks 6–8):
  - `jojo_proto::{SessionId, ProjectId}`: `Copy + Eq + Hash + Display + Serialize`; `SessionId::new()`, `SessionId::to_bytes(self) -> [u8; 16]`, `SessionId::from_bytes([u8; 16]) -> SessionId`; `ProjectId::new()`.
  - `jojo_proto::{ControlMsg, ProjectInfo, SessionInfo, SessionKind, AgentState}` (definitions below).
  - `jojo_proto::Frame` with variants `Control(ControlMsg)`, `Input { session_id, bytes }`, `Output { session_id, bytes }`, `Snapshot { session_id, bytes }`.
  - `Frame::encode(&self) -> Vec<u8>`, `jojo_proto::read_frame(&mut R) -> io::Result<Option<Frame>>`, `jojo_proto::write_frame(&mut W, &Frame) -> io::Result<()>`, `jojo_proto::MAX_FRAME_LEN`.

- [ ] **Step 1: Write the failing tests**

Create `crates/jojo-proto/src/frame.rs` containing only the tests for now:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ControlMsg, ProjectId, SessionId, SessionKind};

    async fn roundtrip(frame: Frame) -> Frame {
        let (mut a, mut b) = tokio::io::duplex(1024 * 1024);
        write_frame(&mut a, &frame).await.unwrap();
        read_frame(&mut b).await.unwrap().expect("one frame")
    }

    #[tokio::test]
    async fn control_frame_roundtrips() {
        let msg = ControlMsg::CreateSession {
            project_id: ProjectId::new(),
            kind: SessionKind::Claude,
            cols: 120,
            rows: 40,
            shell: None,
        };
        let frame = Frame::Control(msg);
        assert_eq!(roundtrip(frame.clone()).await, frame);
    }

    #[tokio::test]
    async fn data_frames_roundtrip_with_binary_payload() {
        let id = SessionId::new();
        let bytes: Vec<u8> = (0u8..=255).collect();
        for frame in [
            Frame::Input { session_id: id, bytes: bytes.clone() },
            Frame::Output { session_id: id, bytes: bytes.clone() },
            Frame::Snapshot { session_id: id, bytes: bytes.clone() },
        ] {
            assert_eq!(roundtrip(frame.clone()).await, frame);
        }
    }

    #[tokio::test]
    async fn data_frame_layout_is_length_type_id_bytes() {
        let id = SessionId::new();
        let encoded = Frame::Output { session_id: id, bytes: b"hi".to_vec() }.encode();
        let len = u32::from_le_bytes(encoded[0..4].try_into().unwrap());
        assert_eq!(len as usize, 1 + 16 + 2);
        assert_eq!(encoded[4], FRAME_OUTPUT);
        assert_eq!(&encoded[5..21], &id.to_bytes());
        assert_eq!(&encoded[21..], b"hi");
    }

    #[tokio::test]
    async fn clean_eof_between_frames_returns_none() {
        let (a, mut b) = tokio::io::duplex(64);
        drop(a);
        assert!(read_frame(&mut b).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn oversized_length_is_rejected() {
        let (mut a, mut b) = tokio::io::duplex(64);
        use tokio::io::AsyncWriteExt;
        a.write_all(&((MAX_FRAME_LEN as u32) + 1).to_le_bytes()).await.unwrap();
        let err = read_frame(&mut b).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn unknown_frame_type_is_rejected() {
        let (mut a, mut b) = tokio::io::duplex(64);
        use tokio::io::AsyncWriteExt;
        a.write_all(&1u32.to_le_bytes()).await.unwrap();
        a.write_all(&[99u8]).await.unwrap();
        let err = read_frame(&mut b).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn control_json_uses_type_tag() {
        let json = serde_json::to_string(&ControlMsg::ListSessions).unwrap();
        assert_eq!(json, r#"{"type":"ListSessions"}"#);
    }
}
```

Replace `crates/jojo-proto/src/lib.rs`:
```rust
//! Jojo wire protocol: ids, control messages and the frame codec.

mod frame;
mod ids;
mod msg;

pub use frame::{read_frame, write_frame, Frame, MAX_FRAME_LEN};
pub use ids::{ProjectId, SessionId};
pub use msg::{AgentState, ControlMsg, ProjectInfo, SessionInfo, SessionKind};

/// Protocol version exchanged in `Hello`. Both sides require an exact match.
pub const PROTOCOL_VERSION: u32 = 1;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jojo-proto`
Expected: compile errors (`file not found for module ids`, unresolved `Frame`, `write_frame`, etc.).

- [ ] **Step 3: Implement ids, messages and the codec**

`crates/jojo-proto/src/ids.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::fmt;
use ulid::Ulid;

/// Identifies one terminal session. Serialized as a ULID string in JSON and
/// as 16 raw bytes in data frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub Ulid);

impl SessionId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    pub fn to_bytes(self) -> [u8; 16] {
        self.0.to_bytes()
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Ulid::from_bytes(bytes))
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Identifies one project. Unique only within one daemon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(pub Ulid);

impl ProjectId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for ProjectId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
```

`crates/jojo-proto/src/msg.rs`:
```rust
use crate::{ProjectId, SessionId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    Claude,
    Shell,
}

/// Session state shown as a badge (spec section 7.2). `Running` is used for
/// shell sessions, which have no hook-driven states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Starting,
    Idle,
    Working,
    NeedsYou,
    Done,
    Error,
    Running,
    Exited,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub id: ProjectId,
    /// Absolute path on the daemon's machine.
    pub path: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: SessionId,
    pub project_id: ProjectId,
    pub kind: SessionKind,
    pub command: String,
    pub args: Vec<String>,
    pub label: String,
    /// Unix time in milliseconds.
    pub created_at: u64,
    pub state: AgentState,
    /// Unix time in milliseconds.
    pub state_changed_at: u64,
    pub exit_code: Option<i32>,
    pub claude_session_id: Option<String>,
}

/// Every control message. Serialized as JSON with a `type` tag naming the
/// variant, for example `{"type":"ListSessions"}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ControlMsg {
    Hello { protocol_version: u32, jojod_version: String },

    ListProjects,
    Projects { projects: Vec<ProjectInfo> },
    AddProject { path: String, name: String },
    ProjectAdded { project: ProjectInfo },
    RemoveProject { id: ProjectId },
    ProjectRemoved { id: ProjectId },
    ListDir { path: String },
    DirEntries { path: String, entries: Vec<String> },

    ListSessions,
    Sessions { sessions: Vec<SessionInfo> },
    CreateSession {
        project_id: ProjectId,
        kind: SessionKind,
        cols: u16,
        rows: u16,
        /// Shell program for `SessionKind::Shell`; `None` uses the daemon default.
        shell: Option<String>,
    },
    SessionCreated { session: SessionInfo },
    SessionUpdated { session: SessionInfo },
    Attach { session_id: SessionId },
    Detach { session_id: SessionId },
    Resize { session_id: SessionId, cols: u16, rows: u16 },
    Kill { session_id: SessionId },
    CloseSession { session_id: SessionId },
    SessionRemoved { session_id: SessionId },
    RenameSession { session_id: SessionId, label: String },
    SessionExited { session_id: SessionId, exit_code: Option<i32> },

    Shutdown,
    Error { request: String, message: String },
}
```

Prepend the implementation to `crates/jojo-proto/src/frame.rs`, above the `#[cfg(test)]` module:
```rust
use crate::{ControlMsg, SessionId};
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Largest accepted value of the length prefix (type byte + payload).
pub const MAX_FRAME_LEN: usize = 16 * 1024 * 1024;

pub(crate) const FRAME_CONTROL: u8 = 1;
pub(crate) const FRAME_INPUT: u8 = 2;
pub(crate) const FRAME_OUTPUT: u8 = 3;
pub(crate) const FRAME_SNAPSHOT: u8 = 4;

#[derive(Clone, Debug, PartialEq)]
pub enum Frame {
    Control(ControlMsg),
    Input { session_id: SessionId, bytes: Vec<u8> },
    Output { session_id: SessionId, bytes: Vec<u8> },
    Snapshot { session_id: SessionId, bytes: Vec<u8> },
}

impl Frame {
    /// Encodes `[u32 LE length][u8 type][payload]`, where length counts the
    /// type byte plus the payload.
    pub fn encode(&self) -> Vec<u8> {
        let (frame_type, payload) = match self {
            Frame::Control(msg) => (
                FRAME_CONTROL,
                serde_json::to_vec(msg).expect("ControlMsg always serializes"),
            ),
            Frame::Input { session_id, bytes } => (FRAME_INPUT, data_payload(*session_id, bytes)),
            Frame::Output { session_id, bytes } => (FRAME_OUTPUT, data_payload(*session_id, bytes)),
            Frame::Snapshot { session_id, bytes } => {
                (FRAME_SNAPSHOT, data_payload(*session_id, bytes))
            }
        };
        let len = (1 + payload.len()) as u32;
        let mut out = Vec::with_capacity(4 + len as usize);
        out.extend_from_slice(&len.to_le_bytes());
        out.push(frame_type);
        out.extend_from_slice(&payload);
        out
    }

    fn decode(frame_type: u8, payload: &[u8]) -> io::Result<Frame> {
        match frame_type {
            FRAME_CONTROL => serde_json::from_slice(payload)
                .map(Frame::Control)
                .map_err(|e| invalid(format!("bad control JSON: {e}"))),
            FRAME_INPUT | FRAME_OUTPUT | FRAME_SNAPSHOT => {
                if payload.len() < 16 {
                    return Err(invalid("data frame shorter than its 16-byte session id"));
                }
                let id_bytes: [u8; 16] = payload[..16].try_into().expect("checked length");
                let session_id = SessionId::from_bytes(id_bytes);
                let bytes = payload[16..].to_vec();
                Ok(match frame_type {
                    FRAME_INPUT => Frame::Input { session_id, bytes },
                    FRAME_OUTPUT => Frame::Output { session_id, bytes },
                    _ => Frame::Snapshot { session_id, bytes },
                })
            }
            other => Err(invalid(format!("unknown frame type {other}"))),
        }
    }
}

fn data_payload(session_id: SessionId, bytes: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(16 + bytes.len());
    payload.extend_from_slice(&session_id.to_bytes());
    payload.extend_from_slice(bytes);
    payload
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// Reads one frame. Returns `Ok(None)` when the stream ends cleanly between
/// frames; an end of stream inside a frame is an `UnexpectedEof` error.
pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<Option<Frame>> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 || len > MAX_FRAME_LEN {
        return Err(invalid(format!("frame length {len} out of range")));
    }
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    Frame::decode(buf[0], &buf[1..]).map(Some)
}

pub async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, frame: &Frame) -> io::Result<()> {
    writer.write_all(&frame.encode()).await?;
    writer.flush().await
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jojo-proto`
Expected: `test result: ok. 7 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/jojo-proto
git commit -m "feat(proto): add ids, control messages and frame codec

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: TermGrid basics (feed, resize, replies, visible text)

**Files:**
- Modify: `crates/jojo-grid/src/lib.rs`

**Interfaces:**
- Consumes: `alacritty_terminal` 0.25.
- Produces (used by Tasks 4, 6, 8):
  - `jojo_grid::TermGrid` with `new(cols: u16, rows: u16, scrollback: usize) -> TermGrid`, `feed(&mut self, bytes: &[u8])`, `resize(&mut self, cols: u16, rows: u16)`, `take_replies(&mut self) -> Vec<u8>`, `visible_text(&self) -> String`, `cols(&self) -> u16`, `rows(&self) -> u16`.
  - `TermGrid` is `Send`.

- [ ] **Step 1: Write the failing tests**

Replace `crates/jojo-grid/src/lib.rs`:
```rust
//! Terminal grid for one session, backed by `alacritty_terminal`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_shows_text_on_screen() {
        let mut g = TermGrid::new(80, 24, 1000);
        g.feed(b"hello\r\nworld");
        let text = g.visible_text();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines, vec!["hello", "world"]);
    }

    #[test]
    fn colors_do_not_appear_in_visible_text() {
        let mut g = TermGrid::new(80, 24, 1000);
        g.feed(b"\x1b[31mred\x1b[0m plain");
        assert_eq!(g.visible_text(), "red plain");
    }

    #[test]
    fn resize_changes_dimensions() {
        let mut g = TermGrid::new(80, 24, 1000);
        g.resize(100, 30);
        assert_eq!((g.cols(), g.rows()), (100, 30));
    }

    #[test]
    fn cursor_position_report_is_collected_as_reply() {
        let mut g = TermGrid::new(80, 24, 1000);
        g.feed(b"ab\x1b[6n");
        assert_eq!(g.take_replies(), b"\x1b[1;3R".to_vec());
        assert!(g.take_replies().is_empty());
    }

    #[test]
    fn grid_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TermGrid>();
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jojo-grid`
Expected: compile error `cannot find type TermGrid`.

- [ ] **Step 3: Implement TermGrid**

Insert above the test module in `crates/jojo-grid/src/lib.rs`:
```rust
use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use std::sync::{Arc, Mutex};

/// Collects bytes the emulator wants to send back to the program, such as
/// the answer to a cursor position request (`ESC [ 6 n`).
#[derive(Clone, Default)]
struct ReplyCollector(Arc<Mutex<Vec<u8>>>);

impl EventListener for ReplyCollector {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event {
            self.0.lock().expect("reply lock").extend_from_slice(text.as_bytes());
        }
    }
}

/// Grid size in the form `alacritty_terminal` expects.
struct GridSize {
    cols: usize,
    rows: usize,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

/// The screen and scrollback of one session.
pub struct TermGrid {
    term: Term<ReplyCollector>,
    parser: Processor,
    replies: ReplyCollector,
}

impl TermGrid {
    pub fn new(cols: u16, rows: u16, scrollback: usize) -> Self {
        let replies = ReplyCollector::default();
        let config = Config { scrolling_history: scrollback, ..Config::default() };
        let size = GridSize { cols: cols as usize, rows: rows as usize };
        let term = Term::new(config, &size, replies.clone());
        Self { term, parser: Processor::new(), replies }
    }

    /// Interprets program output.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.term.resize(GridSize { cols: cols as usize, rows: rows as usize });
    }

    /// Returns and clears the bytes the emulator generated as replies to
    /// queries in the output. The daemon writes these to the PTY only when
    /// no window is attached, because an attached xterm.js answers itself.
    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut *self.replies.0.lock().expect("reply lock"))
    }

    pub fn cols(&self) -> u16 {
        self.term.grid().columns() as u16
    }

    pub fn rows(&self) -> u16 {
        self.term.grid().screen_lines() as u16
    }

    /// The visible screen as plain text: one line per row with trailing
    /// spaces removed, and trailing empty rows removed.
    pub fn visible_text(&self) -> String {
        let grid = self.term.grid();
        let mut lines = Vec::with_capacity(grid.screen_lines());
        for line in 0..grid.screen_lines() as i32 {
            let row = &grid[Line(line)];
            let mut text = String::new();
            for col in 0..grid.columns() {
                let cell = &row[Column(col)];
                if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                    continue;
                }
                text.push(if cell.c == '\t' { ' ' } else { cell.c });
                if let Some(extra) = cell.zerowidth() {
                    text.extend(extra.iter());
                }
            }
            lines.push(text.trim_end().to_string());
        }
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        lines.join("\n")
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jojo-grid`
Expected: `test result: ok. 5 passed`. If an `alacritty_terminal` path or method name differs in the resolved version, look it up at `https://docs.rs/alacritty_terminal/<resolved version>` and adjust the `use` lines only; the behavior required by the tests does not change.

- [ ] **Step 5: Commit**

```bash
git add crates/jojo-grid
git commit -m "feat(grid): add TermGrid wrapper over alacritty_terminal

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Snapshot serializer and round-trip golden tests

**Files:**
- Create: `crates/jojo-grid/src/snapshot.rs`, `crates/jojo-grid/src/dump.rs`, `crates/jojo-grid/tests/roundtrip.rs`
- Modify: `crates/jojo-grid/src/lib.rs` (module declarations)

**Interfaces:**
- Consumes: `TermGrid` from Task 3.
- Produces (used by Tasks 6 and 8):
  - `TermGrid::snapshot(&self) -> Vec<u8>`: VT bytes that, fed into a fresh `TermGrid` of the same size and scrollback, reproduce the cells, scrollback, cursor position, cursor style, pen and restored modes.
  - `TermGrid::debug_dump(&self) -> String`: canonical text form of the full grid state, used to compare two grids in tests.

- [ ] **Step 1: Write the failing tests**

`crates/jojo-grid/tests/roundtrip.rs`:
```rust
//! Golden round-trip tests for TermGrid::snapshot (spec section 11, item 1).
//! Each fixture is fed into grid A; A's snapshot is fed into a fresh grid B;
//! the canonical dumps of A and B must be identical.

use jojo_grid::TermGrid;

const COLS: u16 = 80;
const ROWS: u16 = 24;
const SCROLLBACK: usize = 10_000;

fn assert_roundtrip(name: &str, input: &[u8]) {
    let mut a = TermGrid::new(COLS, ROWS, SCROLLBACK);
    a.feed(input);
    let snapshot = a.snapshot();
    let mut b = TermGrid::new(COLS, ROWS, SCROLLBACK);
    b.feed(&snapshot);
    let (da, db) = (a.debug_dump(), b.debug_dump());
    if da != db {
        let first_diff = da
            .lines()
            .zip(db.lines())
            .find(|(x, y)| x != y)
            .map(|(x, y)| format!("original: {x}\nrestored: {y}"))
            .unwrap_or_else(|| "dumps differ in length".to_string());
        panic!("fixture {name} did not round-trip\n{first_diff}");
    }
}

#[test]
fn empty_grid() {
    assert_roundtrip("empty", b"");
}

#[test]
fn colors_and_attributes() {
    let mut s = Vec::new();
    s.extend_from_slice(b"\x1b[1mbold\x1b[0m \x1b[2mdim\x1b[0m \x1b[3mitalic\x1b[0m \x1b[4munder\x1b[0m\r\n");
    s.extend_from_slice(b"\x1b[7minverse\x1b[0m \x1b[9mstrike\x1b[0m \x1b[8mhidden\x1b[0m\r\n");
    for i in 0..8 {
        s.extend_from_slice(format!("\x1b[{}m{i}\x1b[{}m{i}", 30 + i, 100 + i).as_bytes());
    }
    s.extend_from_slice(b"\x1b[0m\r\n\x1b[38;5;208m256-orange\x1b[48;5;24m on blue\x1b[0m\r\n");
    s.extend_from_slice(b"\x1b[38;2;155;140;255mtruecolor violet\x1b[48;2;7;10;22m on navy\x1b[0m");
    assert_roundtrip("colors_and_attributes", &s);
}

#[test]
fn background_colored_trailing_spaces_survive() {
    assert_roundtrip("bg_spaces", b"text\x1b[44m      \x1b[0m\r\n\x1b[41m\x1b[2K\x1b[0mafter erase");
}

#[test]
fn long_line_wraps() {
    let line: String = (0..200).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
    assert_roundtrip("wrap", line.as_bytes());
}

#[test]
fn scrollback_beyond_capacity() {
    let mut s = Vec::new();
    for i in 0..12_000 {
        s.extend_from_slice(format!("line {i}\r\n").as_bytes());
    }
    assert_roundtrip("scrollback", &s);
}

#[test]
fn emoji_cjk_and_combining_marks() {
    let mut s = Vec::new();
    s.extend_from_slice("日本語テキスト 🎉 ok e\u{301}\r\n".as_bytes());
    s.extend_from_slice("x".repeat(79).as_bytes());
    s.extend_from_slice("漢 wide char forced to wrap".as_bytes());
    assert_roundtrip("wide", &s);
}

#[test]
fn claude_style_permission_prompt_with_redraw() {
    let mut s = Vec::new();
    s.extend_from_slice(b"\x1b[?25l\x1b[?2004h");
    s.extend_from_slice("\x1b[38;2;155;140;255m●\x1b[0m Bash(git commit -m \"docs: add spec\")\r\n".as_bytes());
    s.extend_from_slice("\x1b[38;2;255;179;71m╭──────────────────────────╮\x1b[0m\r\n".as_bytes());
    s.extend_from_slice("\x1b[38;2;255;179;71m│\x1b[0m Do you want to proceed?  \x1b[38;2;255;179;71m│\x1b[0m\r\n".as_bytes());
    s.extend_from_slice("\x1b[38;2;255;179;71m╰──────────────────────────╯\x1b[0m\r\n".as_bytes());
    s.extend_from_slice(b"spinner frame 1");
    s.extend_from_slice(b"\r\x1b[2Kspinner frame 2\x1b[3A\x1b[5G");
    assert_roundtrip("permission_prompt", &s);
}

#[test]
fn alternate_screen_with_mouse_modes() {
    let mut s = Vec::new();
    s.extend_from_slice(b"primary content\r\n");
    s.extend_from_slice(b"\x1b[?1049h\x1b[H\x1b[2J");
    s.extend_from_slice(b"\x1b[?1h\x1b=\x1b[?1000h\x1b[?1002h\x1b[?1006h\x1b[?1004h");
    s.extend_from_slice(b"\x1b[5;10Hvim-like content\x1b[24;1H-- INSERT --\x1b[2;3H");
    s.extend_from_slice(b"\x1b[6 q");
    assert_roundtrip("alt_screen", &s);
}

#[test]
fn line_wrap_disabled() {
    assert_roundtrip("nowrap", b"\x1b[?7lno wrap mode");
}
```

Add to `crates/jojo-grid/src/lib.rs`, directly below the `//!` line:
```rust
mod dump;
mod snapshot;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jojo-grid --test roundtrip`
Expected: compile errors (`file not found for module dump`, no method `snapshot`).

- [ ] **Step 3: Implement the dump**

`crates/jojo-grid/src/dump.rs`:
```rust
//! Canonical text form of a TermGrid, used to compare grids in tests and
//! when debugging a snapshot mismatch.

use crate::TermGrid;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use std::fmt::Write;

impl TermGrid {
    /// One line per grid row (oldest scrollback first), each listing every
    /// cell as `char/fg/bg/flags`, followed by cursor, pen, mode and cursor
    /// style lines. Tab cells are written as spaces (see snapshot.rs).
    pub fn debug_dump(&self) -> String {
        let grid = self.term.grid();
        let mut out = String::new();
        let history = grid.history_size() as i32;
        writeln!(out, "size {}x{} history {}", grid.columns(), grid.screen_lines(), history).unwrap();
        for line in -history..grid.screen_lines() as i32 {
            let row = &grid[Line(line)];
            write!(out, "{line:>6}:").unwrap();
            for col in 0..grid.columns() {
                let cell = &row[Column(col)];
                let c = if cell.c == '\t' { ' ' } else { cell.c };
                write!(out, " {c:?}/{:?}/{:?}/{:#x}", cell.fg, cell.bg, cell.flags.bits()).unwrap();
                if let Some(extra) = cell.zerowidth() {
                    write!(out, "+{extra:?}").unwrap();
                }
            }
            out.push('\n');
        }
        let cursor = &grid.cursor;
        writeln!(out, "cursor {:?} {:?}", cursor.point.line, cursor.point.column).unwrap();
        writeln!(
            out,
            "pen {:?}/{:?}/{:#x}",
            cursor.template.fg,
            cursor.template.bg,
            (cursor.template.flags & crate::snapshot::STYLE_FLAGS).bits()
        )
        .unwrap();
        writeln!(out, "mode {:#x}", (*self.term.mode() & crate::snapshot::RESTORED_MODES).bits()).unwrap();
        writeln!(out, "cursor_style {:?}", self.term.cursor_style()).unwrap();
        out
    }
}
```

- [ ] **Step 4: Implement the snapshot**

`crates/jojo-grid/src/snapshot.rs`:
```rust
//! Converts a TermGrid back into VT sequences (spec section 5.5).
//!
//! Limits (Plan 1, "Known Gaps Carried Forward"):
//! 1. On the alternate screen only the alternate screen is serialized;
//!    alacritty_terminal keeps the inactive primary grid private.
//! 2. Scroll region (DECSTBM), origin mode and title are not restored.
//! 3. Tab cells are written as spaces. alacritty_terminal stores '\t' only in
//!    the first cell a tab covers, so writing '\t' would move the cursor
//!    past cells that are then written again.

use crate::TermGrid;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor};

/// Cell flags that map to SGR attributes.
pub(crate) const STYLE_FLAGS: Flags = Flags::BOLD
    .union(Flags::DIM)
    .union(Flags::ITALIC)
    .union(Flags::UNDERLINE)
    .union(Flags::DOUBLE_UNDERLINE)
    .union(Flags::UNDERCURL)
    .union(Flags::DOTTED_UNDERLINE)
    .union(Flags::DASHED_UNDERLINE)
    .union(Flags::INVERSE)
    .union(Flags::HIDDEN)
    .union(Flags::STRIKEOUT);

/// Terminal modes the snapshot restores. Used by debug_dump to compare only
/// what is restored.
pub(crate) const RESTORED_MODES: TermMode = TermMode::ALT_SCREEN
    .union(TermMode::APP_CURSOR)
    .union(TermMode::APP_KEYPAD)
    .union(TermMode::BRACKETED_PASTE)
    .union(TermMode::MOUSE_REPORT_CLICK)
    .union(TermMode::MOUSE_DRAG)
    .union(TermMode::MOUSE_MOTION)
    .union(TermMode::SGR_MOUSE)
    .union(TermMode::UTF8_MOUSE)
    .union(TermMode::FOCUS_IN_OUT)
    .union(TermMode::SHOW_CURSOR)
    .union(TermMode::LINE_WRAP);

/// DEC private modes restored when set, as (mode flag, parameter).
const PRIVATE_MODES: [(TermMode, &str); 8] = [
    (TermMode::APP_CURSOR, "1"),
    (TermMode::MOUSE_REPORT_CLICK, "1000"),
    (TermMode::MOUSE_DRAG, "1002"),
    (TermMode::MOUSE_MOTION, "1003"),
    (TermMode::FOCUS_IN_OUT, "1004"),
    (TermMode::UTF8_MOUSE, "1005"),
    (TermMode::SGR_MOUSE, "1006"),
    (TermMode::BRACKETED_PASTE, "2004"),
];

#[derive(Clone, Copy, PartialEq)]
struct Pen {
    fg: Color,
    bg: Color,
    flags: Flags,
}

impl Pen {
    const DEFAULT: Pen = Pen {
        fg: Color::Named(NamedColor::Foreground),
        bg: Color::Named(NamedColor::Background),
        flags: Flags::empty(),
    };

    fn of(cell: &Cell) -> Pen {
        Pen { fg: cell.fg, bg: cell.bg, flags: cell.flags & STYLE_FLAGS }
    }

    /// Writes a full SGR sequence that starts from reset.
    fn write(&self, out: &mut Vec<u8>) {
        let mut s = String::from("\x1b[0");
        let attrs = [
            (Flags::BOLD, ";1"),
            (Flags::DIM, ";2"),
            (Flags::ITALIC, ";3"),
            (Flags::UNDERLINE, ";4"),
            (Flags::DOUBLE_UNDERLINE, ";21"),
            (Flags::UNDERCURL, ";4:3"),
            (Flags::DOTTED_UNDERLINE, ";4:4"),
            (Flags::DASHED_UNDERLINE, ";4:5"),
            (Flags::INVERSE, ";7"),
            (Flags::HIDDEN, ";8"),
            (Flags::STRIKEOUT, ";9"),
        ];
        for (flag, code) in attrs {
            if self.flags.contains(flag) {
                s.push_str(code);
            }
        }
        push_color(&mut s, self.fg, false);
        push_color(&mut s, self.bg, true);
        s.push('m');
        out.extend_from_slice(s.as_bytes());
    }
}

fn push_color(s: &mut String, color: Color, background: bool) {
    match color {
        Color::Named(named) => {
            let n = named as usize;
            if n < 8 {
                s.push_str(&format!(";{}", if background { 40 } else { 30 } + n));
            } else if n < 16 {
                s.push_str(&format!(";{}", if background { 100 } else { 90 } + n - 8));
            }
            // Foreground, Background and the other named colors are the
            // terminal defaults, which the leading reset already selects.
        }
        Color::Indexed(i) => s.push_str(&format!(";{};5;{i}", if background { 48 } else { 38 })),
        Color::Spec(rgb) => s.push_str(&format!(
            ";{};2;{};{};{}",
            if background { 48 } else { 38 },
            rgb.r,
            rgb.g,
            rgb.b
        )),
    }
}

fn is_blank(cell: &Cell) -> bool {
    cell.c == ' '
        && Pen::of(cell) == Pen::DEFAULT
        && cell.zerowidth().is_none()
}

impl TermGrid {
    /// VT bytes that reproduce this grid in a fresh terminal of the same size
    /// and scrollback capacity.
    pub fn snapshot(&self) -> Vec<u8> {
        let grid = self.term.grid();
        let mode = *self.term.mode();
        let cols = grid.columns();
        let mut out = Vec::with_capacity(64 * 1024);

        // Full reset, then clear scrollback, so the receiver starts empty.
        out.extend_from_slice(b"\x1bc\x1b[3J");

        let alt = mode.contains(TermMode::ALT_SCREEN);
        if alt {
            out.extend_from_slice(b"\x1b[?1049h\x1b[H\x1b[2J");
        }

        // Rows from the oldest scrollback line to the bottom of the screen.
        // Writing history + screen rows with CRLF between them scrolls the
        // history rows into the receiver's scrollback.
        let top = if alt { 0 } else { -(grid.history_size() as i32) };
        let bottom = grid.screen_lines() as i32 - 1;
        let mut pen = Pen::DEFAULT;
        for line in top..=bottom {
            let row = &grid[Line(line)];
            let wrapped = row[Column(cols - 1)].flags.contains(Flags::WRAPLINE);
            // A wrapped row is written in full so the next character wraps
            // naturally and sets WRAPLINE in the receiver. Other rows stop
            // after the last non-blank cell.
            let end = if wrapped {
                cols
            } else {
                (0..cols).rev().find(|&c| !is_blank(&row[Column(c)])).map_or(0, |c| c + 1)
            };
            for col in 0..end {
                let cell = &row[Column(col)];
                if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                    continue;
                }
                let cell_pen = Pen::of(cell);
                if cell_pen != pen {
                    cell_pen.write(&mut out);
                    pen = cell_pen;
                }
                let c = if cell.c == '\t' { ' ' } else { cell.c };
                let mut buf = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                if let Some(extra) = cell.zerowidth() {
                    for z in extra {
                        out.extend_from_slice(z.encode_utf8(&mut buf).as_bytes());
                    }
                }
            }
            if line != bottom && !wrapped {
                // Reset before the line feed so a scroll does not fill the
                // new row with the current background color.
                if pen != Pen::DEFAULT {
                    out.extend_from_slice(b"\x1b[0m");
                    pen = Pen::DEFAULT;
                }
                out.extend_from_slice(b"\r\n");
            }
        }

        // Cursor position (1-based), then the pen the program had selected.
        let cursor = &grid.cursor;
        out.extend_from_slice(
            format!("\x1b[{};{}H", cursor.point.line.0 + 1, cursor.point.column.0 + 1).as_bytes(),
        );
        Pen::of(&cursor.template).write(&mut out);

        for (flag, param) in PRIVATE_MODES {
            if mode.contains(flag) {
                out.extend_from_slice(format!("\x1b[?{param}h").as_bytes());
            }
        }
        if mode.contains(TermMode::APP_KEYPAD) {
            out.extend_from_slice(b"\x1b=");
        }
        if !mode.contains(TermMode::LINE_WRAP) {
            out.extend_from_slice(b"\x1b[?7l");
        }
        if !mode.contains(TermMode::SHOW_CURSOR) {
            out.extend_from_slice(b"\x1b[?25l");
        }

        let style = self.term.cursor_style();
        let base = match style.shape {
            CursorShape::Block => Some(1),
            CursorShape::Underline => Some(3),
            CursorShape::Beam => Some(5),
            _ => None,
        };
        if let Some(base) = base {
            let n = if style.blinking { base } else { base + 1 };
            out.extend_from_slice(format!("\x1b[{n} q").as_bytes());
        }

        out
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p jojo-grid`
Expected: `test result: ok` for the 5 unit tests and the 9 round-trip tests. When a fixture fails, the panic message shows the first differing dump line. Fix the serializer, not the fixture, unless the difference is one of the three documented gaps. For a documented gap, write the expected loss into the fixture's name and a comment.

- [ ] **Step 6: Commit**

```bash
git add crates/jojo-grid
git commit -m "feat(grid): serialize grid to VT snapshot with golden round-trip tests

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Store (state.json) and command resolution

**Files:**
- Create: `crates/jojod/src/store.rs`, `crates/jojod/src/command.rs`, `crates/jojod/src/paths.rs`
- Modify: `crates/jojod/src/lib.rs`

**Interfaces:**
- Consumes: `jojo_proto::{ProjectInfo, SessionInfo, SessionId, AgentState, SessionKind}`.
- Produces (used by Tasks 7–8):
  - `jojod::store::Store` with `load(path: &Path) -> io::Result<Store>`, `save(&self) -> io::Result<()>`, public field `data: StoreData { projects: Vec<ProjectInfo>, sessions: Vec<SessionInfo> }`, `session_mut(&mut self, id: SessionId) -> Option<&mut SessionInfo>`.
  - `jojod::store::now_ms() -> u64`.
  - `jojod::command::command_for(kind: SessionKind, shell: Option<&str>) -> (String, Vec<String>)`, `jojod::command::default_label(kind: SessionKind, program: &str) -> String`, `jojod::command::find_on_path(name: &str) -> Option<PathBuf>`.
  - `jojod::paths::{default_state_dir() -> PathBuf, default_endpoint() -> Endpoint}` and `jojod::paths::Endpoint(pub String)`.

- [ ] **Step 1: Write the failing tests**

`crates/jojod/src/store.rs`, tests only for now:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use jojo_proto::{AgentState, ProjectId, SessionId, SessionInfo, SessionKind};

    fn session(state: AgentState, exit_code: Option<i32>) -> SessionInfo {
        SessionInfo {
            id: SessionId::new(),
            project_id: ProjectId::new(),
            kind: SessionKind::Shell,
            command: "pwsh".into(),
            args: vec![],
            label: "pwsh".into(),
            created_at: 1,
            state,
            state_changed_at: 1,
            exit_code,
            claude_session_id: None,
        }
    }

    #[test]
    fn missing_file_loads_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::load(&dir.path().join("state.json")).unwrap();
        assert!(store.data.projects.is_empty() && store.data.sessions.is_empty());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let mut store = Store::load(&path).unwrap();
        store.data.sessions.push(session(AgentState::Exited, Some(0)));
        store.save().unwrap();
        assert!(!dir.path().join("state.json.tmp").exists());
        let loaded = Store::load(&path).unwrap();
        assert_eq!(loaded.data.sessions, store.data.sessions);
    }

    #[test]
    fn load_marks_live_sessions_exited_with_null_code() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let mut store = Store::load(&path).unwrap();
        store.data.sessions.push(session(AgentState::Working, None));
        store.data.sessions.push(session(AgentState::Exited, Some(3)));
        store.save().unwrap();
        let loaded = Store::load(&path).unwrap();
        assert_eq!(loaded.data.sessions[0].state, AgentState::Exited);
        assert_eq!(loaded.data.sessions[0].exit_code, None);
        assert_eq!(loaded.data.sessions[1].exit_code, Some(3));
    }

    #[test]
    fn corrupt_file_is_renamed_and_store_starts_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, b"{ not json").unwrap();
        let store = Store::load(&path).unwrap();
        assert!(store.data.sessions.is_empty());
        let renamed: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("state.json.corrupt-"))
            .collect();
        assert_eq!(renamed.len(), 1);
        assert!(!path.exists());
    }
}
```

`crates/jojod/src/command.rs`, tests only for now:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use jojo_proto::SessionKind;

    #[test]
    fn finds_a_program_known_to_exist() {
        #[cfg(windows)]
        assert!(find_on_path("cmd.exe").is_some());
        #[cfg(unix)]
        assert!(find_on_path("sh").is_some());
    }

    #[test]
    fn missing_program_is_none() {
        assert!(find_on_path("definitely-not-a-real-program-jojo").is_none());
    }

    #[test]
    fn shell_override_is_used() {
        assert_eq!(command_for(SessionKind::Shell, Some("bash")), ("bash".to_string(), vec![]));
    }

    #[test]
    fn labels() {
        assert_eq!(default_label(SessionKind::Claude, "claude.exe"), "claude");
        assert_eq!(default_label(SessionKind::Shell, "C:\\Program Files\\PowerShell\\7\\pwsh.exe"), "pwsh");
        assert_eq!(default_label(SessionKind::Shell, "/bin/bash"), "bash");
    }
}
```

Replace `crates/jojod/src/lib.rs`:
```rust
//! jojod: the Jojo session daemon.

pub mod command;
pub mod paths;
pub mod store;

/// Daemon version reported in `Hello` and by `jojod --version`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Scrollback lines kept per session (spec section 5.2).
pub const DEFAULT_SCROLLBACK: usize = 10_000;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jojod --lib`
Expected: compile errors (`file not found for module paths`, `cannot find type Store`).

- [ ] **Step 3: Implement paths, store and command resolution**

`crates/jojod/src/paths.rs`:
```rust
//! Default locations (spec sections 4 and 8.1).

use std::path::PathBuf;

/// Where the daemon listens: a named pipe name on Windows, a socket path on
/// Linux.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint(pub String);

/// `%LOCALAPPDATA%\Jojo` on Windows, `~/.jojo` on Linux.
pub fn default_state_dir() -> PathBuf {
    #[cfg(windows)]
    {
        let base = std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA is set on Windows");
        PathBuf::from(base).join("Jojo")
    }
    #[cfg(unix)]
    {
        let home = std::env::var_os("HOME").expect("HOME is set");
        PathBuf::from(home).join(".jojo")
    }
}

/// `\\.\pipe\jojo-<username>` on Windows, `~/.jojo/jojod.sock` on Linux.
pub fn default_endpoint() -> Endpoint {
    #[cfg(windows)]
    {
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".to_string());
        let safe: String = user
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        Endpoint(format!(r"\\.\pipe\jojo-{safe}"))
    }
    #[cfg(unix)]
    {
        Endpoint(default_state_dir().join("jojod.sock").to_string_lossy().into_owned())
    }
}
```

Prepend to `crates/jojod/src/store.rs`, above the test module:
```rust
//! Persistent daemon metadata in state.json (spec section 4).

use jojo_proto::{AgentState, ProjectInfo, SessionId, SessionInfo};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StoreData {
    pub projects: Vec<ProjectInfo>,
    pub sessions: Vec<SessionInfo>,
}

pub struct Store {
    path: PathBuf,
    pub data: StoreData,
}

/// Current Unix time in milliseconds.
pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl Store {
    /// Loads state.json. A missing file gives an empty store. An unreadable
    /// file is renamed to `state.json.corrupt-<unix seconds>` and the store
    /// starts empty. Every session not already exited is marked exited with
    /// no exit code, because its process ended with the previous daemon.
    pub fn load(path: &Path) -> io::Result<Store> {
        let mut data = match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice::<StoreData>(&bytes) {
                Ok(data) => data,
                Err(_) => {
                    let secs = now_ms() / 1000;
                    let mut corrupt = path.as_os_str().to_owned();
                    corrupt.push(format!(".corrupt-{secs}"));
                    std::fs::rename(path, PathBuf::from(corrupt))?;
                    StoreData::default()
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => StoreData::default(),
            Err(e) => return Err(e),
        };
        let now = now_ms();
        for s in &mut data.sessions {
            if s.state != AgentState::Exited {
                s.state = AgentState::Exited;
                s.exit_code = None;
                s.state_changed_at = now;
            }
        }
        Ok(Store { path: path.to_path_buf(), data })
    }

    /// Writes `state.json.tmp`, then renames it over `state.json`.
    pub fn save(&self) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut tmp = self.path.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        let bytes = serde_json::to_vec_pretty(&self.data).map_err(io::Error::other)?;
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &self.path)
    }

    pub fn session_mut(&mut self, id: SessionId) -> Option<&mut SessionInfo> {
        self.data.sessions.iter_mut().find(|s| s.id == id)
    }
}
```

Prepend to `crates/jojod/src/command.rs`, above the test module:
```rust
//! Which program a session runs (spec section 5.1).

use jojo_proto::SessionKind;
use std::path::{Path, PathBuf};

/// Searches each directory on PATH for a file named exactly `name`.
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|dir| dir.join(name)).find(|p| p.is_file())
}

/// Program and arguments for a new session.
pub fn command_for(kind: SessionKind, shell: Option<&str>) -> (String, Vec<String>) {
    match kind {
        SessionKind::Claude => claude_command(),
        SessionKind::Shell => (shell.map(str::to_owned).unwrap_or_else(default_shell), vec![]),
    }
}

/// `claude.exe` when present; otherwise an npm-installed `claude.cmd` run
/// through `cmd.exe`, because ConPTY starts executables, not batch files.
/// When neither exists, `claude.exe` is returned so the spawn fails with a
/// "not found" error that names the program.
#[cfg(windows)]
fn claude_command() -> (String, Vec<String>) {
    if find_on_path("claude.exe").is_some() {
        ("claude.exe".into(), vec![])
    } else if find_on_path("claude.cmd").is_some() {
        ("cmd.exe".into(), vec!["/d".into(), "/c".into(), "claude".into()])
    } else {
        ("claude.exe".into(), vec![])
    }
}

#[cfg(unix)]
fn claude_command() -> (String, Vec<String>) {
    ("claude".into(), vec![])
}

#[cfg(windows)]
fn default_shell() -> String {
    if find_on_path("pwsh.exe").is_some() { "pwsh.exe".into() } else { "powershell.exe".into() }
}

#[cfg(unix)]
fn default_shell() -> String {
    std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/bash".into())
}

/// Initial sidebar label: `claude` for Claude sessions, otherwise the
/// program's file stem (`pwsh`, `bash`). Parses both `\` and `/` separators
/// so Windows paths work on any platform.
pub fn default_label(kind: SessionKind, program: &str) -> String {
    match kind {
        SessionKind::Claude => "claude".into(),
        SessionKind::Shell => {
            let last = program.rsplit(['\\', '/']).next().unwrap_or(program);
            Path::new(last).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| last.to_string())
        }
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jojod --lib`
Expected: `test result: ok. 8 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/jojod
git commit -m "feat(jojod): add state.json store, default paths and command resolution

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: PTY session runtime

**Files:**
- Create: `crates/jojod/src/session.rs`
- Modify: `crates/jojod/src/lib.rs` (add `pub mod session;`)

**Interfaces:**
- Consumes: `jojo_grid::TermGrid` (Tasks 3–4), `jojo_proto::{Frame, SessionId}`.
- Produces (used by Task 7):
  - `jojod::session::ClientId = u64`.
  - `jojod::session::SpawnSpec { program: String, args: Vec<String>, cwd: PathBuf, env: Vec<(String, String)>, cols: u16, rows: u16, scrollback: usize }`.
  - `Session::spawn(id: SessionId, spec: SpawnSpec, on_exit: impl FnOnce(Option<i32>) + Send + 'static) -> anyhow::Result<Arc<Session>>`.
  - `Session::attach(&self, client: ClientId, tx: UnboundedSender<Frame>)`: sends one `Frame::Snapshot`, then live `Frame::Output` frames.
  - `Session::detach(&self, client: ClientId)`, `Session::write(&self, bytes: Vec<u8>)`, `Session::resize(&self, cols: u16, rows: u16)`, `Session::kill(&self)`, `Session::visible_text(&self) -> String`.

- [ ] **Step 1: Write the failing tests**

`crates/jojod/src/session.rs`, tests only for now:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use jojo_grid::TermGrid;
    use std::time::{Duration, Instant};

    fn test_spec() -> SpawnSpec {
        #[cfg(windows)]
        let program = "cmd.exe";
        #[cfg(unix)]
        let program = "sh";
        SpawnSpec {
            program: program.into(),
            args: vec![],
            cwd: std::env::temp_dir(),
            env: vec![("JOJO_SESSION_ID".into(), "test".into())],
            cols: 80,
            rows: 24,
            scrollback: 1000,
        }
    }

    fn has_line(grid: &TermGrid, want: &str) -> bool {
        grid.visible_text().lines().any(|l| l.trim() == want)
    }

    #[test]
    fn attach_receives_snapshot_then_output_and_exit_is_reported() {
        let (exit_tx, exit_rx) = std::sync::mpsc::channel();
        let session = Session::spawn(SessionId::new(), test_spec(), move |code| {
            let _ = exit_tx.send(code);
        })
        .unwrap();

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        session.attach(1, tx);
        assert!(matches!(rx.blocking_recv(), Some(Frame::Snapshot { .. })));

        session.write(b"echo jojo-hi\r".to_vec());
        let mut grid = TermGrid::new(80, 24, 1000);
        let deadline = Instant::now() + Duration::from_secs(15);
        while !has_line(&grid, "jojo-hi") {
            match rx.try_recv() {
                Ok(Frame::Output { bytes, .. }) => grid.feed(&bytes),
                Ok(_) => {}
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
            assert!(Instant::now() < deadline, "no echo output; screen:\n{}", grid.visible_text());
        }
        assert!(has_line_text(&session.visible_text(), "jojo-hi"));

        session.write(b"exit\r".to_vec());
        let code = exit_rx.recv_timeout(Duration::from_secs(15)).expect("exit reported");
        assert_eq!(code, Some(0));
    }

    fn has_line_text(text: &str, want: &str) -> bool {
        text.lines().any(|l| l.trim() == want)
    }

    #[test]
    fn kill_ends_the_process() {
        let (exit_tx, exit_rx) = std::sync::mpsc::channel();
        let session = Session::spawn(SessionId::new(), test_spec(), move |code| {
            let _ = exit_tx.send(code);
        })
        .unwrap();
        session.kill();
        exit_rx.recv_timeout(Duration::from_secs(15)).expect("exit reported after kill");
    }

    #[test]
    fn detached_client_receives_no_output() {
        let session = Session::spawn(SessionId::new(), test_spec(), |_| {}).unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        session.attach(7, tx);
        let _snapshot = rx.blocking_recv();
        session.detach(7);
        session.write(b"echo after-detach\r".to_vec());
        std::thread::sleep(Duration::from_millis(1500));
        while let Ok(frame) = rx.try_recv() {
            assert!(!matches!(frame, Frame::Output { .. }), "output after detach");
        }
        session.kill();
    }

    #[test]
    fn missing_program_fails_to_spawn() {
        let mut spec = test_spec();
        spec.program = "definitely-not-a-real-program-jojo".into();
        assert!(Session::spawn(SessionId::new(), spec, |_| {}).is_err());
    }
}
```

Add `pub mod session;` to `crates/jojod/src/lib.rs` after `pub mod paths;`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jojod --lib session`
Expected: compile errors (`cannot find type Session`).

- [ ] **Step 3: Implement the session runtime**

Prepend to `crates/jojod/src/session.rs`, above the test module:
```rust
//! One PTY session: the process, its grid, and attached clients.
//!
//! Threads per session:
//! - reader: reads PTY output, feeds the grid and sends Output frames to
//!   subscribers, all while holding `shared`;
//! - writer: writes queued input to the PTY so a slow program never blocks
//!   the daemon's async tasks;
//! - waiter: waits for the process to exit, closes the PTY master (which
//!   ends the reader on ConPTY) and calls `on_exit`.
//!
//! `attach` takes the snapshot and registers the subscriber while holding
//! `shared`, the same lock the reader holds while feeding. No output can
//! fall between the snapshot and the first Output frame, and none appears
//! in both (spec section 5.5, rule 3).

use anyhow::Context;
use jojo_grid::TermGrid;
use jojo_proto::{Frame, SessionId};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{mpsc as std_mpsc, Arc, Mutex};
use std::thread;
use tokio::sync::mpsc::UnboundedSender;

pub type ClientId = u64;

pub struct SpawnSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub cols: u16,
    pub rows: u16,
    pub scrollback: usize,
}

struct Shared {
    grid: TermGrid,
    subscribers: Vec<(ClientId, UnboundedSender<Frame>)>,
}

pub struct Session {
    pub id: SessionId,
    shared: Arc<Mutex<Shared>>,
    input_tx: std_mpsc::Sender<Vec<u8>>,
    master: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
}

fn pty_size(cols: u16, rows: u16) -> PtySize {
    PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }
}

impl Session {
    pub fn spawn(
        id: SessionId,
        spec: SpawnSpec,
        on_exit: impl FnOnce(Option<i32>) + Send + 'static,
    ) -> anyhow::Result<Arc<Session>> {
        let pair = native_pty_system()
            .openpty(pty_size(spec.cols, spec.rows))
            .context("open PTY")?;

        let mut cmd = CommandBuilder::new(&spec.program);
        cmd.args(&spec.args);
        cmd.cwd(&spec.cwd);
        for (key, value) in &spec.env {
            cmd.env(key, value);
        }
        let mut child = pair
            .slave
            .spawn_command(cmd)
            .with_context(|| format!("start {}", spec.program))?;
        drop(pair.slave);

        let killer = child.clone_killer();
        let mut reader = pair.master.try_clone_reader().context("PTY reader")?;
        let mut writer = pair.master.take_writer().context("PTY writer")?;
        let master = Arc::new(Mutex::new(Some(pair.master)));
        let shared = Arc::new(Mutex::new(Shared {
            grid: TermGrid::new(spec.cols, spec.rows, spec.scrollback),
            subscribers: Vec::new(),
        }));
        let (input_tx, input_rx) = std_mpsc::channel::<Vec<u8>>();

        thread::Builder::new().name(format!("pty-in-{id}")).spawn(move || {
            for bytes in input_rx {
                if writer.write_all(&bytes).and_then(|_| writer.flush()).is_err() {
                    break;
                }
            }
        })?;

        {
            let shared = shared.clone();
            let input_tx = input_tx.clone();
            thread::Builder::new().name(format!("pty-out-{id}")).spawn(move || {
                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    let n = match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    let mut s = shared.lock().expect("session lock");
                    s.grid.feed(&buf[..n]);
                    let replies = s.grid.take_replies();
                    let chunk = &buf[..n];
                    s.subscribers.retain(|(_, tx)| {
                        tx.send(Frame::Output { session_id: id, bytes: chunk.to_vec() }).is_ok()
                    });
                    // With no window attached, nobody else will answer
                    // queries such as ESC [ 6 n, so the daemon answers.
                    if s.subscribers.is_empty() && !replies.is_empty() {
                        let _ = input_tx.send(replies);
                    }
                }
            })?;
        }

        {
            let master = master.clone();
            thread::Builder::new().name(format!("pty-wait-{id}")).spawn(move || {
                let code = child.wait().ok().map(|status| status.exit_code() as i32);
                // Dropping the master closes the pseudoconsole, which ends
                // the reader thread's blocking read on Windows.
                master.lock().expect("master lock").take();
                on_exit(code);
            })?;
        }

        Ok(Arc::new(Session { id, shared, input_tx, master, killer: Mutex::new(killer) }))
    }

    pub fn attach(&self, client: ClientId, tx: UnboundedSender<Frame>) {
        let mut s = self.shared.lock().expect("session lock");
        let bytes = s.grid.snapshot();
        if tx.send(Frame::Snapshot { session_id: self.id, bytes }).is_ok() {
            s.subscribers.retain(|(c, _)| *c != client);
            s.subscribers.push((client, tx));
        }
    }

    pub fn detach(&self, client: ClientId) {
        self.shared.lock().expect("session lock").subscribers.retain(|(c, _)| *c != client);
    }

    pub fn write(&self, bytes: Vec<u8>) {
        let _ = self.input_tx.send(bytes);
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        if let Some(master) = self.master.lock().expect("master lock").as_ref() {
            let _ = master.resize(pty_size(cols, rows));
        }
        self.shared.lock().expect("session lock").grid.resize(cols, rows);
    }

    pub fn kill(&self) {
        let _ = self.killer.lock().expect("killer lock").kill();
    }

    pub fn visible_text(&self) -> String {
        self.shared.lock().expect("session lock").grid.visible_text()
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jojod --lib session -- --test-threads=1`
Expected: `test result: ok. 4 passed`. `--test-threads=1` keeps the PTY tests from competing for console resources; the tests also pass in parallel, but serial runs make failures easier to read.

- [ ] **Step 5: Commit**

```bash
git add crates/jojod
git commit -m "feat(jojod): add PTY session runtime with snapshot-on-attach

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Transport, daemon, server and client

**Files:**
- Create: `crates/jojod/src/transport.rs`, `crates/jojod/src/pipe_security.rs`, `crates/jojod/src/daemon.rs`, `crates/jojod/src/server.rs`, `crates/jojod/src/client.rs`
- Create: `crates/jojod/tests/daemon_roundtrip.rs`
- Modify: `crates/jojod/src/lib.rs`

**Interfaces:**
- Consumes: everything from Tasks 2–6.
- Produces (used by Task 8 and by Plans 2–5):
  - `jojod::transport::{Listener, connect, BoxRead, BoxWrite}`: `Listener::bind(&Endpoint) -> io::Result<Listener>`, `Listener::accept(&mut self) -> io::Result<(BoxRead, BoxWrite)>`, `connect(&Endpoint) -> io::Result<(BoxRead, BoxWrite)>`.
  - `jojod::daemon::{Daemon, DaemonConfig}`: `DaemonConfig { endpoint: Endpoint, state_dir: PathBuf, scrollback: usize }`.
  - `jojod::server::run(cfg: DaemonConfig) -> anyhow::Result<()>`: serves until a `Shutdown` message, then kills all sessions and returns.
  - `jojod::client::Client`: `connect(&Endpoint) -> io::Result<Client>` (performs the `Hello` exchange), `send(&mut self, Frame) -> io::Result<()>`, `recv(&mut self) -> io::Result<Frame>`, `recv_until(&mut self, Duration, impl FnMut(&Frame) -> bool) -> io::Result<Frame>`.

- [ ] **Step 1: Write the failing end-to-end tests**

`crates/jojod/tests/daemon_roundtrip.rs`:
```rust
//! End-to-end tests over the real transport (spec section 11, item 4).

use jojo_grid::TermGrid;
use jojo_proto::{ControlMsg, Frame, ProjectInfo, SessionInfo, SessionKind};
use jojod::client::Client;
use jojod::daemon::DaemonConfig;
use jojod::paths::Endpoint;
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(15);

fn test_endpoint(dir: &std::path::Path) -> Endpoint {
    #[cfg(windows)]
    {
        let _ = dir;
        Endpoint(format!(r"\\.\pipe\jojo-test-{}", jojo_proto::SessionId::new()))
    }
    #[cfg(unix)]
    {
        Endpoint(dir.join("jojod.sock").to_string_lossy().into_owned())
    }
}

fn test_shell() -> String {
    #[cfg(windows)]
    return "cmd.exe".into();
    #[cfg(unix)]
    return "sh".into();
}

async fn start(dir: &std::path::Path) -> (Endpoint, tokio::task::JoinHandle<anyhow::Result<()>>) {
    let endpoint = test_endpoint(dir);
    let cfg = DaemonConfig { endpoint: endpoint.clone(), state_dir: dir.to_path_buf(), scrollback: 1000 };
    let handle = tokio::spawn(jojod::server::run(cfg));
    for _ in 0..100 {
        if Client::connect(&endpoint).await.is_ok() {
            return (endpoint, handle);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("daemon did not start listening");
}

async fn add_project(c: &mut Client, dir: &std::path::Path) -> ProjectInfo {
    c.send(Frame::Control(ControlMsg::AddProject {
        path: dir.to_string_lossy().into_owned(),
        name: "tmp".into(),
    }))
    .await
    .unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::ProjectAdded { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::ProjectAdded { project }) => project,
        _ => unreachable!(),
    }
}

async fn create_shell(c: &mut Client, project: &ProjectInfo) -> SessionInfo {
    c.send(Frame::Control(ControlMsg::CreateSession {
        project_id: project.id,
        kind: SessionKind::Shell,
        cols: 80,
        rows: 24,
        shell: Some(test_shell()),
    }))
    .await
    .unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::SessionCreated { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::SessionCreated { session }) => session,
        _ => unreachable!(),
    }
}

fn has_line(grid: &TermGrid, want: &str) -> bool {
    grid.visible_text().lines().any(|l| l.trim() == want)
}

#[tokio::test(flavor = "multi_thread")]
async fn create_attach_echo_detach_reattach_shutdown() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();

    let project = add_project(&mut c, dir.path()).await;
    let session = create_shell(&mut c, &project).await;
    assert_eq!(session.label, test_shell().trim_end_matches(".exe"));

    c.send(Frame::Control(ControlMsg::Attach { session_id: session.id })).await.unwrap();
    c.recv_until(WAIT, |f| matches!(f, Frame::Snapshot { .. })).await.unwrap();

    c.send(Frame::Input { session_id: session.id, bytes: b"echo jojo-hi\r".to_vec() }).await.unwrap();
    let mut grid = TermGrid::new(80, 24, 1000);
    let deadline = tokio::time::Instant::now() + WAIT;
    while !has_line(&grid, "jojo-hi") {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(left, c.recv()).await {
            Ok(Ok(Frame::Output { bytes, .. })) => grid.feed(&bytes),
            Ok(Ok(_)) => {}
            Ok(Err(e)) => panic!("connection error: {e}"),
            Err(_) => panic!("no echo; screen:\n{}", grid.visible_text()),
        }
    }

    c.send(Frame::Control(ControlMsg::Detach { session_id: session.id })).await.unwrap();
    c.send(Frame::Control(ControlMsg::Attach { session_id: session.id })).await.unwrap();
    let snapshot = match c.recv_until(WAIT, |f| matches!(f, Frame::Snapshot { .. })).await.unwrap() {
        Frame::Snapshot { bytes, .. } => bytes,
        _ => unreachable!(),
    };
    let mut restored = TermGrid::new(80, 24, 1000);
    restored.feed(&snapshot);
    assert!(has_line(&restored, "jojo-hi"), "snapshot missing output:\n{}", restored.visible_text());

    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.expect("server stops").unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn exit_is_broadcast_and_persisted_across_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    let project = add_project(&mut c, dir.path()).await;
    let session = create_shell(&mut c, &project).await;

    c.send(Frame::Input { session_id: session.id, bytes: b"exit\r".to_vec() }).await.unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::SessionExited { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::SessionExited { session_id, exit_code }) => {
            assert_eq!(session_id, session.id);
            assert_eq!(exit_code, Some(0));
        }
        _ => unreachable!(),
    }
    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
    drop(c);

    // Same state directory, new endpoint: the second daemon reads the
    // state.json the first one wrote.
    let (endpoint, server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    c.send(Frame::Control(ControlMsg::ListSessions)).await.unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Sessions { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::Sessions { sessions }) => {
            assert_eq!(sessions.len(), 1);
            assert_eq!(sessions[0].exit_code, Some(0));
        }
        _ => unreachable!(),
    }
    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn second_daemon_on_same_endpoint_fails() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, server) = start(dir.path()).await;
    let cfg = DaemonConfig { endpoint: endpoint.clone(), state_dir: dir.path().to_path_buf(), scrollback: 1000 };
    assert!(jojod::server::run(cfg).await.is_err());
    let mut c = Client::connect(&endpoint).await.unwrap();
    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn errors_go_to_requester() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    c.send(Frame::Control(ControlMsg::Attach { session_id: jojo_proto::SessionId::new() })).await.unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Error { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::Error { request, .. }) => assert_eq!(request, "Attach"),
        _ => unreachable!(),
    }
    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
}
```

Replace `crates/jojod/src/lib.rs`:
```rust
//! jojod: the Jojo session daemon.

pub mod client;
pub mod command;
pub mod daemon;
pub mod paths;
#[cfg(windows)]
mod pipe_security;
pub mod server;
pub mod session;
pub mod store;
pub mod transport;

/// Daemon version reported in `Hello` and by `jojod --version`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Scrollback lines kept per session (spec section 5.2).
pub const DEFAULT_SCROLLBACK: usize = 10_000;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jojod --test daemon_roundtrip`
Expected: compile errors (missing modules `client`, `daemon`, `server`, `transport`).

- [ ] **Step 3: Implement the Windows pipe security descriptor**

`crates/jojod/src/pipe_security.rs`:
```rust
//! Security attributes that grant pipe access only to the current user's
//! SID (spec section 8.1). Without them, the default named pipe DACL grants
//! read access to Everyone.

use std::ffi::c_void;
use std::io;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, HANDLE};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

pub struct UserOnly {
    descriptor: PSECURITY_DESCRIPTOR,
    attributes: SECURITY_ATTRIBUTES,
}

impl UserOnly {
    /// Boxed so the pointer returned by `as_ptr` stays valid while the box lives.
    pub fn new() -> io::Result<Box<UserOnly>> {
        let sid = current_user_sid()?;
        let sddl: Vec<u16> = format!("D:P(A;;GA;;;{sid})").encode_utf16().chain(Some(0)).collect();
        let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
        // SAFETY: sddl is NUL-terminated; descriptor receives a LocalAlloc'd
        // buffer freed in Drop.
        let ok = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Box::new(UserOnly {
            descriptor,
            attributes: SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor,
                bInheritHandle: 0,
            },
        }))
    }

    pub fn as_ptr(&mut self) -> *mut c_void {
        &mut self.attributes as *mut SECURITY_ATTRIBUTES as *mut c_void
    }
}

impl Drop for UserOnly {
    fn drop(&mut self) {
        // SAFETY: descriptor came from ConvertStringSecurityDescriptorToSecurityDescriptorW.
        unsafe {
            LocalFree(self.descriptor as _);
        }
    }
}

/// The current process user's SID in string form, for example `S-1-5-21-...`.
fn current_user_sid() -> io::Result<String> {
    // SAFETY: standard token query sequence; every handle and buffer is
    // released before returning.
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut len = 0u32;
        GetTokenInformation(token, TokenUser, null_mut(), 0, &mut len);
        // u64 storage keeps TOKEN_USER correctly aligned.
        let mut buf = vec![0u64; (len as usize).div_ceil(8)];
        let ok = GetTokenInformation(token, TokenUser, buf.as_mut_ptr() as *mut c_void, len, &mut len);
        CloseHandle(token);
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut wide: *mut u16 = null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut wide) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut n = 0;
        while *wide.add(n) != 0 {
            n += 1;
        }
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(wide, n));
        LocalFree(wide as _);
        Ok(sid)
    }
}
```

- [ ] **Step 4: Implement the transport**

`crates/jojod/src/transport.rs`:
```rust
//! Byte-stream transport between the daemon and its clients: a named pipe
//! on Windows, a Unix socket on Linux (spec section 3.1).

use crate::paths::Endpoint;
use std::io;
use tokio::io::{AsyncRead, AsyncWrite};

pub type BoxRead = Box<dyn AsyncRead + Unpin + Send>;
pub type BoxWrite = Box<dyn AsyncWrite + Unpin + Send>;

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::pipe_security::UserOnly;
    use std::time::Duration;
    use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions};

    const ERROR_PIPE_BUSY: i32 = 231;

    pub struct Listener {
        name: String,
        next: NamedPipeServer,
    }

    fn create(name: &str, first: bool) -> io::Result<NamedPipeServer> {
        let mut security = UserOnly::new()?;
        let mut options = ServerOptions::new();
        // first_pipe_instance(true) fails if another daemon already owns the
        // name, which makes the daemon single-instance per user.
        options.first_pipe_instance(first).reject_remote_clients(true);
        // SAFETY: the security attributes outlive the create call.
        unsafe { options.create_with_security_attributes_raw(name, security.as_ptr()) }
    }

    impl Listener {
        pub fn bind(endpoint: &Endpoint) -> io::Result<Listener> {
            Ok(Listener { name: endpoint.0.clone(), next: create(&endpoint.0, true)? })
        }

        pub async fn accept(&mut self) -> io::Result<(BoxRead, BoxWrite)> {
            self.next.connect().await?;
            let fresh = create(&self.name, false)?;
            let connected = std::mem::replace(&mut self.next, fresh);
            let (r, w) = tokio::io::split(connected);
            Ok((Box::new(r), Box::new(w)))
        }
    }

    pub async fn connect(endpoint: &Endpoint) -> io::Result<(BoxRead, BoxWrite)> {
        loop {
            match ClientOptions::new().open(&endpoint.0) {
                Ok(client) => {
                    let (r, w) = tokio::io::split(client);
                    return Ok((Box::new(r), Box::new(w)));
                }
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

#[cfg(unix)]
mod imp {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use tokio::net::{UnixListener, UnixStream};

    pub struct Listener {
        inner: UnixListener,
    }

    impl Listener {
        pub fn bind(endpoint: &Endpoint) -> io::Result<Listener> {
            let path = Path::new(&endpoint.0);
            if path.exists() {
                if std::os::unix::net::UnixStream::connect(path).is_ok() {
                    return Err(io::Error::new(io::ErrorKind::AddrInUse, "jojod is already running"));
                }
                std::fs::remove_file(path)?;
            }
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
                std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
            }
            let inner = UnixListener::bind(path)?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
            Ok(Listener { inner })
        }

        pub async fn accept(&mut self) -> io::Result<(BoxRead, BoxWrite)> {
            let (stream, _) = self.inner.accept().await?;
            let (r, w) = stream.into_split();
            Ok((Box::new(r), Box::new(w)))
        }
    }

    pub async fn connect(endpoint: &Endpoint) -> io::Result<(BoxRead, BoxWrite)> {
        let (r, w) = UnixStream::connect(&endpoint.0).await?.into_split();
        Ok((Box::new(r), Box::new(w)))
    }
}

pub use imp::{connect, Listener};
```

- [ ] **Step 5: Implement the daemon**

`crates/jojod/src/daemon.rs`:
```rust
//! Request handling. Mutations are broadcast to every client as events;
//! only `Error` replies go to the requester alone (Plan 1, "Protocol
//! Additions").

use crate::command::{command_for, default_label};
use crate::paths::Endpoint;
use crate::session::{ClientId, Session, SpawnSpec};
use crate::store::{now_ms, Store};
use jojo_proto::{
    AgentState, ControlMsg, Frame, ProjectId, ProjectInfo, SessionId, SessionInfo, SessionKind,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc::UnboundedSender, Notify};

pub struct DaemonConfig {
    pub endpoint: Endpoint,
    pub state_dir: PathBuf,
    pub scrollback: usize,
}

pub type Reply = UnboundedSender<Frame>;

pub struct Daemon {
    pub cfg: DaemonConfig,
    store: Mutex<Store>,
    sessions: Mutex<HashMap<SessionId, Arc<Session>>>,
    events: broadcast::Sender<ControlMsg>,
    next_client: AtomicU64,
    pub shutdown: Notify,
}

impl Daemon {
    pub fn new(cfg: DaemonConfig) -> anyhow::Result<Arc<Daemon>> {
        let store = Store::load(&cfg.state_dir.join("state.json"))?;
        store.save()?;
        let (events, _) = broadcast::channel(1024);
        Ok(Arc::new(Daemon {
            cfg,
            store: Mutex::new(store),
            sessions: Mutex::new(HashMap::new()),
            events,
            next_client: AtomicU64::new(1),
            shutdown: Notify::new(),
        }))
    }

    pub fn next_client_id(&self) -> ClientId {
        self.next_client.fetch_add(1, Ordering::Relaxed)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ControlMsg> {
        self.events.subscribe()
    }

    fn broadcast(&self, msg: ControlMsg) {
        let _ = self.events.send(msg);
    }

    fn error(reply: &Reply, request: &str, message: impl Into<String>) {
        let _ = reply.send(Frame::Control(ControlMsg::Error {
            request: request.to_string(),
            message: message.into(),
        }));
    }

    fn save(&self, store: &Store) {
        // A failed write is retried on the next change; the in-memory state
        // stays authoritative while the daemon runs.
        let _ = store.save();
    }

    fn session(&self, id: SessionId) -> Option<Arc<Session>> {
        self.sessions.lock().expect("sessions lock").get(&id).cloned()
    }

    /// Handles one frame from `client`.
    pub fn handle(self: &Arc<Self>, client: ClientId, reply: &Reply, frame: Frame) {
        match frame {
            Frame::Input { session_id, bytes } => {
                if let Some(s) = self.session(session_id) {
                    s.write(bytes);
                }
            }
            Frame::Control(msg) => self.handle_control(client, reply, msg),
            Frame::Output { .. } | Frame::Snapshot { .. } => {
                Self::error(reply, "Frame", "clients do not send Output or Snapshot frames")
            }
        }
    }

    fn handle_control(self: &Arc<Self>, client: ClientId, reply: &Reply, msg: ControlMsg) {
        match msg {
            ControlMsg::ListProjects => {
                let projects = self.store.lock().expect("store lock").data.projects.clone();
                let _ = reply.send(Frame::Control(ControlMsg::Projects { projects }));
            }
            ControlMsg::AddProject { path, name } => {
                if !std::path::Path::new(&path).is_dir() {
                    return Self::error(reply, "AddProject", format!("not a directory: {path}"));
                }
                let project = ProjectInfo { id: ProjectId::new(), path, name };
                let mut store = self.store.lock().expect("store lock");
                store.data.projects.push(project.clone());
                self.save(&store);
                drop(store);
                self.broadcast(ControlMsg::ProjectAdded { project });
            }
            ControlMsg::RemoveProject { id } => {
                let mut store = self.store.lock().expect("store lock");
                let running = store
                    .data
                    .sessions
                    .iter()
                    .filter(|s| s.project_id == id && s.state != AgentState::Exited)
                    .count();
                if running > 0 {
                    return Self::error(reply, "RemoveProject", format!("project has {running} running sessions"));
                }
                store.data.projects.retain(|p| p.id != id);
                store.data.sessions.retain(|s| s.project_id != id);
                self.save(&store);
                drop(store);
                self.broadcast(ControlMsg::ProjectRemoved { id });
            }
            ControlMsg::ListDir { path } => match list_dirs(&path) {
                Ok(entries) => {
                    let _ = reply.send(Frame::Control(ControlMsg::DirEntries { path, entries }));
                }
                Err(e) => Self::error(reply, "ListDir", format!("{path}: {e}")),
            },
            ControlMsg::ListSessions => {
                let sessions = self.store.lock().expect("store lock").data.sessions.clone();
                let _ = reply.send(Frame::Control(ControlMsg::Sessions { sessions }));
            }
            ControlMsg::CreateSession { project_id, kind, cols, rows, shell } => {
                self.create_session(reply, project_id, kind, cols, rows, shell.as_deref())
            }
            ControlMsg::Attach { session_id } => match self.session(session_id) {
                Some(s) => s.attach(client, reply.clone()),
                None => Self::error(reply, "Attach", "session has no screen (not running in this daemon)"),
            },
            ControlMsg::Detach { session_id } => {
                if let Some(s) = self.session(session_id) {
                    s.detach(client);
                }
            }
            ControlMsg::Resize { session_id, cols, rows } => {
                if let Some(s) = self.session(session_id) {
                    s.resize(cols, rows);
                }
            }
            ControlMsg::Kill { session_id } => {
                if let Some(s) = self.session(session_id) {
                    s.kill();
                }
            }
            ControlMsg::CloseSession { session_id } => {
                if let Some(s) = self.sessions.lock().expect("sessions lock").remove(&session_id) {
                    s.kill();
                }
                let mut store = self.store.lock().expect("store lock");
                store.data.sessions.retain(|s| s.id != session_id);
                self.save(&store);
                drop(store);
                self.broadcast(ControlMsg::SessionRemoved { session_id });
            }
            ControlMsg::RenameSession { session_id, label } => {
                let mut store = self.store.lock().expect("store lock");
                let Some(info) = store.session_mut(session_id) else {
                    return Self::error(reply, "RenameSession", "unknown session");
                };
                info.label = label;
                let session = info.clone();
                self.save(&store);
                drop(store);
                self.broadcast(ControlMsg::SessionUpdated { session });
            }
            ControlMsg::Shutdown => self.shutdown.notify_one(),
            other => Self::error(reply, &variant_name(&other), "not a client request"),
        }
    }

    fn create_session(
        self: &Arc<Self>,
        reply: &Reply,
        project_id: ProjectId,
        kind: SessionKind,
        cols: u16,
        rows: u16,
        shell: Option<&str>,
    ) {
        let project = {
            let store = self.store.lock().expect("store lock");
            store.data.projects.iter().find(|p| p.id == project_id).cloned()
        };
        let Some(project) = project else {
            return Self::error(reply, "CreateSession", "unknown project");
        };
        let (program, args) = command_for(kind, shell);
        let id = SessionId::new();
        let now = now_ms();
        let info = SessionInfo {
            id,
            project_id,
            kind,
            command: program.clone(),
            args: args.clone(),
            label: default_label(kind, &program),
            created_at: now,
            state: if kind == SessionKind::Claude { AgentState::Starting } else { AgentState::Running },
            state_changed_at: now,
            exit_code: None,
            claude_session_id: None,
        };
        // Record the session before spawning so an immediate exit always
        // finds its entry.
        {
            let mut store = self.store.lock().expect("store lock");
            store.data.sessions.push(info.clone());
            self.save(&store);
        }
        self.broadcast(ControlMsg::SessionCreated { session: info });

        let spec = SpawnSpec {
            program: program.clone(),
            args,
            cwd: PathBuf::from(&project.path),
            env: vec![
                ("TERM".into(), "xterm-256color".into()),
                ("COLORTERM".into(), "truecolor".into()),
                ("JOJO_SESSION_ID".into(), id.to_string()),
            ],
            cols,
            rows,
            scrollback: self.cfg.scrollback,
        };
        let weak = Arc::downgrade(self);
        let on_exit = move |code: Option<i32>| {
            if let Some(daemon) = weak.upgrade() {
                daemon.on_session_exit(id, code);
            }
        };
        match Session::spawn(id, spec, on_exit) {
            Ok(session) => {
                self.sessions.lock().expect("sessions lock").insert(id, session);
            }
            Err(e) => {
                let path = std::env::var("PATH").unwrap_or_default();
                Self::error(reply, "CreateSession", format!("{e:#}. PATH searched: {path}"));
                self.on_session_exit(id, Some(-1));
            }
        }
    }

    fn on_session_exit(&self, id: SessionId, exit_code: Option<i32>) {
        let mut store = self.store.lock().expect("store lock");
        if let Some(info) = store.session_mut(id) {
            info.state = AgentState::Exited;
            info.exit_code = exit_code;
            info.state_changed_at = now_ms();
            self.save(&store);
            drop(store);
            self.broadcast(ControlMsg::SessionExited { session_id: id, exit_code });
        }
    }

    /// Removes `client` from every session's subscribers.
    pub fn client_gone(&self, client: ClientId) {
        for s in self.sessions.lock().expect("sessions lock").values() {
            s.detach(client);
        }
    }

    pub fn kill_all(&self) {
        for s in self.sessions.lock().expect("sessions lock").values() {
            s.kill();
        }
    }
}

fn list_dirs(path: &str) -> std::io::Result<Vec<String>> {
    let mut entries: Vec<String> = std::fs::read_dir(path)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort_by_key(|s| s.to_lowercase());
    Ok(entries)
}

/// The JSON `type` tag of a message, for example `"Projects"`.
fn variant_name(msg: &ControlMsg) -> String {
    serde_json::to_value(msg)
        .ok()
        .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(str::to_owned))
        .unwrap_or_else(|| "Unknown".to_string())
}
```

- [ ] **Step 6: Implement the server and client**

`crates/jojod/src/server.rs`:
```rust
//! Accept loop and per-connection loop.

use crate::daemon::{Daemon, DaemonConfig};
use crate::transport::{BoxRead, BoxWrite, Listener};
use jojo_proto::{read_frame, write_frame, ControlMsg, Frame, PROTOCOL_VERSION};
use std::sync::Arc;
use tokio::sync::{broadcast::error::RecvError, mpsc};

/// Runs the daemon until a `Shutdown` request, then kills every session.
pub async fn run(cfg: DaemonConfig) -> anyhow::Result<()> {
    let mut listener = Listener::bind(&cfg.endpoint)?;
    let daemon = Daemon::new(cfg)?;
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (reader, writer) = accepted?;
                tokio::spawn(serve_connection(daemon.clone(), reader, writer));
            }
            _ = daemon.shutdown.notified() => break,
        }
    }
    daemon.kill_all();
    Ok(())
}

async fn serve_connection(daemon: Arc<Daemon>, mut reader: BoxRead, writer: BoxWrite) {
    let client = daemon.next_client_id();
    let (tx, mut rx) = mpsc::unbounded_channel::<Frame>();

    let writer_task = tokio::spawn(async move {
        let mut writer = writer;
        while let Some(frame) = rx.recv().await {
            if write_frame(&mut writer, &frame).await.is_err() {
                break;
            }
        }
    });

    let _ = tx.send(Frame::Control(ControlMsg::Hello {
        protocol_version: PROTOCOL_VERSION,
        jojod_version: crate::VERSION.to_string(),
    }));

    match read_frame(&mut reader).await {
        Ok(Some(Frame::Control(ControlMsg::Hello { protocol_version, .. })))
            if protocol_version == PROTOCOL_VERSION => {}
        Ok(Some(Frame::Control(ControlMsg::Hello { protocol_version, .. }))) => {
            let _ = tx.send(Frame::Control(ControlMsg::Error {
                request: "Hello".into(),
                message: format!("protocol mismatch: daemon {PROTOCOL_VERSION}, client {protocol_version}"),
            }));
            drop(tx);
            let _ = writer_task.await;
            return;
        }
        _ => {
            drop(tx);
            let _ = writer_task.await;
            return;
        }
    }

    let mut events = daemon.subscribe();
    let event_tx = tx.clone();
    let event_task = tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(msg) => {
                    if event_tx.send(Frame::Control(msg)).is_err() {
                        break;
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    });

    while let Ok(Some(frame)) = read_frame(&mut reader).await {
        daemon.handle(client, &tx, frame);
    }

    daemon.client_gone(client);
    event_task.abort();
    drop(tx);
    let _ = writer_task.await;
}
```

`crates/jojod/src/client.rs`:
```rust
//! Client side of the protocol, used by `jojod shutdown` and the tests.

use crate::paths::Endpoint;
use crate::transport::{connect, BoxRead, BoxWrite};
use jojo_proto::{read_frame, write_frame, ControlMsg, Frame, PROTOCOL_VERSION};
use std::io;
use std::time::Duration;

pub struct Client {
    reader: BoxRead,
    writer: BoxWrite,
}

impl Client {
    /// Connects and completes the `Hello` exchange.
    pub async fn connect(endpoint: &Endpoint) -> io::Result<Client> {
        let (reader, writer) = connect(endpoint).await?;
        let mut client = Client { reader, writer };
        client
            .send(Frame::Control(ControlMsg::Hello {
                protocol_version: PROTOCOL_VERSION,
                jojod_version: crate::VERSION.to_string(),
            }))
            .await?;
        match client.recv().await? {
            Frame::Control(ControlMsg::Hello { protocol_version, .. }) if protocol_version == PROTOCOL_VERSION => {
                Ok(client)
            }
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unexpected first frame from daemon: {other:?}"),
            )),
        }
    }

    pub async fn send(&mut self, frame: Frame) -> io::Result<()> {
        write_frame(&mut self.writer, &frame).await
    }

    pub async fn recv(&mut self) -> io::Result<Frame> {
        read_frame(&mut self.reader)
            .await?
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "daemon closed the connection"))
    }

    /// Receives frames until `pred` matches one, discarding the others.
    pub async fn recv_until(
        &mut self,
        timeout: Duration,
        mut pred: impl FnMut(&Frame) -> bool,
    ) -> io::Result<Frame> {
        tokio::time::timeout(timeout, async {
            loop {
                let frame = self.recv().await?;
                if pred(&frame) {
                    return Ok(frame);
                }
            }
        })
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "timed out waiting for frame"))?
    }
}
```

- [ ] **Step 7: Run all jojod tests**

Run: `cargo test -p jojod -- --test-threads=1`
Expected: all unit tests from Tasks 5–6 and the 4 tests in `daemon_roundtrip.rs` pass. If `create_with_security_attributes_raw` returns `ERROR_ACCESS_DENIED`, print the SDDL string in a temporary `eprintln!` and check it with `ConvertStringSecurityDescriptorToSecurityDescriptorW` documentation; the SID must start with `S-1-5-`.

- [ ] **Step 8: Commit**

```bash
git add crates/jojod
git commit -m "feat(jojod): serve the protocol over a user-only named pipe

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: CLI (`serve`, `shutdown`, `--version`) and manual smoke test

**Files:**
- Modify: `crates/jojod/src/main.rs`
- Create: `docs/manual-test-checklist.md`

**Interfaces:**
- Consumes: `jojod::server::run`, `jojod::client::Client`, `jojod::paths`, `jojod::DEFAULT_SCROLLBACK`, `jojo_proto::PROTOCOL_VERSION`.
- Produces: the `jojod` binary with `jojod serve [--endpoint <name>] [--state-dir <dir>]`, `jojod shutdown [--endpoint <name>]`, `jojod --version` printing `jojod <version> (protocol <n>)`. Plan 2's app starts `jojod.exe serve`.

- [ ] **Step 1: Write the CLI**

`crates/jojod/src/main.rs`:
```rust
use clap::{CommandFactory, Parser, Subcommand};
use jojo_proto::{ControlMsg, Frame, PROTOCOL_VERSION};
use jojod::client::Client;
use jojod::daemon::DaemonConfig;
use jojod::paths::{default_endpoint, default_state_dir, Endpoint};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "jojod", about = "Jojo session daemon", disable_version_flag = true)]
struct Cli {
    /// Print the daemon version and protocol version.
    #[arg(short = 'V', long = "version")]
    version: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the daemon in the foreground.
    Serve {
        /// Pipe name (Windows) or socket path (Linux). Defaults to the per-user endpoint.
        #[arg(long)]
        endpoint: Option<String>,
        /// Directory for state.json. Defaults to %LOCALAPPDATA%\Jojo or ~/.jojo.
        #[arg(long)]
        state_dir: Option<PathBuf>,
    },
    /// Ask the running daemon to end all sessions and exit.
    Shutdown {
        #[arg(long)]
        endpoint: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if cli.version {
        println!("jojod {} (protocol {})", jojod::VERSION, PROTOCOL_VERSION);
        return Ok(());
    }
    match cli.command {
        Some(Command::Serve { endpoint, state_dir }) => {
            let cfg = DaemonConfig {
                endpoint: endpoint.map(Endpoint).unwrap_or_else(default_endpoint),
                state_dir: state_dir.unwrap_or_else(default_state_dir),
                scrollback: jojod::DEFAULT_SCROLLBACK,
            };
            jojod::server::run(cfg).await
        }
        Some(Command::Shutdown { endpoint }) => {
            let endpoint = endpoint.map(Endpoint).unwrap_or_else(default_endpoint);
            let mut client = Client::connect(&endpoint).await?;
            client.send(Frame::Control(ControlMsg::Shutdown)).await?;
            Ok(())
        }
        None => {
            Cli::command().print_help()?;
            Ok(())
        }
    }
}
```

- [ ] **Step 2: Verify the version output**

Run: `cargo run -q -p jojod -- --version`
Expected: `jojod 0.1.0 (protocol 1)`.

- [ ] **Step 3: Run the manual smoke test**

In terminal 1:
```powershell
cargo run -q -p jojod -- serve --state-dir "$env:TEMP\jojo-smoke"
```
Expected: the command keeps running with no output.

In terminal 2:
```powershell
Get-ChildItem \\.\pipe\ | Where-Object Name -like 'jojo-*'
cargo run -q -p jojod -- shutdown
Get-Content "$env:TEMP\jojo-smoke\state.json"
```
Expected: the first command lists `jojo-<your username>`. After `shutdown`, terminal 1's command exits with code 0. `state.json` contains `{"projects": [], "sessions": []}` (pretty-printed).

- [ ] **Step 4: Write the manual test checklist**

`docs/manual-test-checklist.md`:
```markdown
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
```

- [ ] **Step 5: Run the full workspace test suite**

Run: `cargo test --workspace -- --test-threads=1`
Expected: every test in `jojo-proto`, `jojo-grid` and `jojod` passes.

- [ ] **Step 6: Commit**

```bash
git add crates/jojod/src/main.rs docs/manual-test-checklist.md
git commit -m "feat(jojod): add serve, shutdown and --version commands

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Self-Review Notes

- **Spec coverage for Plan 1's scope:** 3.1 daemon and transport (Tasks 6–7); 3.2 `serve`, `shutdown`, `--version` (Task 8). The `--daemonize`, `attach --stdio`, `hook` and `install-hooks` subcommands are in Plans 4–5. 4 store (Task 5); 5.1 creation with env and command rules (Tasks 5, 7); 5.2 scrollback (Tasks 3, 7); 5.3 exit keeps grid (Task 6: grid lives until `CloseSession`); 5.4 restart marking (Task 5; Resume in Plan 4); 5.5 attach/snapshot/detach and the single-lock rule (Tasks 4, 6); 5.6 resize (Task 6); 6.1 framing (Task 2); 6.2 messages (Task 2, plus additions listed above); 8.1 pipe name, user-only ACL, single instance (Task 7; auto-start from the app in Plan 2); 10 rows for spawn failure, corrupt `state.json` and protocol mismatch (Tasks 5, 7); 11 items 1 (synthetic fixtures; recorded fixtures and the `@xterm/headless` cross-check in Plan 2), 3 (round trip; `ts-rs` check in Plan 2), 4 (Task 7).
- **Type consistency:** `SessionId::to_bytes/from_bytes`, `Frame` variants, `SpawnSpec` fields, `Session::{spawn, attach, detach, write, resize, kill, visible_text}`, `DaemonConfig { endpoint, state_dir, scrollback }` and `Client::{connect, send, recv, recv_until}` are used with the same names and types in every task.
