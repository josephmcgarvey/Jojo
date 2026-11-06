//! Accept loop and per-connection loop.

use crate::daemon::{Daemon, DaemonConfig};
use crate::transport::{Accept, BoxRead, BoxWrite, Listener};
use jojo_proto::{read_frame, write_frame, ControlMsg, Frame, PROTOCOL_VERSION};
use std::sync::Arc;
use tokio::sync::{broadcast::error::RecvError, mpsc};

/// Runs the daemon until a `Shutdown` request, then kills every session.
pub async fn run(cfg: DaemonConfig) -> anyhow::Result<()> {
    let listener = Listener::bind(&cfg.endpoint)?;
    run_with_listener(cfg, listener).await
}

/// Separated from `run` so the accept loop's error handling can be driven by
/// a fake `Accept` in tests instead of a real OS transport (Finding F1).
async fn run_with_listener(cfg: DaemonConfig, mut listener: impl Accept + 'static) -> anyhow::Result<()> {
    let daemon = Daemon::new(cfg)?;
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                match accepted {
                    Ok((reader, writer)) => {
                        tokio::spawn(serve_connection(daemon.clone(), reader, writer));
                    }
                    Err(e) => {
                        // A single bad connection (resource exhaustion,
                        // a transient OS error) must not end a daemon that
                        // owns every running session (Finding F1). Logged
                        // metadata only: the error text, never terminal
                        // bytes.
                        eprintln!("jojod: accept error: {e}");
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            }
            _ = daemon.shutdown.notified() => break,
        }
    }
    daemon.kill_all();
    Ok(())
}

async fn serve_connection(daemon: Arc<Daemon>, mut reader: BoxRead, writer: BoxWrite) {
    let client = daemon.next_client_id();
    let (tx, mut rx) = mpsc::unbounded_channel::<Frame>();

    let writer_task = tokio::spawn(async move {
        let mut writer = writer;
        while let Some(frame) = rx.recv().await {
            if write_frame(&mut writer, &frame).await.is_err() {
                break;
            }
        }
    });

    let _ = tx.send(Frame::Control(ControlMsg::Hello {
        protocol_version: PROTOCOL_VERSION,
        jojod_version: crate::VERSION.to_string(),
    }));

    match read_frame(&mut reader).await {
        Ok(Some(Frame::Control(ControlMsg::Hello { protocol_version, .. })))
            if protocol_version == PROTOCOL_VERSION => {}
        Ok(Some(Frame::Control(ControlMsg::Hello { protocol_version, .. }))) => {
            let _ = tx.send(Frame::Control(ControlMsg::Error {
                request: "Hello".into(),
                message: format!("protocol mismatch: daemon {PROTOCOL_VERSION}, client {protocol_version}"),
                session_id: None,
            }));
            drop(tx);
            let _ = writer_task.await;
            return;
        }
        _ => {
            drop(tx);
            let _ = writer_task.await;
            return;
        }
    }

    let mut events = daemon.subscribe();
    let event_tx = tx.clone();
    let event_task = tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(msg) => {
                    if event_tx.send(Frame::Control(msg)).is_err() {
                        break;
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    });

    loop {
        match read_frame(&mut reader).await {
            Ok(Some(frame)) => daemon.handle(client, &tx, frame),
            Ok(None) => break,
            Err(e) if jojo_proto::is_bad_payload(&e) => {
                let _ = tx.send(Frame::Control(ControlMsg::Error {
                    request: "Frame".into(),
                    message: e.to_string(),
                    session_id: None,
                }));
            }
            Err(_) => break,
        }
    }

    daemon.client_gone(client);
    event_task.abort();
    drop(tx);
    let _ = writer_task.await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Endpoint;
    use std::io;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::io::DuplexStream;
    use tokio::sync::mpsc::UnboundedSender;

    /// An `Accept` whose first call fails (simulating Finding F1's accept
    /// error), whose second call hands out one in-memory duplex connection,
    /// and whose further calls never resolve (the test shuts the daemon
    /// down before a third `accept()` matters).
    struct FlakyThenDuplex {
        calls: AtomicUsize,
        client_tx: UnboundedSender<DuplexStream>,
    }

    impl Accept for FlakyThenDuplex {
        async fn accept(&mut self) -> io::Result<(BoxRead, BoxWrite)> {
            match self.calls.fetch_add(1, Ordering::SeqCst) {
                0 => Err(io::Error::other("simulated accept failure")),
                1 => {
                    let (server, client) = tokio::io::duplex(64 * 1024);
                    let _ = self.client_tx.send(client);
                    let (r, w) = tokio::io::split(server);
                    Ok((Box::new(r), Box::new(w)))
                }
                _ => std::future::pending().await,
            }
        }
    }

    /// Finding F1: a single accept error must not terminate the daemon or
    /// skip the connections that follow it.
    #[tokio::test(flavor = "multi_thread")]
    async fn accept_error_does_not_stop_the_daemon() {
        let dir = tempfile::tempdir().unwrap();
        let (client_tx, mut client_rx) = mpsc::unbounded_channel();
        let listener = FlakyThenDuplex { calls: AtomicUsize::new(0), client_tx };
        let cfg = DaemonConfig {
            endpoint: Endpoint("unused".into()),
            state_dir: dir.path().to_path_buf(),
            scrollback: 1000,
        };
        let handle = tokio::spawn(run_with_listener(cfg, listener));

        // The first accept() call errors; the daemon must keep running and
        // serve the connection handed out by the second call rather than
        // returning from `run_with_listener`.
        let mut client = tokio::time::timeout(Duration::from_secs(5), client_rx.recv())
            .await
            .expect("daemon did not keep accepting after an accept error")
            .expect("accept channel closed without a connection");

        let hello = read_frame(&mut client).await.unwrap().unwrap();
        assert!(
            matches!(hello, Frame::Control(ControlMsg::Hello { protocol_version, .. })
                if protocol_version == PROTOCOL_VERSION),
            "expected a Hello frame, got {hello:?}"
        );
        write_frame(
            &mut client,
            &Frame::Control(ControlMsg::Hello { protocol_version: PROTOCOL_VERSION, jojod_version: "test".into() }),
        )
        .await
        .unwrap();

        // Shutdown must still break the loop and run kill_all (no sessions
        // exist here, so this only exercises the control flow).
        write_frame(&mut client, &Frame::Control(ControlMsg::Shutdown)).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("server task timed out")
            .expect("server task panicked")
            .expect("run_with_listener returned an error");
    }
}
