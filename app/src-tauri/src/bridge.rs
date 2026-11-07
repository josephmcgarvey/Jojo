//! Carries daemon frames and connection status to the frontend, and frame
//! bodies from the frontend to the daemon.
//!
//! Envelope sent to the frontend (one Tauri Channel message each): byte 0 is
//! the kind, the rest is the body. `ENVELOPE_FRAME`: a daemon frame without
//! its length prefix (`[u8 type][payload]`). `ENVELOPE_STATUS`: UTF-8 JSON
//! `ConnectionStatus`.

use serde::Serialize;
use std::sync::Mutex;
use tokio::sync::{mpsc, Notify};
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

pub struct Bridge {
    sink: Mutex<Option<Sink>>,
    last_status: Mutex<ConnectionStatus>,
    outbound: Mutex<Option<mpsc::UnboundedSender<Vec<u8>>>>,
    /// Signaled by the `restart_daemon` command.
    pub restart: Notify,
}

impl Default for Bridge {
    fn default() -> Self {
        Bridge {
            sink: Mutex::new(None),
            last_status: Mutex::new(ConnectionStatus::Starting),
            outbound: Mutex::new(None),
            restart: Notify::new(),
        }
    }
}

impl Bridge {
    /// Installs the frontend's channel and sends it the latest status, so a
    /// frontend that subscribes late still learns the connection state.
    pub fn set_sink(&self, sink: Sink) {
        let status = self.last_status.lock().unwrap().clone();
        sink(envelope(ENVELOPE_STATUS, &serde_json::to_vec(&status).expect("status serializes")));
        *self.sink.lock().unwrap() = Some(sink);
    }

    pub fn emit_status(&self, status: ConnectionStatus) {
        let bytes = envelope(ENVELOPE_STATUS, &serde_json::to_vec(&status).expect("status serializes"));
        *self.last_status.lock().unwrap() = status;
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink(bytes);
        }
    }

    pub fn emit_frame(&self, body: &[u8]) {
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink(envelope(ENVELOPE_FRAME, body));
        }
    }

    pub fn set_outbound(&self, tx: Option<mpsc::UnboundedSender<Vec<u8>>>) {
        *self.outbound.lock().unwrap() = tx;
    }

    /// Queues a frame body for the daemon. Refused while disconnected:
    /// keystrokes typed into a terminal the user cannot see must not be
    /// replayed later (spec section 8.3, rule 4).
    pub fn send(&self, body: Vec<u8>) -> Result<(), &'static str> {
        match self.outbound.lock().unwrap().as_ref() {
            Some(tx) if tx.send(body).is_ok() => Ok(()),
            _ => Err("not connected to jojod"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

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
}
