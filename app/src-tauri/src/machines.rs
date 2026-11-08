//! `%APPDATA%\Jojo\machines.toml` (spec 4, Machine row; spec 8).
//!
//! The file lists the machines the sidebar shows. This PC (`id = "local"`,
//! `kind = "local"`) is always present and always first: `load` adds it when
//! the file lacks it and moves it to the front, and `remove` refuses it. A
//! missing file means This PC alone. A file that cannot be parsed is renamed
//! to `machines.toml.corrupt-<unix ms>` and replaced by This PC alone, so a
//! hand edit gone wrong never stops the app from starting.

use crate::bridge::LOCAL_MACHINE;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MachineKind {
    Local,
    Ssh,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct Machine {
    /// `local` for This PC; a ULID for each remote machine.
    pub id: String,
    pub label: String,
    pub kind: MachineKind,
    /// An alias from `~/.ssh/config` or `user@host`; `None` for This PC.
    #[serde(default)]
    pub ssh_host: Option<String>,
    /// A disabled machine is listed but not connected.
    #[serde(default = "enabled_default")]
    pub enabled: bool,
}

fn enabled_default() -> bool {
    true
}

/// The file's top level: `[[machines]]` tables.
#[derive(Debug, Default, Serialize, Deserialize)]
struct MachinesFile {
    #[serde(default)]
    machines: Vec<Machine>,
}

pub fn this_pc() -> Machine {
    Machine { id: LOCAL_MACHINE.into(), label: "This PC".into(), kind: MachineKind::Local, ssh_host: None, enabled: true }
}

pub fn default_path() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("Jojo").join("machines.toml")
}

/// This PC first, then the file's remote machines in file order.
fn normalized(machines: Vec<Machine>) -> Vec<Machine> {
    let local = machines.iter().find(|m| m.id == LOCAL_MACHINE).cloned().map(|m| Machine { kind: MachineKind::Local, ssh_host: None, enabled: true, ..m });
    let mut out = vec![local.unwrap_or_else(this_pc)];
    out.extend(machines.into_iter().filter(|m| m.id != LOCAL_MACHINE && m.kind == MachineKind::Ssh && m.ssh_host.is_some()));
    out
}

pub fn load(path: &Path) -> Vec<Machine> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => return vec![this_pc()],
    };
    match toml::from_str::<MachinesFile>(&text) {
        Ok(file) => normalized(file.machines),
        Err(_) => {
            let mut aside = path.as_os_str().to_owned();
            aside.push(format!(".corrupt-{}", jojod::store::now_ms()));
            let _ = std::fs::rename(path, PathBuf::from(aside));
            vec![this_pc()]
        }
    }
}

/// Writes `machines.toml.tmp`, then renames it over `machines.toml`.
pub fn save(path: &Path, machines: &[Machine]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = toml::to_string(&MachinesFile { machines: normalized(machines.to_vec()) }).map_err(std::io::Error::other)?;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(id: &str, host: &str) -> Machine {
        Machine { id: id.into(), label: host.into(), kind: MachineKind::Ssh, ssh_host: Some(host.into()), enabled: true }
    }

    #[test]
    fn a_missing_file_is_this_pc_alone() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(&dir.path().join("machines.toml")), vec![this_pc()]);
    }

    #[test]
    fn round_trips_with_this_pc_first() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("machines.toml");
        save(&path, &[remote("01A", "onprem-server"), this_pc()]).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded, vec![this_pc(), remote("01A", "onprem-server")]);
        assert!(!dir.path().join("machines.toml.tmp").exists());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[[machines]]") && text.contains("ssh_host = \"onprem-server\""), "{text}");
    }

    #[test]
    fn adds_this_pc_and_drops_entries_that_cannot_connect() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("machines.toml");
        std::fs::write(&path, "[[machines]]\nid = \"01A\"\nlabel = \"box\"\nkind = \"ssh\"\n\n[[machines]]\nid = \"01B\"\nlabel = \"no host\"\nkind = \"ssh\"\n").unwrap();
        let loaded = load(&path);
        assert_eq!(loaded, vec![this_pc()], "an ssh machine without ssh_host is dropped");
    }

    #[test]
    fn a_corrupt_file_is_set_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("machines.toml");
        std::fs::write(&path, "not = [toml").unwrap();
        assert_eq!(load(&path), vec![this_pc()]);
        assert!(!path.exists());
        assert!(std::fs::read_dir(dir.path()).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with("machines.toml.corrupt-")));
    }
}
