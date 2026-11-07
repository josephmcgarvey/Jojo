//! Generates the TypeScript types for the app backend's own messages
//! (app/src/lib/backend). scripts/check-protocol.ps1 fails when they drift.

use jojo_app::bridge::ConnectionStatus;
use jojo_app::settings::Settings;
use std::path::PathBuf;
use ts_rs::{Config, TS};

#[test]
fn export_backend_bindings() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/lib/backend");
    let cfg = Config::new().with_out_dir(&dir);
    ConnectionStatus::export_all(&cfg).unwrap();
    Settings::export_all(&cfg).unwrap();
    let status = std::fs::read_to_string(dir.join("ConnectionStatus.ts")).unwrap();
    assert!(status.contains("\"state\": \"version_mismatch\""), "{status}");
    let settings = std::fs::read_to_string(dir.join("Settings.ts")).unwrap();
    assert!(settings.contains("terminalFontSize: number"), "{settings}");
    assert!(!settings.contains("bigint"), "{settings}");
}
