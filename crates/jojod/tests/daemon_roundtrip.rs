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
    let session_id = jojo_proto::SessionId::new();
    c.send(Frame::Control(ControlMsg::CreateSession {
        session_id,
        project_id: project.id,
        kind: SessionKind::Shell,
        cols: 80,
        rows: 24,
        shell: Some(test_shell()),
        resume: None,
        replaces: None,
    }))
    .await
    .unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::SessionCreated { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::SessionCreated { session }) => {
            assert_eq!(session.id, session_id, "daemon must use the client-chosen session id");
            session
        }
        _ => unreachable!(),
    }
}

/// True when a screen line is exactly `want`, or `want` after a `$ ` prompt.
/// Input sent before `sh` has printed its first prompt is echoed by the
/// terminal first; `sh` then prints `$ ` and the command's output lands on
/// the same line (`$ jojo-hi`). The echoed command line (`echo jojo-hi`)
/// never matches.
fn has_line(grid: &TermGrid, want: &str) -> bool {
    grid.visible_text().lines().any(|l| {
        let line = l.trim();
        line == want || line.strip_prefix("$ ").is_some_and(|rest| rest.trim() == want)
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn create_attach_echo_detach_reattach_shutdown() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();

    let project = add_project(&mut c, dir.path()).await;
    let session = create_shell(&mut c, &project).await;
    assert_eq!(session.label, test_shell().trim_end_matches(".exe"));

    c.send(Frame::Control(ControlMsg::Attach { session_id: session.id, cols: 80, rows: 24 })).await.unwrap();
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
    c.send(Frame::Control(ControlMsg::Attach { session_id: session.id, cols: 80, rows: 24 })).await.unwrap();
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

/// A session still running when the daemon is shut down (`jojod shutdown`,
/// which the app's Update jojod uses) is recorded as ended because jojod
/// stopped: exited with no exit code, which the app's Restore sessions
/// brings back. Before the fix, kill_all's own kill was recorded as the
/// session's exit code, so it was never restored.
#[tokio::test(flavor = "multi_thread")]
async fn a_session_running_at_shutdown_has_no_exit_code_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    let project = add_project(&mut c, dir.path()).await;
    let session = create_shell(&mut c, &project).await;
    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
    // The open connection keeps the stopped daemon alive (its connection
    // task holds it) while the killed session's waiter records the exit, up
    // to READER_DRAIN_TIMEOUT (2 s) later. This is what happens on Linux,
    // where the exit is recorded before `jojod shutdown` finishes.
    tokio::time::sleep(Duration::from_secs(3)).await;
    drop(c);

    let (endpoint, server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    c.send(Frame::Control(ControlMsg::ListSessions)).await.unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Sessions { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::Sessions { sessions }) => {
            let s = sessions.iter().find(|s| s.id == session.id).expect("session listed");
            assert_eq!(s.state, jojo_proto::AgentState::Exited);
            assert_eq!(s.exit_code, None, "a session ended by the daemon's shutdown keeps no exit code");
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

    c.send(Frame::Control(ControlMsg::Attach { session_id: session.id, cols: 80, rows: 24 })).await.unwrap();
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
    c.send(Frame::Control(ControlMsg::Attach { session_id: jojo_proto::SessionId::new(), cols: 80, rows: 24 })).await.unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Error { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::Error { request, .. }) => assert_eq!(request, "Attach"),
        _ => unreachable!(),
    }
    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
}

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
    let requested_id = jojo_proto::SessionId::new();
    c.send(Frame::Control(ControlMsg::CreateSession {
        session_id: requested_id,
        project_id: project.id,
        kind: SessionKind::Shell,
        cols: 80,
        rows: 24,
        shell: Some("definitely-not-a-real-program-jojo".into()),
        resume: None,
        replaces: None,
    }))
    .await
    .unwrap();
    // `SessionCreated` and `Error` are not ordered relative to each other:
    // `SessionCreated` reaches this client through its broadcast subscription
    // (forwarded asynchronously by a per-connection task), while `Error` is
    // sent directly to the requester's reply channel. Collect both,
    // whichever arrives first.
    let (mut created, mut error) = (None, None);
    while created.is_none() || error.is_none() {
        match c
            .recv_until(WAIT, |f| {
                matches!(f, Frame::Control(ControlMsg::SessionCreated { .. } | ControlMsg::Error { .. }))
            })
            .await
            .unwrap()
        {
            Frame::Control(ControlMsg::SessionCreated { session }) => created = Some(session),
            Frame::Control(ControlMsg::Error { request, message, session_id }) => {
                error = Some((request, message, session_id))
            }
            _ => unreachable!(),
        }
    }
    let created = created.unwrap();
    let (request, message, session_id) = error.unwrap();
    assert_eq!(created.id, requested_id, "the echoed session id must equal the requested one");
    assert!(!created.attachable);
    assert_eq!(request, "CreateSession");
    assert_eq!(session_id, Some(created.id));
    assert!(message.contains("PATH searched"), "{message}");

    c.send(Frame::Control(ControlMsg::ListSessions)).await.unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Sessions { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::Sessions { sessions }) => {
            let session = sessions.iter().find(|s| s.id == created.id).expect("session listed");
            assert!(!session.attachable);
            assert_eq!(session.exit_code, Some(-1));
        }
        _ => unreachable!(),
    }

    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
}

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

#[tokio::test(flavor = "multi_thread")]
async fn create_session_rejects_a_duplicate_id() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    let project = add_project(&mut c, dir.path()).await;
    let id = jojo_proto::SessionId::new();

    c.send(Frame::Control(ControlMsg::CreateSession {
        session_id: id,
        project_id: project.id,
        kind: SessionKind::Shell,
        cols: 80,
        rows: 24,
        shell: Some(test_shell()),
        resume: None,
        replaces: None,
    }))
    .await
    .unwrap();
    c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::SessionCreated { .. }))).await.unwrap();

    // A second CreateSession with the same client-chosen id, live this time
    // (not just a row in state.json), must be rejected rather than silently
    // taking over the running session.
    c.send(Frame::Control(ControlMsg::CreateSession {
        session_id: id,
        project_id: project.id,
        kind: SessionKind::Shell,
        cols: 80,
        rows: 24,
        shell: Some(test_shell()),
        resume: None,
        replaces: None,
    }))
    .await
    .unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Error { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::Error { request, message, session_id }) => {
            assert_eq!(request, "CreateSession");
            assert_eq!(session_id, Some(id));
            assert!(message.contains("already in use"), "{message}");
        }
        _ => unreachable!(),
    }

    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn create_session_with_out_of_range_size_reports_the_requested_session_id() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    let id = jojo_proto::SessionId::new();

    c.send(Frame::Control(ControlMsg::CreateSession {
        session_id: id,
        project_id: jojo_proto::ProjectId::new(),
        kind: SessionKind::Shell,
        cols: 0,
        rows: 0,
        shell: Some(test_shell()),
        resume: None,
        replaces: None,
    }))
    .await
    .unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Error { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::Error { request, session_id, .. }) => {
            assert_eq!(request, "CreateSession");
            assert_eq!(session_id, Some(id));
        }
        _ => unreachable!(),
    }

    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
}

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

