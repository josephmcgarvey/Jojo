//! Keeps the app connected to the local jojod and relays frames
//! (spec sections 8.1 and 10).

use crate::bridge::{Bridge, ConnectionStatus};
use crate::daemon_manager::{connect_or_start, install_hooks, kill_daemons, ConnectError, DaemonPaths};
use jojo_proto::read_frame;
use jojod::client::Client;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

/// Reconnect attempts before the status changes from reconnecting to failed
/// (retries continue either way).
const FAILED_AFTER: u32 = 10;

/// A connection that did not last at least this long is treated as a failed
/// attempt for backoff purposes, even though it briefly succeeded: a daemon
/// that accepts a connection and then immediately drops it (crash loop,
/// handshake race, being killed out from under us) must still back off and
/// eventually reach `Failed`, rather than spinning at full speed forever
/// because every `connect_or_start` call nominally "succeeds" (Important #5).
const SHORT_CONNECTION: Duration = Duration::from_secs(2);

pub fn backoff(attempt: u32) -> Duration {
    let secs = 1u64 << attempt.saturating_sub(1).min(5);
    Duration::from_secs(secs.min(30))
}

/// Runs `kill_daemons` (which shells out to `taskkill`) on a blocking thread
/// so it never stalls an async runtime worker (Important #6).
async fn kill_daemons_async() {
    let _ = tokio::task::spawn_blocking(kill_daemons).await;
}

/// Runs until the task is aborted.
pub async fn run(paths: DaemonPaths, bridge: Arc<Bridge>) {
    let paths = &paths;
    // After the first successful connection, the staged jojod.exe matches
    // the daemon's protocol (a mismatched daemon is replaced before this
    // succeeds), so the hooks are registered to run that file.
    let hooks_installed = std::sync::atomic::AtomicBool::new(false);
    let hooks_installed = &hooks_installed;
    run_with(
        move || async move {
            let result = connect_or_start(paths).await;
            if result.is_ok() && paths.register_hooks && !hooks_installed.swap(true, std::sync::atomic::Ordering::Relaxed) {
                let jojod = paths.bin_dir.join("jojod.exe");
                tokio::task::spawn_blocking(move || {
                    if let Err(e) = install_hooks(&jojod) {
                        eprintln!("jojo: could not register Claude Code hooks with {}: {e}", jojod.display());
                    }
                });
            }
            result
        },
        bridge,
    )
    .await
}

/// The connection loop, with the connect step passed in so tests can drive
/// the loop without starting a daemon.
pub async fn run_with<F, Fut>(mut connect: F, bridge: Arc<Bridge>)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<Client, ConnectError>>,
{
    let mut attempt: u32 = 0;
    loop {
        // Read before connecting: connect_or_start can take up to its start
        // deadline (about 10 s) while it starts a daemon, and a Retry or
        // Restart clicked during that time must cut short the wait that
        // follows a failure instead of being lost (Plan 2b carry-over 3.3
        // item 1).
        let since = bridge.restart_generation();
        match connect().await {
            Ok(client) => {
                let started = Instant::now();
                let reason = serve(client, &bridge).await;
                bridge.set_outbound(None);
                if started.elapsed() < SHORT_CONNECTION {
                    attempt += 1;
                    let message = reason;
                    bridge.emit_status(if attempt >= FAILED_AFTER {
                        ConnectionStatus::Failed { message }
                    } else {
                        ConnectionStatus::Reconnecting { attempt, message }
                    });
                    // serve() itself reacts to restarts requested while it
                    // ran, so this wait counts only restarts requested from
                    // here on.
                    let since = bridge.restart_generation();
                    tokio::select! {
                        _ = tokio::time::sleep(backoff(attempt)) => {}
                        _ = bridge.wait_for_restart_after(since) => { attempt = 0; }
                    }
                } else {
                    attempt = 1;
                    bridge.emit_status(ConnectionStatus::Reconnecting { attempt, message: reason });
                }
            }
            Err(ConnectError::Elevated) => {
                bridge.emit_status(ConnectionStatus::Elevated);
                let _ = tokio::time::timeout(Duration::from_secs(10), bridge.wait_for_restart_after(since)).await;
            }
            Err(e @ ConnectError::Mismatch { .. }) => {
                bridge.emit_status(ConnectionStatus::VersionMismatch { message: e.to_string() });
                bridge.wait_for_restart_after(since).await;
                kill_daemons_async().await;
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
                    _ = bridge.wait_for_restart_after(since) => { attempt = 0; }
                }
            }
        }
    }
}

