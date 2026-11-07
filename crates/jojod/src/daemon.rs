//! Request handling. Mutations are broadcast to every client as events;
//! only `Error` replies go to the requester alone (Plan 1, "Protocol
//! Additions").

use crate::command::{command_for, default_label};
use crate::paths::Endpoint;
use crate::session::{ClientId, Session, SpawnSpec};
use crate::store::{now_ms, Store};
use jojo_grid::{MAX_COLS, MAX_ROWS, MIN_COLS, MIN_ROWS};
use jojo_proto::{
    AgentState, ControlMsg, Frame, ProjectId, ProjectInfo, SessionId, SessionInfo, SessionKind,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use tokio::sync::{broadcast, Notify};

pub struct DaemonConfig {
    pub endpoint: Endpoint,
    pub state_dir: PathBuf,
    pub scrollback: usize,
}

pub type Reply = crate::client_queue::ClientTx;

/// The fields of `ControlMsg::CreateSession`, bundled so `create_session`
/// takes one argument for them instead of five (clippy's
/// `too_many_arguments`).
struct NewSession<'a> {
    id: SessionId,
    project_id: ProjectId,
    kind: SessionKind,
    cols: u16,
    rows: u16,
    shell: Option<&'a str>,
}

pub struct Daemon {
    pub cfg: DaemonConfig,
    store: Mutex<Store>,
    sessions: Mutex<HashMap<SessionId, Arc<Session>>>,
    events: broadcast::Sender<ControlMsg>,
    next_client: AtomicU64,
    pub shutdown: Notify,
}

impl Daemon {
    pub fn new(cfg: DaemonConfig) -> anyhow::Result<Arc<Daemon>> {
        let store = Store::load(&cfg.state_dir.join("state.json"))?;
        store.save()?;
        let (events, _) = broadcast::channel(1024);
        Ok(Arc::new(Daemon {
            cfg,
            store: Mutex::new(store),
            sessions: Mutex::new(HashMap::new()),
            events,
            next_client: AtomicU64::new(1),
            shutdown: Notify::new(),
        }))
    }

