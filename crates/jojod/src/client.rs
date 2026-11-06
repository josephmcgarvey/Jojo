//! Client side of the protocol, used by `jojod shutdown` and the tests.

use crate::paths::Endpoint;
use crate::transport::{connect, BoxRead, BoxWrite};
use jojo_proto::{read_frame, write_frame, ControlMsg, Frame, PROTOCOL_VERSION};
use std::io;
use std::time::Duration;

pub struct Client {
    reader: BoxRead,
    writer: BoxWrite,
}

impl Client {
    /// Connects and completes the `Hello` exchange.
    pub async fn connect(endpoint: &Endpoint) -> io::Result<Client> {
        let (reader, writer) = connect(endpoint).await?;
        let mut client = Client { reader, writer };
        client
            .send(Frame::Control(ControlMsg::Hello {
                protocol_version: PROTOCOL_VERSION,
                jojod_version: crate::VERSION.to_string(),
            }))
            .await?;
        match client.recv().await? {
            Frame::Control(ControlMsg::Hello { protocol_version, .. }) if protocol_version == PROTOCOL_VERSION => {
                Ok(client)
            }
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unexpected first frame from daemon: {other:?}"),
            )),
        }
    }

    pub async fn send(&mut self, frame: Frame) -> io::Result<()> {
        write_frame(&mut self.writer, &frame).await
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
