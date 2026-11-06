use serde::{Deserialize, Serialize};
use std::fmt;
use ulid::Ulid;

/// Identifies one terminal session. Serialized as a ULID string in JSON and
/// as 16 raw bytes in data frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub Ulid);

impl SessionId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    pub fn to_bytes(self) -> [u8; 16] {
        self.0.to_bytes()
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Ulid::from_bytes(bytes))
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Identifies one project. Unique only within one daemon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(pub Ulid);

impl ProjectId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for ProjectId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
