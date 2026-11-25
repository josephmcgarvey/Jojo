//! Finds, installs, starts and connects to the local jojod (spec section 8.1).

use jojod::client::{is_protocol_mismatch, Client};
use jojod::paths::{default_endpoint, Endpoint};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The daemon's file name in the daemon folder.
pub const JOJOD_FILE: &str = if cfg!(windows) { "jojod.exe" } else { "jojod" };

/// Files installed next to each other in the daemon folder. jojod is
/// required; on Windows the ConPTY pair is optional (without it the system
/// ConPTY is used).
#[cfg(windows)]
pub const DAEMON_FILES: &[&str] = &["jojod.exe", "conpty.dll", "OpenConsole.exe"];
#[cfg(unix)]
pub const DAEMON_FILES: &[&str] = &["jojod"];

/// How long to wait for a freshly started daemon to accept connections. On
/// macOS and Linux the daemon starts through the user's interactive login
/// shell, which reads ~/.zprofile and ~/.zshrc first (see `start_detached`).
const START_DEADLINE: Duration = Duration::from_secs(if cfg!(windows) { 5 } else { 15 });

#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x0000_0008;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// How long staging retries a daemon file that is in use (see `stage_binaries`).
#[cfg(windows)]
const SHARING_RETRY: Duration = Duration::from_secs(2);
#[cfg(windows)]
const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

pub struct DaemonPaths {
    /// Where the app's bundled daemon files are (the `daemon` resource folder).
    pub source_dir: PathBuf,
    /// Where the daemon runs from: `%LOCALAPPDATA%\Jojo\bin` on Windows,
    /// `~/.local/bin` on macOS and Linux (where Add machine installs it on
    /// servers, so the hooks and paths match).
    pub bin_dir: PathBuf,
    pub endpoint: Endpoint,
    /// Arguments for jojod (`["serve"]` in the app).
    pub serve_args: Vec<String>,
    /// Whether the connection loop runs `<bin_dir>/jojod install-hooks`,
    /// which edits the user's real `~/.claude/settings.json`. Only the app
    /// (`for_app`) sets it; tests use temporary `bin_dir`s that are deleted
    /// afterwards, and hooks registered to them would break every Claude
    /// Code session on the machine.
    pub register_hooks: bool,
}

impl DaemonPaths {
    pub fn for_app(source_dir: PathBuf) -> DaemonPaths {
        #[cfg(windows)]
        let bin_dir = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("Jojo").join("bin");
        #[cfg(unix)]
        let bin_dir = crate::platform::home_dir().unwrap_or_else(std::env::temp_dir).join(".local").join("bin");
        DaemonPaths {
            source_dir,
            bin_dir,
            endpoint: default_endpoint(),
            serve_args: vec!["serve".into()],
            register_hooks: true,
        }
    }
}

#[derive(Debug)]
pub enum ConnectError {
    /// The app is elevated, and jojod refuses to run elevated.
    Elevated,
    /// A jojod with another protocol version is running.
    Mismatch { daemon: u32 },
    /// SSH refused the key (spec 8.3 rule 5); ssh's stderr.
    AuthFailed(String),
    /// A remote daemon ends when the SSH connection that started it closes
    /// (the server runs each connection in its own temporary container), so
    /// retrying cannot help; the text explains what to change on the server.
    Unsupported(String),
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
            ConnectError::AuthFailed(stderr) => write!(f, "SSH refused the key: {stderr}"),
            ConnectError::Unsupported(message) => f.write_str(message),
            ConnectError::Io(e) => write!(f, "{e}"),
        }
    }
}

