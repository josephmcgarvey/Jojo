//! Installs jojod on a real server and connects to it over SSH, exactly as
//! Add machine does. It runs only when `JOJO_SSH_TEST_HOST` names a host
//! (an alias from ~/.ssh/config); every other test run skips it, so the
//! suite never touches a server on its own.
//!
//! Run: JOJO_SSH_TEST_HOST=onprem-server cargo test -p jojo-app --test ssh_live -- --nocapture
//! (after scripts/stage-daemon.ps1 has staged the Linux binaries).

use jojo_proto::{ControlMsg, Frame};
use std::path::PathBuf;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
async fn installs_and_connects_to_a_real_server() {
    let Ok(host) = std::env::var("JOJO_SSH_TEST_HOST") else {
        eprintln!("JOJO_SSH_TEST_HOST is not set; skipping");
        return;
    };
    let daemon_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join("daemon");
    jojo_app::install::install(&host, &daemon_dir, false, |step| eprintln!("[{}] {}: {}", step.status, step.step, step.message))
        .await
        .expect("install");

    let mut client = jojo_app::ssh::connect(&host).await.expect("connect over ssh");
    eprintln!("connected to jojod {}", client.daemon_version());
    client.send(Frame::Control(ControlMsg::ListProjects)).await.unwrap();
    let reply = client
        .recv_until(Duration::from_secs(15), |f| matches!(f, Frame::Control(ControlMsg::Projects { .. })))
        .await
        .expect("Projects reply");
    if let Frame::Control(ControlMsg::Projects { projects }) = reply {
        eprintln!("{} project(s) on {host}", projects.len());
    }
}
