//! Jojo wire protocol: ids, control messages and the frame codec.

/// Protocol version exchanged in `Hello`. Both sides require an exact match.
pub const PROTOCOL_VERSION: u32 = 1;
