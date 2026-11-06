//! Accept loop and per-connection loop.

use crate::daemon::{Daemon, DaemonConfig};
use crate::transport::{BoxRead, BoxWrite, Listener};
use jojo_proto::{read_frame, write_frame, ControlMsg, Frame, PROTOCOL_VERSION};
use std::sync::Arc;
use tokio::sync::{broadcast::error::RecvError, mpsc};

/// Runs the daemon until a `Shutdown` request, then kills every session.
pub async fn run(cfg: DaemonConfig) -> anyhow::Result<()> {
    let mut listener = Listener::bind(&cfg.endpoint)?;
    let daemon = Daemon::new(cfg)?;
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (reader, writer) = accepted?;
                tokio::spawn(serve_connection(daemon.clone(), reader, writer));
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

    while let Ok(Some(frame)) = read_frame(&mut reader).await {
        daemon.handle(client, &tx, frame);
    }

    daemon.client_gone(client);
    event_task.abort();
    drop(tx);
    let _ = writer_task.await;
}
