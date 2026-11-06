# Jojo Plan 2a: Protocol and Daemon Preparation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the `jojod` daemon and wire protocol to the state the Tauri app (Plan 2b) builds on: protocol version 2, connection safety, bounded client queues, logging, generated TypeScript protocol types, a sideloaded current ConPTY, and snapshot tests driven by recordings of real Windows sessions.

**Architecture:** All work is in the existing Cargo workspace (`crates/jojo-proto`, `crates/jojo-grid`, `crates/jojod`) plus two new folders: `scripts/` (PowerShell helpers) and `tests/` (recordings and a small Node package that checks snapshots against `@xterm/headless`). `app/src/lib/protocol/` is created here only to receive the generated TypeScript types; the rest of `app/` is Plan 2b.

**Tech Stack:** Rust stable 1.99, tokio 1, alacritty_terminal 0.25.1, vte 0.15, portable-pty 0.9.0, windows-sys 0.59, ts-rs 12, tracing 0.1 / tracing-subscriber 0.3 / tracing-appender 0.2; Node 22 with pnpm 10, `@xterm/headless` 6.0.0, `@xterm/addon-unicode11` 0.9.0, vitest.

**Inputs:** Spec `docs/superpowers/specs/2026-11-06-jojo-design.md`. Plan 1 carry-over `docs/superpowers/plans/2026-11-06-jojo-plan-1-carryover.md`, sections 2.1, 2.3 and 2.4 (section 2.2 is app behavior and belongs to Plan 2b).

## Why Plan 2 Is Split

The roadmap in Plan 1 listed the app shell as one plan. The carry-over added protocol changes that must be settled before TypeScript types are generated, plus daemon work the app depends on. Plan 2a (this plan) produces a tested daemon and protocol with no UI. Plan 2b (written after 2a is complete) builds the Tauri window on top.

## Global Constraints

- All Plan 1 global constraints still apply (names, paths, frame format, `state.json` rules, US spelling, commit trailer naming the authoring model).
- `PROTOCOL_VERSION = 2`.
- Accepted session sizes: columns `2..=1000`, rows `1..=1000` (`jojo_grid::{MIN_COLS, MAX_COLS, MIN_ROWS, MAX_ROWS}`).
- Largest snapshot payload: `MAX_FRAME_LEN - 1 - 16` bytes (frame type byte and 16-byte session id).
- Per-client queued output limit: `MAX_CLIENT_QUEUE_BYTES = 8 * 1024 * 1024`.
- Client connect deadline: 5 seconds.
- Lock order inside a session: `shared` before `master`. Lock order in the daemon: `store` before `sessions`. Never acquire in the reverse order.
- Logs record metadata only (session ids, states, connection events, errors), never terminal input or output bytes.
- Windows log location: `%LOCALAPPDATA%\Jojo\logs\jojod.<date>.log`; Linux: `~/.jojo/logs/jojod.<date>.log`. Rotated daily, 7 files kept. (Spec section 10 named `~/.jojo/jojod.log` on Linux; a `logs` folder is used on both platforms for consistency.)

## Deferred From Carry-Over

Moving blocking work (`Session::spawn`, snapshotting, `store.save()`, `read_dir`) onto `spawn_blocking` is deferred: it is a performance change that requires making `Daemon::handle` asynchronous, and nothing in Plan 2b depends on it. It is recorded again in Plan 2b's inputs.

## File Structure

```
crates/jojo-proto/
  Cargo.toml                         # + ts-rs
  src/msg.rs                         # Attach size, Error.session_id, SessionInfo.attachable, TS derives
  src/ids.rs                         # TS derives
  src/frame.rs                       # BadPayload, write_frame size check
  src/lib.rs                         # PROTOCOL_VERSION = 2, re-exports
  tests/bindings.rs                  # exports TypeScript into app/src/lib/protocol
crates/jojo-grid/
  src/snapshot.rs                    # snapshot_skipping, snapshot_bounded, pending-wrap fixes
  tests/roundtrip.rs                 # new fixtures
  tests/recordings.rs                # round-trips every recording, writes cross-check inputs
crates/jojod/
  Cargo.toml                         # + tracing crates; windows-sys features
  src/client_queue.rs                # ClientTx / ClientRx with byte accounting
  src/session.rs                     # attach with size, queue limit, Drop, spawn-error cleanup
  src/daemon.rs                      # v2 handling, request hygiene, attachable, logging
  src/server.rs                      # malformed frames, lag disconnect, ClientRx writer
  src/pipe_security.rs               # is_elevated, verify_server_is_current_user
  src/transport.rs                   # owner check, connect deadline
  src/client.rs                      # connect deadline, send_raw
  src/logging.rs                     # tracing setup
  src/store.rs                       # corrupt-file suffix in ms, attachable reset
  src/main.rs                        # elevation refusal, logging init
  examples/record.rs                 # records a PTY session to tests/recordings
  tests/daemon_roundtrip.rs          # new end-to-end tests
  tests/conpty_sideload.rs           # proves the sideloaded conpty.dll is used
scripts/fetch-conpty.ps1             # downloads the pinned ConPTY NuGet package
scripts/check-protocol.ps1           # regenerates TS types, fails on drift
resources/conpty/SHA256SUMS          # pinned hashes (binaries git-ignored)
tests/recordings/*.bin               # committed recordings
tests/xterm-crosscheck/              # Node package: package.json, crosscheck.test.ts
app/src/lib/protocol/*.ts            # generated, committed
```

---

### Task 1: Protocol version 2

**Files:**
- Modify: `crates/jojo-proto/src/msg.rs`, `crates/jojo-proto/src/frame.rs`, `crates/jojo-proto/src/lib.rs`
- Modify: `crates/jojo-grid/src/snapshot.rs`, `crates/jojo-grid/src/lib.rs`
- Modify: `crates/jojod/src/session.rs`, `crates/jojod/src/daemon.rs`, `crates/jojod/src/server.rs`, `crates/jojod/src/store.rs`, `crates/jojod/src/client.rs`
- Test: `crates/jojo-proto/src/frame.rs` (tests module), `crates/jojo-grid/tests/roundtrip.rs`, `crates/jojod/src/session.rs` (tests module), `crates/jojod/tests/daemon_roundtrip.rs`

**Interfaces:**
- Consumes: Plan 1 code as merged on `master` (`cc5d21b`).
- Produces:
  - `ControlMsg::Attach { session_id: SessionId, cols: u16, rows: u16 }`.
  - `ControlMsg::Error { request: String, message: String, session_id: Option<SessionId> }` (`session_id` defaults to `None` when absent in JSON).
  - `SessionInfo.attachable: bool` (defaults to `false` when absent in JSON; computed by the daemon, never trusted from `state.json`).
  - `jojo_proto::BadPayload`, `jojo_proto::is_bad_payload(&io::Error) -> bool`.
  - `write_frame` returns `io::ErrorKind::InvalidInput` for a frame whose length field would exceed `MAX_FRAME_LEN`.
  - `TermGrid::snapshot_bounded(&self, max_len: usize) -> Vec<u8>`.
  - `jojod::session::MAX_SNAPSHOT_BYTES: usize`.
  - `Session::attach(&self, client: ClientId, tx: UnboundedSender<Frame>, cols: u16, rows: u16)`; `Session::size(&self) -> (u16, u16)`.
  - `Client::send_raw(&mut self, bytes: &[u8]) -> io::Result<()>`.

- [ ] **Step 1: Write the failing protocol tests**

Add to the tests module in `crates/jojo-proto/src/frame.rs`:
```rust
    #[tokio::test]
    async fn bad_control_json_is_reported_and_stream_stays_aligned() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        use tokio::io::AsyncWriteExt;
        let body = b"{not json";
        a.write_all(&((1 + body.len()) as u32).to_le_bytes()).await.unwrap();
        a.write_all(&[FRAME_CONTROL]).await.unwrap();
        a.write_all(body).await.unwrap();
        write_frame(&mut a, &Frame::Control(ControlMsg::ListSessions)).await.unwrap();

        let err = read_frame(&mut b).await.unwrap_err();
        assert!(is_bad_payload(&err), "expected BadPayload, got {err:?}");
        let next = read_frame(&mut b).await.unwrap().expect("next frame");
        assert_eq!(next, Frame::Control(ControlMsg::ListSessions));
    }

    #[tokio::test]
    async fn oversized_length_is_not_a_bad_payload() {
        let (mut a, mut b) = tokio::io::duplex(64);
        use tokio::io::AsyncWriteExt;
        a.write_all(&((MAX_FRAME_LEN as u32) + 1).to_le_bytes()).await.unwrap();
        let err = read_frame(&mut b).await.unwrap_err();
        assert!(!is_bad_payload(&err));
    }

    #[tokio::test]
    async fn write_frame_refuses_frames_over_the_limit() {
        let (mut a, _b) = tokio::io::duplex(64);
        let frame = Frame::Snapshot { session_id: SessionId::new(), bytes: vec![b'x'; MAX_FRAME_LEN] };
        let err = write_frame(&mut a, &frame).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn error_without_session_id_still_parses() {
        let json = r#"{"type":"Error","request":"Attach","message":"x"}"#;
        let msg: ControlMsg = serde_json::from_str(json).unwrap();
        assert_eq!(
            msg,
            ControlMsg::Error { request: "Attach".into(), message: "x".into(), session_id: None }
        );
    }

    #[test]
    fn attach_carries_size() {
        let id = SessionId::new();
        let json = serde_json::to_string(&ControlMsg::Attach { session_id: id, cols: 100, rows: 30 }).unwrap();
        assert!(json.contains(r#""cols":100"#) && json.contains(r#""rows":30"#), "{json}");
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p jojo-proto`
Expected: compile errors (`is_bad_payload` not found; `Attach` has no field `cols`; `Error` has no field `session_id`).

- [ ] **Step 3: Implement the protocol changes**