    pub fn next_client_id(&self) -> ClientId {
        self.next_client.fetch_add(1, Ordering::Relaxed)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ControlMsg> {
        self.events.subscribe()
    }

    fn broadcast(&self, msg: ControlMsg) {
        let _ = self.events.send(msg);
    }

    fn error(reply: &Reply, request: &str, message: impl Into<String>) {
        Self::error_for(reply, request, None, message)
    }

    fn error_for(reply: &Reply, request: &str, session_id: Option<SessionId>, message: impl Into<String>) {
        let _ = reply.send(Frame::Control(ControlMsg::Error {
            request: request.to_string(),
            message: message.into(),
            session_id,
        }));
    }

    /// Every stored session with `attachable` computed from the live map.
    /// Lock order: store, then sessions.
    fn session_infos(&self) -> Vec<SessionInfo> {
        let store = self.store.lock().unwrap_or_else(PoisonError::into_inner);
        let live = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        store
            .data
            .sessions
            .iter()
            .cloned()
            .map(|mut s| {
                s.attachable = live.contains_key(&s.id);
                s
            })
            .collect()
    }

    fn save(&self, store: &Store) {
        // A failed write is retried on the next change; the in-memory state
        // stays authoritative while the daemon runs.
        if let Err(e) = store.save() {
            tracing::error!(error = %e, "failed to write state.json");
        }
    }

    fn session(&self, id: SessionId) -> Option<Arc<Session>> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner).get(&id).cloned()
    }

    /// Handles one frame from `client`.
    pub fn handle(self: &Arc<Self>, client: ClientId, reply: &Reply, frame: Frame) {
        match frame {
            Frame::Input { session_id, bytes } => {
                if let Some(s) = self.session(session_id) {
                    s.write(bytes);
                }
            }
            Frame::Control(msg) => self.handle_control(client, reply, msg),
            Frame::Output { .. } | Frame::Snapshot { .. } => {
                Self::error(reply, "Frame", "clients do not send Output or Snapshot frames")
            }
        }
    }

    fn handle_control(self: &Arc<Self>, client: ClientId, reply: &Reply, msg: ControlMsg) {
        match msg {
            ControlMsg::ListProjects => {
                let projects = self.store.lock().unwrap_or_else(PoisonError::into_inner).data.projects.clone();
                let _ = reply.send(Frame::Control(ControlMsg::Projects { projects }));
            }
            ControlMsg::AddProject { path, name } => {
                if !std::path::Path::new(&path).is_absolute() {
                    return Self::error(reply, "AddProject", format!("path must be absolute: {path}"));
                }
                if !std::path::Path::new(&path).is_dir() {
                    return Self::error(reply, "AddProject", format!("not a directory: {path}"));
                }
                let mut store = self.store.lock().unwrap_or_else(PoisonError::into_inner);
                if store.data.projects.iter().any(|p| p.path == path) {
                    return Self::error(reply, "AddProject", format!("project already added: {path}"));
                }
                let project = ProjectInfo { id: ProjectId::new(), path, name };
                store.data.projects.push(project.clone());
                self.save(&store);
                // Broadcast while still holding `store` (tokio's
                // broadcast::Sender::send does not block), so broadcast order
                // always matches the order of store changes.
                self.broadcast(ControlMsg::ProjectAdded { project });
                drop(store);
            }
            ControlMsg::RemoveProject { id } => {
                let mut store = self.store.lock().unwrap_or_else(PoisonError::into_inner);
                if !store.data.projects.iter().any(|p| p.id == id) {
                    return Self::error(reply, "RemoveProject", "unknown project");
                }
                let running = store
                    .data
                    .sessions
                    .iter()
                    .filter(|s| s.project_id == id && s.state != AgentState::Exited)
                    .count();
                if running > 0 {
                    return Self::error(reply, "RemoveProject", format!("project has {running} running sessions"));
                }
                let removed_sessions: Vec<SessionId> =
                    store.data.sessions.iter().filter(|s| s.project_id == id).map(|s| s.id).collect();
                store.data.projects.retain(|p| p.id != id);
                store.data.sessions.retain(|s| s.project_id != id);
                self.save(&store);
                // Lock order: store, then sessions. The removed session rows
                // have already exited (checked above), so dropping their
                // `Session` objects here only cleans up state the process
                // itself no longer needs. Both kinds of broadcast happen
                // below while `store` is still held, so their order always
                // matches the order of the store changes above.
                {
                    let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
                    for session_id in &removed_sessions {
                        sessions.remove(session_id);
                    }
                }
                for session_id in removed_sessions {
                    self.broadcast(ControlMsg::SessionRemoved { session_id });
                }
                self.broadcast(ControlMsg::ProjectRemoved { id });
                drop(store);
            }
            ControlMsg::ListDir { path } => match list_dirs(&path) {
                Ok(entries) => {
                    let _ = reply.send(Frame::Control(ControlMsg::DirEntries { path, entries }));
                }
                Err(e) => Self::error(reply, "ListDir", format!("{path}: {e}")),
            },
            ControlMsg::ListSessions => {
                let sessions = self.session_infos();
                let _ = reply.send(Frame::Control(ControlMsg::Sessions { sessions }));
            }
            ControlMsg::CreateSession { session_id, project_id, kind, cols, rows, shell } => {
                if !size_in_range(cols, rows) {
                    return Self::error_for(reply, "CreateSession", Some(session_id), size_error_message(cols, rows));
                }
                self.create_session(
                    reply,
                    NewSession { id: session_id, project_id, kind, cols, rows, shell: shell.as_deref() },
                )
            }
            ControlMsg::Attach { session_id, cols, rows } => {
                if !size_in_range(cols, rows) {
                    return Self::error_for(reply, "Attach", Some(session_id), size_error_message(cols, rows));
                }
                match self.session(session_id) {
                    Some(s) => s.attach(client, reply.clone(), cols, rows),
                    None => Self::error_for(
                        reply,
                        "Attach",
                        Some(session_id),
                        "session has no screen (not running in this daemon)",
                    ),
                }
            }
            ControlMsg::Detach { session_id } => {
                if let Some(s) = self.session(session_id) {
                    s.detach(client);
                }
            }
            ControlMsg::Resize { session_id, cols, rows } => {
                if !size_in_range(cols, rows) {
                    return Self::error_for(reply, "Resize", Some(session_id), size_error_message(cols, rows));
                }
                if let Some(s) = self.session(session_id) {
                    s.resize(cols, rows);
                }
            }
            ControlMsg::Kill { session_id } => {
                if let Some(s) = self.session(session_id) {
                    s.kill();
                }
            }
            ControlMsg::CloseSession { session_id } => {
                // The unknown-id check and the removal happen under one
                // `store` lock acquisition (lock order: store, then
                // sessions), so two concurrent `CloseSession` calls for the
                // same id cannot both see it present and both broadcast
                // `SessionRemoved`: the second to acquire `store` finds the
                // row already gone and reports "unknown session" instead.
                let mut store = self.store.lock().unwrap_or_else(PoisonError::into_inner);
                let in_store = store.data.sessions.iter().any(|s| s.id == session_id);
                let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
                let in_live = sessions.contains_key(&session_id);
                if !in_store && !in_live {
                    drop(sessions);
                    drop(store);
                    return Self::error_for(reply, "CloseSession", Some(session_id), "unknown session");
                }
                // Take the session out of the live map instead of killing it
                // here: `kill()` can block briefly on the OS call that ends
                // the process, and must not run while either lock is held,
                // or it would stall every other request this daemon is
                // handling for that long. `taken` is dropped only after both
                // locks are released below; `Session::drop` is what actually
                // calls `kill()`.
                let taken = sessions.remove(&session_id);
                drop(sessions);
                store.data.sessions.retain(|s| s.id != session_id);
                self.save(&store);
                self.broadcast(ControlMsg::SessionRemoved { session_id });
                drop(store);
                // Explicit, not just relying on `Drop`: makes the kill
                // unmistakable at the call site, rather than depending on a
                // reader noticing that letting `taken` go out of scope here
                // is what ends the process. `Session::drop` still runs too
                // when `s` is dropped at the end of this block; `kill()` is
                // safe to call more than once (see its own doc comment).
                if let Some(s) = taken {
                    s.kill();
                }
            }
            ControlMsg::RenameSession { session_id, label } => {
                let mut store = self.store.lock().unwrap_or_else(PoisonError::into_inner);
                let Some(info) = store.session_mut(session_id) else {
                    return Self::error_for(reply, "RenameSession", Some(session_id), "unknown session");
                };
                info.label = label;
                let mut session = info.clone();
                session.attachable = self.sessions.lock().unwrap_or_else(PoisonError::into_inner).contains_key(&session_id);
                self.save(&store);
                self.broadcast(ControlMsg::SessionUpdated { session });
                drop(store);
            }
            ControlMsg::Shutdown => self.shutdown.notify_one(),
            other => Self::error(reply, &variant_name(&other), "not a client request"),
        }
    }

    fn create_session(self: &Arc<Self>, reply: &Reply, req: NewSession<'_>) {
        let NewSession { id, project_id, kind, cols, rows, shell } = req;
        let (program, args) = command_for(kind, shell);
        let now = now_ms();
        let info = SessionInfo {
            id,
            project_id,
            kind,
            command: program.clone(),
            args: args.clone(),
            label: default_label(kind, &program),
            created_at: now,
            state: if kind == SessionKind::Claude { AgentState::Starting } else { AgentState::Running },
            state_changed_at: now,
            exit_code: None,
            claude_session_id: None,
            attachable: false,
        };
        // Hold the store lock across the duplicate-id check, the project
        // lookup, spawn, and the SessionCreated broadcast. The store and the
        // live `self.sessions` map are always updated together, so the store
        // check alone is sufficient to catch a duplicate id; the live-map
        // check (lock order: store, then sessions) is defense in depth,
        // guarding against the two ever diverging in the future.
        // `on_session_exit` also takes the store lock, so
        // even a program that exits immediately cannot have its
        // SessionExited broadcast before SessionCreated. The project lookup
        // happens inside this same acquisition (not a separate earlier
        // one), so a concurrent RemoveProject cannot remove the project
        // between the lookup and the session row being inserted.
        let mut store = self.store.lock().unwrap_or_else(PoisonError::into_inner);
        if store.data.sessions.iter().any(|s| s.id == id)
            || self.sessions.lock().unwrap_or_else(PoisonError::into_inner).contains_key(&id)
        {
            return Self::error_for(reply, "CreateSession", Some(id), "session id already in use");
        }
        let Some(project) = store.data.projects.iter().find(|p| p.id == project_id).cloned() else {
            return Self::error_for(reply, "CreateSession", Some(id), "unknown project");
        };
        store.data.sessions.push(info.clone());
        let spec = SpawnSpec {
            program: program.clone(),
            args,
            cwd: PathBuf::from(&project.path),
            env: vec![
                ("TERM".into(), "xterm-256color".into()),
                ("COLORTERM".into(), "truecolor".into()),
                ("JOJO_SESSION_ID".into(), id.to_string()),
            ],
            cols,
            rows,
            scrollback: self.cfg.scrollback,
        };
        let weak = Arc::downgrade(self);
        let on_exit = move |code: Option<i32>| {
            if let Some(daemon) = weak.upgrade() {
                daemon.on_session_exit(id, code);
            }
        };
        match Session::spawn(id, spec, on_exit) {
            Ok(session) => {
                tracing::info!(session = %id, kind = ?kind, "session created");
                self.sessions.lock().unwrap_or_else(PoisonError::into_inner).insert(id, session);
                self.save(&store);
                // Broadcast while still holding `store` (tokio's
                // broadcast::Sender::send does not block): `on_session_exit`
                // also takes the store lock, so if this release the lock
                // first, a program that exits during spawn could have its
                // SessionExited broadcast (from the waiter thread blocked on
                // that same lock in `on_session_exit`) race ahead of this
                // SessionCreated.
                self.broadcast(ControlMsg::SessionCreated { session: SessionInfo { attachable: true, ..info } });
                drop(store);
            }
            Err(e) => {
                tracing::warn!(session = %id, error = %e, "session failed to start");
                let exit_code = Some(-1);
                if let Some(stored) = store.session_mut(id) {
                    stored.state = AgentState::Exited;
                    stored.exit_code = exit_code;
                }
                self.save(&store);
                let failed = SessionInfo { state: AgentState::Exited, exit_code, attachable: false, ..info };
                // Broadcast both events before dropping `store`, for the same
                // reason as the Ok branch above.
                self.broadcast(ControlMsg::SessionCreated { session: failed });
                self.broadcast(ControlMsg::SessionExited { session_id: id, exit_code });
                drop(store);
                let path = std::env::var("PATH").unwrap_or_default();
                Self::error_for(reply, "CreateSession", Some(id), format!("{e:#}. PATH searched: {path}"));
            }
        }
    }

    fn on_session_exit(&self, id: SessionId, exit_code: Option<i32>) {
        tracing::info!(session = %id, exit_code = ?exit_code, "session exited");
        let mut store = self.store.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(info) = store.session_mut(id) {
            info.state = AgentState::Exited;
            info.exit_code = exit_code;
            info.state_changed_at = now_ms();
            self.save(&store);
            // Broadcast while still holding `store`, matching every other
            // handler in this file, so event order always matches store order.
            self.broadcast(ControlMsg::SessionExited { session_id: id, exit_code });
            drop(store);
        }
    }

    /// Removes `client` from every session's subscribers.
    pub fn client_gone(&self, client: ClientId) {
        for s in self.sessions.lock().unwrap_or_else(PoisonError::into_inner).values() {
            s.detach(client);
        }
    }

    pub fn kill_all(&self) {
        for s in self.sessions.lock().unwrap_or_else(PoisonError::into_inner).values() {
            s.kill();
        }
    }
}

