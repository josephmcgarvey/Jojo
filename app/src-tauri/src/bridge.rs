//! Carries daemon frames and connection status to the frontend, and frame
//! bodies from the frontend to the daemon.
//!
//! Envelope sent to the frontend (one Tauri Channel message each): byte 0 is
//! the kind, the rest is the body. `ENVELOPE_FRAME`: a daemon frame without
//! its length prefix (`[u8 type][payload]`). `ENVELOPE_STATUS`: UTF-8 JSON
//! `ConnectionStatus`.

use serde::Serialize;
use std::sync::Mutex;
use tokio::sync::{mpsc, watch};
use ts_rs::TS;

pub const ENVELOPE_FRAME: u8 = 0;
pub const ENVELOPE_STATUS: u8 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ConnectionStatus {
    Starting,
    Connected { jojod_version: String },
    Reconnecting { attempt: u32, message: String },
    Elevated,
    VersionMismatch { message: String },
    Failed { message: String },
}

pub fn envelope(kind: u8, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + body.len());
    out.push(kind);
    out.extend_from_slice(body);
    out
}

type Sink = Box<dyn Fn(Vec<u8>) + Send + Sync>;

/// `sink` and `last_status` share one lock so a status change can never be
/// lost to a `set_sink`/`emit_status` race: whichever call wins the lock
/// either fully replays-then-installs or fully updates-then-sends before the
/// other can observe the state (Task 4 fix round 1, Important #2).
struct Inner {
    sink: Option<Sink>,
    last_status: ConnectionStatus,
}

pub struct Bridge {
    inner: Mutex<Inner>,
    outbound: Mutex<Option<mpsc::UnboundedSender<Vec<u8>>>>,
    /// A generation counter bumped by `request_restart`. Waiters capture the
    /// generation in effect when they start waiting and only react to a
    /// later one, so a restart requested before a wait began (for example,
    /// before `connection::serve` started relaying a freshly connected
    /// daemon) cannot be mistaken for a restart requested during that wait
    /// and tear down a daemon the click was never meant for (Task 4 fix
    /// round 1, Important #4).
    restart: watch::Sender<u64>,
}

impl Default for Bridge {
    fn default() -> Self {
        let (restart, _rx) = watch::channel(0u64);
        Bridge {
            inner: Mutex::new(Inner { sink: None, last_status: ConnectionStatus::Starting }),
            outbound: Mutex::new(None),
            restart,
        }
    }
}

impl Bridge {
    /// Installs the frontend's channel and sends it the latest status, so a
    /// frontend that subscribes late still learns the connection state.
    pub fn set_sink(&self, sink: Sink) {
        let mut inner = self.inner.lock().unwrap();
        sink(envelope(ENVELOPE_STATUS, &serde_json::to_vec(&inner.last_status).expect("status serializes")));
        inner.sink = Some(sink);
    }

    pub fn emit_status(&self, status: ConnectionStatus) {
        let bytes = envelope(ENVELOPE_STATUS, &serde_json::to_vec(&status).expect("status serializes"));
        let mut inner = self.inner.lock().unwrap();
        inner.last_status = status;
        if let Some(sink) = inner.sink.as_ref() {
            sink(bytes);
        }
    }

    pub fn emit_frame(&self, body: &[u8]) {
        let inner = self.inner.lock().unwrap();
        if let Some(sink) = inner.sink.as_ref() {
            sink(envelope(ENVELOPE_FRAME, body));
        }
    }

    pub fn set_outbound(&self, tx: Option<mpsc::UnboundedSender<Vec<u8>>>) {
        *self.outbound.lock().unwrap() = tx;
    }

