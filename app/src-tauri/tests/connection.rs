//! Starts a real jojod (target/debug/jojod.exe) through the app's daemon
//! manager. Build it first: `cargo build -p jojod`.

use jojo_app::daemon_manager::{connect_or_start, DaemonPaths};
use jojo_proto::{ControlMsg, Frame};
use jojod::client::Client;
use jojod::paths::Endpoint;
use std::path::{Path, PathBuf};
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
