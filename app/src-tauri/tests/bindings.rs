//! Generates the TypeScript types for the app backend's own messages and
//! files (app/src/lib/backend). scripts/check-protocol.ps1 fails when they
//! drift.

use jojo_app::bridge::ConnectionStatus;
use jojo_app::config::{ImportPreview, ImportResult};
use jojo_app::layout::LayoutFile;
use jojo_app::install::{InstallStep, SshHost};
use jojo_app::machines::Machine;
use jojo_app::settings::Settings;
use jojo_app::shells::ShellOption;
use jojo_app::snippets::Snippet;
use std::path::PathBuf;
use ts_rs::{Config, TS};

#[test]
fn export_backend_bindings() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/lib/backend");
    let cfg = Config::new().with_out_dir(&dir);
    ConnectionStatus::export_all(&cfg).unwrap();
    Settings::export_all(&cfg).unwrap();
    LayoutFile::export_all(&cfg).unwrap();
    ShellOption::export_all(&cfg).unwrap();
    Machine::export_all(&cfg).unwrap();
    InstallStep::export_all(&cfg).unwrap();
    SshHost::export_all(&cfg).unwrap();
    Snippet::export_all(&cfg).unwrap();
    ImportPreview::export_all(&cfg).unwrap();
    ImportResult::export_all(&cfg).unwrap();
    let status = std::fs::read_to_string(dir.join("ConnectionStatus.ts")).unwrap();
    assert!(status.contains("\"state\": \"version_mismatch\""), "{status}");
    let settings = std::fs::read_to_string(dir.join("Settings.ts")).unwrap();
    assert!(settings.contains("terminalFontSize: number"), "{settings}");
    assert!(!settings.contains("bigint"), "{settings}");
    let node = std::fs::read_to_string(dir.join("LayoutNode.ts")).unwrap();
    assert!(node.contains("\"type\": \"pane\", session_id: string,"), "{node}");
    assert!(node.contains("children: Array<LayoutNode>"), "{node}");
    assert!(node.contains("sizes: Array<number>"), "{node}");
    let tab = std::fs::read_to_string(dir.join("TabLayout.ts")).unwrap();
    assert!(tab.contains("zoomed: string | null"), "{tab}");
    let file = std::fs::read_to_string(dir.join("LayoutFile.ts")).unwrap();
    assert!(file.contains("ProjectLayout"), "{file}");
    assert!(!file.contains("bigint"), "{file}");
    let machine = std::fs::read_to_string(dir.join("Machine.ts")).unwrap();
    assert!(machine.contains("ssh_host: string | null"), "{machine}");
    let status_auth = std::fs::read_to_string(dir.join("ConnectionStatus.ts")).unwrap();
    assert!(status_auth.contains("\"state\": \"auth_failed\""), "{status_auth}");
    let shell = std::fs::read_to_string(dir.join("ShellOption.ts")).unwrap();
    assert!(shell.contains("label: string,") && shell.contains("program: string,"), "{shell}");
}