/// True when `cols`/`rows` are within the session size the daemon accepts
/// for `CreateSession` and `Resize` (C2). The bounds (`MIN_COLS..=MAX_COLS`,
/// `MIN_ROWS..=MAX_ROWS`) are defined once in `jojo_grid` and reused here so
/// the daemon's rejection and `TermGrid`'s own clamp (defense in depth) never
/// drift apart. Outside this range, alacritty_terminal's grid resize/
/// allocation path used to panic while the session's `shared` lock was held:
/// 0x0 underflows a subtraction in the resize path, and 65535x65535 attempts
/// to allocate roughly 100 GB and aborts the process. That panic poisoned
/// `shared`, and through `client_gone` and the daemon-wide `sessions`/`store`
/// locks (see the `Mutex::lock` calls below, fixed for I4), cascaded into
/// every later request handled by this daemon.
fn size_in_range(cols: u16, rows: u16) -> bool {
    (MIN_COLS..=MAX_COLS).contains(&cols) && (MIN_ROWS..=MAX_ROWS).contains(&rows)
}

fn size_error_message(cols: u16, rows: u16) -> String {
    format!("size out of range: {cols}x{rows} (cols {MIN_COLS}-{MAX_COLS}, rows {MIN_ROWS}-{MAX_ROWS})")
}

fn list_dirs(path: &str) -> std::io::Result<Vec<String>> {
    let mut entries: Vec<String> = std::fs::read_dir(path)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort_by_key(|s| s.to_lowercase());
    Ok(entries)
}

/// The JSON `type` tag of a message, for example `"Projects"`.
fn variant_name(msg: &ControlMsg) -> String {
    serde_json::to_value(msg)
        .ok()
        .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(str::to_owned))
        .unwrap_or_else(|| "Unknown".to_string())
}
