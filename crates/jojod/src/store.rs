//! Persistent daemon metadata in state.json (spec section 4).

use jojo_proto::{AgentState, ProjectInfo, SessionId, SessionInfo};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StoreData {
    pub projects: Vec<ProjectInfo>,
    pub sessions: Vec<SessionInfo>,
}

pub struct Store {
    path: PathBuf,
    pub data: StoreData,
}

/// Current Unix time in milliseconds.
pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl Store {
    /// Loads state.json. A missing file gives an empty store. An unreadable
    /// file is renamed to `state.json.corrupt-<unix seconds>` and the store
    /// starts empty. Every session not already exited is marked exited with
    /// no exit code, because its process ended with the previous daemon.
    pub fn load(path: &Path) -> io::Result<Store> {
        let mut data = match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice::<StoreData>(&bytes) {
                Ok(data) => data,
                Err(_) => {
                    let mut stamp = now_ms();
                    let corrupt = loop {
                        let mut name = path.as_os_str().to_owned();
                        name.push(format!(".corrupt-{stamp}"));
                        let candidate = PathBuf::from(name);
                        if !candidate.exists() {
                            break candidate;
                        }
                        stamp += 1;
                    };
                    std::fs::rename(path, corrupt)?;
                    StoreData::default()
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => StoreData::default(),
            Err(e) => return Err(e),
        };
        let now = now_ms();
        for s in &mut data.sessions {
            s.attachable = false;
            if s.state != AgentState::Exited {
                s.state = AgentState::Exited;
                s.exit_code = None;
                s.state_changed_at = now;
                // The pending tool or error belonged to a process that is gone.
                s.state_detail = None;
            }
        }
        Ok(Store { path: path.to_path_buf(), data })
    }

    /// Writes `state.json.tmp`, then renames it over `state.json`.
    pub fn save(&self) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut tmp = self.path.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        let bytes = serde_json::to_vec_pretty(&self.data).map_err(io::Error::other)?;
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &self.path)
    }

    pub fn session_mut(&mut self, id: SessionId) -> Option<&mut SessionInfo> {
        self.data.sessions.iter_mut().find(|s| s.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jojo_proto::{AgentState, ProjectId, SessionId, SessionInfo, SessionKind};

    fn session(state: AgentState, exit_code: Option<i32>) -> SessionInfo {
        SessionInfo {
            id: SessionId::new(),
            project_id: ProjectId::new(),
            kind: SessionKind::Shell,
            command: "pwsh".into(),
            args: vec![],
            label: "pwsh".into(),
            created_at: 1,
            state,
            state_changed_at: 1,
            exit_code,
            claude_session_id: None,
            state_detail: None,
            renamed: false,
            attachable: false,
        }
    }

    #[test]
    fn missing_file_loads_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::load(&dir.path().join("state.json")).unwrap();
        assert!(store.data.projects.is_empty() && store.data.sessions.is_empty());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let mut store = Store::load(&path).unwrap();
        store.data.sessions.push(session(AgentState::Exited, Some(0)));
        store.save().unwrap();
        assert!(!dir.path().join("state.json.tmp").exists());
        let loaded = Store::load(&path).unwrap();
        assert_eq!(loaded.data.sessions, store.data.sessions);
    }

    #[test]
    fn load_marks_live_sessions_exited_with_null_code() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let mut store = Store::load(&path).unwrap();
        store.data.sessions.push(session(AgentState::Working, None));
        store.data.sessions.push(session(AgentState::Exited, Some(3)));
        store.save().unwrap();
        let loaded = Store::load(&path).unwrap();
        assert_eq!(loaded.data.sessions[0].state, AgentState::Exited);
        assert_eq!(loaded.data.sessions[0].exit_code, None);
        assert_eq!(loaded.data.sessions[1].exit_code, Some(3));
    }

    #[test]
    fn corrupt_file_is_renamed_and_store_starts_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, b"{ not json").unwrap();
        let store = Store::load(&path).unwrap();
        assert!(store.data.sessions.is_empty());
        let renamed: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("state.json.corrupt-"))
            .collect();
        assert_eq!(renamed.len(), 1);
        assert!(!path.exists());
    }

    #[test]
    fn two_corrupt_loads_keep_both_copies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, b"{ bad").unwrap();
        Store::load(&path).unwrap();
        std::fs::write(&path, b"{ bad again").unwrap();
        Store::load(&path).unwrap();
        let copies = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("state.json.corrupt-"))
            .count();
        assert_eq!(copies, 2);
    }

    #[test]
    fn load_resets_attachable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let mut store = Store::load(&path).unwrap();
        let mut s = session(AgentState::Exited, Some(0));
        s.attachable = true;
        store.data.sessions.push(s);
        store.save().unwrap();
        assert!(!Store::load(&path).unwrap().data.sessions[0].attachable);
    }
}