    /// Queues a frame body for the daemon. Refused while disconnected:
    /// keystrokes typed into a terminal the user cannot see must not be
    /// replayed later (spec section 8.3, rule 4). Also refused for a body
    /// that could never be a valid frame: empty (no type byte) or over
    /// `jojo_proto::MAX_FRAME_LEN`, which `jojo_proto::write_frame` would
    /// otherwise reject after it has already been dequeued.
    pub fn send(&self, body: Vec<u8>) -> Result<(), &'static str> {
        if body.is_empty() {
            return Err("cannot send an empty frame");
        }
        if body.len() > jojo_proto::MAX_FRAME_LEN {
            return Err("frame exceeds the maximum frame length");
        }
        match self.outbound.lock().unwrap().as_ref() {
            Some(tx) if tx.send(body).is_ok() => Ok(()),
            _ => Err("not connected to jojod"),
        }
    }

    /// Bumps the restart generation, waking any waiter whose `since` is the
    /// generation now being superseded.
    pub fn request_restart(&self) {
        self.restart.send_modify(|g| *g = g.wrapping_add(1));
    }

    /// The current restart generation, to pass to `wait_for_restart_after`
    /// right before waiting.
    pub fn restart_generation(&self) -> u64 {
        *self.restart.borrow()
    }

    /// Resolves once the restart generation differs from `since`, i.e. once
    /// a restart has been requested that was not already accounted for when
    /// the caller observed `since` via `restart_generation`.
    pub async fn wait_for_restart_after(&self, since: u64) {
        let mut rx = self.restart.subscribe();
        loop {
            if *rx.borrow() != since {
                return;
            }
            if rx.changed().await.is_err() {
                // The sender lives on `self` and is never dropped while this
                // call can still be polled, so this is unreachable; park
                // forever rather than busy-loop if it somehow happens.
                std::future::pending::<()>().await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn collecting_bridge() -> (Bridge, Arc<Mutex<Vec<Vec<u8>>>>) {
        let bridge = Bridge::default();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        bridge.set_sink(Box::new(move |bytes| sink.lock().unwrap().push(bytes)));
        (bridge, seen)
    }

    #[test]
    fn envelope_prefixes_the_kind() {
        assert_eq!(envelope(ENVELOPE_FRAME, &[3, 9]), vec![0, 3, 9]);
    }

    #[test]
    fn subscribing_replays_the_latest_status() {
        let bridge = Bridge::default();
        bridge.emit_status(ConnectionStatus::Connected { jojod_version: "1.2.3".into() });
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        bridge.set_sink(Box::new(move |bytes| sink.lock().unwrap().push(bytes)));
        let first = seen.lock().unwrap()[0].clone();
        assert_eq!(first[0], ENVELOPE_STATUS);
        let json: serde_json::Value = serde_json::from_slice(&first[1..]).unwrap();
        assert_eq!(json["state"], "connected");
        assert_eq!(json["jojod_version"], "1.2.3");
    }

    #[test]
    fn sending_while_disconnected_is_refused() {
        let (bridge, _) = collecting_bridge();
        assert!(bridge.send(vec![1, b'{', b'}']).is_err());
    }

    /// A status emitted just before `set_sink` must still be the one
    /// replayed, and a status emitted just after must still reach the sink:
    /// with `sink` and `last_status` under one lock neither call can see a
    /// half-updated `Inner` (Important #2).
    #[test]
    fn status_changes_are_not_lost_around_subscribe() {
        let bridge = Bridge::default();
        bridge.emit_status(ConnectionStatus::Connected { jojod_version: "1.0.0".into() });

        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        bridge.set_sink(Box::new(move |bytes| sink.lock().unwrap().push(bytes)));

        let replay: serde_json::Value = serde_json::from_slice(&seen.lock().unwrap()[0][1..]).unwrap();
        assert_eq!(replay["state"], "connected", "the replay on subscribe must be the latest status, not stale");

        bridge.emit_status(ConnectionStatus::Reconnecting { attempt: 1, message: "x".into() });
        let delivered: serde_json::Value = serde_json::from_slice(&seen.lock().unwrap()[1][1..]).unwrap();
        assert_eq!(delivered["state"], "reconnecting", "a status emitted after subscribing must still be delivered");
    }

    #[test]
    fn send_rejects_empty_and_oversized_bodies() {
        let bridge = Bridge::default();
        let (tx, _rx) = mpsc::unbounded_channel();
        bridge.set_outbound(Some(tx));
        assert!(bridge.send(Vec::new()).is_err(), "an empty body is never a valid frame");
        let oversized = vec![0u8; jojo_proto::MAX_FRAME_LEN + 1];
        assert!(bridge.send(oversized).is_err(), "a body over MAX_FRAME_LEN must be rejected before queuing");
    }

    #[test]
    fn send_is_refused_once_the_connection_ends() {
        let bridge = Bridge::default();
        let (tx, mut rx) = mpsc::unbounded_channel();
        bridge.set_outbound(Some(tx));
        assert!(bridge.send(vec![1, 2]).is_ok(), "connected bridge accepts frames");
        assert_eq!(rx.try_recv().unwrap(), vec![1, 2]);

        // `connection::run` calls this once `serve` returns.
        bridge.set_outbound(None);
        assert!(bridge.send(vec![3, 4]).is_err(), "a body sent after the connection ended must be refused");
    }

    #[tokio::test]
    async fn restart_requested_before_a_wait_began_does_not_fire_it() {
        let bridge = Bridge::default();
        bridge.request_restart();
        let since = bridge.restart_generation();
        let fired = tokio::time::timeout(Duration::from_millis(100), bridge.wait_for_restart_after(since)).await;
        assert!(fired.is_err(), "a restart already accounted for in `since` must not fire this wait");
    }

    #[tokio::test]
    async fn restart_requested_after_a_wait_began_fires_it() {
        let bridge = Arc::new(Bridge::default());
        let since = bridge.restart_generation();
        let waiter = bridge.clone();
        let handle = tokio::spawn(async move { waiter.wait_for_restart_after(since).await });
        tokio::time::sleep(Duration::from_millis(20)).await;
        bridge.request_restart();
        tokio::time::timeout(Duration::from_secs(2), handle).await.expect("fires promptly").unwrap();
    }
}
