//! Finds, installs, starts and connects to the local jojod (spec section 8.1).

use jojod::client::{is_protocol_mismatch, Client};
use jojod::paths::{default_endpoint, Endpoint};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Files installed next to each other in the daemon folder. jojod.exe is
/// required; the ConPTY pair is optional (without it the system ConPTY is used).
pub const DAEMON_FILES: [&str; 3] = ["jojod.exe", "conpty.dll", "OpenConsole.exe"];

/// How long to wait for a freshly started daemon to accept connections.
const START_DEADLINE: Duration = Duration::from_secs(5);

const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

pub struct DaemonPaths {
    /// Where the app's bundled daemon files are (the `daemon` resource folder).
    pub source_dir: PathBuf,
    /// Where the daemon runs from (`%LOCALAPPDATA%\Jojo\bin`).
    pub bin_dir: PathBuf,
    pub endpoint: Endpoint,
    /// Arguments for jojod.exe (`["serve"]` in the app).
    pub serve_args: Vec<String>,
}

impl DaemonPaths {
    pub fn for_app(source_dir: PathBuf) -> DaemonPaths {
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        DaemonPaths {
            source_dir,
            bin_dir: local.join("Jojo").join("bin"),
            endpoint: default_endpoint(),
            serve_args: vec!["serve".into()],
        }
    }
}

#[derive(Debug)]
pub enum ConnectError {
    /// The app is elevated, and jojod refuses to run elevated.
    Elevated,
    /// A jojod with another protocol version is running.
    Mismatch { daemon: u32 },
    Io(io::Error),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectError::Elevated => f.write_str("Jojo is running as administrator, and jojod will not run elevated"),
            ConnectError::Mismatch { daemon } => write!(
                f,
                "the running jojod speaks protocol {daemon}; this Jojo needs protocol {}",
                jojo_proto::PROTOCOL_VERSION
            ),
            ConnectError::Io(e) => write!(f, "{e}"),
        }
    }
}

/// Copies each daemon file that is missing in `dest` or differs from
/// `source`. Returns whether anything was copied. A file that cannot be
/// replaced because the running daemon holds it open is skipped: the running
/// daemon keeps its version, and a protocol mismatch is reported on connect.
pub fn stage_binaries(source: &Path, dest: &Path) -> io::Result<bool> {
    if !source.join("jojod.exe").is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("jojod.exe not found in {}", source.display()),
        ));
    }
    std::fs::create_dir_all(dest)?;
    let mut copied = false;
    for name in DAEMON_FILES {
        let from = source.join(name);
        if !from.is_file() {
            continue;
        }
        let to = dest.join(name);
        if to.is_file() && std::fs::read(&to)? == std::fs::read(&from)? {
            continue;
        }
        match std::fs::copy(&from, &to) {
            Ok(_) => copied = true,
            // ERROR_SHARING_VIOLATION: the daemon is running from this file.
            Err(e) if e.raw_os_error() == Some(32) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(copied)
}

/// Starts jojod detached from the app, so closing the window never stops it.
pub fn start_detached(paths: &DaemonPaths) -> io::Result<()> {
    let exe = paths.bin_dir.join("jojod.exe");
    let spawn = |flags: u32| {
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(&paths.serve_args)
            .current_dir(&paths.bin_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(flags);
        }
        cmd.spawn().map(|_| ())
    };
    match spawn(DETACHED_PROCESS | CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB) {
        // ERROR_ACCESS_DENIED: the job the app runs in forbids breakaway.
        Err(e) if e.raw_os_error() == Some(5) => spawn(DETACHED_PROCESS | CREATE_NO_WINDOW),
        other => other,
    }
}

/// Connects to the running daemon, or installs and starts one and connects.
pub async fn connect_or_start(paths: &DaemonPaths) -> Result<Client, ConnectError> {
    match Client::connect(&paths.endpoint).await {
        Ok(client) => return Ok(client),
        Err(e) => {
            if let Some(daemon) = is_protocol_mismatch(&e) {
                return Err(ConnectError::Mismatch { daemon });
            }
        }
    }
    if jojod::pipe_security_is_elevated().unwrap_or(false) {
        return Err(ConnectError::Elevated);
    }
    stage_binaries(&paths.source_dir, &paths.bin_dir).map_err(ConnectError::Io)?;
    start_detached(paths).map_err(ConnectError::Io)?;
    let deadline = Instant::now() + START_DEADLINE;
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        match Client::connect(&paths.endpoint).await {
            Ok(client) => return Ok(client),
            Err(e) => {
                if let Some(daemon) = is_protocol_mismatch(&e) {
                    return Err(ConnectError::Mismatch { daemon });
                }
                if Instant::now() >= deadline {
                    return Err(ConnectError::Io(e));
                }
            }
        }
    }
}

/// Runs `<jojod> install-hooks`, which registers Jojo's Claude Code hooks in
/// `~/.claude/settings.json` (spec 7.1). The merge is idempotent, so the app
/// runs it on every start; the hooks then always point at the jojod.exe the
/// app staged. A failure is logged by the caller and leaves Jojo working
/// without agent badges.
pub fn install_hooks(jojod: &Path) -> io::Result<()> {
    let mut cmd = std::process::Command::new(jojod);
    cmd.arg("install-hooks");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd.output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(String::from_utf8_lossy(&output.stderr).trim().to_owned()))
    }
}

/// Ends every jojod process of this user (used to replace a daemon that
/// speaks an older protocol; its sessions end).
pub fn kill_daemons() -> io::Result<()> {
    let mut cmd = std::process::Command::new("taskkill");
    cmd.args(["/IM", "jojod.exe", "/F"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.output().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, bytes: &[u8]) {
        std::fs::write(dir.join(name), bytes).unwrap();
    }

    #[test]
    fn stages_missing_and_changed_files_only() {
        let source = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        write(source.path(), "jojod.exe", b"v1");
        write(source.path(), "conpty.dll", b"dll");
        assert!(stage_binaries(source.path(), dest.path()).unwrap(), "first run copies");
        assert!(!stage_binaries(source.path(), dest.path()).unwrap(), "unchanged files are not copied");
        write(source.path(), "jojod.exe", b"v2");
        assert!(stage_binaries(source.path(), dest.path()).unwrap(), "a changed file is copied");
        assert_eq!(std::fs::read(dest.path().join("jojod.exe")).unwrap(), b"v2");
        assert!(!dest.path().join("OpenConsole.exe").exists(), "absent optional files are skipped");
    }

    #[test]
    fn staging_requires_jojod() {
        let source = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        let err = stage_binaries(source.path(), dest.path()).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }
}