#[tokio::test(flavor = "multi_thread")]
async fn close_session_twice_yields_one_removed_and_one_error() {
    // F1: the unknown-id check and the removal happen under one `store`
    // lock acquisition, so closing the same id twice (even back to back,
    // with no reply awaited in between) must produce exactly one
    // `SessionRemoved` broadcast and exactly one `Error` reply, never two of
    // either.
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    let project = add_project(&mut c, dir.path()).await;
    let session = create_shell(&mut c, &project).await;

    c.send(Frame::Control(ControlMsg::CloseSession { session_id: session.id })).await.unwrap();
    c.send(Frame::Control(ControlMsg::CloseSession { session_id: session.id })).await.unwrap();

    let (mut removed, mut errors) = (0, 0);
    for _ in 0..2 {
        match c
            .recv_until(WAIT, |f| {
                matches!(f, Frame::Control(ControlMsg::SessionRemoved { .. } | ControlMsg::Error { .. }))
            })
            .await
            .unwrap()
        {
            Frame::Control(ControlMsg::SessionRemoved { session_id }) => {
                assert_eq!(session_id, session.id);
                removed += 1;
            }
            Frame::Control(ControlMsg::Error { request, session_id, .. }) => {
                assert_eq!(request, "CloseSession");
                assert_eq!(session_id, Some(session.id));
                errors += 1;
            }
            _ => unreachable!(),
        }
    }
    assert_eq!(removed, 1, "expected exactly one SessionRemoved");
    assert_eq!(errors, 1, "expected exactly one Error for the second close");

    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
}

