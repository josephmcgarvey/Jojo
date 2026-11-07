use crate::{ControlMsg, SessionId};
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Largest accepted value of the length prefix (type byte + payload).
pub const MAX_FRAME_LEN: usize = 16 * 1024 * 1024;

pub(crate) const FRAME_CONTROL: u8 = 1;
pub(crate) const FRAME_INPUT: u8 = 2;
pub(crate) const FRAME_OUTPUT: u8 = 3;
pub(crate) const FRAME_SNAPSHOT: u8 = 4;

#[derive(Clone, Debug, PartialEq)]
pub enum Frame {
    Control(ControlMsg),
    Input { session_id: SessionId, bytes: Vec<u8> },
    Output { session_id: SessionId, bytes: Vec<u8> },
    Snapshot { session_id: SessionId, bytes: Vec<u8> },
}

impl Frame {
    /// Encodes `[u32 LE length][u8 type][payload]`, where length counts the
    /// type byte plus the payload.
    pub fn encode(&self) -> Vec<u8> {
        let (frame_type, payload) = match self {
            Frame::Control(msg) => (
                FRAME_CONTROL,
                serde_json::to_vec(msg).expect("ControlMsg always serializes"),
            ),
            Frame::Input { session_id, bytes } => (FRAME_INPUT, data_payload(*session_id, bytes)),
            Frame::Output { session_id, bytes } => (FRAME_OUTPUT, data_payload(*session_id, bytes)),
            Frame::Snapshot { session_id, bytes } => {
                (FRAME_SNAPSHOT, data_payload(*session_id, bytes))
            }
        };
        let len = (1 + payload.len()) as u32;
        let mut out = Vec::with_capacity(4 + len as usize);
        out.extend_from_slice(&len.to_le_bytes());
        out.push(frame_type);
        out.extend_from_slice(&payload);
        out
    }

    fn decode(frame_type: u8, payload: &[u8]) -> io::Result<Frame> {
        match frame_type {
            FRAME_CONTROL => serde_json::from_slice(payload)
                .map(Frame::Control)
                .map_err(|e| bad_payload(format!("bad control JSON: {e}"))),
            FRAME_INPUT | FRAME_OUTPUT | FRAME_SNAPSHOT => {
                if payload.len() < 16 {
                    return Err(bad_payload("data frame shorter than its 16-byte session id"));
                }
                let id_bytes: [u8; 16] = payload[..16].try_into().expect("checked length");
                let session_id = SessionId::from_bytes(id_bytes);
                let bytes = payload[16..].to_vec();
                Ok(match frame_type {
                    FRAME_INPUT => Frame::Input { session_id, bytes },
                    FRAME_OUTPUT => Frame::Output { session_id, bytes },
                    _ => Frame::Snapshot { session_id, bytes },
                })
            }
            other => Err(bad_payload(format!("unknown frame type {other}"))),
        }
    }
}

fn data_payload(session_id: SessionId, bytes: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(16 + bytes.len());
    payload.extend_from_slice(&session_id.to_bytes());
    payload.extend_from_slice(bytes);
    payload
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// Marks a frame whose length prefix and body were read completely but whose
/// payload could not be decoded (bad JSON, unknown frame type, short data
/// frame). The stream is still positioned at the next frame, so a server can
/// reply with an error and keep the connection open.
#[derive(Debug)]
pub struct BadPayload(pub String);

impl std::fmt::Display for BadPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for BadPayload {}

fn bad_payload(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, BadPayload(message.into()))
}

/// True when `err` came from a frame that was fully read but could not be
/// decoded. Any other `read_frame` error leaves the stream misaligned.
pub fn is_bad_payload(err: &io::Error) -> bool {
    err.get_ref().is_some_and(|inner| inner.is::<BadPayload>())
}

/// Reads one frame. Returns `Ok(None)` when the stream ends cleanly between
/// frames; an end of stream inside a frame is an `UnexpectedEof` error.
pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<Option<Frame>> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 || len > MAX_FRAME_LEN {
        return Err(invalid(format!("frame length {len} out of range")));
    }
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    Frame::decode(buf[0], &buf[1..]).map(Some)
}

