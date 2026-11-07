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
