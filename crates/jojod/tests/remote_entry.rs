//! The commands the app runs on a remote machine (spec 8.2 step 5 and 8.3):
//! `attach --stdio` on every platform, and `serve --daemonize` on Linux.

use jojo_proto::{ControlMsg, Frame};
use jojod::client::Client;
use jojod::daemon::DaemonConfig;
use jojod::paths::Endpoint;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(15);

/// The jojod executable built with these tests. `CARGO_BIN_EXE_jojod` is the
/// path at build time, which does not exist when Linux test executables
/// built on Windows run in WSL; the executable is then found next to the
/// test executable's `deps` directory.
fn jojod_exe() -> PathBuf {
    let built = PathBuf::from(env!("CARGO_BIN_EXE_jojod"));
    if built.exists() {
        return built;
    }
    let deps = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
    deps.parent().unwrap().join(if cfg!(windows) { "jojod.exe" } else { "jojod" })
}

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

#[tokio::test(flavor = "multi_thread")]
async fn attach_stdio_relays_the_protocol_between_stdio_and_the_daemon() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = test_endpoint(dir.path());
    let cfg = DaemonConfig { endpoint: endpoint.clone(), state_dir: dir.path().to_path_buf(), scrollback: 1000 };
    let server = tokio::spawn(jojod::server::run(cfg));
    for _ in 0..100 {
        if Client::connect(&endpoint).await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let mut child = tokio::process::Command::new(jojod_exe())
        .args(["attach", "--stdio", "--endpoint", &endpoint.0])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let stdin = child.stdin.take().unwrap();
    // The app does exactly this with ssh.exe's pipes.
    let mut client = Client::handshake(Box::new(stdout), Box::new(stdin), WAIT).await.unwrap();
    client.send(Frame::Control(ControlMsg::ListProjects)).await.unwrap();
    let reply = client.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Projects { .. }))).await.unwrap();
    assert!(matches!(reply, Frame::Control(ControlMsg::Projects { projects }) if projects.is_empty()));

    // Closing the relay's stdin (SSH going away) ends it.
    drop(client);
    let status = tokio::time::timeout(WAIT, child.wait()).await.expect("attach exits when stdin closes").unwrap();
    assert!(status.success(), "{status}");

    let mut admin = Client::connect(&endpoint).await.unwrap();
    admin.send(Frame::Control(ControlMsg::Shutdown)).await.unwrap();
    tokio::time::timeout(WAIT, server).await.unwrap().unwrap().unwrap();
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn serve_daemonize_returns_once_listening_and_a_second_serve_exits_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("state");
    let socket = state.join("jojod.sock");
    let serve = |daemonize: bool| {
        let mut cmd = std::process::Command::new(jojod_exe());
        cmd.args(["serve", "--endpoint"]).arg(&socket).arg("--state-dir").arg(&state);
        if daemonize {
            cmd.arg("--daemonize");
        }
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        cmd
    };

    let first = serve(true).status().unwrap();
    assert!(first.success(), "serve --daemonize: {first}");
    // It returned only after the daemon accepted connections.
    let endpoint = Endpoint(socket.to_string_lossy().into_owned());
    let mut client = Client::connect(&endpoint).await.unwrap();

    // A second serve, daemonized or not, finds the lock held and exits 0.
    assert!(serve(true).status().unwrap().success());
    assert!(serve(false).status().unwrap().success());
    client.send(Frame::Control(ControlMsg::ListProjects)).await.unwrap();
    client.recv_until(WAIT, |f| matches!(f, Frame::Control(ControlMsg::Projects { .. }))).await.unwrap();

    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(&state).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(std::fs::metadata(&socket).unwrap().permissions().mode() & 0o777, 0o600);

    let status = std::process::Command::new(jojod_exe()).args(["shutdown", "--endpoint"]).arg(&socket).status().unwrap();
    assert!(status.success());
}
