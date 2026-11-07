//! One PTY session: the process, its grid, and attached clients.
//!
//! Threads per session:
//! - reader: reads PTY output, feeds the grid and sends Output frames to
//!   subscribers, all while holding `shared`; signals (by dropping its end
//!   of a channel) once its loop ends, as the last thing it does;
//! - writer: writes queued input to the PTY so a slow program never blocks
//!   the daemon's async tasks;
//! - waiter: waits for the process to exit, closes the PTY master (which
//!   ends the reader on ConPTY), waits for the reader's signal (bounded by
//!   `READER_DRAIN_TIMEOUT`, not an unbounded join — see that constant's
//!   doc comment for why Unix cannot guarantee EOF within any bound), and
//!   only then calls `on_exit`. The waiter never holds the master's mutex
//!   while waiting on that signal: it takes the master out of its own
//!   mutex and drops the guard before dropping the master itself, so a
//!   concurrent `resize` is never blocked behind `ClosePseudoConsole`.
//!
//! `attach` takes the snapshot and registers the subscriber while holding
//! `shared`, the same lock the reader holds while feeding. No output can
//! fall between the snapshot and the first Output frame, and none appears
//! in both (spec section 5.5, rule 3).
//!
//! Lock order: `shared` before `master`. `attach` and `resize` each take
//! `shared` first and `master` second (through `resize_pty`); never acquire
//! them in the reverse order.
//!
//! The daemon itself answers terminal queries (DSR, DA, DECRQM, and the
//! like) by parsing the PTY output through `TermGrid` and writing
//! `TermGrid::take_replies`'s result back to the PTY (see the reader thread
//! below); an attached window must never also answer them, since the
//! program issuing the query only expects one reply.

use anyhow::Context;
use jojo_grid::TermGrid;
use jojo_proto::{Frame, SessionId};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc as std_mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use crate::client_queue::ClientTx;

pub type ClientId = u64;

/// Bounds how long the waiter waits for the reader thread to signal (by
/// dropping its end of a channel) that it has reached EOF and finished
/// fanning out the session's final output, before giving up and reporting
/// exit anyway. On Windows, dropping the PTY master forces EOF via
/// `ClosePseudoConsole`, so the reader signals promptly and this bound is
/// rarely hit. On Unix, the reader holds a `dup` of the master fd and only
/// sees EOF once every slave fd referencing the pty is closed; that never
/// happens if a grandchild process inherited the slave fd and is still
/// running after the direct child being waited on has already exited (for
/// example `sh -c 'sleep 600 &'`, or a background job a shell leaves
/// running after its own `exit`). Without this bound, waiting unboundedly
/// for the reader would then block forever, `on_exit` would never run, and
/// the daemon would never broadcast `SessionExited` for that session.
const READER_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

/// Largest snapshot payload that still fits in one frame together with the
/// frame type byte and the 16-byte session id.
pub const MAX_SNAPSHOT_BYTES: usize = jojo_proto::MAX_FRAME_LEN - 1 - 16;

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
    subscribers: Vec<(ClientId, ClientTx)>,
}

pub struct Session {
    pub id: SessionId,
    shared: Arc<Mutex<Shared>>,
    input_tx: std_mpsc::Sender<Vec<u8>>,
    master: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    /// Set by the waiter thread, immediately after `child.wait()` returns,
    /// before it does anything else (close the master, wait for the reader,
    /// call `on_exit`). `kill()` checks this first and returns without
    /// calling the killer once it is set: on Unix, portable-pty's killer
    /// signals the child by its raw PID, and once `wait()` has reaped that
    /// process the OS is free to recycle the PID for an unrelated process,
    /// so a `kill()` racing in after reaping could signal the wrong process
    /// entirely. On Windows the killer holds a process handle rather than a
    /// PID, so a `kill()` after exit is merely a harmless no-op there, but
    /// the check is cheap and applies uniformly on both platforms. There is
    /// a narrow window between the OS reaping the process inside `wait()`
    /// and this flag being set a moment later; closing it completely would
    /// need a lock held across `wait()` itself (blocking `kill()` for the
    /// process's entire lifetime), so it is left open but is far smaller
    /// than the window this fixes (which spanned the process's exit all the
    /// way to `on_exit` returning and the `Session` being dropped or
    /// explicitly closed).
    exited: Arc<AtomicBool>,
}