/// Relays frames until the connection ends; returns why it ended.
///
/// Reading runs on its own task that owns the reader exclusively, rather
/// than inside this function's `tokio::select!`. `jojo_proto::read_frame`
/// is built on `AsyncReadExt::read_exact`, which is *not* cancellation-safe:
/// if a `tokio::select!` branch reading a frame loses the race to another
/// branch (for example a keystroke arriving via `rx.recv()`), the `select!`
/// drops that branch's in-progress future, discarding whatever partial frame
/// bytes it had already read from the kernel buffer. The next call to
/// `read_frame` then reads a fresh 4-byte length prefix starting in the
/// middle of the daemon's byte stream, which almost always decodes to a
/// nonsense length and is rejected as "frame length out of range", dropping
/// the connection (Critical #1). Giving the reader its own task means it is
/// always polled by `.await`ing the *same* future (the `JoinHandle`) across
/// loop iterations, so a `read_exact` in progress is never abandoned
/// mid-frame by this loop selecting another branch.
async fn serve(client: Client, bridge: &Arc<Bridge>) -> String {
    let jojod_version = client.daemon_version().to_string();
    let (reader, mut writer) = client.into_parts();
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    // Installed before the Connected status is emitted, so a frontend that
    // reacts to "connected" by sending a frame can never race `send_frame`
    // ahead of the outbound channel existing (Important #3).
    bridge.set_outbound(Some(tx));
    bridge.emit_status(ConnectionStatus::Connected { jojod_version });

    let reader_bridge = bridge.clone();
    let mut reader_task = tokio::spawn(async move {
        let mut reader = reader;
        loop {
            match read_frame(&mut reader).await {
                Ok(Some(frame)) => {
                    let encoded = frame.encode();
                    reader_bridge.emit_frame(&encoded[4..]);
                }
                Ok(None) => return "jojod closed the connection".to_string(),
                Err(e) => return format!("lost the connection to jojod: {e}"),
            }
        }
    });

    // Captured once, right before the loop starts waiting: a restart
    // requested earlier (for example while the previous attempt was still
    // reconnecting) is already reflected here and must not immediately tear
    // down this brand-new connection (Important #4).
    let since = bridge.restart_generation();

    let reason = loop {
        tokio::select! {
            result = &mut reader_task => {
                break match result {
                    Ok(reason) => reason,
                    Err(e) => format!("the reader task failed: {e}"),
                };
            }
            body = rx.recv() => {
                let Some(body) = body else { break "outbound queue closed".into() };
                let len = (body.len() as u32).to_le_bytes();
                let write = async {
                    writer.write_all(&len).await?;
                    writer.write_all(&body).await?;
                    writer.flush().await
                };
                if let Err(e) = write.await {
                    break format!("could not write to jojod: {e}");
                }
            }
            _ = bridge.wait_for_restart_after(since) => {
                kill_daemons_async().await;
                break "restarting jojod".into();
            }
        }
    };
    // Every exit path lands here: the reader task is aborted whether it was
    // the one that ended the connection (a harmless no-op abort on an
    // already-finished task) or whether the write or restart arm ended it
    // instead (where the reader task is otherwise still running).
    reader_task.abort();
    reason
}

#[cfg(test)]
mod tests {
    use super::*;
    use jojo_proto::{write_frame, ControlMsg, Frame};
    use tokio::io::AsyncWriteExt;

    #[test]
    fn backoff_doubles_to_thirty_seconds() {
        let secs: Vec<u64> = (1..=7).map(|a| backoff(a).as_secs()).collect();
        assert_eq!(secs, vec![1, 2, 4, 8, 16, 30, 30]);
    }

    /// Deterministic regression test for Critical #1, at the level `serve`
    /// actually has the bug: a length prefix split across two separate
    /// reads must not be lost to a `send` that becomes ready while the
    /// first two bytes are still sitting in an in-progress read.
    ///
    /// `tests/connection.rs::concurrent_input_and_output_keep_the_connection_alive`
    /// exercises the same bug against a real jojod over a real named pipe,
    /// but that is a timing-dependent reproduction: whether a frame's bytes
    /// actually arrive at the reader in more than one chunk depends on OS
    /// pipe buffering and process scheduling that a test cannot control,
    /// so it is not guaranteed to fail on the old code on every run (it was
    /// confirmed to still pass against the pre-fix `serve` on this machine
    /// during fix-round-1 development). This test instead uses an in-memory
    /// `tokio::io::duplex` and writes the frame's bytes in two explicit
    /// pieces with a delay between them, so the partial read is forced to
    /// happen and to still be in progress at the moment the concurrent
    /// `send` is issued, regardless of machine or OS.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_read_split_across_two_chunks_is_not_lost_to_a_concurrent_send() {
        let (client_io, daemon_io) = tokio::io::duplex(64 * 1024);
        let (cr, cw) = tokio::io::split(client_io);
        let (_dr, mut dw) = tokio::io::split(daemon_io);