In `crates/jojo-proto/src/msg.rs`:
- Change the `Attach` variant to `Attach { session_id: SessionId, cols: u16, rows: u16 },`.
- Change the `Error` variant to:
```rust
    Error {
        request: String,
        message: String,
        /// The session the error concerns, when there is one (for example
        /// a `CreateSession` whose program failed to start).
        #[serde(default)]
        session_id: Option<SessionId>,
    },
```
- Add to `SessionInfo`, after `claude_session_id`:
```rust
    /// True when this daemon still holds a screen for the session, so
    /// `Attach` will succeed. Computed by the daemon for every message that
    /// carries a `SessionInfo`; the value stored in state.json is ignored.
    #[serde(default)]
    pub attachable: bool,
```

In `crates/jojo-proto/src/frame.rs`, add below `invalid`:
```rust
/// Marks a frame whose length prefix and body were read completely but whose
/// payload could not be decoded (bad JSON, unknown frame type, short data
/// frame). The stream is still positioned at the next frame, so a server can
/// reply with an error and keep the connection open.
#[derive(Debug)]
pub struct BadPayload(pub String);

impl std::fmt::Display for BadPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for BadPayload {}

fn bad_payload(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, BadPayload(message.into()))
}

/// True when `err` came from a frame that was fully read but could not be
/// decoded. Any other `read_frame` error leaves the stream misaligned.
pub fn is_bad_payload(err: &io::Error) -> bool {
    err.get_ref().is_some_and(|inner| inner.is::<BadPayload>())
}
```
In `Frame::decode`, replace each `invalid(...)` call with `bad_payload(...)` (three places: bad control JSON, short data frame, unknown frame type). Keep `invalid` for the length check in `read_frame`.

Replace `write_frame` with:
```rust
pub async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, frame: &Frame) -> io::Result<()> {
    let bytes = frame.encode();
    if bytes.len() - 4 > MAX_FRAME_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("frame of {} bytes exceeds MAX_FRAME_LEN", bytes.len() - 4),
        ));
    }
    writer.write_all(&bytes).await?;
    writer.flush().await
}
```

In `crates/jojo-proto/src/lib.rs`: export `BadPayload` and `is_bad_payload` from `frame`, and set `pub const PROTOCOL_VERSION: u32 = 2;`.

- [ ] **Step 4: Run the protocol tests**

Run: `cargo test -p jojo-proto`
Expected: all tests pass. (The `jojod` crate does not compile yet; that is fixed in Step 8.)

- [ ] **Step 5: Write the failing snapshot-bound test**

Add to `crates/jojo-grid/tests/roundtrip.rs`:
```rust
#[test]
fn bounded_snapshot_fits_and_keeps_the_visible_screen() {
    let mut a = TermGrid::new(80, 24, 10_000);
    let mut input = Vec::new();
    for i in 0..10_000u32 {
        // Every cell carries its own truecolor foreground and background so
        // the full snapshot is far larger than the bound used below.
        for c in 0..80u32 {
            input.extend_from_slice(
                format!("\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}mx", c, i % 256, 7, 255 - c, 3, i % 200).as_bytes(),
            );
        }
        input.extend_from_slice(b"\x1b[0m\r\n");
    }
    input.extend_from_slice(b"last visible line");
    a.feed(&input);

    let full = a.snapshot();
    let limit = 1_000_000;
    assert!(full.len() > limit, "fixture too small: {}", full.len());

    let bounded = a.snapshot_bounded(limit);
    assert!(bounded.len() <= limit, "bounded snapshot is {} bytes", bounded.len());

    let mut b = TermGrid::new(80, 24, 10_000);
    b.feed(&bounded);
    assert_eq!(b.visible_text(), a.visible_text());
}

#[test]
fn bounded_snapshot_equals_full_snapshot_when_it_fits() {
    let mut a = TermGrid::new(80, 24, 1000);
    a.feed(b"\x1b[31mhello\x1b[0m\r\nworld");
    assert_eq!(a.snapshot_bounded(1_000_000), a.snapshot());
}
```

- [ ] **Step 6: Run it to verify it fails**

Run: `cargo test -p jojo-grid --test roundtrip bounded`
Expected: compile error, `no method named snapshot_bounded`.

- [ ] **Step 7: Implement the bounded snapshot**

In `crates/jojo-grid/src/snapshot.rs`, rename the body of `pub fn snapshot(&self) -> Vec<u8>` to a private `fn snapshot_skipping(&self, skip_history: usize) -> Vec<u8>`, and in it change the computation of the first row from

```rust
        let top = if alt { 0 } else { -(grid.history_size() as i32) };
```
to
```rust
        let history = grid.history_size();
        let kept = history - skip_history.min(history);
        let top = if alt { 0 } else { -(kept as i32) };
```

Then add:
```rust
    /// VT bytes that reproduce this grid in a fresh terminal of the same size
    /// and scrollback capacity.
    pub fn snapshot(&self) -> Vec<u8> {
        self.snapshot_skipping(0)
    }

    /// Like `snapshot`, but drops the oldest scrollback rows until the result
    /// is at most `max_len` bytes. A frame larger than `MAX_FRAME_LEN` would be
    /// rejected by the client and end its whole connection (Plan 1 carry-over
    /// 2.1, item 2). If the visible screen alone is still too large, which only
    /// a very large grid of truecolor cells can produce, the result is the
    /// screen as plain text with no attributes.
    pub fn snapshot_bounded(&self, max_len: usize) -> Vec<u8> {
        let full = self.snapshot_skipping(0);
        if full.len() <= max_len {
            return full;
        }
        // Smallest number of skipped history rows whose snapshot fits.
        let (mut lo, mut hi) = (1usize, self.term.grid().history_size());
        let mut best = None;
        while lo <= hi {
            let mid = lo + (hi - lo) / 2;
            let candidate = self.snapshot_skipping(mid);
            if candidate.len() <= max_len {
                best = Some(candidate);
                hi = mid - 1;
            } else {
                lo = mid + 1;
            }
        }
        best.unwrap_or_else(|| self.plain_screen_snapshot())
    }

    /// The visible screen as plain text after a full reset.
    fn plain_screen_snapshot(&self) -> Vec<u8> {
        let mut out = b"\x1bc\x1b[3J".to_vec();
        out.extend_from_slice(self.visible_text().replace('\n', "\r\n").as_bytes());
        out
    }
```
(`hi = mid - 1` cannot underflow: `mid >= lo >= 1`.)

- [ ] **Step 8: Write the failing session and daemon tests**

In the tests module of `crates/jojod/src/session.rs`, update every `session.attach(client, tx)` call to `session.attach(client, tx, 80, 24)`, then add:
```rust
    #[test]
    fn attach_with_a_new_size_resizes_the_session() {
        let session = Session::spawn(SessionId::new(), test_spec(), |_| {}).unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        session.attach(1, tx, 100, 30);
        assert!(matches!(rx.blocking_recv(), Some(Frame::Snapshot { .. })));
        assert_eq!(session.size(), (100, 30));
        session.kill();
    }
```

In `crates/jojod/tests/daemon_roundtrip.rs`, update every `ControlMsg::Attach { session_id: ... }` to include `cols: 80, rows: 24`, and every pattern or constructor of `ControlMsg::Error { request, message }` to include `session_id` (patterns already using `..` need no change). Then add:
```rust
#[tokio::test(flavor = "multi_thread")]
async fn bad_json_frame_gets_an_error_and_the_connection_survives() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    let body = b"{not json";
    let mut raw = ((1 + body.len()) as u32).to_le_bytes().to_vec();
    raw.push(1); // control frame
    raw.extend_from_slice(body);
    c.send_raw(&raw).await.unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Error { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::Error { request, .. }) => assert_eq!(request, "Frame"),
        _ => unreachable!(),
    }
    c.send(Frame::Control(ControlMsg::ListProjects)).await.unwrap();
    c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Projects { .. }))).await.unwrap();
    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn spawn_failure_reports_session_id_and_marks_session_unattachable() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    let project = add_project(&mut c, dir.path()).await;
    c.send(Frame::Control(ControlMsg::CreateSession {
        project_id: project.id,
        kind: SessionKind::Shell,
        cols: 80,
        rows: 24,
        shell: Some("definitely-not-a-real-program-jojo".into()),
    }))
    .await
    .unwrap();
    let created = match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::SessionCreated { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::SessionCreated { session }) => session,
        _ => unreachable!(),
    };
    assert!(!created.attachable);
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Error { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::Error { request, message, session_id }) => {
            assert_eq!(request, "CreateSession");
            assert_eq!(session_id, Some(created.id));
            assert!(message.contains("PATH searched"), "{message}");
        }
        _ => unreachable!(),
    }
    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn live_sessions_are_attachable_and_attach_validates_size() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    let project = add_project(&mut c, dir.path()).await;
    let session = create_shell(&mut c, &project).await;
    assert!(session.attachable);

    c.send(Frame::Control(ControlMsg::ListSessions)).await.unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Sessions { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::Sessions { sessions }) => assert!(sessions[0].attachable),
        _ => unreachable!(),
    }

    c.send(Frame::Control(ControlMsg::Attach { session_id: session.id, cols: 0, rows: 0 })).await.unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Error { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::Error { request, .. }) => assert_eq!(request, "Attach"),
        _ => unreachable!(),
    }
    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
}
```

In the tests module of `crates/jojod/src/store.rs`, add `attachable: false,` to the `SessionInfo` literal in the `session` helper, and add:
```rust
    #[test]
    fn load_resets_attachable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let mut store = Store::load(&path).unwrap();
        let mut s = session(AgentState::Exited, Some(0));
        s.attachable = true;
        store.data.sessions.push(s);
        store.save().unwrap();
        assert!(!Store::load(&path).unwrap().data.sessions[0].attachable);
    }
```

- [ ] **Step 9: Run them to verify they fail**

Run: `cargo test -p jojod -- --test-threads=1`
Expected: compile errors (`attach` takes 2 arguments, no method `size`, no method `send_raw`, missing `attachable` in initializers).

- [ ] **Step 10: Implement the daemon side**

