//! Generates the TypeScript protocol types used by the app
//! (app/src/lib/protocol). `scripts/check-protocol.ps1` runs this and fails
//! when the committed files differ, so the Rust structs stay the single
//! source of truth (spec section 6.1).

use jojo_proto::ControlMsg;
use std::path::PathBuf;
use ts_rs::{Config, TS};

fn out_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../app/src/lib/protocol")
}

#[test]
fn export_typescript_bindings() {
    let dir = out_dir();
    ControlMsg::export_all(&Config::new().with_out_dir(&dir)).expect("export bindings");

    let control = std::fs::read_to_string(dir.join("ControlMsg.ts")).unwrap();
    assert!(control.contains("\"type\": \"Attach\""), "{control}");
    assert!(control.contains("cols: number"), "{control}");
    let session = std::fs::read_to_string(dir.join("SessionInfo.ts")).unwrap();
    assert!(session.contains("created_at: number"), "u64 must be number, not bigint: {session}");
    assert!(session.contains("attachable: boolean"), "{session}");
    let id = std::fs::read_to_string(dir.join("SessionId.ts")).unwrap();
    assert!(id.contains("string"), "{id}");
}
