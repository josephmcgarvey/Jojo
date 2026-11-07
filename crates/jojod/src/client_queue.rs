//! Per-client outbound queue with byte accounting (Plan 1 carry-over 2.3,
//! item 1). A window that stops reading must not grow daemon memory without
//! limit: once more than the limit of live Output is waiting, further
//! Output frames are refused, and the session drops that subscriber and
//! tells it to attach again for a fresh snapshot. The limit is per client
//! connection, shared across every session that client is attached to (one
//! `ClientTx`/`ClientRx` pair serves one connection, not one session).
//!
//! Only `Output` bytes count toward the limit. `Snapshot` and `Control`
//! frames are always queued uncounted: a snapshot can be large (bounded by
//! `jojod::session::MAX_SNAPSHOT_BYTES`, tens of MiB), and counting it would
//! let one queued snapshot alone exceed the limit, which would refuse every
//! subsequent Output, which would detach the client, which makes it
//! re-attach and receive another full snapshot — a re-attach loop that
//! never lets the client catch up. Counting only Output, which this queue
//! actually exists to bound, avoids that loop while still shedding a client
//! that cannot keep up with live output.
//!
//! A second, total cap (`MAX_CLIENT_TOTAL_BYTES`) counts every frame queued
//! for the client, Snapshot and Control included. The per-session Output
//! limit above detaches one session's stream from a client that cannot keep
//! up with it; the total limit means the client has stopped reading from its
//! connection altogether (every session attached to it, plus whatever
//! control traffic is queued, is piling up), and the connection itself is
//! closed rather than just one session's stream.

use jojo_proto::Frame;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Output bytes that may wait for one client before it is detached.
pub const MAX_CLIENT_QUEUE_BYTES: usize = 8 * 1024 * 1024;

/// Total bytes (every frame kind) that may wait for one client connection
/// before it is closed outright. See the module doc for how this differs
/// from `MAX_CLIENT_QUEUE_BYTES`.
pub const MAX_CLIENT_TOTAL_BYTES: usize = 64 * 1024 * 1024;

/// The accounting weight of a control frame that does not carry a byte
/// payload, used so the total cap still bounds a client that is flooded with
/// small control messages rather than raw output.
const CONTROL_FRAME_WEIGHT: usize = 256;

#[derive(Clone)]
pub struct ClientTx {
    tx: mpsc::UnboundedSender<Frame>,
    queued: Arc<AtomicUsize>,
    limit: usize,
    total: Arc<AtomicUsize>,
    total_limit: usize,
    overflow: Arc<AtomicBool>,
}

pub struct ClientRx {
    rx: mpsc::UnboundedReceiver<Frame>,
    queued: Arc<AtomicUsize>,
    total: Arc<AtomicUsize>,
}

/// Convenience wrapper over `client_queue_with_total`, using
/// `MAX_CLIENT_TOTAL_BYTES` as the total cap.
pub fn client_queue(limit: usize) -> (ClientTx, ClientRx) {
    client_queue_with_total(limit, MAX_CLIENT_TOTAL_BYTES)
}

pub fn client_queue_with_total(output_limit: usize, total_limit: usize) -> (ClientTx, ClientRx) {
    let (tx, rx) = mpsc::unbounded_channel();
    let queued = Arc::new(AtomicUsize::new(0));
    let total = Arc::new(AtomicUsize::new(0));
    (
        ClientTx {
            tx,
            queued: queued.clone(),
            limit: output_limit,
            total: total.clone(),
            total_limit,
            overflow: Arc::new(AtomicBool::new(false)),
        },
        ClientRx { rx, queued, total },
    )
}

/// The queued-byte accounting length of `frame`: only `Output` counts (see
/// the module doc for why `Snapshot` must not).
fn output_len(frame: &Frame) -> usize {
    match frame {
        Frame::Output { bytes, .. } => bytes.len(),
        _ => 0,
    }
}