`crates/jojod/src/session.rs`:
- Add below `READER_DRAIN_TIMEOUT`:
```rust
/// Largest snapshot payload that still fits in one frame together with the
/// frame type byte and the 16-byte session id.
pub const MAX_SNAPSHOT_BYTES: usize = jojo_proto::MAX_FRAME_LEN - 1 - 16;
```
- Replace `attach` and `resize`, and add `size`:
```rust
    /// Sends one Snapshot laid out for `cols` x `rows`, then live Output.
    /// The grid and PTY are resized first when the size differs, because a
    /// snapshot replays correctly only into a terminal of the grid's size.
    /// Lock order: `shared`, then `master` (same as `resize`).
    pub fn attach(&self, client: ClientId, tx: UnboundedSender<Frame>, cols: u16, rows: u16) {
        let mut s = self.shared.lock().expect("session lock");
        if (cols, rows) != (s.grid.cols(), s.grid.rows()) {
            s.grid.resize(cols, rows);
            self.resize_pty(s.grid.cols(), s.grid.rows());
        }
        let bytes = s.grid.snapshot_bounded(MAX_SNAPSHOT_BYTES);
        if tx.send(Frame::Snapshot { session_id: self.id, bytes }).is_ok() {
            s.subscribers.retain(|(c, _)| *c != client);
            s.subscribers.push((client, tx));
        }
    }

    /// Resizes the grid first, then the PTY, so ConPTY's repaint at the new
    /// size is never fed into a grid of the old size.
    pub fn resize(&self, cols: u16, rows: u16) {
        let mut s = self.shared.lock().expect("session lock");
        s.grid.resize(cols, rows);
        self.resize_pty(s.grid.cols(), s.grid.rows());
    }

    fn resize_pty(&self, cols: u16, rows: u16) {
        if let Some(master) = self.master.lock().expect("master lock").as_ref() {
            let _ = master.resize(pty_size(cols, rows));
        }
    }

    pub fn size(&self) -> (u16, u16) {
        let s = self.shared.lock().expect("session lock");
        (s.grid.cols(), s.grid.rows())
    }
```
Update the module doc comment's lock paragraph to state the order `shared` before `master`.

`crates/jojod/src/store.rs`: in `Store::load`, inside the loop over sessions, add `s.attachable = false;` for every session (before the `if s.state != Exited` check).

`crates/jojod/src/daemon.rs`:
- Change `error` to take the session id, and add a helper used everywhere else:
```rust
    fn error(reply: &Reply, request: &str, message: impl Into<String>) {
        Self::error_for(reply, request, None, message)
    }

    fn error_for(reply: &Reply, request: &str, session_id: Option<SessionId>, message: impl Into<String>) {
        let _ = reply.send(Frame::Control(ControlMsg::Error {
            request: request.to_string(),
            message: message.into(),
            session_id,
        }));
    }

    /// Every stored session with `attachable` computed from the live map.
    /// Lock order: store, then sessions.
    fn session_infos(&self) -> Vec<SessionInfo> {
        let store = self.store.lock().unwrap_or_else(PoisonError::into_inner);
        let live = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        store
            .data
            .sessions
            .iter()
            .cloned()
            .map(|mut s| {
                s.attachable = live.contains_key(&s.id);
                s
            })
            .collect()
    }
```
- `ListSessions` replies with `self.session_infos()`.
- `Attach` validates the size the same way `Resize` does, with request name `"Attach"`, then calls `s.attach(client, reply.clone(), cols, rows)`; the no-session error uses `Self::error_for(reply, "Attach", Some(session_id), ...)`.
- `RenameSession` sets `session.attachable` from the live map before broadcasting `SessionUpdated` (take the `sessions` lock after the `store` lock, keeping the order).
- Replace the part of `create_session` from "Record the session before spawning" to the end of the function with:
```rust
        // Hold the store lock across spawn and the SessionCreated broadcast.
        // `on_session_exit` also takes the store lock, so even a program that
        // exits immediately cannot have its SessionExited broadcast before
        // SessionCreated. Lock order: store, then sessions.
        let mut store = self.store.lock().unwrap_or_else(PoisonError::into_inner);
        store.data.sessions.push(info.clone());
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
                self.sessions.lock().unwrap_or_else(PoisonError::into_inner).insert(id, session);
                self.save(&store);
                drop(store);
                self.broadcast(ControlMsg::SessionCreated { session: SessionInfo { attachable: true, ..info } });
            }
            Err(e) => {
                let exit_code = Some(-1);
                if let Some(stored) = store.session_mut(id) {
                    stored.state = AgentState::Exited;
                    stored.exit_code = exit_code;
                }
                self.save(&store);
                drop(store);
                let failed = SessionInfo { state: AgentState::Exited, exit_code, attachable: false, ..info };
                self.broadcast(ControlMsg::SessionCreated { session: failed });
                self.broadcast(ControlMsg::SessionExited { session_id: id, exit_code });
                let path = std::env::var("PATH").unwrap_or_default();
                Self::error_for(reply, "CreateSession", Some(id), format!("{e:#}. PATH searched: {path}"));
            }
        }
```
and add `attachable: false,` to the `SessionInfo` literal earlier in the function.

`crates/jojod/src/server.rs`:
- Every `ControlMsg::Error { request, message }` constructor gains `session_id: None`.
- Replace the request loop `while let Ok(Some(frame)) = read_frame(&mut reader).await { ... }` with:
```rust
    loop {
        match read_frame(&mut reader).await {
            Ok(Some(frame)) => daemon.handle(client, &tx, frame),
            Ok(None) => break,
            Err(e) if jojo_proto::is_bad_payload(&e) => {
                let _ = tx.send(Frame::Control(ControlMsg::Error {
                    request: "Frame".into(),
                    message: e.to_string(),
                    session_id: None,
                }));
            }
            Err(_) => break,
        }
    }
```

`crates/jojod/src/client.rs`: add
```rust
    /// Writes raw bytes to the daemon. Used by tests and diagnostics to send
    /// frames the codec would never produce.
    pub async fn send_raw(&mut self, bytes: &[u8]) -> io::Result<()> {
        use tokio::io::AsyncWriteExt;
        self.writer.write_all(bytes).await?;
        self.writer.flush().await
    }
```

- [ ] **Step 11: Run the whole workspace**

Run: `cargo test --workspace -- --test-threads=1`
Expected: every test passes, no warnings.

- [ ] **Step 12: Commit**

```bash
git add crates
git commit -m "feat: protocol v2 with sized attach, bounded snapshots and session-scoped errors"
```

---

### Task 2: Connection safety and bounded client queues

**Files:**
- Create: `crates/jojod/src/client_queue.rs`
- Modify: `crates/jojod/src/pipe_security.rs`, `crates/jojod/src/transport.rs`, `crates/jojod/src/client.rs`, `crates/jojod/src/session.rs`, `crates/jojod/src/daemon.rs`, `crates/jojod/src/server.rs`, `crates/jojod/src/main.rs`, `crates/jojod/src/lib.rs`, `Cargo.toml`
- Test: `crates/jojod/src/client_queue.rs`, `crates/jojod/src/pipe_security.rs`, `crates/jojod/src/session.rs`

**Interfaces:**
- Consumes: Task 1.
- Produces:
  - `jojod::client_queue::{ClientTx, ClientRx, client_queue, MAX_CLIENT_QUEUE_BYTES}`. `client_queue(limit: usize) -> (ClientTx, ClientRx)`. `ClientTx: Clone` with `send(&self, frame: Frame) -> bool`. `ClientRx::recv(&mut self) -> Option<Frame>` (async).
  - `Session::attach(&self, client: ClientId, tx: ClientTx, cols: u16, rows: u16)` (type change from Task 1).
  - `daemon::Reply = ClientTx`.
  - Windows only: `pipe_security::is_elevated() -> io::Result<bool>`, `pipe_security::verify_server_is_current_user(pipe: HANDLE) -> io::Result<()>`.
  - `transport::connect_with_deadline(endpoint: &Endpoint, deadline: Duration) -> io::Result<(BoxRead, BoxWrite)>`; `transport::connect` uses a 5 s deadline.

- [ ] **Step 1: Write the failing queue tests**

Create `crates/jojod/src/client_queue.rs` with only the tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use jojo_proto::{ControlMsg, SessionId};

    fn output(n: usize) -> Frame {
        Frame::Output { session_id: SessionId::new(), bytes: vec![b'x'; n] }
    }

    #[tokio::test]
    async fn output_over_the_limit_is_refused_and_control_still_queues() {
        let (tx, mut rx) = client_queue(100);
        assert!(tx.send(output(60)));
        assert!(!tx.send(output(60)), "second output would exceed 100 bytes queued");
        assert!(tx.send(Frame::Control(ControlMsg::ListSessions)));
        assert!(matches!(rx.recv().await, Some(Frame::Output { .. })));
        assert!(tx.send(output(60)), "space was freed by recv");
    }

    #[tokio::test]
    async fn snapshots_always_queue_and_count() {
        let (tx, _rx) = client_queue(100);
        assert!(tx.send(Frame::Snapshot { session_id: SessionId::new(), bytes: vec![0; 500] }));
        assert!(!tx.send(output(1)), "queue already holds 500 bytes");
    }

    #[tokio::test]
    async fn send_after_receiver_dropped_returns_false() {
        let (tx, rx) = client_queue(100);
        drop(rx);
        assert!(!tx.send(Frame::Control(ControlMsg::ListSessions)));
    }
}
```
Add `pub mod client_queue;` to `crates/jojod/src/lib.rs`.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p jojod --lib client_queue`
Expected: compile error, `cannot find function client_queue`.

- [ ] **Step 3: Implement the queue**

