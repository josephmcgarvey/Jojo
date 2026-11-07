//! Proves that a conpty.dll placed next to the executable is the one
//! portable-pty loads. Skips (passes with a message) when
//! scripts/fetch-conpty.ps1 has not been run.
#![cfg(windows)]

use jojo_proto::SessionId;
use jojod::session::{Session, SpawnSpec};
use std::path::PathBuf;
use std::time::Duration;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;

#[test]
fn sideloaded_conpty_is_used() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/conpty/x64");
    if !source.join("conpty.dll").exists() {
        eprintln!("skipped: run scripts/fetch-conpty.ps1 first");
        return;
    }
    // The test executable's folder is the first place Windows searches.
    let exe_dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
    for name in ["conpty.dll", "OpenConsole.exe"] {
        std::fs::copy(source.join(name), exe_dir.join(name)).unwrap();
    }

    let (exit_tx, exit_rx) = std::sync::mpsc::channel();
    let session = Session::spawn(
        SessionId::new(),
        SpawnSpec {
            program: "cmd.exe".into(),
            args: vec!["/d".into(), "/c".into(), "echo sideload".into()],
            cwd: std::env::temp_dir(),
            env: vec![],
            cols: 80,
            rows: 24,
            scrollback: 100,
        },
        move |code| {
            let _ = exit_tx.send(code);
        },
    )
    .unwrap();
    exit_rx.recv_timeout(Duration::from_secs(15)).expect("session exits");

    let name: Vec<u16> = "conpty.dll".encode_utf16().chain(Some(0)).collect();
    // SAFETY: name is NUL-terminated; GetModuleHandleW does not take ownership.
    let module = unsafe { GetModuleHandleW(name.as_ptr()) };
    assert!(!module.is_null(), "conpty.dll was not loaded; the built-in ConPTY was used");
    drop(session);
}
