//! Per-client outbound queue with byte accounting (Plan 1 carry-over 2.3,
//! item 1). A window that stops reading must not grow daemon memory without
//! limit: once more than the limit of output is waiting, further Output
//! frames are refused, and the session drops that subscriber and tells it
//! to attach again for a fresh snapshot.

use jojo_proto::Frame;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Output bytes that may wait for one client before it is detached.
pub const MAX_CLIENT_QUEUE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone)]
pub struct ClientTx {
    tx: mpsc::UnboundedSender<Frame>,
    queued: Arc<AtomicUsize>,
    limit: usize,
}

pub struct ClientRx {
    rx: mpsc::UnboundedReceiver<Frame>,
    queued: Arc<AtomicUsize>,
}

pub fn client_queue(limit: usize) -> (ClientTx, ClientRx) {
    let (tx, rx) = mpsc::unbounded_channel();
    let queued = Arc::new(AtomicUsize::new(0));
    (ClientTx { tx, queued: queued.clone(), limit }, ClientRx { rx, queued })
}

fn data_len(frame: &Frame) -> usize {
    match frame {
        Frame::Output { bytes, .. } | Frame::Snapshot { bytes, .. } => bytes.len(),
        _ => 0,
    }
}

impl ClientTx {
    /// Queues `frame`. Returns false when the receiver is gone, or when the
    /// frame is Output and would push the queued output past the limit.
    /// Snapshots and control frames are always queued (a snapshot is the
    /// recovery path after a refusal).
    pub fn send(&self, frame: Frame) -> bool {
        let len = data_len(&frame);
        if matches!(frame, Frame::Output { .. }) && self.queued.load(Ordering::Relaxed) + len > self.limit {
            return false;
        }
        self.queued.fetch_add(len, Ordering::Relaxed);
        if self.tx.send(frame).is_err() {
            self.queued.fetch_sub(len, Ordering::Relaxed);
            return false;
        }
        true
    }
}

impl ClientRx {
    pub async fn recv(&mut self) -> Option<Frame> {
        let frame = self.rx.recv().await?;
        self.queued.fetch_sub(data_len(&frame), Ordering::Relaxed);
        Some(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jojo_proto::{ControlMsg, SessionId};

    fn output(n: usize) -> Frame {
        Frame::Output { session_id: SessionId::new(), bytes: vec![b'x'; n] }
    }

    #[tokio::test]
    async fn output_over_the_limit_is_refused_and_control_still_queues() {
        let (tx, mut rx) = client_queue(100);
        assert!(tx.send(output(60)));
        assert!(!tx.send(output(60)), "second output would exceed 100 bytes queued");
        assert!(tx.send(Frame::Control(ControlMsg::ListSessions)));
        assert!(matches!(rx.recv().await, Some(Frame::Output { .. })));
        assert!(tx.send(output(60)), "space was freed by recv");
    }

    #[tokio::test]
    async fn snapshots_always_queue_and_count() {
        let (tx, _rx) = client_queue(100);
        assert!(tx.send(Frame::Snapshot { session_id: SessionId::new(), bytes: vec![0; 500] }));
        assert!(!tx.send(output(1)), "queue already holds 500 bytes");
    }

    #[tokio::test]
    async fn send_after_receiver_dropped_returns_false() {
        let (tx, rx) = client_queue(100);
        drop(rx);
        assert!(!tx.send(Frame::Control(ControlMsg::ListSessions)));
    }
}
