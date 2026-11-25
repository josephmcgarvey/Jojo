//! Carries daemon frames and connection status to the frontend, and frame
//! bodies from the frontend to the daemon.
//!
//! There is one `Bridge` per machine (spec 8) and one `Frontend`, the
//! window's Tauri Channel, shared by all of them.
//!
//! Envelope sent to the frontend (one Tauri Channel message each): byte 0 is
//! the kind, byte 1 the length of the machine id, then the machine id in
//! UTF-8, then the body. `ENVELOPE_FRAME`: a daemon frame without its length
//! prefix (`[u8 type][payload]`). `ENVELOPE_STATUS`: UTF-8 JSON
//! `ConnectionStatus`.

use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
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
    /// SSH refused the key (spec 8.3 rule 5). The loop stops retrying until
    /// the user asks to reconnect.
    AuthFailed { message: String },
    /// The server cannot keep a daemon running between SSH connections
    /// (see `ssh::daemon_does_not_persist`). The loop stops retrying until
    /// the user asks to reconnect.
    Unsupported { message: String },
}

/// The machine id of This PC (spec 8.1).
pub const LOCAL_MACHINE: &str = "local";

pub fn envelope(kind: u8, machine: &str, body: &[u8]) -> Vec<u8> {
    let id = &machine.as_bytes()[..machine.len().min(u8::MAX as usize)];
    let mut out = Vec::with_capacity(2 + id.len() + body.len());
    out.push(kind);
    out.push(id.len() as u8);
    out.extend_from_slice(id);
    out.extend_from_slice(body);
    out
}

/// The machine id of an envelope.
pub fn envelope_machine(envelope: &[u8]) -> &str {
    let len = envelope[1] as usize;
    std::str::from_utf8(&envelope[2..2 + len]).unwrap_or("")
}

/// The body of an envelope (after the kind and the machine id).
pub fn envelope_body(envelope: &[u8]) -> &[u8] {
    &envelope[2 + envelope[1] as usize..]
}

type Sink = Box<dyn Fn(Vec<u8>) + Send + Sync>;

/// `sink` and every machine's last status share one lock, so a status
/// change can never be lost to a `set_sink`/`emit_status` race: whichever
/// call wins the lock either fully replays-then-installs or fully
/// updates-then-sends before the other can observe the state (Plan 2b Task 4
/// fix round 1, Important #2).
#[derive(Default)]
struct FrontendInner {
    sink: Option<Sink>,
    statuses: BTreeMap<String, ConnectionStatus>,
}

/// The window's channel, shared by every machine's `Bridge`.
#[derive(Default)]
pub struct Frontend {
    inner: Mutex<FrontendInner>,
}

impl Frontend {
    /// Installs the window's channel and sends it every machine's latest
    /// status, so a window that subscribes late still learns them.
    pub fn set_sink(&self, sink: Sink) {
        let mut inner = self.inner.lock().unwrap();
        for (machine, status) in &inner.statuses {
            sink(envelope(ENVELOPE_STATUS, machine, &serde_json::to_vec(status).expect("status serializes")));
        }
        inner.sink = Some(sink);
    }

    fn emit_status(&self, machine: &str, status: ConnectionStatus) {
        let bytes = envelope(ENVELOPE_STATUS, machine, &serde_json::to_vec(&status).expect("status serializes"));
        let mut inner = self.inner.lock().unwrap();
        inner.statuses.insert(machine.to_owned(), status);
        if let Some(sink) = inner.sink.as_ref() {
            sink(bytes);
        }
    }

    fn emit_frame(&self, machine: &str, body: &[u8]) {
        let inner = self.inner.lock().unwrap();
        if let Some(sink) = inner.sink.as_ref() {
            sink(envelope(ENVELOPE_FRAME, machine, body));
        }
    }

    /// Drops a removed machine's status, so a later subscribe does not
    /// replay it.
    pub fn forget(&self, machine: &str) {
        self.inner.lock().unwrap().statuses.remove(machine);
    }
}

pub struct Bridge {
    machine: String,
    frontend: Arc<Frontend>,
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

/// This PC's bridge with a frontend of its own (tests and single-machine use).
impl Default for Bridge {
    fn default() -> Self {
        Bridge::new(LOCAL_MACHINE, Arc::new(Frontend::default()))
    }
}

impl Bridge {
    /// The bridge for `machine`, whose status starts as `Starting`.
    pub fn new(machine: &str, frontend: Arc<Frontend>) -> Bridge {
        let (restart, _rx) = watch::channel(0u64);
        frontend.emit_status(machine, ConnectionStatus::Starting);
        Bridge { machine: machine.to_owned(), frontend, outbound: Mutex::new(None), restart }
    }

    pub fn machine(&self) -> &str {
        &self.machine
    }

    /// Installs the window's channel on this bridge's frontend (see
    /// `Frontend::set_sink`).
    pub fn set_sink(&self, sink: Sink) {
        self.frontend.set_sink(sink);
    }

    pub fn emit_status(&self, status: ConnectionStatus) {
        self.frontend.emit_status(&self.machine, status);
    }

    pub fn emit_frame(&self, body: &[u8]) {
        self.frontend.emit_frame(&self.machine, body);
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
    use std::sync::Mutex;
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
        let e = envelope(ENVELOPE_FRAME, "m1", &[3, 9]);
        assert_eq!(e, vec![0, 2, b'm', b'1', 3, 9]);
        assert_eq!(envelope_machine(&e), "m1");
        assert_eq!(envelope_body(&e), &[3, 9]);
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
        let json: serde_json::Value = serde_json::from_slice(envelope_body(&first)).unwrap();
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

        let replay: serde_json::Value = serde_json::from_slice(envelope_body(&seen.lock().unwrap()[0])).unwrap();
        assert_eq!(replay["state"], "connected", "the replay on subscribe must be the latest status, not stale");

        bridge.emit_status(ConnectionStatus::Reconnecting { attempt: 1, message: "x".into() });
        let delivered: serde_json::Value = serde_json::from_slice(envelope_body(&seen.lock().unwrap()[1])).unwrap();
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
