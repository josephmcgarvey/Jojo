//! Jojo wire protocol: ids, control messages and the frame codec.

mod frame;
mod ids;
mod msg;

pub use frame::{is_bad_payload, read_frame, write_frame, BadPayload, Frame, MAX_FRAME_LEN};
pub use ids::{ProjectId, SessionId};
pub use msg::{AgentState, ControlMsg, ProjectInfo, SearchHit, SessionInfo, SessionKind};

/// Protocol version exchanged in `Hello`. Both sides require an exact match.
pub const PROTOCOL_VERSION: u32 = 3;
