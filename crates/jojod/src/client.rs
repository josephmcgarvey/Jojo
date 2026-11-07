//! Client side of the protocol, used by `jojod shutdown` and the tests.

use crate::paths::Endpoint;
use crate::transport::{connect, BoxRead, BoxWrite};
use jojo_proto::{read_frame, write_frame, ControlMsg, Frame, PROTOCOL_VERSION};
use std::io;
use std::time::Duration;

/// How long either side waits for the other's Hello before giving up.
pub const HELLO_TIMEOUT: Duration = Duration::from_secs(5);

/// The daemon speaks a different protocol version than this client.
#[derive(Debug)]
pub struct ProtocolMismatch {
    pub daemon: u32,
}

impl std::fmt::Display for ProtocolMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "jojod speaks protocol {}, this client speaks {}", self.daemon, PROTOCOL_VERSION)
    }
}

impl std::error::Error for ProtocolMismatch {}

/// The daemon's protocol version when `err` is a version mismatch.
pub fn is_protocol_mismatch(err: &io::Error) -> Option<u32> {
    err.get_ref().and_then(|e| e.downcast_ref::<ProtocolMismatch>()).map(|m| m.daemon)
}

pub struct Client {
    reader: BoxRead,
    writer: BoxWrite,
    jojod_version: String,
}

// `Result::unwrap_err` requires the `Ok` type to implement `Debug` (so it can
// format the value if the result turns out to be `Ok` and the call panics).
// `reader`/`writer` are `Box<dyn AsyncRead/AsyncWrite + ...>` trait objects
// that do not implement `Debug`, so this cannot be `#[derive(Debug)]`; only
// the daemon version is shown.
impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").field("jojod_version", &self.jojod_version).finish_non_exhaustive()
    }
}

impl Client {
    /// Connects and completes the `Hello` exchange.
    pub async fn connect(endpoint: &Endpoint) -> io::Result<Client> {
        let (reader, writer) = connect(endpoint).await?;
        Self::handshake(reader, writer, HELLO_TIMEOUT).await
    }

    /// Sends our Hello and waits up to `timeout` for the daemon's.
    pub async fn handshake(reader: BoxRead, writer: BoxWrite, timeout: Duration) -> io::Result<Client> {
        let mut client = Client { reader, writer, jojod_version: String::new() };
        client
            .send(Frame::Control(ControlMsg::Hello {
                protocol_version: PROTOCOL_VERSION,
                jojod_version: crate::VERSION.to_string(),
            }))
            .await?;
        let first = tokio::time::timeout(timeout, client.recv())
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "jojod did not say hello"))??;
        match first {
            Frame::Control(ControlMsg::Hello { protocol_version, jojod_version }) => {
                if protocol_version != PROTOCOL_VERSION {
                    return Err(io::Error::other(ProtocolMismatch { daemon: protocol_version }));
                }
                client.jojod_version = jojod_version;
                Ok(client)
            }
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unexpected first frame from jojod: {other:?}"),
            )),
        }
    }

    /// The daemon's version string, as reported in its `Hello`.
    pub fn daemon_version(&self) -> &str {
        &self.jojod_version
    }

    /// The raw halves, for callers that read and write concurrently.
    pub fn into_parts(self) -> (BoxRead, BoxWrite) {
        (self.reader, self.writer)
    }

    pub async fn send(&mut self, frame: Frame) -> io::Result<()> {
        write_frame(&mut self.writer, &frame).await
    }

    /// Writes raw bytes to the daemon. Used by tests and diagnostics to send
    /// frames the codec would never produce.
    pub async fn send_raw(&mut self, bytes: &[u8]) -> io::Result<()> {
        use tokio::io::AsyncWriteExt;
        self.writer.write_all(bytes).await?;
        self.writer.flush().await
    }

    pub async fn recv(&mut self) -> io::Result<Frame> {
        read_frame(&mut self.reader)
            .await?
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "daemon closed the connection"))
    }

    /// Receives frames until `pred` matches one, discarding the others.
    pub async fn recv_until(
        &mut self,
        timeout: Duration,
        mut pred: impl FnMut(&Frame) -> bool,
    ) -> io::Result<Frame> {
        tokio::time::timeout(timeout, async {
            loop {
                let frame = self.recv().await?;
                if pred(&frame) {
                    return Ok(frame);
                }
            }
        })
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "timed out waiting for frame"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jojo_proto::{write_frame, ControlMsg, Frame};

    #[tokio::test]
    async fn handshake_times_out_when_the_daemon_never_says_hello() {
        let (client_side, _daemon_side) = tokio::io::duplex(1024);
        let (r, w) = tokio::io::split(client_side);
        let started = std::time::Instant::now();
        let err = Client::handshake(Box::new(r), Box::new(w), Duration::from_millis(200)).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn handshake_reports_a_protocol_mismatch() {
        let (client_side, daemon_side) = tokio::io::duplex(1024);
        let (r, w) = tokio::io::split(client_side);
        let (_dr, mut dw) = tokio::io::split(daemon_side);
        write_frame(&mut dw, &Frame::Control(ControlMsg::Hello { protocol_version: 1, jojod_version: "0.0.1".into() }))
            .await
            .unwrap();
        let err = Client::handshake(Box::new(r), Box::new(w), HELLO_TIMEOUT).await.unwrap_err();
        assert_eq!(is_protocol_mismatch(&err), Some(1));
    }

    #[tokio::test]
    async fn handshake_keeps_the_daemon_version() {
        let (client_side, daemon_side) = tokio::io::duplex(1024);
        let (r, w) = tokio::io::split(client_side);
        let (_dr, mut dw) = tokio::io::split(daemon_side);
        write_frame(
            &mut dw,
            &Frame::Control(ControlMsg::Hello { protocol_version: jojo_proto::PROTOCOL_VERSION, jojod_version: "9.9.9".into() }),
        )
        .await
        .unwrap();
        let client = Client::handshake(Box::new(r), Box::new(w), HELLO_TIMEOUT).await.unwrap();
        assert_eq!(client.daemon_version(), "9.9.9");
    }
}