/// Waits for the next SessionUpdated for `id` and returns its session.
async fn next_update(c: &mut Client, id: jojo_proto::SessionId) -> SessionInfo {
    match c
        .recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::SessionUpdated { session }) if session.id == id))
        .await
        .unwrap()
    {
        Frame::Control(ControlMsg::SessionUpdated { session }) => session,
        _ => unreachable!(),
    }
}

/// Sends one hook payload the way `jojod hook` does: a separate connection
/// that says Hello, sends HookEvent and disconnects.
async fn relay(endpoint: &Endpoint, id: jojo_proto::SessionId, payload: &str) {
    let mut relay = Client::connect(endpoint).await.unwrap();
    relay.send(Frame::Control(ControlMsg::HookEvent { session_id: id, payload: payload.into() })).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn hook_events_drive_state_label_and_claude_session_id() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, _server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    let project = add_project(&mut c, dir.path()).await;
    let session = create_shell(&mut c, &project).await;

    relay(&endpoint, session.id, r#"{"hook_event_name":"SessionStart","session_id":"claude-abc"}"#).await;
    let started = next_update(&mut c, session.id).await;
    assert_eq!(started.state, jojo_proto::AgentState::Idle);
    assert_eq!(started.claude_session_id.as_deref(), Some("claude-abc"));

    relay(&endpoint, session.id, r#"{"hook_event_name":"UserPromptSubmit","prompt":"Fix the login bug"}"#).await;
    let working = next_update(&mut c, session.id).await;
    assert_eq!(working.state, jojo_proto::AgentState::Working);
    // The prompt is recorded beside the label; the label itself is unchanged.
    assert_eq!(working.prompt_label.as_deref(), Some("Fix the login bug"));
    assert_eq!(working.label, session.label);
    assert!(!working.renamed);

    relay(&endpoint, session.id, r#"{"hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"git push"}}"#).await;
    let asking = next_update(&mut c, session.id).await;
    assert_eq!(asking.state, jojo_proto::AgentState::NeedsYou);
    assert_eq!(asking.state_detail.as_deref(), Some("Needs permission: Bash(git push)"));

    relay(&endpoint, session.id, r#"{"hook_event_name":"UserPromptSubmit","prompt":"A second prompt"}"#).await;
    let second = next_update(&mut c, session.id).await;
    assert_eq!(second.prompt_label.as_deref(), Some("Fix the login bug"), "only the first prompt is recorded");
    assert_eq!(second.state_detail, None);

    relay(&endpoint, session.id, r#"{"hook_event_name":"Stop"}"#).await;
    assert_eq!(next_update(&mut c, session.id).await.state, jojo_proto::AgentState::Done);

    c.send(Frame::Control(ControlMsg::MarkSeen { session_id: session.id })).await.unwrap();
    assert_eq!(next_update(&mut c, session.id).await.state, jojo_proto::AgentState::Idle);

    // A payload that is not hook JSON, and a hook for an unknown session,
    // change nothing and produce no reply; the next real event still applies.
    relay(&endpoint, session.id, "not json").await;
    relay(&endpoint, jojo_proto::SessionId::new(), r#"{"hook_event_name":"Stop"}"#).await;
    relay(&endpoint, session.id, r#"{"hook_event_name":"PreToolUse"}"#).await;
    assert_eq!(next_update(&mut c, session.id).await.state, jojo_proto::AgentState::Working);

    // Claude Code exits inside this shell session: it shows running again.
    relay(&endpoint, session.id, r#"{"hook_event_name":"SessionEnd"}"#).await;
    let ended = next_update(&mut c, session.id).await;
    assert_eq!(ended.state, jojo_proto::AgentState::Running);
    assert_eq!(ended.claude_session_id, None, "a shell Claude Code has left has no conversation to resume");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_renamed_session_keeps_its_label_and_still_records_the_first_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, _server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    let project = add_project(&mut c, dir.path()).await;
    let session = create_shell(&mut c, &project).await;

    c.send(Frame::Control(ControlMsg::RenameSession { session_id: session.id, label: "mine".into() })).await.unwrap();
    let renamed = next_update(&mut c, session.id).await;
    assert!(renamed.renamed);

    relay(&endpoint, session.id, r#"{"hook_event_name":"UserPromptSubmit","prompt":"Fix the login bug"}"#).await;
    let working = next_update(&mut c, session.id).await;
    assert_eq!(working.label, "mine");
    assert_eq!(working.prompt_label.as_deref(), Some("Fix the login bug"));
    assert_eq!(working.state, jojo_proto::AgentState::Working);
}

#[tokio::test(flavor = "multi_thread")]
async fn resume_keeps_the_label_of_the_session_it_continues() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, _server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    let project = add_project(&mut c, dir.path()).await;
    let original = create_shell(&mut c, &project).await;
    relay(&endpoint, original.id, r#"{"hook_event_name":"SessionStart","session_id":"0f3c2a9e-1b2c-4d5e-8f90-123456789abc"}"#).await;
    next_update(&mut c, original.id).await;
    c.send(Frame::Control(ControlMsg::RenameSession { session_id: original.id, label: "mine".into() })).await.unwrap();
    next_update(&mut c, original.id).await;

    // The resumed session runs claude, which may not be installed here; the
    // SessionCreated that answers the request carries the row either way.
    let resumed_id = jojo_proto::SessionId::new();
    c.send(Frame::Control(ControlMsg::CreateSession {
        session_id: resumed_id,
        project_id: project.id,
        kind: SessionKind::Claude,
        cols: 80,
        rows: 24,
        shell: None,
        resume: Some("0f3c2a9e-1b2c-4d5e-8f90-123456789abc".into()),
        replaces: None,
    }))
    .await
    .unwrap();
    let resumed = match c
        .recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::SessionCreated { session }) if session.id == resumed_id))
        .await
        .unwrap()
    {
        Frame::Control(ControlMsg::SessionCreated { session }) => session,
        _ => unreachable!(),
    };
    assert_eq!(resumed.label, "mine");
    assert!(resumed.renamed);
    assert_eq!(resumed.claude_session_id.as_deref(), Some("0f3c2a9e-1b2c-4d5e-8f90-123456789abc"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_project_path_starting_with_a_tilde_is_the_home_directory() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, _server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    c.send(Frame::Control(ControlMsg::AddProject { path: "~".into(), name: "home".into() })).await.unwrap();
    let added = match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::ProjectAdded { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::ProjectAdded { project }) => project,
        _ => unreachable!(),
    };
    let home = std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap();
    assert_eq!(added.path, home);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_renamed_project_is_broadcast_and_kept_across_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    let project = add_project(&mut c, dir.path()).await;

    c.send(Frame::Control(ControlMsg::RenameProject { id: project.id, name: "  Website  ".into() })).await.unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::ProjectUpdated { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::ProjectUpdated { project: updated }) => {
            assert_eq!(updated.id, project.id);
            assert_eq!(updated.name, "Website", "the name is trimmed");
            assert_eq!(updated.path, project.path, "renaming does not move the folder");
        }
        _ => unreachable!(),
    }

    c.send(Frame::Control(ControlMsg::RenameProject { id: project.id, name: "   ".into() })).await.unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Error { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::Error { request, .. }) => assert_eq!(request, "RenameProject"),
        _ => unreachable!(),
    }
    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
    drop(c);

    let (endpoint, server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    c.send(Frame::Control(ControlMsg::ListProjects)).await.unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Projects { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::Projects { projects }) => assert_eq!(projects[0].name, "Website"),
        _ => unreachable!(),
    }
    c.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn hook_events_report_the_context_usage_from_the_transcript() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, _server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    let project = add_project(&mut c, dir.path()).await;
    let session = create_shell(&mut c, &project).await;

    let transcript = dir.path().join("transcript.jsonl");
    std::fs::write(
        &transcript,
        concat!(
            r#"{"type":"user","message":{"content":"Fix the login bug"}}"#,
            "\n",
            r#"{"type":"assistant","isSidechain":false,"message":{"model":"claude-sonnet-5","usage":{"input_tokens":4,"cache_creation_input_tokens":2000,"cache_read_input_tokens":148000,"output_tokens":300}}}"#,
            "\n",
        ),
    )
    .unwrap();
    let payload = serde_json::json!({ "hook_event_name": "Stop", "transcript_path": transcript }).to_string();
    relay(&endpoint, session.id, &payload).await;
    let updated = next_update(&mut c, session.id).await;
    assert_eq!(updated.context_tokens, Some(150_004));
    assert_eq!(updated.context_window, Some(200_000));

    // Claude Code exits inside the shell: the meter goes away with it.
    relay(&endpoint, session.id, r#"{"hook_event_name":"SessionEnd"}"#).await;
    let ended = next_update(&mut c, session.id).await;
    assert_eq!(ended.context_tokens, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn search_finds_text_in_a_sessions_output() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, _server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    let project = add_project(&mut c, dir.path()).await;
    let session = create_shell(&mut c, &project).await;

    c.send(Frame::Input { session_id: session.id, bytes: b"echo JOJO-SEARCH-MARKER\r".to_vec() }).await.unwrap();
    // The shell echoes the command, then prints its output; ask until the
    // output has reached the session's grid.
    let deadline = std::time::Instant::now() + WAIT;
    let hits = loop {
        c.send(Frame::Control(ControlMsg::Search { request_id: "r1".into(), query: "jojo-search-marker".into() })).await.unwrap();
        let hits = match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::SearchResults { .. }))).await.unwrap() {
            Frame::Control(ControlMsg::SearchResults { request_id, hits }) => {
                assert_eq!(request_id, "r1");
                hits
            }
            _ => unreachable!(),
        };
        if !hits.is_empty() || std::time::Instant::now() > deadline {
            break hits;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    };
    assert!(!hits.is_empty(), "the marker was not found");
    assert!(hits.iter().all(|h| h.session_id == session.id));
    assert!(hits.iter().any(|h| h.text.contains("JOJO-SEARCH-MARKER")));

    // A one-character query finds nothing.
    c.send(Frame::Control(ControlMsg::Search { request_id: "r2".into(), query: "J".into() })).await.unwrap();
    match c.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::SearchResults { .. }))).await.unwrap() {
        Frame::Control(ControlMsg::SearchResults { hits, .. }) => assert!(hits.is_empty()),
        _ => unreachable!(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_session_that_replaces_another_keeps_its_name() {
    let dir = tempfile::tempdir().unwrap();
    let (endpoint, _server) = start(dir.path()).await;
    let mut c = Client::connect(&endpoint).await.unwrap();
    let project = add_project(&mut c, dir.path()).await;
    let old = create_shell(&mut c, &project).await;
    c.send(Frame::Control(ControlMsg::RenameSession { session_id: old.id, label: "dev server".into() })).await.unwrap();
    assert!(next_update(&mut c, old.id).await.renamed);

    // A restored or restarted shell names the session it takes the place of.
    let session_id = jojo_proto::SessionId::new();
    c.send(Frame::Control(ControlMsg::CreateSession {
        session_id,
        project_id: project.id,
        kind: SessionKind::Shell,
        cols: 80,
        rows: 24,
        shell: Some(test_shell()),
        resume: None,
        replaces: Some(old.id),
    }))
    .await
    .unwrap();
    let created = match c
        .recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::SessionCreated { session }) if session.id == session_id))
        .await
        .unwrap()
    {
        Frame::Control(ControlMsg::SessionCreated { session }) => session,
        _ => unreachable!(),
    };
    assert_eq!(created.label, "dev server");
    assert!(created.renamed, "a name the user chose stays a user name");

    // Without `replaces`, a new shell gets the default name.
    let fresh = create_shell(&mut c, &project).await;
    assert_ne!(fresh.label, "dev server");
}