Prepend to `crates/jojod/src/client_queue.rs`:
```rust
//! Per-client outbound queue with byte accounting (Plan 1 carry-over 2.3,
//! item 1). A window that stops reading must not grow daemon memory without
//! limit: once more than the limit of output is waiting, further Output
//! frames are refused, and the session drops that subscriber and tells it
//! to attach again for a fresh snapshot.

use jojo_proto::Frame;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Output bytes that may wait for one client before it is detached.
pub const MAX_CLIENT_QUEUE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone)]
pub struct ClientTx {
    tx: mpsc::UnboundedSender<Frame>,
    queued: Arc<AtomicUsize>,
    limit: usize,
}

pub struct ClientRx {
    rx: mpsc::UnboundedReceiver<Frame>,
    queued: Arc<AtomicUsize>,
}

pub fn client_queue(limit: usize) -> (ClientTx, ClientRx) {
    let (tx, rx) = mpsc::unbounded_channel();
    let queued = Arc::new(AtomicUsize::new(0));
    (ClientTx { tx, queued: queued.clone(), limit }, ClientRx { rx, queued })
}

fn data_len(frame: &Frame) -> usize {
    match frame {
        Frame::Output { bytes, .. } | Frame::Snapshot { bytes, .. } => bytes.len(),
        _ => 0,
    }
}

impl ClientTx {
    /// Queues `frame`. Returns false when the receiver is gone, or when the
    /// frame is Output and would push the queued output past the limit.
    /// Snapshots and control frames are always queued (a snapshot is the
    /// recovery path after a refusal).
    pub fn send(&self, frame: Frame) -> bool {
        let len = data_len(&frame);
        if matches!(frame, Frame::Output { .. }) && self.queued.load(Ordering::Relaxed) + len > self.limit {
            return false;
        }
        self.queued.fetch_add(len, Ordering::Relaxed);
        if self.tx.send(frame).is_err() {
            self.queued.fetch_sub(len, Ordering::Relaxed);
            return false;
        }
        true
    }
}

impl ClientRx {
    pub async fn recv(&mut self) -> Option<Frame> {
        let frame = self.rx.recv().await?;
        self.queued.fetch_sub(data_len(&frame), Ordering::Relaxed);
        Some(frame)
    }
}
```

- [ ] **Step 4: Run the queue tests**

Run: `cargo test -p jojod --lib client_queue`
Expected: 3 passed.

- [ ] **Step 5: Switch sessions, daemon and server to the queue (failing test first)**

In the tests module of `crates/jojod/src/session.rs`, replace every `tokio::sync::mpsc::unbounded_channel()` with `crate::client_queue::client_queue(crate::client_queue::MAX_CLIENT_QUEUE_BYTES)` and every `rx.blocking_recv()` / `rx.try_recv()` with a helper:
```rust
    /// Blocking receive with a timeout, for tests outside an async runtime.
    fn recv_blocking(rx: &mut crate::client_queue::ClientRx, timeout: Duration) -> Option<Frame> {
        let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
        rt.block_on(async { tokio::time::timeout(timeout, rx.recv()).await.ok().flatten() })
    }
```
(`try_recv` loops become `recv_blocking(&mut rx, Duration::from_millis(50))` returning `None` on timeout.) Then add:
```rust
    #[test]
    fn a_client_that_falls_behind_is_detached_with_an_error() {
        let session = Session::spawn(SessionId::new(), test_spec(), |_| {}).unwrap();
        let (tx, mut rx) = crate::client_queue::client_queue(4096);
        session.attach(1, tx, 80, 24);
        #[cfg(windows)]
        session.write(b"for /L %i in (1,1,3000) do @echo xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\r".to_vec());
        #[cfg(unix)]
        session.write(b"i=0; while [ $i -lt 3000 ]; do echo xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx; i=$((i+1)); done\n".to_vec());
        // Do not read until the session has produced far more than 4 KiB.
        std::thread::sleep(Duration::from_secs(3));
        let mut saw_error = false;
        let mut output_after_error = false;
        while let Some(frame) = recv_blocking(&mut rx, Duration::from_millis(500)) {
            match frame {
                Frame::Control(jojo_proto::ControlMsg::Error { request, .. }) if request == "Output" => saw_error = true,
                Frame::Output { .. } if saw_error => output_after_error = true,
                _ => {}
            }
        }
        assert!(saw_error, "expected an Output error once the queue limit was hit");
        assert!(!output_after_error, "no Output may follow the error");
        session.kill();
    }
```

Run: `cargo test -p jojod --lib session -- --test-threads=1`
Expected: compile errors (`attach` expects `UnboundedSender`).

- [ ] **Step 6: Implement the switch**

`crates/jojod/src/session.rs`:
- Replace `use tokio::sync::mpsc::UnboundedSender;` with `use crate::client_queue::ClientTx;`, change `subscribers: Vec<(ClientId, ClientTx)>` and the `attach` parameter type to `ClientTx`.
- Replace the fan-out `retain` in the reader thread with:
```rust
                    s.subscribers.retain(|(_, tx)| {
                        if tx.send(Frame::Output { session_id: id, bytes: chunk.to_vec() }) {
                            return true;
                        }
                        // Refused: the client fell behind or is gone. Tell it
                        // (a no-op if it is gone) and stop sending to it.
                        tx.send(Frame::Control(jojo_proto::ControlMsg::Error {
                            request: "Output".into(),
                            message: "client fell behind; attach again for a fresh snapshot".into(),
                            session_id: Some(id),
                        }));
                        false
                    });
```
- In `attach`, the snapshot send is `if tx.send(Frame::Snapshot { .. })`.

`crates/jojod/src/daemon.rs`: `pub type Reply = crate::client_queue::ClientTx;`; every `let _ = reply.send(...)` stays valid because `send` returns `bool` (remove `let _ =` only if the compiler warns).

`crates/jojod/src/server.rs`: in `serve_connection`, replace the channel with `let (tx, mut rx) = crate::client_queue::client_queue(crate::client_queue::MAX_CLIENT_QUEUE_BYTES);`; the writer task loop is unchanged in shape (`while let Some(frame) = rx.recv().await`). In the event task, replace `if event_tx.send(Frame::Control(msg)).is_err() { break; }` with `if !event_tx.send(Frame::Control(msg)) { break; }`.

- [ ] **Step 7: Close the connection when events are lost**

In `serve_connection`, add before the event task:
```rust
    let lagged = Arc::new(tokio::sync::Notify::new());
    let lagged_signal = lagged.clone();
```
In the event task, replace `Err(RecvError::Lagged(_)) => continue,` with:
```rust
                Err(RecvError::Lagged(missed)) => {
                    // The client's view of projects and sessions is now
                    // incomplete. Tell it, then close the connection so it
                    // reconnects and lists everything again.
                    event_tx.send(Frame::Control(ControlMsg::Error {
                        request: "Events".into(),
                        message: format!("missed {missed} events; reconnect"),
                        session_id: None,
                    }));
                    lagged_signal.notify_one();
                    break;
                }
```
Wrap the request loop from Task 1 in `tokio::select!` so a lag notification ends it:
```rust
    loop {
        tokio::select! {
            read = read_frame(&mut reader) => match read {
                Ok(Some(frame)) => daemon.handle(client, &tx, frame),
                Ok(None) => break,
                Err(e) if jojo_proto::is_bad_payload(&e) => {
                    tx.send(Frame::Control(ControlMsg::Error {
                        request: "Frame".into(),
                        message: e.to_string(),
                        session_id: None,
                    }));
                }
                Err(_) => break,
            },
            // Cancelling a partly read frame is safe here: the connection
            // is being closed.
            _ = lagged.notified() => break,
        }
    }
```
No automated test covers this path: triggering it needs more than 1024 events queued behind one slow connection. The reasoning above is the review target.

- [ ] **Step 8: Write the failing Windows security tests**

Add to `crates/jojod/src/pipe_security.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::io::AsRawHandle;

    #[test]
    fn elevation_can_be_queried() {
        is_elevated().expect("query token elevation");
    }

    #[tokio::test]
    async fn a_pipe_created_by_this_process_passes_the_owner_check() {
        let name = format!(r"\\.\pipe\jojo-owner-test-{}", std::process::id());
        let mut security = UserOnly::new().unwrap();
        let mut options = tokio::net::windows::named_pipe::ServerOptions::new();
        options.first_pipe_instance(true);
        // SAFETY: the security attributes outlive the create call.
        let _server = unsafe { options.create_with_security_attributes_raw(&name, security.as_ptr()) }.unwrap();
        let client = tokio::net::windows::named_pipe::ClientOptions::new().open(&name).unwrap();
        verify_server_is_current_user(client.as_raw_handle() as HANDLE).expect("owner check");
    }
}
```

Run: `cargo test -p jojod --lib pipe_security`
Expected: compile errors (`is_elevated`, `verify_server_is_current_user` not found).

- [ ] **Step 9: Implement the Windows security functions**

In the workspace `Cargo.toml`, add `"Win32_System_Pipes"` and `"Win32_System_LibraryLoader"` to the `windows-sys` feature list (the second is used by Task 6).

In `crates/jojod/src/pipe_security.rs`:
- Add imports: `GetNamedPipeServerProcessId` from `windows_sys::Win32::System::Pipes`; `OpenProcess`, `PROCESS_QUERY_LIMITED_INFORMATION` from `windows_sys::Win32::System::Threading`; `EqualSid`, `TokenElevation`, `TOKEN_ELEVATION` from `windows_sys::Win32::Security`. (If a name lives at a different path in windows-sys 0.59, find it under `~/.cargo/registry/src/*/windows-sys-0.59.*/src/Windows/` and record the change.)
- Refactor `current_user_sid` into a reusable token reader, and fix the error capture order:
```rust
/// The TOKEN_USER structure of `token`, in u64 storage for alignment.
/// SAFETY: `token` must be a valid token handle opened with TOKEN_QUERY.
unsafe fn token_user(token: HANDLE) -> io::Result<Vec<u64>> {
    let mut len = 0u32;
    GetTokenInformation(token, TokenUser, null_mut(), 0, &mut len);
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    if GetTokenInformation(token, TokenUser, buf.as_mut_ptr() as *mut c_void, len, &mut len) == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(buf)
}

/// Opens the token of `process` for querying. The caller closes it.
/// SAFETY: `process` must be a valid process handle.
unsafe fn open_token(process: HANDLE) -> io::Result<HANDLE> {
    let mut token: HANDLE = null_mut();
    if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(token)
}
```
  Rewrite `current_user_sid` to use them: open the current process token, call `token_user`, **capture its result before** `CloseHandle(token)`, then convert the SID with `ConvertSidToStringSidW` as before.
