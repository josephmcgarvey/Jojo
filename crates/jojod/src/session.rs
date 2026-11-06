//! One PTY session: the process, its grid, and attached clients.
//!
//! Threads per session:
//! - reader: reads PTY output, feeds the grid and sends Output frames to
//!   subscribers, all while holding `shared`;
//! - writer: writes queued input to the PTY so a slow program never blocks
//!   the daemon's async tasks;
//! - waiter: waits for the process to exit, closes the PTY master (which
//!   ends the reader on ConPTY), joins the reader thread so every buffered
//!   Output frame has already been sent, and only then calls `on_exit`. The
//!   waiter never holds `shared` while joining: it takes the master out of
//!   its mutex and drops the guard before dropping the master itself, so a
//!   concurrent `resize` is never blocked behind `ClosePseudoConsole`.
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

        let reader_handle = {
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
            })?
        };

        {
            let master = master.clone();
            thread::Builder::new().name(format!("pty-wait-{id}")).spawn(move || {
                let code = child.wait().ok().map(|status| status.exit_code() as i32);
                // Take the master out of its mutex and drop the guard before
                // dropping the master itself (Finding F3): dropping it is
                // what closes the pseudoconsole (ClosePseudoConsole), which
                // ends the reader thread's blocking read on Windows, and
                // that close must not run while `shared`'s master mutex is
                // held, or a concurrent `resize` would block on it.
                let taken = { master.lock().expect("master lock").take() };
                drop(taken);
                // Wait for the reader thread to reach EOF and finish fanning
                // out whatever output was still buffered in the
                // pseudoconsole. Without this, `on_exit` (and so the
                // daemon's SessionExited broadcast) could race ahead of the
                // session's final Output frames (Finding F3).
                let _ = reader_handle.join();
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
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let (exit_tx, exit_rx) = std::sync::mpsc::channel();

        let session = Session::spawn(SessionId::new(), exit_immediately_spec(), move |code| {
            // Checked synchronously, on the waiter thread, at the instant
            // `on_exit` fires: has the reader thread already delivered the
            // session's final output to this subscriber? No further
            // waiting happens here; that immediacy is the point of the
            // test.
            let mut grid = TermGrid::new(80, 24, 1000);
            while let Ok(frame) = rx.try_recv() {
                match frame {
                    Frame::Snapshot { bytes, .. } | Frame::Output { bytes, .. } => grid.feed(&bytes),
                    _ => {}
                }
            }
            let seen = has_line_text(&grid.visible_text(), "jojo-last-line");
            let _ = exit_tx.send((code, seen));
        })
        .unwrap();

        session.attach(1, tx);
        let (code, seen_before_exit) = exit_rx.recv_timeout(Duration::from_secs(15)).expect("exit reported");
        assert_eq!(code, Some(0));
        assert!(seen_before_exit, "SessionExited fired before the subscriber had the session's final output");
    }
}