/// The accounting length of `frame` toward the total cap: `Output` and
/// `Snapshot` count their byte payload, every other (control) frame counts a
/// flat `CONTROL_FRAME_WEIGHT`.
fn total_len(frame: &Frame) -> usize {
    match frame {
        Frame::Output { bytes, .. } | Frame::Snapshot { bytes, .. } => bytes.len(),
        _ => CONTROL_FRAME_WEIGHT,
    }
}

impl ClientTx {
    /// Queues `frame`. Returns false when the receiver is gone, or when the
    /// frame is Output and would push the queued output past the limit.
    /// Snapshots and control frames are always queued and never counted
    /// (see the module doc): a snapshot is the recovery path after a
    /// refusal, and counting it toward the same limit it is meant to
    /// recover from would make that recovery refuse itself.
    pub fn send(&self, frame: Frame) -> bool {
        let len = output_len(&frame);
        if len > 0 {
            // Exact CAS loop: a plain load-then-add would let two
            // concurrent senders each observe room for `len` and both add,
            // overshooting `limit`. `try_update` (the stable successor to
            // `fetch_update`) retries on a concurrent change and only
            // commits the add when it still fits.
            let accepted = self
                .queued
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                    (current + len <= self.limit).then_some(current + len)
                })
                .is_ok();
            if !accepted {
                return false;
            }
        }

        let total_len = total_len(&frame);
        let total_accepted = self
            .total
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                (current + total_len <= self.total_limit).then_some(current + total_len)
            })
            .is_ok();
        if !total_accepted {
            self.overflow.store(true, Ordering::Relaxed);
            if len > 0 {
                self.queued.fetch_sub(len, Ordering::Relaxed);
            }
            return false;
        }

        if self.tx.send(frame).is_err() {
            if len > 0 {
                self.queued.fetch_sub(len, Ordering::Relaxed);
            }
            self.total.fetch_sub(total_len, Ordering::Relaxed);
            return false;
        }
        true
    }

    /// True once a frame has been refused for exceeding the total cap. The
    /// server closes the connection once this is set (see the module doc).
    pub fn overflowed(&self) -> bool {
        self.overflow.load(Ordering::Relaxed)
    }
}

impl ClientRx {
    pub async fn recv(&mut self) -> Option<Frame> {
        let frame = self.rx.recv().await?;
        self.queued.fetch_sub(output_len(&frame), Ordering::Relaxed);
        self.total.fetch_sub(total_len(&frame), Ordering::Relaxed);
        Some(frame)
    }

    /// Non-blocking receive, for tests and loops outside an async wait
    /// point. Returns `None` both when the queue is momentarily empty and
    /// when the sender is gone; callers that must distinguish those use
    /// `recv` instead.
    pub fn try_recv(&mut self) -> Option<Frame> {
        let frame = self.rx.try_recv().ok()?;
        self.queued.fetch_sub(output_len(&frame), Ordering::Relaxed);
        self.total.fetch_sub(total_len(&frame), Ordering::Relaxed);
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
    async fn snapshots_and_control_do_not_count_toward_the_limit() {
        let (tx, _rx) = client_queue(100);
        assert!(tx.send(Frame::Snapshot { session_id: SessionId::new(), bytes: vec![0; 500] }));
        assert!(tx.send(output(60)), "the snapshot must not count toward the 100-byte limit");
    }

    #[tokio::test]
    async fn send_after_receiver_dropped_returns_false() {
        let (tx, rx) = client_queue(100);
        drop(rx);
        assert!(!tx.send(Frame::Control(ControlMsg::ListSessions)));
    }

    #[tokio::test]
    async fn total_cap_counts_every_frame_and_marks_overflow() {
        let (tx, _rx) = client_queue_with_total(1_000_000, 300);
        assert!(tx.send(Frame::Snapshot { session_id: SessionId::new(), bytes: vec![0; 200] }));
        assert!(!tx.overflowed());
        assert!(!tx.send(Frame::Snapshot { session_id: SessionId::new(), bytes: vec![0; 200] }));
        assert!(tx.overflowed(), "a refused frame past the total cap marks the client overflowed");
    }
}