- Add:
```rust
/// True when this process runs with an elevated (administrator) token.
pub fn is_elevated() -> io::Result<bool> {
    // SAFETY: standard token query; the handle is closed on every path.
    unsafe {
        let token = open_token(GetCurrentProcess())?;
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut TOKEN_ELEVATION as *mut c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        let result = if ok == 0 { Err(io::Error::last_os_error()) } else { Ok(elevation.TokenIsElevated != 0) };
        CloseHandle(token);
        result
    }
}

/// Fails unless the process serving `pipe` runs as the same user as this
/// process. Without this check, another local user who creates
/// `\\.\pipe\jojo-<username>` first would receive this user's keystrokes.
pub fn verify_server_is_current_user(pipe: HANDLE) -> io::Result<()> {
    // SAFETY: every handle opened here is closed before returning; SID
    // pointers point into buffers that live until EqualSid returns.
    unsafe {
        let mut pid = 0u32;
        if GetNamedPipeServerProcessId(pipe, &mut pid) == 0 {
            return Err(io::Error::last_os_error());
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return Err(io::Error::last_os_error());
        }
        let server_token = open_token(process);
        CloseHandle(process);
        let server_token = server_token?;
        let server_user = token_user(server_token);
        CloseHandle(server_token);
        let server_user = server_user?;

        let own_token = open_token(GetCurrentProcess())?;
        let own_user = token_user(own_token);
        CloseHandle(own_token);
        let own_user = own_user?;

        let server_sid = (*(server_user.as_ptr() as *const TOKEN_USER)).User.Sid;
        let own_sid = (*(own_user.as_ptr() as *const TOKEN_USER)).User.Sid;
        if EqualSid(server_sid, own_sid) == 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "the jojod pipe is served by a process of another user",
            ));
        }
        Ok(())
    }
}
```

- [ ] **Step 10: Use the check, add the connect deadline and the elevation refusal**

`crates/jojod/src/transport.rs` (Windows `connect`): after `ClientOptions::new().open(...)` succeeds, call `crate::pipe_security::verify_server_is_current_user(client.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE)?;` before splitting (add `use std::os::windows::io::AsRawHandle;`). Then rename both platform `connect` functions to `connect_unbounded` (private) and add at module level:
```rust
/// How long `connect` keeps retrying a busy pipe before giving up.
pub const CONNECT_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);

pub async fn connect(endpoint: &Endpoint) -> io::Result<(BoxRead, BoxWrite)> {
    connect_with_deadline(endpoint, CONNECT_DEADLINE).await
}

pub async fn connect_with_deadline(
    endpoint: &Endpoint,
    deadline: std::time::Duration,
) -> io::Result<(BoxRead, BoxWrite)> {
    tokio::time::timeout(deadline, imp::connect_unbounded(endpoint))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "timed out connecting to jojod"))?
}
```
(Adjust the `pub use imp::{...}` line accordingly.)

`crates/jojod/src/main.rs`, at the start of the `Serve` arm:
```rust
            #[cfg(windows)]
            if jojod::pipe_security_is_elevated()? {
                anyhow::bail!(
                    "jojod will not run elevated: its pipe would let non-elevated processes drive \
                     elevated shells. Start it from a terminal that is not running as administrator."
                );
            }
```
and in `crates/jojod/src/lib.rs` expose the check without making the whole module public:
```rust
/// True when this process runs elevated (Windows only).
#[cfg(windows)]
pub fn pipe_security_is_elevated() -> std::io::Result<bool> {
    pipe_security::is_elevated()
}
```

- [ ] **Step 11: Run the whole workspace**

Run: `cargo test --workspace -- --test-threads=1`
Expected: every test passes, no warnings. If the Claude Code terminal running these tests is elevated, `cargo run -p jojod -- serve` refuses to start; the tests are unaffected because they call `server::run` directly.

- [ ] **Step 12: Commit**

```bash
git add Cargo.toml Cargo.lock crates
git commit -m "feat(jojod): bound client queues, verify pipe owner, refuse elevated serve"
```

---

### Task 3: Request hygiene, session cleanup and logging

**Files:**
- Create: `crates/jojod/src/logging.rs`, `crates/jojod/tests/logging.rs`
- Modify: `crates/jojod/src/daemon.rs`, `crates/jojod/src/session.rs`, `crates/jojod/src/store.rs`, `crates/jojod/src/server.rs`, `crates/jojod/src/main.rs`, `crates/jojod/src/lib.rs`, `crates/jojod/Cargo.toml`, `Cargo.toml`
- Test: `crates/jojod/tests/daemon_roundtrip.rs`, `crates/jojod/src/session.rs`, `crates/jojod/src/store.rs`, `crates/jojod/tests/logging.rs`

**Interfaces:**
- Consumes: Tasks 1–2.
- Produces: `jojod::logging::init(log_dir: &Path) -> anyhow::Result<tracing_appender::non_blocking::WorkerGuard>`; `impl Drop for Session` (kills the child).

- [ ] **Step 1: Write the failing tests**

Add to `crates/jojod/tests/daemon_roundtrip.rs`:
```rust
async fn expect_error(c: &mut Client, request: &str) -> String {
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Error { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::Error { request: r, message, .. }) => {
            assert_eq!(r, request);
            message
        }
        _ => unreachable!(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn project_and_session_requests_are_validated() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();

    c.send(Frame::Control(ControlMsg::AddProject { path: ".".into(), name: "rel".into() })).await.unwrap();
    assert!(expect_error(&mut c, "AddProject").await.contains("absolute"));

    let project = add_project(&mut c, dir.path()).await;
    c.send(Frame::Control(ControlMsg::AddProject {
        path: dir.path().to_string_lossy().into_owned(),
        name: "again".into(),
    }))
    .await
    .unwrap();
    assert!(expect_error(&mut c, "AddProject").await.contains("already"));

    c.send(Frame::Control(ControlMsg::CloseSession { session_id: jojo_proto::SessionId::new() })).await.unwrap();
    expect_error(&mut c, "CloseSession").await;
    c.send(Frame::Control(ControlMsg::RemoveProject { id: jojo_proto::ProjectId::new() })).await.unwrap();
    expect_error(&mut c, "RemoveProject").await;

    // A project whose only session has exited can be removed, and its
    // session row is reported as removed too.
    let session = create_shell(&mut c, &project).await;
    c.send(Frame::Input { session_id: session.id, bytes: b"exit\r".to_vec() }).await.unwrap();
    c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::SessionExited { .. }))).await.unwrap();
    c.send(Frame::Control(ControlMsg::RemoveProject { id: project.id })).await.unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::SessionRemoved { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::SessionRemoved { session_id }) => assert_eq!(session_id, session.id),
        _ => unreachable!(),
    }
    c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::ProjectRemoved { .. }))).await.unwrap();

    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
}
```

Add to the tests module of `crates/jojod/src/session.rs`:
```rust
    #[test]
    fn dropping_a_session_kills_its_process() {
        let (exit_tx, exit_rx) = std::sync::mpsc::channel();
        let session = Session::spawn(SessionId::new(), test_spec(), move |code| {
            let _ = exit_tx.send(code);
        })
        .unwrap();
        drop(session);
        exit_rx.recv_timeout(Duration::from_secs(15)).expect("exit reported after drop");
    }
```

Add to the tests module of `crates/jojod/src/store.rs`:
```rust
    #[test]
    fn two_corrupt_loads_keep_both_copies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, b"{ bad").unwrap();
        Store::load(&path).unwrap();
        std::fs::write(&path, b"{ bad again").unwrap();
        Store::load(&path).unwrap();
        let copies = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("state.json.corrupt-"))
            .count();
        assert_eq!(copies, 2);
    }
```

Create `crates/jojod/tests/logging.rs` (its own test binary, because a process can install only one global tracing subscriber):
```rust
#[test]
fn log_lines_are_written_to_a_dated_file() {
    let dir = tempfile::tempdir().unwrap();
    let guard = jojod::logging::init(dir.path()).unwrap();
    tracing::info!(session = "test", "logging works");
    drop(guard); // flushes the background writer
    let files: Vec<_> = std::fs::read_dir(dir.path()).unwrap().filter_map(|e| e.ok()).collect();
    assert_eq!(files.len(), 1, "{files:?}");
    let name = files[0].file_name().to_string_lossy().into_owned();
    assert!(name.starts_with("jojod.") && name.ends_with(".log"), "{name}");
    let text = std::fs::read_to_string(files[0].path()).unwrap();
    assert!(text.contains("logging works"), "{text}");
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p jojod -- --test-threads=1`
Expected: compile error for `jojod::logging`; once that module exists the other new tests fail on their assertions.

- [ ] **Step 3: Implement logging**

Workspace `Cargo.toml` `[workspace.dependencies]`: add
```toml
tracing = "0.1"
tracing-appender = "0.2"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
```
and in `crates/jojod/Cargo.toml` `[dependencies]`: `tracing.workspace = true`, `tracing-appender.workspace = true`, `tracing-subscriber.workspace = true`.

Create `crates/jojod/src/logging.rs`:
```rust
//! File logging (spec section 10). Logs carry metadata only: session ids,
//! states, connection events and errors. Terminal input and output bytes are
//! never logged.

use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};

/// Writes `jojod.<date>.log` files into `log_dir`, rotated daily, keeping
/// the newest 7. Keep the returned guard alive for the life of the process;
/// dropping it flushes and stops the background writer.
pub fn init(log_dir: &Path) -> anyhow::Result<WorkerGuard> {
    std::fs::create_dir_all(log_dir)?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("jojod")
        .filename_suffix("log")
        .max_log_files(7)
        .build(log_dir)?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("JOJO_LOG").unwrap_or_else(|_| "info".into()),
        )
        .try_init()
        .map_err(|e| anyhow::anyhow!("install log subscriber: {e}"))?;
    Ok(guard)
}
```
Add `pub mod logging;` to `lib.rs`. In `main.rs`, in the `Serve` arm after the elevation check: `let _log_guard = jojod::logging::init(&cfg.state_dir.join("logs"))?;` and `tracing::info!(version = jojod::VERSION, endpoint = %cfg.endpoint.0, "jojod starting");` (construct `cfg` before these lines).

