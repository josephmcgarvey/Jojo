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
//! A second, total cap (`MAX_CLIENT_TOTAL_BYTES`) catches a client that has
//! stopped reading from its connection altogether: it counts `Output` bytes
//! plus a flat per-frame weight (`CONTROL_FRAME_WEIGHT`) for every other
//! frame, `Control` and `Snapshot` alike. Snapshots do not count their real
//! byte size toward this cap even though one can be tens of MiB: a snapshot
//! is already bounded per attach (`jojod::session::MAX_SNAPSHOT_BYTES`,
//! about 16 MiB) and the app keeps at most one outstanding `Attach` per
//! session, but counting a snapshot's actual size here would let a burst of
//! re-attaches (several sessions re-attaching at once, for example after a
//! missed-events reconnect) trip the 64 MiB cap on its own, closing the
//! connection and sending the client straight back into another round of
//! re-attaches. The per-session Output limit above detaches one session's
//! stream from a client that cannot keep up with it; the total limit closes
//! the whole connection once the client has stopped reading from it at all
//! (Output plus whatever control traffic, snapshots included, is piling up).

use jojo_proto::Frame;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Notify};

/// Output bytes that may wait for one client before it is detached.
pub const MAX_CLIENT_QUEUE_BYTES: usize = 8 * 1024 * 1024;

/// Total bytes (every frame kind) that may wait for one client connection
/// before it is closed outright. See the module doc for how this differs
/// from `MAX_CLIENT_QUEUE_BYTES`.
pub const MAX_CLIENT_TOTAL_BYTES: usize = 64 * 1024 * 1024;

/// The accounting weight toward the total cap for any frame that does not
/// count its own byte payload: every frame other than `Output`, `Snapshot`
/// included (see the module doc for why a snapshot's real size is excluded).
const CONTROL_FRAME_WEIGHT: usize = 256;

#[derive(Clone)]
pub struct ClientTx {
    tx: mpsc::UnboundedSender<Frame>,
    queued: Arc<AtomicUsize>,
    limit: usize,
    total: Arc<AtomicUsize>,
    total_limit: usize,
    overflow: Arc<AtomicBool>,
    overflow_notify: Arc<Notify>,
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
            overflow_notify: Arc::new(Notify::new()),
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

/// The accounting length of `frame` toward the total cap: `Output` counts
/// its byte payload; every other frame, `Snapshot` included, counts a flat
/// `CONTROL_FRAME_WEIGHT` regardless of its actual size (see the module doc
/// for why a snapshot's real size must not count here).
fn total_len(frame: &Frame) -> usize {
    match frame {
        Frame::Output { bytes, .. } => bytes.len(),
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
            // `swap` rather than `store`: only the thread that actually
            // flips `overflow` from false to true notifies, so a client
            // already marked overflowed does not keep calling `notify_one`
            // on every subsequent refused frame. `overflow_notified` relies
            // on exactly one notification having been issued by the time it
            // is first observed as true.
            let was_false = !self.overflow.swap(true, Ordering::Relaxed);
            if was_false {
                self.overflow_notify.notify_one();
            }
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

    /// True once the client has stopped reading at all: a frame has been
    /// refused for exceeding the total cap. The server closes the connection
    /// once this is set (see the module doc).
    pub fn overflowed(&self) -> bool {
        self.overflow.load(Ordering::Relaxed)
    }

    /// Resolves once this client has overflowed the total cap (see
    /// `overflowed`), including if it already has by the time this is
    /// called. Lets the server react to a client that stops reading
    /// altogether — never sending another frame itself — instead of only
    /// noticing overflow the next time it happens to handle one.
    pub fn overflow_notified(&self) -> impl std::future::Future<Output = ()> {
        let notify = self.overflow_notify.clone();
        async move { notify.notified().await }
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
    use std::time::Duration;

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
    async fn total_cap_counts_output_and_marks_overflow() {
        // A high output limit isolates this from the per-session Output
        // check, so only the 300-byte total cap is under test.
        let (tx, _rx) = client_queue_with_total(1_000_000, 300);
        assert!(tx.send(output(200)));
        assert!(!tx.overflowed());
        assert!(!tx.send(output(200)), "second output would exceed the 300-byte total cap");
        assert!(tx.overflowed(), "a refused frame past the total cap marks the client overflowed");
    }

    #[tokio::test]
    async fn snapshots_count_as_a_control_frame_toward_the_total_cap() {
        // A 10,000-byte snapshot would overflow a 600-byte total cap
        // instantly if it counted its real size; it must instead count no
        // more than CONTROL_FRAME_WEIGHT (256), the same as any other
        // control frame, leaving room for one more such frame under 600
        // (2 * 256 = 512).
        let (tx, _rx) = client_queue_with_total(1_000_000, 600);
        assert!(tx.send(Frame::Snapshot { session_id: SessionId::new(), bytes: vec![0; 10_000] }));
        assert!(!tx.overflowed());
        assert!(tx.send(Frame::Control(ControlMsg::ListSessions)));
        assert!(!tx.overflowed(), "a snapshot plus one control frame must not exceed a 600-byte total cap");
    }

    #[tokio::test]
    async fn total_cap_is_released_as_frames_are_taken_off_the_queue() {
        let (tx, mut rx) = client_queue_with_total(1_000_000, 300);
        assert!(tx.send(output(150)));
        assert!(tx.send(output(150)));
        assert!(!tx.send(output(1)), "no room left under the 300-byte total cap");

        // Both ways of taking a frame off the queue must release its total
        // accounting, not just `recv`.
        assert!(matches!(rx.recv().await, Some(Frame::Output { .. })));
        assert!(matches!(rx.try_recv(), Some(Frame::Output { .. })));

        assert!(tx.send(output(300)), "space was freed by recv and try_recv");
    }

    #[tokio::test]
    async fn a_refused_output_undoes_its_output_reservation() {
        // A generous output limit (100) but a tight total cap (300): the
        // control frame below pre-fills the total cap to 256/300, leaving
        // room for only 44 more bytes.
        let (tx, mut rx) = client_queue_with_total(100, 300);
        assert!(tx.send(Frame::Control(ControlMsg::ListSessions)));

        // This Output fits the 100-byte output limit on its own but not the
        // 44 bytes of total room left; it must be refused, and must not
        // leave its 100-byte output reservation in place.
        assert!(!tx.send(output(100)), "100 bytes of output plus 256 already queued exceeds the 300-byte total cap");
        assert!(tx.overflowed());

        // If the refused Output's reservation against the 100-byte output
        // limit were not rolled back, this smaller Output (which fits both
        // the output limit and the remaining 44 bytes of total room) would
        // wrongly be refused too.
        assert!(tx.send(output(40)), "a smaller Output that fits both limits must still succeed");
        assert!(matches!(rx.try_recv(), Some(Frame::Control(_))));
        assert!(matches!(rx.try_recv(), Some(Frame::Output { .. })));
    }

    #[tokio::test]
    async fn overflow_notified_resolves_only_once_the_total_cap_is_exceeded() {
        let (tx, _rx) = client_queue_with_total(1_000_000, 300);
        assert!(
            tokio::time::timeout(Duration::from_millis(100), tx.overflow_notified()).await.is_err(),
            "no frame has overflowed the total cap yet"
        );

        assert!(tx.send(output(200)));
        assert!(!tx.send(output(200)), "exceeds the 300-byte total cap");

        tokio::time::timeout(Duration::from_millis(100), tx.overflow_notified())
            .await
            .expect("overflow_notified must resolve once the client has overflowed");
    }
}
