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
    // This test binary is single-threaded at this point and owns its
    // environment, so setting a process-wide variable here cannot race with
    // another thread of this process also reading or writing it.
    std::env::set_var("CLAUDECODE", "1");
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