/// Copies each daemon file that is missing in `dest` or differs from
/// `source`. Returns whether anything was copied.
///
/// On Windows a file that cannot be replaced because the running daemon
/// holds it open is skipped: the running daemon keeps its version, and a
/// protocol mismatch is reported on connect. On macOS and Linux the new file
/// is written next to the old one and renamed over it, so a running daemon
/// keeps its own (now unlinked) file. Overwriting a running executable in
/// place fails on Linux (ETXTBSY) and on macOS gets the running process
/// killed when its code signature no longer matches its pages.
pub fn stage_binaries(source: &Path, dest: &Path) -> io::Result<bool> {
    if !source.join(JOJOD_FILE).is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{JOJOD_FILE} not found in {}", source.display()),
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
        // ERROR_SHARING_VIOLATION: a daemon is running from this file. The
        // copy is tried for up to SHARING_RETRY: right after Restart daemon
        // ends jojod with `taskkill /F`, Windows releases the ended
        // process's executable a moment later, and copying at once used to
        // fail and keep the old daemon (an upgrade then never took effect).
        // A daemon that is really running keeps the file locked throughout,
        // and its version stays, as before.
        #[cfg(windows)]
        {
            let deadline = Instant::now() + SHARING_RETRY;
            loop {
                match std::fs::copy(&from, &to) {
                    Ok(_) => {
                        copied = true;
                        break;
                    }
                    Err(e) if e.raw_os_error() == Some(32) => {
                        if Instant::now() >= deadline {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let tmp = dest.join(format!("{name}.new"));
            std::fs::copy(&from, &tmp)?;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
            // The copy inherits the app bundle's com.apple.quarantine
            // attribute (std::fs::copy uses fcopyfile, which copies extended
            // attributes), and Gatekeeper then refuses to run the unsigned
            // daemon. A missing attribute makes removexattr fail with
            // ENOATTR, which is ignored.
            #[cfg(target_os = "macos")]
            {
                use std::os::unix::ffi::OsStrExt;
                if let Ok(path) = std::ffi::CString::new(tmp.as_os_str().as_bytes()) {
                    // SAFETY: both arguments are NUL-terminated strings that outlive the call.
                    unsafe { libc::removexattr(path.as_ptr(), c"com.apple.quarantine".as_ptr(), 0) };
                }
            }
            std::fs::rename(&tmp, &to)?;
            copied = true;
        }
    }
    Ok(copied)
}

/// Starts jojod detached from the app, so closing the window never stops it.
#[cfg(windows)]
pub fn start_detached(paths: &DaemonPaths) -> io::Result<()> {
    let exe = paths.bin_dir.join(JOJOD_FILE);
    let spawn = |flags: u32| {
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(&paths.serve_args)
            .current_dir(&paths.bin_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
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

/// Starts `jojod serve --daemonize` through the user's interactive login
/// shell. An app opened from the Finder or the Dock gets launchd's minimal
/// environment (PATH is /usr/bin:/bin:/usr/sbin:/sbin), so Claude Code in
/// ~/.local/bin, Homebrew in /opt/homebrew/bin and nvm's node would be
/// missing from every session. `-l` reads ~/.zprofile (where Homebrew's
/// installer puts its PATH line) and `-i` reads ~/.zshrc (where nvm and the
/// Claude Code installer put theirs); stdin is /dev/null, so a prompt in a
/// startup file reads end-of-file instead of waiting. `--daemonize` forks,
/// detaches with setsid and returns once the socket accepts connections,
/// so the shell exits and the daemon is reparented to launchd. The shell is
/// waited for on a thread, so it leaves no zombie process behind.
#[cfg(unix)]
pub fn start_detached(paths: &DaemonPaths) -> io::Result<()> {
    let exe = paths.bin_dir.join(JOJOD_FILE);
    let shell = crate::platform::login_shell().filter(|s| Path::new(s).is_file()).unwrap_or_else(|| "/bin/sh".into());
    let mut args: Vec<String> = paths.serve_args.iter().map(|a| shell_quote(a)).collect();
    args.push("--daemonize".into());
    let command = format!("exec {} {}", shell_quote(&exe.to_string_lossy()), args.join(" "));
    let mut child = std::process::Command::new(&shell)
        .args(["-l", "-i", "-c", &command])
        .env("SHELL", &shell)
        .current_dir(crate::platform::home_dir().unwrap_or_else(|| paths.bin_dir.clone()))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// `value` as one single-quoted POSIX shell word.
#[cfg(unix)]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
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
    #[cfg(windows)]
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

/// Ends This PC's daemon (Restart daemon, or replacing a daemon that speaks
/// an older protocol; its sessions end). In the default profile every
/// jojod process of this user is ended. In a profile (`JOJO_PROFILE`) only
/// that profile's daemon is, found by the process id it writes to
/// `jojod.pid` in its state folder, so the user's other daemon and its
/// sessions keep running.
pub fn kill_daemons() -> io::Result<()> {
    match jojod::paths::profile() {
        Some(_) => kill_profile_daemon(&jojod::paths::default_state_dir()),
        None => kill_all_daemons(),
    }
}

/// Ends the daemon whose process id is in `state_dir`'s `jojod.pid`, when
/// that process is still a jojod (a stale file never ends another program).
/// A missing or unreadable file ends nothing.
fn kill_profile_daemon(state_dir: &Path) -> io::Result<()> {
    let Some(pid) = std::fs::read_to_string(state_dir.join(jojod::server::PID_FILE)).ok().and_then(|s| s.trim().parse::<u32>().ok()) else {
        return Ok(());
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new("taskkill");
        // The image-name filter makes taskkill skip the process unless it is jojod.exe.
        cmd.args(["/PID", &pid.to_string(), "/FI", "IMAGENAME eq jojod.exe", "/F"]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.output().map(|_| ())
    }
    #[cfg(unix)]
    {
        let name = std::process::Command::new("ps").args(["-p", &pid.to_string(), "-o", "comm="]).output()?;
        if String::from_utf8_lossy(&name.stdout).trim().ends_with("jojod") {
            std::process::Command::new("kill").args(["-9", &pid.to_string()]).output()?;
        }
        Ok(())
    }
}

/// Ends every jojod process of this user.
#[cfg(unix)]
fn kill_all_daemons() -> io::Result<()> {
    // SAFETY: getuid has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    std::process::Command::new("pkill").args(["-x", "-U", &uid.to_string(), "jojod"]).output().map(|_| ())
}

/// Ends every jojod process of this user.
#[cfg(windows)]
fn kill_all_daemons() -> io::Result<()> {
    use std::os::windows::process::CommandExt;
    let mut cmd = std::process::Command::new("taskkill");
    cmd.args(["/IM", "jojod.exe", "/F"]);
    cmd.creation_flags(CREATE_NO_WINDOW);
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
        write(source.path(), JOJOD_FILE, b"v1");
        if cfg!(windows) {
            write(source.path(), "conpty.dll", b"dll");
        }
        assert!(stage_binaries(source.path(), dest.path()).unwrap(), "first run copies");
        assert!(!stage_binaries(source.path(), dest.path()).unwrap(), "unchanged files are not copied");
        write(source.path(), JOJOD_FILE, b"v2");
        assert!(stage_binaries(source.path(), dest.path()).unwrap(), "a changed file is copied");
        assert_eq!(std::fs::read(dest.path().join(JOJOD_FILE)).unwrap(), b"v2");
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