Replace logging calls (metadata only):
- `server.rs`: `eprintln!("jojod: accept error: {e}")` → `tracing::warn!(error = %e, "accept failed")`; log `tracing::info!(client, "client connected")` after the Hello exchange succeeds and `tracing::info!(client, "client disconnected")` at the end of `serve_connection`; log protocol mismatches with `tracing::warn!`.
- `daemon.rs`: in `save`, replace `let _ = store.save();` with `if let Err(e) = store.save() { tracing::error!(error = %e, "failed to write state.json"); }`; log `tracing::info!(session = %id, kind = ?kind, "session created")` on spawn success, `tracing::warn!(session = %id, error = %e, "session failed to start")` on failure, and `tracing::info!(session = %id, exit_code = ?exit_code, "session exited")` in `on_session_exit`.

- [ ] **Step 4: Implement request hygiene**

In `crates/jojod/src/daemon.rs`:
- `AddProject`: before the `is_dir` check, reject relative paths with `Self::error(reply, "AddProject", format!("path must be absolute: {path}"))`; after it, reject a path already present (`store.data.projects.iter().any(|p| p.path == path)`) with `format!("project already added: {path}")`.
- `RemoveProject`: return `Self::error(reply, "RemoveProject", "unknown project")` when no project has `id`. When removal proceeds, collect the ids of the removed session rows, remove each from the live `sessions` map (after the store lock is dropped), broadcast `SessionRemoved { session_id }` for each, then broadcast `ProjectRemoved`.
- `CloseSession`: when the id is in neither the live map nor the store, reply `Self::error_for(reply, "CloseSession", Some(session_id), "unknown session")` and stop.

In `crates/jojod/src/session.rs`:
- Add:
```rust
impl Drop for Session {
    /// A session is dropped when the daemon closes or forgets it; its
    /// process must not outlive it.
    fn drop(&mut self) {
        self.kill();
    }
}
```
- In `Session::spawn`, once `spawn_command` has succeeded, a later `?` (reader clone, writer take, thread spawns) would leak a running child. Kill it on those paths: after `let killer = child.clone_killer();`, wrap the remaining fallible setup in a closure or block, and on `Err` call `let _ = child.kill();` before returning the error.

In `crates/jojod/src/store.rs`, in the corrupt-file branch of `load`, name the copy with milliseconds and avoid overwriting an existing copy:
```rust
                Err(_) => {
                    let mut stamp = now_ms();
                    let corrupt = loop {
                        let mut name = path.as_os_str().to_owned();
                        name.push(format!(".corrupt-{stamp}"));
                        let candidate = PathBuf::from(name);
                        if !candidate.exists() {
                            break candidate;
                        }
                        stamp += 1;
                    };
                    std::fs::rename(path, corrupt)?;
                    StoreData::default()
                }
```

- [ ] **Step 5: Run the whole workspace**