fn pty_size(cols: u16, rows: u16) -> PtySize {
    PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }
}

/// What `Session::spawn`'s early setup (reader clone, writer take, the
/// `pty-in` and `pty-out` thread spawns) produces once it succeeds: the
/// pieces needed to build the final `Session` and to spawn the `pty-wait`
/// thread, which alone takes ownership of `child`.
type SessionSetup = (
    Arc<Mutex<Shared>>,
    std_mpsc::Sender<Vec<u8>>,
    Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>,
    std_mpsc::Receiver<()>,
);

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
        // The session is a fresh terminal, not a child of whatever Claude
        // Code process may have started the daemon: without this, a daemon
        // launched from inside a Claude Code shell would have every session
        // it spawns inherit CLAUDECODE and appear nested inside Claude Code.
        cmd.env_remove("CLAUDECODE");
        // Applied after the removal above, so a caller that explicitly sets
        // CLAUDECODE in `spec.env` (for example a test) still wins.
        for (key, value) in &spec.env {
            cmd.env(key, value);
        }
        let mut child = pair
            .slave
            .spawn_command(cmd)
            .with_context(|| format!("start {}", spec.program))?;
        drop(pair.slave);

        let mut killer = child.clone_killer();

        // `child` must not be moved into this closure: if any `?` inside it
        // fails after `spawn_command` has already started the process
        // (reader clone, writer take, or either of these two thread spawns),
        // the error path below kills that process instead of leaking a
        // running child with nothing left in `Session` to stop it.
        let setup = (move || -> anyhow::Result<SessionSetup> {
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

            // Signaled, by being dropped, once the reader thread's loop ends
            // (EOF or a read error) and it has finished fanning out its last
            // chunk of output: the waiter below waits on this, bounded by
            // `READER_DRAIN_TIMEOUT`, instead of joining the reader thread
            // outright (Finding F3 fix round 2; see that constant's doc comment
            // for why an unbounded join is unsound on Unix).
            let (reader_done_tx, reader_done_rx) = std_mpsc::channel::<()>();

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
                        // The grid is the VT emulator of record (it is what
                        // computed `replies` by parsing the stream), so it
                        // always answers protocol queries such as ESC [ 6 n
                        // itself; it never waits on an attached client to do
                        // so, since Frame::Input carries user keystrokes, not
                        // emulator-generated replies, and a session with no
                        // client attached must still unblock the child.
                        if !replies.is_empty() {
                            let _ = input_tx.send(replies);
                        }
                    }
                    drop(reader_done_tx);
                })?;
            }

            Ok((shared, input_tx, master, reader_done_rx))
        })();

        let (shared, input_tx, master, reader_done_rx) = match setup {
            Ok(v) => v,
            Err(e) => {
                let _ = child.kill();
                return Err(e);
            }
        };

        let exited = Arc::new(AtomicBool::new(false));

        {
            let master = master.clone();
            let exited = exited.clone();
            // `child` (and so the only handle able to reap the process) is
            // moved into this closure. If spawning the thread itself fails,
            // the closure is dropped unrun and `child` goes with it, without
            // ever being waited on or killed: the process would otherwise
            // keep running with nothing left in `Session` able to stop it.
            // `killer` was cloned from `child` earlier and is still owned
            // here (not yet moved into the `Session` being built), so it is
            // used to kill the process before returning the error.
            if let Err(e) = thread::Builder::new().name(format!("pty-wait-{id}")).spawn(move || {
                let code = child.wait().ok().map(|status| status.exit_code() as i32);
                // Set before anything else below: see `Session::exited`'s
                // doc comment for why `kill()` must stop calling the killer
                // from this point on.
                exited.store(true, Ordering::SeqCst);
                // Take the master out of its own mutex and drop the guard
                // before dropping the master itself (Finding F3): dropping
                // it is what closes the pseudoconsole (ClosePseudoConsole),
                // which ends the reader thread's blocking read on Windows,
                // and that close must not run while the master's own mutex
                // is held, or a concurrent `resize` would block on it.
                let taken = { master.lock().expect("master lock").take() };
                drop(taken);
                // Wait, bounded, for the reader thread to signal that it has
                // reached EOF and finished fanning out whatever output was
                // still buffered in the pseudoconsole, so `on_exit` (and so
                // the daemon's SessionExited broadcast) does not race ahead
                // of the session's final Output frames (Finding F3). Bounded
                // rather than joined outright: see `READER_DRAIN_TIMEOUT`'s
                // doc comment for why EOF is not guaranteed within any
                // bound on Unix. Past the timeout, exit is still reported;
                // the alternative is a `SessionExited` that never arrives.
                let _ = reader_done_rx.recv_timeout(READER_DRAIN_TIMEOUT);
                on_exit(code);
            }) {
                let _ = killer.kill();
                return Err(e.into());
            }
        }

        Ok(Arc::new(Session { id, shared, input_tx, master, killer: Mutex::new(killer), exited }))
    }

    /// Sends one Snapshot laid out for `cols` x `rows`, then live Output.
    /// The grid and PTY are resized first when the size differs, because a
    /// snapshot replays correctly only into a terminal of the grid's size.
    /// Lock order: `shared`, then `master` (same as `resize`).
    pub fn attach(&self, client: ClientId, tx: ClientTx, cols: u16, rows: u16) {
        let mut s = self.shared.lock().expect("session lock");
        if (cols, rows) != (s.grid.cols(), s.grid.rows()) {
            s.grid.resize(cols, rows);
            self.resize_pty(s.grid.cols(), s.grid.rows());
        }
        let bytes = s.grid.snapshot_bounded(MAX_SNAPSHOT_BYTES);
        if tx.send(Frame::Snapshot { session_id: self.id, bytes }) {
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

    pub fn kill(&self) {
        // See `exited`'s doc comment: once the process has already been
        // reaped, the killer's PID (on Unix) may since have been recycled
        // for an unrelated process, so signaling it would be worse than a
        // no-op.
        if self.has_exited() {
            return;
        }
        let _ = self.killer.lock().expect("killer lock").kill();
    }

    /// True once the waiter thread has observed the process exit (`wait()`
    /// returned). Used by `kill()` to skip signaling a reaped process, and
    /// by tests.
    pub(crate) fn has_exited(&self) -> bool {
        self.exited.load(Ordering::SeqCst)
    }

    pub fn visible_text(&self) -> String {
        self.shared.lock().expect("session lock").grid.visible_text()
    }
}

impl Drop for Session {
    /// A session is dropped when the daemon closes or forgets it; its
    /// process must not outlive it.
    fn drop(&mut self) {
        self.kill();
    }
}

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

    /// Blocking receive with a timeout, for tests outside an async runtime.
    fn recv_blocking(rx: &mut crate::client_queue::ClientRx, timeout: Duration) -> Option<Frame> {
        let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
        rt.block_on(async { tokio::time::timeout(timeout, rx.recv()).await.ok().flatten() })
    }

    #[test]
    fn attach_receives_snapshot_then_output_and_exit_is_reported() {
        let (exit_tx, exit_rx) = std::sync::mpsc::channel();
        let session = Session::spawn(SessionId::new(), test_spec(), move |code| {
            let _ = exit_tx.send(code);
        })
        .unwrap();

        let (tx, mut rx) = crate::client_queue::client_queue(crate::client_queue::MAX_CLIENT_QUEUE_BYTES);
        session.attach(1, tx, 80, 24);
        assert!(matches!(recv_blocking(&mut rx, Duration::from_secs(15)), Some(Frame::Snapshot { .. })));

        session.write(b"echo jojo-hi\r".to_vec());
        let mut grid = TermGrid::new(80, 24, 1000);
        let deadline = Instant::now() + Duration::from_secs(15);
        while !has_line(&grid, "jojo-hi") {
            match rx.try_recv() {
                Some(Frame::Output { bytes, .. }) => grid.feed(&bytes),
                Some(_) => {}
                None => std::thread::sleep(Duration::from_millis(20)),
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
    fn dropping_a_session_kills_its_process() {
        let (exit_tx, exit_rx) = std::sync::mpsc::channel();
        let session = Session::spawn(SessionId::new(), test_spec(), move |code| {
            let _ = exit_tx.send(code);
        })
        .unwrap();
        drop(session);
        exit_rx.recv_timeout(Duration::from_secs(15)).expect("exit reported after drop");
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
    fn kill_after_exit_does_not_call_the_killer() {
        #[cfg(windows)]
        let args = vec!["/d".into(), "/c".into(), "exit".into(), "0".into()];
        #[cfg(unix)]
        let args = vec!["-c".into(), "exit 0".into()];
        let mut spec = test_spec();
        spec.args = args;

        let (exit_tx, exit_rx) = std::sync::mpsc::channel();
        let session = Session::spawn(SessionId::new(), spec, move |code| {
            let _ = exit_tx.send(code);
        })
        .unwrap();
        assert!(!session.has_exited(), "freshly spawned session should not be reported exited");

        let code = exit_rx.recv_timeout(Duration::from_secs(15)).expect("exit reported");
        assert_eq!(code, Some(0));
        assert!(session.has_exited());

        // Neither must panic, even though the process has already been
        // reaped and `kill()` must not signal its (possibly recycled) PID.
        session.kill();
        drop(session);
    }

    #[test]
    fn detached_client_receives_no_output() {
        let session = Session::spawn(SessionId::new(), test_spec(), |_| {}).unwrap();
        let (tx, mut rx) = crate::client_queue::client_queue(crate::client_queue::MAX_CLIENT_QUEUE_BYTES);
        session.attach(7, tx, 80, 24);
        let _snapshot = recv_blocking(&mut rx, Duration::from_secs(15));
        session.detach(7);
        session.write(b"echo after-detach\r".to_vec());
        std::thread::sleep(Duration::from_millis(1500));
        while let Some(frame) = rx.try_recv() {
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

    /// A command that prints a distinctive line and exits immediately,
    /// instead of `test_spec`'s interactive shell, to make the window
    /// between the process truly exiting and the PTY's buffered output
    /// being delivered as narrow as possible (Finding F3).
    fn exit_immediately_spec() -> SpawnSpec {
        #[cfg(windows)]
        let (program, args) =
            ("cmd.exe".to_string(), vec!["/d".into(), "/c".into(), "echo jojo-last-line".into()]);
        #[cfg(unix)]
        let (program, args) = ("sh".to_string(), vec!["-c".into(), "echo jojo-last-line".into()]);
        SpawnSpec {
            program,
            args,
            cwd: std::env::temp_dir(),
            env: vec![("JOJO_SESSION_ID".into(), "test".into())],
            cols: 80,
            rows: 24,
            scrollback: 1000,
        }
    }

    /// Finding F3: `on_exit` (and so the daemon's `SessionExited` broadcast)
    /// must not fire before the reader thread has fanned out the session's
    /// final output. A subscriber attached as early as possible may see
    /// that output either in the `Snapshot` (if attach happens to run after
    /// the output was already fed into the grid) or in a later `Output`
    /// frame (if attach runs first); both are fed into one grid here so the
    /// assertion holds regardless of which race attach wins.
    #[test]
    fn exit_is_reported_only_after_final_output_is_sent() {
        let (tx, mut rx) = crate::client_queue::client_queue(crate::client_queue::MAX_CLIENT_QUEUE_BYTES);
        let (exit_tx, exit_rx) = std::sync::mpsc::channel();

        let session = Session::spawn(SessionId::new(), exit_immediately_spec(), move |code| {
            // Checked synchronously, on the waiter thread, at the instant
            // `on_exit` fires: has the reader thread already delivered the
            // session's final output to this subscriber? No further
            // waiting happens here; that immediacy is the point of the
            // test.
            let mut grid = TermGrid::new(80, 24, 1000);
            while let Some(frame) = rx.try_recv() {
                match frame {
                    Frame::Snapshot { bytes, .. } | Frame::Output { bytes, .. } => grid.feed(&bytes),
                    _ => {}
                }
            }
            let seen = has_line_text(&grid.visible_text(), "jojo-last-line");
            let _ = exit_tx.send((code, seen));
        })
        .unwrap();

        session.attach(1, tx, 80, 24);
        let (code, seen_before_exit) = exit_rx.recv_timeout(Duration::from_secs(15)).expect("exit reported");
        assert_eq!(code, Some(0));
        assert!(seen_before_exit, "SessionExited fired before the subscriber had the session's final output");
    }

    /// Finding F3 (fix round 2, Unix only): a grandchild that inherited the
    /// slave fd and keeps it open after the direct child has exited must
    /// not block `on_exit` forever. `sh -c 'sleep 30 & echo started'`
    /// backgrounds `sleep 30` (which keeps the pty's slave fd open for 30s)
    /// and then exits immediately; `on_exit` must still fire well within
    /// `READER_DRAIN_TIMEOUT`'s 2s bound, not after `sleep 30` eventually
    /// exits. This cannot run on Windows (ConPTY has no grandchild-fd-open
    /// scenario: `ClosePseudoConsole` forces EOF), so it only compiles and
    /// runs under `#[cfg(unix)]`.
    #[cfg(unix)]
    #[test]
    fn exit_is_reported_despite_a_backgrounded_grandchild_holding_the_pty_open() {
        let (exit_tx, exit_rx) = std::sync::mpsc::channel();
        let mut spec = test_spec();
        spec.args = vec!["-c".into(), "sleep 30 & echo started".into()];
        let session = Session::spawn(SessionId::new(), spec, move |code| {
            let _ = exit_tx.send(code);
        })
        .unwrap();

        let code = exit_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("on_exit must fire within READER_DRAIN_TIMEOUT, not wait for the backgrounded grandchild");
        assert_eq!(code, Some(0));
        session.kill();
    }

    #[test]
    fn attach_with_a_new_size_resizes_the_session() {
        let session = Session::spawn(SessionId::new(), test_spec(), |_| {}).unwrap();
        let (tx, mut rx) = crate::client_queue::client_queue(crate::client_queue::MAX_CLIENT_QUEUE_BYTES);
        session.attach(1, tx, 100, 30);
        assert!(matches!(recv_blocking(&mut rx, Duration::from_secs(15)), Some(Frame::Snapshot { .. })));
        assert_eq!(session.size(), (100, 30));
        session.kill();
    }

    /// The flood writes a sentinel line after its loop, and this test polls
    /// `session.visible_text()` (fed by the reader thread regardless of
    /// whether any subscriber is still attached) for that sentinel instead
    /// of sleeping a fixed duration, so the test does not depend on the
    /// flood actually finishing within an arbitrary wall-clock guess: a
    /// loaded CI runner or a slow shell start can make 3000 lines take
    /// longer than a fixed sleep would allow, which is what made the
    /// previous 3-second-sleep version of this test flaky in a full
    /// workspace run.
    #[test]
    fn a_client_that_falls_behind_is_detached_with_an_error() {
        let session = Session::spawn(SessionId::new(), test_spec(), |_| {}).unwrap();
        let (tx, mut rx) = crate::client_queue::client_queue(4096);
        session.attach(1, tx, 80, 24);
        #[cfg(windows)]
        session.write(
            b"for /L %i in (1,1,3000) do @echo xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\r\necho jojo-flood-done\r"
                .to_vec(),
        );
        #[cfg(unix)]
        session.write(
            b"i=0; while [ $i -lt 3000 ]; do echo xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx; i=$((i+1)); done; echo jojo-flood-done\n"
                .to_vec(),
        );

        // Do not read from `rx` while the flood is in flight, so the
        // client's 4 KiB queue fills and the session detaches it. The grid
        // is fed unconditionally by the reader thread regardless of
        // subscriber state, so polling it here does not race the detach.
        let deadline = Instant::now() + Duration::from_secs(60);
        while !has_line_text(&session.visible_text(), "jojo-flood-done") {
            assert!(
                Instant::now() < deadline,
                "flood did not complete within 60s; screen:\n{}",
                session.visible_text()
            );
            std::thread::sleep(Duration::from_millis(50));
        }

        let mut saw_error = false;
        let mut output_after_error = false;
        while let Some(frame) = rx.try_recv() {
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
}
