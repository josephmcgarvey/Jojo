//! One connection loop per enabled machine (spec 8), all reporting to the
//! window through one shared `Frontend`.

use crate::bridge::{Bridge, Frontend, LOCAL_MACHINE};
use crate::daemon_manager::DaemonPaths;
use crate::machines::{Machine, MachineKind};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::async_runtime::JoinHandle;

/// A running machine: its bridge and its connection loop's task.
type Running = (Arc<Bridge>, JoinHandle<()>);

pub struct Hub {
    pub frontend: Arc<Frontend>,
    running: Mutex<HashMap<String, Running>>,
}

impl Default for Hub {
    fn default() -> Self {
        Hub { frontend: Arc::new(Frontend::default()), running: Mutex::new(HashMap::new()) }
    }
}

impl Hub {
    /// Starts `machine`'s connection loop unless it is disabled or already
    /// running. This PC uses `local_paths` (its staged daemon); a remote
    /// machine uses `ssh <host> jojod attach --stdio`.
    pub fn start(&self, machine: &Machine, local_paths: impl FnOnce() -> DaemonPaths) {
        if !machine.enabled {
            return;
        }
        let mut running = self.running.lock().unwrap();
        if running.contains_key(&machine.id) {
            return;
        }
        let bridge = Arc::new(Bridge::new(&machine.id, self.frontend.clone()));
        let task = match (machine.kind, &machine.ssh_host) {
            (MachineKind::Local, _) => tauri::async_runtime::spawn(crate::connection::run(local_paths(), bridge.clone())),
            (MachineKind::Ssh, Some(host)) => tauri::async_runtime::spawn(crate::connection::run_ssh(host.clone(), bridge.clone())),
            (MachineKind::Ssh, None) => return,
        };
        running.insert(machine.id.clone(), (bridge, task));
    }

    /// Stops a machine's connection loop (dropping the connection ends its
    /// `ssh` process) and forgets its status.
    pub fn stop(&self, machine: &str) {
        if let Some((_, task)) = self.running.lock().unwrap().remove(machine) {
            task.abort();
        }
        self.frontend.forget(machine);
    }

    pub fn bridge(&self, machine: &str) -> Option<Arc<Bridge>> {
        self.running.lock().unwrap().get(machine).map(|(bridge, _)| bridge.clone())
    }

    /// This PC's bridge (always running once the app has started).
    pub fn local(&self) -> Option<Arc<Bridge>> {
        self.bridge(LOCAL_MACHINE)
    }
}