pub async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, frame: &Frame) -> io::Result<()> {
    let bytes = frame.encode();
    if bytes.len() - 4 > MAX_FRAME_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("frame of {} bytes exceeds MAX_FRAME_LEN", bytes.len() - 4),
        ));
    }
    writer.write_all(&bytes).await?;
    writer.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ControlMsg, ProjectId, SessionKind};

    async fn roundtrip(frame: Frame) -> Frame {
        let (mut a, mut b) = tokio::io::duplex(1024 * 1024);
        write_frame(&mut a, &frame).await.unwrap();
        read_frame(&mut b).await.unwrap().expect("one frame")
    }

    #[tokio::test]
    async fn control_frame_roundtrips() {
        let msg = ControlMsg::CreateSession {
            session_id: SessionId::new(),
            project_id: ProjectId::new(),
            kind: SessionKind::Claude,
            cols: 120,
            rows: 40,
            shell: None,
        };
        let frame = Frame::Control(msg);
        assert_eq!(roundtrip(frame.clone()).await, frame);
    }

    #[tokio::test]
    async fn data_frames_roundtrip_with_binary_payload() {
        let id = SessionId::new();
        let bytes: Vec<u8> = (0u8..=255).collect();
        for frame in [
            Frame::Input { session_id: id, bytes: bytes.clone() },
            Frame::Output { session_id: id, bytes: bytes.clone() },
            Frame::Snapshot { session_id: id, bytes: bytes.clone() },
        ] {
            assert_eq!(roundtrip(frame.clone()).await, frame);
        }
    }

    #[tokio::test]
    async fn data_frame_layout_is_length_type_id_bytes() {
        let id = SessionId::new();
        let encoded = Frame::Output { session_id: id, bytes: b"hi".to_vec() }.encode();
        let len = u32::from_le_bytes(encoded[0..4].try_into().unwrap());
        assert_eq!(len as usize, 1 + 16 + 2);
        assert_eq!(encoded[4], FRAME_OUTPUT);
        assert_eq!(&encoded[5..21], &id.to_bytes());
        assert_eq!(&encoded[21..], b"hi");
    }

    #[tokio::test]
    async fn clean_eof_between_frames_returns_none() {
        let (a, mut b) = tokio::io::duplex(64);
        drop(a);
        assert!(read_frame(&mut b).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn oversized_length_is_rejected() {
        let (mut a, mut b) = tokio::io::duplex(64);
        use tokio::io::AsyncWriteExt;
        a.write_all(&((MAX_FRAME_LEN as u32) + 1).to_le_bytes()).await.unwrap();
        let err = read_frame(&mut b).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn unknown_frame_type_is_rejected() {
        let (mut a, mut b) = tokio::io::duplex(64);
        use tokio::io::AsyncWriteExt;
        a.write_all(&1u32.to_le_bytes()).await.unwrap();
        a.write_all(&[99u8]).await.unwrap();
        let err = read_frame(&mut b).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn control_json_uses_type_tag() {
        let json = serde_json::to_string(&ControlMsg::ListSessions).unwrap();
        assert_eq!(json, r#"{"type":"ListSessions"}"#);
    }

    #[tokio::test]
    async fn bad_control_json_is_reported_and_stream_stays_aligned() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        use tokio::io::AsyncWriteExt;
        let body = b"{not json";
        a.write_all(&((1 + body.len()) as u32).to_le_bytes()).await.unwrap();
        a.write_all(&[FRAME_CONTROL]).await.unwrap();
        a.write_all(body).await.unwrap();
        write_frame(&mut a, &Frame::Control(ControlMsg::ListSessions)).await.unwrap();

        let err = read_frame(&mut b).await.unwrap_err();
        assert!(is_bad_payload(&err), "expected BadPayload, got {err:?}");
        let next = read_frame(&mut b).await.unwrap().expect("next frame");
        assert_eq!(next, Frame::Control(ControlMsg::ListSessions));
    }

    #[tokio::test]
    async fn oversized_length_is_not_a_bad_payload() {
        let (mut a, mut b) = tokio::io::duplex(64);
        use tokio::io::AsyncWriteExt;
        a.write_all(&((MAX_FRAME_LEN as u32) + 1).to_le_bytes()).await.unwrap();
        let err = read_frame(&mut b).await.unwrap_err();
        assert!(!is_bad_payload(&err));
    }

    #[tokio::test]
    async fn write_frame_refuses_frames_over_the_limit() {
        let (mut a, _b) = tokio::io::duplex(64);
        let frame = Frame::Snapshot { session_id: SessionId::new(), bytes: vec![b'x'; MAX_FRAME_LEN] };
        let err = write_frame(&mut a, &frame).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn error_without_session_id_still_parses() {
        let json = r#"{"type":"Error","request":"Attach","message":"x"}"#;
        let msg: ControlMsg = serde_json::from_str(json).unwrap();
        assert_eq!(
            msg,
            ControlMsg::Error { request: "Attach".into(), message: "x".into(), session_id: None }
        );
    }

    #[test]
    fn attach_carries_size() {
        let id = SessionId::new();
        let json = serde_json::to_string(&ControlMsg::Attach { session_id: id, cols: 100, rows: 30 }).unwrap();
        assert!(json.contains(r#""cols":100"#) && json.contains(r#""rows":30"#), "{json}");
    }
}