        // Hello handshake, mirroring jojod::client::Client's own tests.
        write_frame(
            &mut dw,
            &Frame::Control(ControlMsg::Hello { protocol_version: jojo_proto::PROTOCOL_VERSION, jojod_version: "test".into() }),
        )
        .await
        .unwrap();
        let client = Client::handshake(Box::new(cr), Box::new(cw), Duration::from_secs(5)).await.unwrap();

        let bridge = Arc::new(Bridge::default());
        let seen = Arc::new(std::sync::Mutex::new(Vec::<Vec<u8>>::new()));
        let sink = seen.clone();
        bridge.set_sink(Box::new(move |bytes| sink.lock().unwrap().push(bytes)));

        let serve_bridge = bridge.clone();
        let serve_task = tokio::spawn(async move { serve(client, &serve_bridge).await });

        // `serve` installs the outbound channel and emits Connected
        // synchronously before its first read; wait for that status so the
        // bridge is guaranteed ready to accept `send`.
        let connected = {
            let mut ok = false;
            for _ in 0..100 {
                if seen.lock().unwrap().iter().any(|e| {
                    e[0] == crate::bridge::ENVELOPE_STATUS && e[1..].starts_with(br#"{"state":"connected""#)
                }) {
                    ok = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            ok
        };
        assert!(connected, "serve never reported Connected");

        // Write only the first 2 of the 4 length-prefix bytes of an Output
        // frame carrying a recognizable marker, then give the reader task
        // time to poll, consume those 2 bytes, and go Pending (2 of 4
        // filled) because no more bytes are available yet.
        let frame = Frame::Output { session_id: jojo_proto::SessionId::new(), bytes: b"marker-bytes".to_vec() };
        let encoded = frame.encode();
        dw.write_all(&encoded[..2]).await.unwrap();
        dw.flush().await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // While those 2 bytes are still in flight, queue an outbound send:
        // under the old code (read_frame as a direct `select!` branch) this
        // wins the race and drops the in-progress read_frame future,
        // discarding the 2 bytes it already consumed from the stream.
        bridge.send(vec![9, 9]).expect("connected bridge accepts frames");
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Now deliver the rest of the frame.
        dw.write_all(&encoded[2..]).await.unwrap();
        dw.flush().await.unwrap();

        let arrived = {
            let mut ok = false;
            for _ in 0..100 {
                if seen.lock().unwrap().iter().any(|e| {
                    e[0] == crate::bridge::ENVELOPE_FRAME && String::from_utf8_lossy(&e[2..]).contains("marker-bytes")
                }) {
                    ok = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            ok
        };
        assert!(arrived, "a frame whose length prefix arrived in two pieces must still be relayed intact, not lost to the concurrent send");

        serve_task.abort();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_retry_clicked_while_connecting_skips_the_backoff() {
        let bridge = Arc::new(Bridge::default());
        let starts = Arc::new(std::sync::Mutex::new(Vec::<Instant>::new()));
        let connect = {
            let starts = starts.clone();
            let bridge = bridge.clone();
            move || {
                let starts = starts.clone();
                let bridge = bridge.clone();
                async move {
                    let first = {
                        let mut starts = starts.lock().unwrap();
                        starts.push(Instant::now());
                        starts.len() == 1
                    };
                    if first {
                        // The user clicks Retry while this attempt is still running.
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        bridge.request_restart();
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    Err::<Client, _>(ConnectError::Io(std::io::Error::other("no daemon")))
                }
            }
        };
        let task = tokio::spawn(run_with(connect, bridge.clone()));
        let deadline = Instant::now() + Duration::from_secs(5);
        while starts.lock().unwrap().len() < 2 && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        task.abort();
        let starts = starts.lock().unwrap();
        assert!(starts.len() >= 2, "the loop never retried");
        // With the click lost, the loop first sleeps backoff(1) = 1 s.
        let gap = starts[1] - starts[0];
        assert!(gap < Duration::from_millis(700), "the second attempt started {gap:?} after the first");
    }
}
