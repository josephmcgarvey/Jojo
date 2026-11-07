//! Keeps the app connected to the local jojod and relays frames
//! (spec sections 8.1 and 10).

use crate::bridge::{Bridge, ConnectionStatus};
use crate::daemon_manager::{connect_or_start, kill_daemons, ConnectError, DaemonPaths};
use jojo_proto::read_frame;
use jojod::client::Client;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

/// Reconnect attempts before the status changes from reconnecting to failed
/// (retries continue either way).
const FAILED_AFTER: u32 = 10;

pub fn backoff(attempt: u32) -> Duration {
    let secs = 1u64 << attempt.saturating_sub(1).min(5);
    Duration::from_secs(secs.min(30))
}

/// Runs until the task is aborted.
pub async fn run(paths: DaemonPaths, bridge: Arc<Bridge>) {
    let mut attempt: u32 = 0;
    loop {
        match connect_or_start(&paths).await {
            Ok(client) => {
                attempt = 0;
                let reason = serve(client, &bridge).await;
                bridge.set_outbound(None);
                attempt += 1;
                bridge.emit_status(ConnectionStatus::Reconnecting { attempt, message: reason });
            }
            Err(ConnectError::Elevated) => {
                bridge.emit_status(ConnectionStatus::Elevated);
                let _ = tokio::time::timeout(Duration::from_secs(10), bridge.restart.notified()).await;
            }
            Err(e @ ConnectError::Mismatch { .. }) => {
                bridge.emit_status(ConnectionStatus::VersionMismatch { message: e.to_string() });
                bridge.restart.notified().await;
                let _ = kill_daemons();
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Err(ConnectError::Io(e)) => {
                attempt += 1;
                let message = e.to_string();
                bridge.emit_status(if attempt >= FAILED_AFTER {
                    ConnectionStatus::Failed { message }
                } else {
                    ConnectionStatus::Reconnecting { attempt, message }
                });
                tokio::select! {
                    _ = tokio::time::sleep(backoff(attempt)) => {}
                    _ = bridge.restart.notified() => { attempt = 0; }
                }
            }
        }
    }
}

/// Relays frames until the connection ends; returns why it ended.
async fn serve(client: Client, bridge: &Bridge) -> String {
    bridge.emit_status(ConnectionStatus::Connected { jojod_version: client.daemon_version().to_string() });
    let (mut reader, mut writer) = client.into_parts();
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    bridge.set_outbound(Some(tx));
    loop {
        tokio::select! {
            frame = read_frame(&mut reader) => match frame {
                Ok(Some(frame)) => {
                    let encoded = frame.encode();
                    bridge.emit_frame(&encoded[4..]);
                }
                Ok(None) => return "jojod closed the connection".into(),
                Err(e) => return format!("lost the connection to jojod: {e}"),
            },
            body = rx.recv() => {
                let Some(body) = body else { return "outbound queue closed".into() };
                let len = (body.len() as u32).to_le_bytes();
                let write = async {
                    writer.write_all(&len).await?;
                    writer.write_all(&body).await?;
                    writer.flush().await
                };
                if let Err(e) = write.await {
                    return format!("could not write to jojod: {e}");
                }
            }
            _ = bridge.restart.notified() => {
                let _ = kill_daemons();
                return "restarting jojod".into();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_to_thirty_seconds() {
        let secs: Vec<u64> = (1..=7).map(|a| backoff(a).as_secs()).collect();
        assert_eq!(secs, vec![1, 2, 4, 8, 16, 30, 30]);
    }
}
