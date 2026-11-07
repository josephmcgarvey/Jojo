//! Starts a real jojod (target/debug/jojod.exe) through the app's daemon
//! manager. Build it first: `cargo build -p jojod`.

use jojo_app::bridge::{Bridge, ENVELOPE_FRAME, ENVELOPE_STATUS};
use jojo_app::daemon_manager::{connect_or_start, DaemonPaths};
use jojo_proto::{ControlMsg, Frame, SessionKind};
use jojod::client::Client;
use jojod::paths::Endpoint;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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

/// Sends `Shutdown` and waits for the daemon to actually exit before
/// returning, so callers can safely delete the folder holding its exe
/// right afterward. Without this, a caller that deletes the `TempDir`
/// immediately after sending `Shutdown` can race a daemon that is still
/// shutting down: on a slow machine this leaves either a leftover jojod
/// process or a failed `TempDir` drop (Windows refuses to delete a file
/// an open process still holds).
///
/// Waiting for EOF on the connection is not enough by itself: jojod closes
/// the pipe as part of exiting, but the process can still be unwinding (and
/// so still hold its own exe open) for a brief moment after the pipe
/// closes. So this also polls for the exe to become deletable, which is the
/// actual property the caller needs.
pub async fn shutdown_and_wait(mut client: Client, exe: &Path) {
    client.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();

    // The daemon closes the connection as it exits; read until that EOF (or
    // any other error, which also means the connection is gone).
    let drained = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if client.recv().await.is_err() {
                return;
            }
        }
    })
    .await;
    assert!(drained.is_ok(), "jojod did not close the connection within 10s of Shutdown");

    // The exe can still be briefly held open after the pipe closes while the
    // process finishes unwinding; retry deleting it until it is free.
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match std::fs::remove_file(exe) {
            Ok(()) => return,
            Err(e) => {
                if Instant::now() >= deadline {
                    panic!("jojod.exe at {} was still held 2s after shutdown: {e}", exe.display());
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
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
    let client = connect_or_start(&paths).await.expect("daemon started and connected");
    assert!(!client.daemon_version().is_empty());
    let exe = paths.bin_dir.join("jojod.exe");
    assert!(exe.exists(), "binaries were staged");
    shutdown_and_wait(client, &exe).await;
}

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
    let paths_bin_exe = paths.bin_dir.join("jojod.exe");
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
    shutdown_and_wait(jojod::client::Client::connect(&endpoint).await.unwrap(), &paths_bin_exe).await;
}

/// Regression test for Task 4 fix round 1, Critical #1: `connection::serve`
/// used to re-create the `read_frame(&mut reader)` future on every
/// `tokio::select!` iteration. `read_frame` is built on
/// `AsyncReadExt::read_exact`, which is not cancellation-safe: if the
/// `rx.recv()` (an outbound frame from the frontend) branch won the race
/// while a `read_frame` call had already consumed part of an incoming frame
/// from the kernel buffer, that partial read was discarded. The next
/// `read_frame` call then resynced on garbage (usually decoding to a bogus
/// length) and the connection dropped into `Reconnecting`.
///
/// This creates a project and a `cmd.exe` session, attaches, starts a
/// `for /L` loop that prints 2000 lines (so there is plenty of incoming
/// `Output` traffic in flight), and, while that is still running, fires 200
/// `Input` frames back-to-back through the bridge with no delay between
/// them. Each of those is a chance for the old code to interrupt an
/// in-progress read. The test passes only if the daemon's last line
/// ("line2000") is eventually seen *and* no `Reconnecting` status was ever
/// emitted in between.
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_input_and_output_keep_the_connection_alive() {
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
    let paths_bin_exe = paths.bin_dir.join("jojod.exe");
    let bridge = Arc::new(Bridge::default());
    let seen = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let sink = seen.clone();
    bridge.set_sink(Box::new(move |bytes| sink.lock().unwrap().push(bytes)));
    let task = tokio::spawn(jojo_app::connection::run(paths, bridge.clone()));

    assert!(
        wait_for(&seen, |e| e[0] == ENVELOPE_STATUS && e[1..].starts_with(br#"{"state":"connected""#)).await,
        "never connected: {:?}",
        printable(&seen)
    );

    let project_dir = scratch.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();
    send_control(&bridge, ControlMsg::AddProject { path: project_dir.to_string_lossy().into_owned(), name: "race".into() });
    let project = wait_for_control(&seen, Duration::from_secs(15), |msg| matches!(msg, ControlMsg::ProjectAdded { .. }))
        .await
        .and_then(|msg| match msg {
            ControlMsg::ProjectAdded { project } => Some(project),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no ProjectAdded reply: {:?}", printable(&seen)));

    let session_id = jojo_proto::SessionId::new();
    send_control(
        &bridge,
        ControlMsg::CreateSession {
            session_id,
            project_id: project.id,
            kind: SessionKind::Shell,
            cols: 80,
            rows: 24,
            shell: Some("cmd.exe".into()),
        },
    );
    assert!(
        wait_for_control(&seen, Duration::from_secs(15), |msg| matches!(msg, ControlMsg::SessionCreated { .. })).await.is_some(),
        "no SessionCreated reply: {:?}",
        printable(&seen)
    );

    send_control(&bridge, ControlMsg::Attach { session_id, cols: 80, rows: 24 });

    // Start a loop that keeps the daemon pushing Output frames...
    bridge
        .send(Frame::Input { session_id, bytes: b"for /L %i in (1,1,2000) do @echo line%i\r".to_vec() }.encode()[4..].to_vec())
        .expect("connected bridge accepts frames");
    // ...and, while it is still running, flood the same connection with
    // Input frames with no delay between them, to maximize the chance of
    // interrupting an in-progress read of an Output frame.
    for i in 0..200 {
        bridge
            .send(Frame::Input { session_id, bytes: format!("echo x{i}\r").into_bytes() }.encode()[4..].to_vec())
            .expect("connected bridge accepts frames");
    }

    let saw_last_line = wait_for_long(&seen, Duration::from_secs(60), |e| {
        e[0] == ENVELOPE_FRAME && String::from_utf8_lossy(&e[2..]).contains("line2000")
    })
    .await;
    assert!(saw_last_line, "never saw the loop's last line: {:?}", printable(&seen).iter().rev().take(20).collect::<Vec<_>>());

    let ever_reconnected = seen.lock().unwrap().iter().any(|e| e[0] == ENVELOPE_STATUS && e[1..].starts_with(br#"{"state":"reconnecting""#));
    assert!(!ever_reconnected, "the connection dropped under load: {:?}", printable(&seen));

    task.abort();
    shutdown_and_wait(jojod::client::Client::connect(&endpoint).await.unwrap(), &paths_bin_exe).await;
}

fn send_control(bridge: &Bridge, msg: ControlMsg) {
    bridge.send(Frame::Control(msg).encode()[4..].to_vec()).expect("connected bridge accepts frames");
}

/// Every `ControlMsg` reply seen so far (frame envelopes whose body decodes
/// as `ControlMsg` JSON; data frames are skipped since they are raw bytes).
fn control_messages(seen: &Arc<Mutex<Vec<Vec<u8>>>>) -> Vec<ControlMsg> {
    seen.lock()
        .unwrap()
        .iter()
        .filter(|e| e[0] == ENVELOPE_FRAME)
        .filter_map(|e| serde_json::from_slice::<ControlMsg>(&e[2..]).ok())
        .collect()
}

async fn wait_for_control(seen: &Arc<Mutex<Vec<Vec<u8>>>>, max_wait: Duration, pred: impl Fn(&ControlMsg) -> bool) -> Option<ControlMsg> {
    let deadline = tokio::time::Instant::now() + max_wait;
    loop {
        if let Some(msg) = control_messages(seen).into_iter().find(&pred) {
            return Some(msg);
        }
        if tokio::time::Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn wait_for_long(seen: &Arc<Mutex<Vec<Vec<u8>>>>, max_wait: Duration, pred: impl Fn(&Vec<u8>) -> bool) -> bool {
    let deadline = tokio::time::Instant::now() + max_wait;
    loop {
        if seen.lock().unwrap().iter().any(&pred) {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
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
