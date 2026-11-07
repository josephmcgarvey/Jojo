//! Proves that a conpty.dll placed next to the executable is the one
//! portable-pty loads. Skips (passes with a message) when
//! scripts/fetch-conpty.ps1 has not been run.
//!
//! The check runs in an isolated copy of this test binary rather than in
//! the normal `target/debug/deps` folder: Windows searches the
//! executable's own folder first, so sideloading `conpty.dll` next to the
//! shared test binary would make every other test binary in that folder
//! (`daemon_roundtrip`, the `jojod` unit tests, and so on) silently pick up
//! the sideloaded ConPTY too. A loaded DLL also cannot be deleted while the
//! process holds it, so there is no way to clean up after the fact. Instead
//! `sideloaded_conpty_is_used` copies the test binary plus `conpty.dll` and
//! `OpenConsole.exe` into a `TempDir` and re-invokes only the ignored
//! `sideload_check_in_isolated_copy` test there, as a child process, so the
//! sideload is confined to a throwaway folder that is deleted (binary and
//! all) once the child process exits and releases its handle on the DLL.
#![cfg(windows)]

use jojo_proto::SessionId;
use jojod::session::{Session, SpawnSpec};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;

#[test]
fn sideloaded_conpty_is_used() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/conpty/x64");
    if !source.join("conpty.dll").exists() || !source.join("OpenConsole.exe").exists() {
        eprintln!("skipped: run scripts/fetch-conpty.ps1 first");
        return;
    }

    let dir = tempfile::tempdir().expect("create temp dir");
    let current_exe = std::env::current_exe().unwrap();
    let exe_name = current_exe.file_name().unwrap();
    let copied_exe = dir.path().join(exe_name);
    std::fs::copy(&current_exe, &copied_exe).expect("copy test binary into temp dir");
    for name in ["conpty.dll", "OpenConsole.exe"] {
        std::fs::copy(source.join(name), dir.path().join(name)).expect("copy ConPTY binary");
    }

    let output = Command::new(&copied_exe)
        .args([
            "--exact",
            "sideload_check_in_isolated_copy",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("JOJO_SIDELOAD_CHILD", "1")
        .output()
        .expect("run isolated copy of the test binary");

    assert!(
        output.status.success(),
        "isolated sideload check failed (status {:?})\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Does the actual spawn-and-check; only runs when invoked as the isolated
/// child process by `sideloaded_conpty_is_used`, which is the only thing
/// that sets `JOJO_SIDELOAD_CHILD`. Running this test directly (e.g. via
/// `cargo test -- --ignored`) is a no-op: it returns immediately rather
/// than sideloading into the shared `target/debug/deps` folder.
#[test]
#[ignore = "run only by sideloaded_conpty_is_used in an isolated copy"]
fn sideload_check_in_isolated_copy() {
    if std::env::var("JOJO_SIDELOAD_CHILD").as_deref() != Ok("1") {
        return;
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