Run: `cargo test --workspace -- --test-threads=1`
Expected: every test passes, no warnings.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates
git commit -m "feat(jojod): validate project and session requests, kill on drop, add file logging"
```

---

### Task 4: Generated TypeScript protocol types

**Files:**
- Modify: `crates/jojo-proto/Cargo.toml`, `crates/jojo-proto/src/ids.rs`, `crates/jojo-proto/src/msg.rs`, `Cargo.toml`
- Create: `crates/jojo-proto/tests/bindings.rs`, `scripts/check-protocol.ps1`, `app/src/lib/protocol/*.ts` (generated)

**Interfaces:**
- Consumes: Task 1 message definitions.
- Produces: `app/src/lib/protocol/ControlMsg.ts`, `SessionInfo.ts`, `ProjectInfo.ts`, `SessionKind.ts`, `AgentState.ts`, `SessionId.ts`, `ProjectId.ts`, each exporting a type of the same name. Plan 2b imports these.

- [ ] **Step 1: Write the failing export test**

Create `crates/jojo-proto/tests/bindings.rs`:
```rust
//! Generates the TypeScript protocol types used by the app
//! (app/src/lib/protocol). `scripts/check-protocol.ps1` runs this and fails
//! when the committed files differ, so the Rust structs stay the single
//! source of truth (spec section 6.1).

use jojo_proto::ControlMsg;
use std::path::PathBuf;
use ts_rs::{Config, TS};

fn out_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../app/src/lib/protocol")
}

#[test]
fn export_typescript_bindings() {
    let dir = out_dir();
    ControlMsg::export_all(&Config::new().with_out_dir(&dir)).expect("export bindings");

    let control = std::fs::read_to_string(dir.join("ControlMsg.ts")).unwrap();
    assert!(control.contains("\"type\": \"Attach\""), "{control}");
    assert!(control.contains("cols: number"), "{control}");
    let session = std::fs::read_to_string(dir.join("SessionInfo.ts")).unwrap();
    assert!(session.contains("created_at: number"), "u64 must be number, not bigint: {session}");
    assert!(session.contains("attachable: boolean"), "{session}");
    let id = std::fs::read_to_string(dir.join("SessionId.ts")).unwrap();
    assert!(id.contains("string"), "{id}");
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p jojo-proto --test bindings`
Expected: compile error, unresolved import `ts_rs`.

- [ ] **Step 3: Derive the TypeScript types**

Workspace `Cargo.toml`: add `ts-rs = "12"` to `[workspace.dependencies]`; `crates/jojo-proto/Cargo.toml`: `ts-rs.workspace = true`.

`crates/jojo-proto/src/ids.rs`: add `use ts_rs::TS;`, add `TS` to both derive lists, and annotate the inner field so the id exports as a string alias:
```rust
pub struct SessionId(#[ts(as = "String")] pub Ulid);
pub struct ProjectId(#[ts(as = "String")] pub Ulid);
```

`crates/jojo-proto/src/msg.rs`: add `use ts_rs::TS;` and `TS` to the derive list of `SessionKind`, `AgentState`, `ProjectInfo`, `SessionInfo` and `ControlMsg`. On `SessionInfo.created_at` and `SessionInfo.state_changed_at` add `#[ts(type = "number")]` (Unix milliseconds fit in a JavaScript number until the year 287396, and the JSON on the wire is a number).

- [ ] **Step 4: Run it**

Run: `cargo test -p jojo-proto --test bindings`
Expected: pass. Read the generated `app/src/lib/protocol/ControlMsg.ts` once to confirm it is a discriminated union on `type` with `session_id: SessionId | null` on `Error`. If an assertion fails only because ts-rs formats a line differently than the assertion expects (for example spacing around `:`), adjust the assertion to the actual format and record it; if the generated *type* is wrong, fix the attributes instead.

- [ ] **Step 5: Add the drift check script**

Create `scripts/check-protocol.ps1`:
```powershell
# Regenerates the TypeScript protocol types and fails when they differ from
# the committed files (spec section 6.1: the Rust structs are the source of
# truth). Run from the repository root.
$ErrorActionPreference = 'Stop'
cargo test -q -p jojo-proto --test bindings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$changes = git status --porcelain -- app/src/lib/protocol
if ($changes) {
    Write-Error "Generated protocol types are out of date. Commit these changes:`n$changes"
    exit 1
}
Write-Output "Protocol types are up to date."
```

- [ ] **Step 6: Verify the script both ways**

Run: `pwsh -NoProfile -File scripts/check-protocol.ps1` before committing the generated files.
Expected: fails, listing the untracked `app/src/lib/protocol/*.ts`.
Commit the generated files (Step 7), then run it again. Expected: `Protocol types are up to date.`

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock crates/jojo-proto scripts/check-protocol.ps1 app/src/lib/protocol
git commit -m "feat(proto): generate TypeScript protocol types with ts-rs"
```

---

### Task 5: Snapshot pending-wrap edge cases and attribute coverage

**Files:**
- Modify: `crates/jojo-grid/src/snapshot.rs`
- Test: `crates/jojo-grid/tests/roundtrip.rs`

**Interfaces:**
- Consumes: Task 1 (`snapshot_skipping`).
- Produces: no interface change.

- [ ] **Step 1: Write the failing fixtures**

Add to `crates/jojo-grid/tests/roundtrip.rs`:
```rust
#[test]
fn combined_attributes() {
    assert_roundtrip(
        "combined",
        b"\x1b[1;3;4;7;38;2;1;2;3;48;5;200mcombo\x1b[0m \x1b[2;9;38;5;45mdim-strike\x1b[0m \x1b[1;4:3;31mbold-curl\x1b[0m",
    );
}

#[test]
fn pending_wrap_after_a_wide_character() {
    let mut s = "x".repeat(78).into_bytes();
    s.extend_from_slice("漢".as_bytes());
    assert_roundtrip("pending_wide", &s);
}
```
Run: `cargo test -p jojo-grid --test roundtrip -- combined pending_wrap_after`
Expected: record whether each passes or fails. A fixture that already passes stays as a regression test; note it in the report.

- [ ] **Step 2: Fix the pending-wrap re-emit**

In the pending-wrap branch of `snapshot_skipping` (added in Plan 1's fix round):
1. Use the cursor's own column instead of `cols - 1`: target `cursor.point.column.0`, stepping back one column when that cell is a `WIDE_CHAR_SPACER`.
2. Map a `'\t'` cell to `' '` exactly as the row loop does before writing it.

Make both changes, then run `cargo test -p jojo-grid`.
Expected: all tests pass, including both new fixtures.

- [ ] **Step 3: Commit**

```bash
git add crates/jojo-grid
git commit -m "fix(grid): restore pending wrap at the cursor column and cover combined attributes"
```

---

### Task 6: Sideloaded ConPTY

**Files:**
- Create: `scripts/fetch-conpty.ps1`, `resources/conpty/SHA256SUMS`, `crates/jojod/tests/conpty_sideload.rs`
- Modify: `.gitignore`

**Interfaces:**
- Consumes: `jojod::session::{Session, SpawnSpec}`; windows-sys feature `Win32_System_LibraryLoader` (added in Task 2).
- Produces: `resources/conpty/x64/conpty.dll` and `resources/conpty/x64/OpenConsole.exe` after running the script (git-ignored). Plan 2b copies both next to `jojod.exe`.

`portable-pty` 0.9.0 loads `conpty.dll` by name before falling back to the version built into Windows (`src/win/psuedocon.rs`, `load_conpty`). Windows searches the executable's own folder first, so placing `conpty.dll` and `OpenConsole.exe` next to `jojod.exe` is sufficient; no code change is needed.

- [ ] **Step 1: Find the current package version**

Run:
```powershell
(Invoke-RestMethod https://api.nuget.org/v3-flatcontainer/microsoft.windows.console.conpty/index.json).versions | Select-Object -Last 5
```
Pick the newest non-prerelease version and use it as `$Version` in Step 2. Record it in the report.

- [ ] **Step 2: Write the fetch script**

Create `scripts/fetch-conpty.ps1`, replacing `<VERSION>` with the version from Step 1:
```powershell
# Downloads the Microsoft.Windows.Console.ConPTY NuGet package and extracts
# the x64 conpty.dll and OpenConsole.exe into resources/conpty/x64, then
# verifies them against resources/conpty/SHA256SUMS. Run from the repository
# root. Pass -WriteSums once after changing the version to record new hashes.
param([switch]$WriteSums)
$ErrorActionPreference = 'Stop'
$Version = '<VERSION>'
$url = "https://api.nuget.org/v3-flatcontainer/microsoft.windows.console.conpty/$Version/microsoft.windows.console.conpty.$Version.nupkg"
$work = Join-Path $env:TEMP "conpty-$Version"
New-Item -ItemType Directory -Force $work | Out-Null
$pkg = Join-Path $work 'pkg.zip'
Invoke-WebRequest $url -OutFile $pkg
Expand-Archive $pkg -DestinationPath (Join-Path $work 'x') -Force
$out = 'resources/conpty/x64'
New-Item -ItemType Directory -Force $out | Out-Null
foreach ($name in 'conpty.dll', 'OpenConsole.exe') {
    $file = Get-ChildItem (Join-Path $work 'x') -Recurse -Filter $name |
        Where-Object { $_.FullName -match '[\\/](x64|win-x64)[\\/]' } |
        Select-Object -First 1
    if (-not $file) { throw "x64 $name not found in package $Version" }
    Copy-Item $file.FullName (Join-Path $out $name) -Force
}
$sums = 'resources/conpty/SHA256SUMS'
$actual = Get-ChildItem $out -File | Sort-Object Name | ForEach-Object {
    "{0}  {1}" -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower(), $_.Name
}
if ($WriteSums) {
    Set-Content $sums ($actual + "# Microsoft.Windows.Console.ConPTY $Version")
    Write-Output "Recorded hashes for $Version."
} else {
    $expected = Get-Content $sums | Where-Object { $_ -notmatch '^#' }
    if (Compare-Object $expected $actual) { throw "Hash mismatch for ConPTY $Version; see $sums" }
    Write-Output "ConPTY $Version verified."
}
```
Append to `.gitignore`:
```
/resources/conpty/x64/
```

- [ ] **Step 3: Fetch and record hashes**

Run: `pwsh -NoProfile -File scripts/fetch-conpty.ps1 -WriteSums`, then `pwsh -NoProfile -File scripts/fetch-conpty.ps1`.
Expected: `Recorded hashes for <version>.`, then `ConPTY <version> verified.`; `resources/conpty/x64/` contains `conpty.dll` and `OpenConsole.exe`.

- [ ] **Step 4: Write the sideload test**

Create `crates/jojod/tests/conpty_sideload.rs`:
```rust
//! Proves that a conpty.dll placed next to the executable is the one
//! portable-pty loads. Skips (passes with a message) when
//! scripts/fetch-conpty.ps1 has not been run.
#![cfg(windows)]

use jojo_proto::SessionId;
use jojod::session::{Session, SpawnSpec};
use std::path::PathBuf;
use std::time::Duration;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;

#[test]
fn sideloaded_conpty_is_used() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/conpty/x64");
    if !source.join("conpty.dll").exists() {
        eprintln!("skipped: run scripts/fetch-conpty.ps1 first");
        return;
    }
    // The test executable's folder is the first place Windows searches.
    let exe_dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
    for name in ["conpty.dll", "OpenConsole.exe"] {
        std::fs::copy(source.join(name), exe_dir.join(name)).unwrap();
    }

    let (exit_tx, exit_rx) = std::sync::mpsc::channel();
    let session = Session::spawn(
        SessionId::new(),
        SpawnSpec {
            program: "cmd.exe".into(),
            args: vec!["/d".into(), "/c".into(), "echo sideload".into()],
            cwd: std::env::temp_dir(),
            env: vec![],
            cols: 80,
            rows: 24,
            scrollback: 100,
        },
        move |code| {
            let _ = exit_tx.send(code);
        },
    )
    .unwrap();
    exit_rx.recv_timeout(Duration::from_secs(15)).expect("session exits");

    let name: Vec<u16> = "conpty.dll".encode_utf16().chain(Some(0)).collect();
    // SAFETY: name is NUL-terminated; GetModuleHandleW does not take ownership.
    let module = unsafe { GetModuleHandleW(name.as_ptr()) };
    assert!(!module.is_null(), "conpty.dll was not loaded; the built-in ConPTY was used");
    drop(session);
}
```
`jojod`'s `[target.'cfg(windows)'.dependencies]` already includes `windows-sys`; integration tests can use it directly.

- [ ] **Step 5: Run it**

Run: `cargo test -p jojod --test conpty_sideload -- --nocapture`
Expected: 1 passed, with no "skipped" message.

- [ ] **Step 6: Commit**

```bash
git add .gitignore scripts/fetch-conpty.ps1 resources/conpty/SHA256SUMS crates/jojod/tests/conpty_sideload.rs
git commit -m "build: fetch a pinned ConPTY and prove portable-pty sideloads it"
```

---

### Task 7: Recorded sessions and the xterm.js cross-check

**Files:**
- Create: `crates/jojod/examples/record.rs`, `crates/jojo-grid/tests/recordings.rs`, `tests/recordings/*.bin`, `tests/recordings/README.md`
- Create: `tests/xterm-crosscheck/package.json`, `tests/xterm-crosscheck/crosscheck.test.ts`, `tests/xterm-crosscheck/vitest.config.ts`
- Modify: `.gitignore`

**Interfaces:**
- Consumes: `jojo_grid::TermGrid` (`feed`, `take_replies`, `snapshot`, `debug_dump`, `visible_text`); `portable_pty`.
- Produces: the committed recordings and two test suites (Rust round-trip over every recording; Node comparison against `@xterm/headless`).

Recording file names encode the terminal size: `<name>.<cols>x<rows>.bin`.

- [ ] **Step 1: Write the recorder**

Create `crates/jojod/examples/record.rs`:
```rust
//! Records the raw output of a program running in a PTY, for snapshot tests.
//!
//! Usage:
//!   cargo run -p jojod --example record -- --out tests/recordings/NAME.COLSxROWS.bin
//!       --cols 80 --rows 24 --duration-ms 6000
//!       [--step DELAY_MS:TEXT]... -- PROGRAM [ARGS]...
//!
//! TEXT accepts the escapes \r \n \t \e (ESC) \x7f (DEL) and \\.
//! The recorder answers terminal queries (such as the cursor position
//! request ConPTY sends at startup) the way the daemon does, so shells do not
//! stall waiting for a reply.

use jojo_grid::TermGrid;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn unescape(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut buf = [0u8; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        match chars.next() {
            Some('r') => out.push(b'\r'),
            Some('n') => out.push(b'\n'),
            Some('t') => out.push(b'\t'),
            Some('e') => out.push(0x1b),
            Some('\\') => out.push(b'\\'),
            Some('x') => {
                let hex: String = chars.by_ref().take(2).collect();
                out.push(u8::from_str_radix(&hex, 16).expect("two hex digits after \\x"));
            }
            other => panic!("unknown escape \\{other:?}"),
        }
    }
    out
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let split = args.iter().position(|a| a == "--").expect("missing -- before the program");
    let (opts, program) = (&args[..split], &args[split + 1..]);
    let mut out = None;
    let (mut cols, mut rows, mut duration) = (80u16, 24u16, 5000u64);
    let mut steps: Vec<(u64, Vec<u8>)> = Vec::new();
    let mut i = 0;
    while i < opts.len() {
        let value = opts.get(i + 1).expect("option needs a value").clone();
        match opts[i].as_str() {
            "--out" => out = Some(value),
            "--cols" => cols = value.parse()?,
            "--rows" => rows = value.parse()?,
            "--duration-ms" => duration = value.parse()?,
            "--step" => {
                let (delay, text) = value.split_once(':').expect("--step DELAY_MS:TEXT");
                steps.push((delay.parse()?, unescape(text)));
            }
            other => anyhow::bail!("unknown option {other}"),
        }
        i += 2;
    }
    let out = out.expect("--out is required");

    let pair = native_pty_system().openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;
    let mut cmd = CommandBuilder::new(&program[0]);
    cmd.args(&program[1..]);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    let mut child = pair.slave.spawn_command(cmd)?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader()?;
    let writer = Arc::new(Mutex::new(pair.master.take_writer()?));

    let captured = Arc::new(Mutex::new(Vec::new()));
    let reader_thread = {
        let captured = captured.clone();
        let writer = writer.clone();
        std::thread::spawn(move || {
            let mut grid = TermGrid::new(cols, rows, 10_000);
            let mut buf = vec![0u8; 64 * 1024];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                captured.lock().unwrap().extend_from_slice(&buf[..n]);
                grid.feed(&buf[..n]);
                let replies = grid.take_replies();
                if !replies.is_empty() {
                    let _ = writer.lock().unwrap().write_all(&replies);
                }
            }
        })
    };

    let start = Instant::now();
    for (delay, text) in steps {
        std::thread::sleep(Duration::from_millis(delay));
        writer.lock().unwrap().write_all(&text)?;
    }
    let remaining = Duration::from_millis(duration).saturating_sub(start.elapsed());
    std::thread::sleep(remaining);

    let bytes = captured.lock().unwrap().clone();
    let _ = child.kill();
    drop(pair.master);
    let _ = reader_thread.join();
    std::fs::write(&out, &bytes)?;
    println!("wrote {} bytes to {out}", bytes.len());
    Ok(())
}
```
Add `[dev-dependencies]` nothing new: examples may use the package's normal dependencies (`jojo-grid`, `portable-pty`, `anyhow`).

- [ ] **Step 2: Record the sessions**

Run each command from the repository root (PowerShell; the doubled backslashes are for PowerShell string literals passed to the recorder's escape parser):
```powershell
cargo run -q -p jojod --example record -- --out tests/recordings/pwsh-redraw.80x24.bin --cols 80 --rows 24 --duration-ms 7000 --step "2500:Write-Output $('a' * 150)" --step "500:$('\x7f' * 120)" --step "500:\e" --step "300:Write-Output done\r" -- pwsh.exe -NoLogo -NoProfile
cargo run -q -p jojod --example record -- --out tests/recordings/cmd-dir.100x30.bin --cols 100 --rows 30 --duration-ms 5000 --step "1500:dir C:\\Windows\\System32\\drivers\r" -- cmd.exe /d /q /k
cargo run -q -p jojod --example record -- --out tests/recordings/pwsh-unicode-color.80x24.bin --cols 80 --rows 24 --duration-ms 7000 --step "2500:Write-Output '日本語テキスト 🎉 é'\r" --step "500:Write-Host 'warning' -ForegroundColor Yellow -BackgroundColor DarkBlue\r" --step '500:Write-Output ("`e[44m" + (''x'' * 200) + "`e[0m")\r' -- pwsh.exe -NoLogo -NoProfile
```
The third step is single-quoted so that the outer PowerShell passes the backticks through: the recorder then types `` `e `` (PowerShell's own escape for ESC) into the inner PowerShell. Typing a raw ESC byte instead would make PSReadLine clear the line.
Expected: each prints `wrote N bytes`, with N well above 1000. Inspect one recording by feeding it through a grid (`cargo test` in Step 4 does this) before committing.

Create `tests/recordings/README.md`:
```markdown
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
```

- [ ] **Step 3: Write the recording round-trip test**

Create `crates/jojo-grid/tests/recordings.rs`:
```rust
//! Round-trips every recording in tests/recordings and writes the inputs
//! for tests/xterm-crosscheck into tests/recordings/out (git-ignored).

use jojo_grid::TermGrid;
use std::path::PathBuf;

fn recordings_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/recordings")
}

/// Parses `name.COLSxROWS.bin` into (name, cols, rows).
fn parse_name(file: &str) -> Option<(String, u16, u16)> {
    let stem = file.strip_suffix(".bin")?;
    let (name, size) = stem.rsplit_once('.')?;
    let (cols, rows) = size.split_once('x')?;
    Some((name.to_string(), cols.parse().ok()?, rows.parse().ok()?))
}

#[test]
fn every_recording_round_trips_and_cross_check_inputs_are_written() {
    let dir = recordings_dir();
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let mut count = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        let Some((name, cols, rows)) = parse_name(&file) else { continue };
        let bytes = std::fs::read(&path).unwrap();

        let mut a = TermGrid::new(cols, rows, 10_000);
        a.feed(&bytes);
        let snapshot = a.snapshot();
        let mut b = TermGrid::new(cols, rows, 10_000);
        b.feed(&snapshot);
        let (da, db) = (a.debug_dump(), b.debug_dump());
        if da != db {
            let diff = da
                .lines()
                .zip(db.lines())
                .find(|(x, y)| x != y)
                .map(|(x, y)| format!("original: {x}\nrestored: {y}"))
                .unwrap_or_else(|| "dumps differ in length".into());
            panic!("recording {file} did not round-trip\n{diff}");
        }

        std::fs::write(out.join(format!("{name}.snap")), &snapshot).unwrap();
        std::fs::write(out.join(format!("{name}.txt")), a.visible_text()).unwrap();
        std::fs::write(out.join(format!("{name}.size")), format!("{cols} {rows}")).unwrap();
        count += 1;
    }
    assert!(count >= 3, "expected at least 3 recordings, found {count}");
}
```
Append to `.gitignore`:
```
/tests/recordings/out/
/tests/xterm-crosscheck/node_modules/
```

- [ ] **Step 4: Run it**

Run: `cargo test -p jojo-grid --test recordings`
Expected: pass. If a recording fails to round-trip, the failure is a real snapshot defect: read the first differing dump line, fix `snapshot.rs`, and add a minimal synthetic fixture to `roundtrip.rs` that reproduces it. If the difference is one of Plan 1's documented gaps (alternate-screen primary scrollback, scroll region, origin mode, title, tab characters), stop and report it with the differing line instead of masking it.

- [ ] **Step 5: Write the xterm.js cross-check**

Create `tests/xterm-crosscheck/package.json`:
```json
{
  "name": "jojo-xterm-crosscheck",
  "private": true,
  "type": "module",
  "scripts": { "test": "vitest run" },
  "devDependencies": {
    "@xterm/addon-unicode11": "0.9.0",
    "@xterm/headless": "6.0.0",
    "vitest": "^5.1.0"
  }
}
```
Create `tests/xterm-crosscheck/vitest.config.ts`:
```ts
import { defineConfig } from "vitest/config";

export default defineConfig({ test: { environment: "node", testTimeout: 30_000 } });
```
Create `tests/xterm-crosscheck/crosscheck.test.ts`:
```ts
// Replays each snapshot written by crates/jojo-grid/tests/recordings.rs into
// @xterm/headless (the emulator the app uses) and compares the visible text
// with alacritty_terminal's (spec section 11, item 1). Run the Rust test
// first: it writes tests/recordings/out.
import { Terminal } from "@xterm/headless";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const outDir = join(import.meta.dirname, "..", "recordings", "out");
const names = readdirSync(outDir)
  .filter((f) => f.endsWith(".snap"))
  .map((f) => f.slice(0, -".snap".length));

function visibleText(term: Terminal, rows: number): string {
  const buffer = term.buffer.active;
  const lines: string[] = [];
  for (let y = 0; y < rows; y++) {
    lines.push(buffer.getLine(buffer.baseY + y)?.translateToString(true) ?? "");
  }
  while (lines.length > 0 && lines[lines.length - 1] === "") lines.pop();
  return lines.join("\n");
}

describe("snapshots replay identically in xterm.js", () => {
  it("found recordings", () => expect(names.length).toBeGreaterThanOrEqual(3));

  for (const name of names) {
    it(name, async () => {
      const [cols, rows] = readFileSync(join(outDir, `${name}.size`), "utf8").split(" ").map(Number);
      const term = new Terminal({ cols, rows, scrollback: 10_000, allowProposedApi: true });
      term.loadAddon(new Unicode11Addon());
      term.unicode.activeVersion = "11";
      const snapshot = readFileSync(join(outDir, `${name}.snap`));
      await new Promise<void>((resolve) => term.write(new Uint8Array(snapshot), resolve));
      const expected = readFileSync(join(outDir, `${name}.txt`), "utf8");
      expect(visibleText(term, rows).split("\n").map((l) => l.trimEnd()).join("\n")).toBe(expected);
    });
  }
});
```

- [ ] **Step 6: Run the cross-check**

Run:
```powershell
pnpm --dir tests/xterm-crosscheck install
cargo test -q -p jojo-grid --test recordings
pnpm --dir tests/xterm-crosscheck test
```
Expected: every recording passes. A mismatch means alacritty_terminal and xterm.js interpret part of the stream differently (spec risk 14.2). Record the recording name and the first differing line in the report, and report DONE_WITH_CONCERNS; do not change the comparison.

- [ ] **Step 7: Commit**

```bash
git add .gitignore crates/jojod/examples crates/jojo-grid/tests/recordings.rs tests/recordings tests/xterm-crosscheck/package.json tests/xterm-crosscheck/pnpm-lock.yaml tests/xterm-crosscheck/vitest.config.ts tests/xterm-crosscheck/crosscheck.test.ts
git commit -m "test: record real Windows sessions and cross-check snapshots against xterm.js"
```

---

## Self-Review Notes

- **Carry-over coverage.** 2.1 items 1–5: Task 1 (Attach size, snapshot bound and encode check, `Error.session_id`, `attachable`, malformed frames). 2.3: item 1 Task 2 (queue); 2 Task 2 (lag disconnect); 3 deferred (stated above); 4 Task 1 (`resize` grid first); 5–6 Task 3; 7 deferred to Plan 2b (exited-grid cap depends on how the app lists sessions; recorded there); 8–11 Task 3 and Task 2 (`last_os_error` order); 12 is satisfied by daemon validation (Task 1 also validates `Attach`). 2.4: items 1 Task 7 (except the Claude Code recording, which needs the app); 2 Task 5; 3 partially (the always-forward reply path is exercised by every ConPTY test, since `cmd.exe` stalls without it; the detached-client assertion is not added and stays a Minor); 4 Tasks 1–3 (spawn failure, unknown ids) and Task 2 (owner check), with the DACL read-back, version-mismatch and shutdown-kills-processes tests not added. Plan 2b's inputs must list the remaining test gaps.
- **Type consistency.** `Session::attach` gains `cols, rows` in Task 1 and changes its sender type to `ClientTx` in Task 2; Task 2 Step 5 updates every call site. `Reply` changes from `UnboundedSender<Frame>` to `ClientTx` in Task 2. `Error` gains `session_id` in Task 1; every constructor in Tasks 1–3 includes it.
