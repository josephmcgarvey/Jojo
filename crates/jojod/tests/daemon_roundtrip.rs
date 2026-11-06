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
async fn resize_out_of_range_is_rejected_and_session_still_echoes() {
    // C2: `Resize { cols: 0, rows: 0 }` used to panic inside alacritty's
    // grid resize while the session's `shared` lock was held, poisoning it
    // and, through `client_gone` and the daemon-wide `sessions`/`store`
    // locks, cascading into every later request. `handle_control` now
    // rejects out-of-range sizes before they reach `Session::resize`, and
    // the session must keep working normally afterward.
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    let project = add_project(&mut c, dir.path()).await;
    let session = create_shell(&mut c, &project).await;

    c.send(Frame::Control(ControlMsg::Resize { session_id: session.id, cols: 0, rows: 0 })).await.unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Error { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::Error { request, .. }) => assert_eq!(request, "Resize"),
        _ => unreachable!(),
    }

    c.send(Frame::Control(ControlMsg::Attach { session_id: session.id })).await.unwrap();
    c.recv_until(WAIT, |f| matches!(f, Frame::Snapshot { .. })).await.unwrap();
    c.send(Frame::Input { session_id: session.id, bytes: b"echo jojo-still-alive\r".to_vec() }).await.unwrap();
    let mut grid = TermGrid::new(80, 24, 1000);
    let deadline = tokio::time::Instant::now() + WAIT;
    while !has_line(&grid, "jojo-still-alive") {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(left, c.recv()).await {
            Ok(Ok(Frame::Output { bytes, .. })) => grid.feed(&bytes),
            Ok(Ok(_)) => {}
            Ok(Err(e)) => panic!("connection error: {e}"),
            Err(_) => panic!("no echo after rejected resize; screen:\n{}", grid.visible_text()),
        }
    }

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
