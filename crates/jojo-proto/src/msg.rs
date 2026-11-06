use crate::{ProjectId, SessionId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    Claude,
    Shell,
}

/// Session state shown as a badge (spec section 7.2). `Running` is used for
/// shell sessions, which have no hook-driven states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Starting,
    Idle,
    Working,
    NeedsYou,
    Done,
    Error,
    Running,
    Exited,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub id: ProjectId,
    /// Absolute path on the daemon's machine.
    pub path: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: SessionId,
    pub project_id: ProjectId,
    pub kind: SessionKind,
    pub command: String,
    pub args: Vec<String>,
    pub label: String,
    /// Unix time in milliseconds.
    pub created_at: u64,
    pub state: AgentState,
    /// Unix time in milliseconds.
    pub state_changed_at: u64,
    pub exit_code: Option<i32>,
    pub claude_session_id: Option<String>,
    /// True when this daemon still holds a screen for the session, so
    /// `Attach` will succeed. Computed by the daemon for every message that
    /// carries a `SessionInfo`; the value stored in state.json is ignored.
    #[serde(default)]
    pub attachable: bool,
}

/// Every control message. Serialized as JSON with a `type` tag naming the
/// variant, for example `{"type":"ListSessions"}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ControlMsg {
    Hello { protocol_version: u32, jojod_version: String },

    ListProjects,
    Projects { projects: Vec<ProjectInfo> },
    AddProject { path: String, name: String },
    ProjectAdded { project: ProjectInfo },
    RemoveProject { id: ProjectId },
    ProjectRemoved { id: ProjectId },
    ListDir { path: String },
    DirEntries { path: String, entries: Vec<String> },

    ListSessions,
    Sessions { sessions: Vec<SessionInfo> },
    CreateSession {
        project_id: ProjectId,
        kind: SessionKind,
        cols: u16,
        rows: u16,
        /// Shell program for `SessionKind::Shell`; `None` uses the daemon default.
        shell: Option<String>,
    },
    SessionCreated { session: SessionInfo },
    SessionUpdated { session: SessionInfo },
    Attach { session_id: SessionId, cols: u16, rows: u16 },
    Detach { session_id: SessionId },
    Resize { session_id: SessionId, cols: u16, rows: u16 },
    Kill { session_id: SessionId },
    CloseSession { session_id: SessionId },
    SessionRemoved { session_id: SessionId },
    RenameSession { session_id: SessionId, label: String },
    SessionExited { session_id: SessionId, exit_code: Option<i32> },

    Shutdown,
    Error {
        request: String,
        message: String,
        /// The session the error concerns, when there is one (for example
        /// a `CreateSession` whose program failed to start).
        #[serde(default)]
        session_id: Option<SessionId>,
    },
}
