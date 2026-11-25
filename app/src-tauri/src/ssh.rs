//! The SSH transport to a remote machine (spec 8.3).
//!
//! The app runs `ssh <host> ~/.local/bin/jojod attach --stdio` and speaks the
//! framed protocol over the child's stdin and stdout, exactly as it does over
//! This PC's named pipe; `attach --stdio` relays the bytes to the remote
//! daemon's socket. The `ssh` process lives as long as the connection: the
//! reader owns it, and dropping the reader kills it.

use crate::daemon_manager::ConnectError;
use jojod::client::{is_protocol_mismatch, Client, HELLO_TIMEOUT};
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};
use tokio::process::{Child, ChildStdout};

/// Where Jojo installs jojod on a remote machine (spec 8.2 step 4). The
/// remote shell expands `~`.
pub const REMOTE_JOJOD: &str = "~/.local/bin/jojod";

/// Runs `inner` (a command without single quotes) through a login shell, so
/// it gets the PATH a login gives. A command run over SSH gets a minimal
/// environment (on Debian and Ubuntu, ~/.profile is not read), so
/// `~/.local/bin` and nvm paths, where Claude Code usually lives, would be
/// missing. The shell is `$SHELL` when that is an executable file, else
/// bash, else /bin/sh: some hosts (RunCloud) set SHELL to a path that does
/// not exist, such as /bin/rc-shell.
pub fn login_shell_command(inner: &str) -> String {
    format!(
        "s=\"$SHELL\"; [ -x \"$s\" ] || s=$(command -v bash) || s=/bin/sh; [ -n \"$s\" ] || s=/bin/sh; exec \"$s\" -lc '{inner}'"
    )
}

/// `serve --daemonize` run through the user's login shell (see
/// `login_shell_command`), so sessions get the PATH a login gives.
pub fn start_daemon_command() -> String {
    login_shell_command(&format!("{REMOTE_JOJOD} serve --daemonize"))
}

/// Where and how ssh signs in to a remote location.
///
/// A location added from `~/.ssh/config` has only `host` (the alias), and
/// ssh takes everything else from the config. A location added with a
/// username and password has `host` = `user@address`, its `port`, and
/// either the key Jojo installed for it (`identity_file`) or the password.
#[derive(Clone, PartialEq, Eq)]
pub struct SshTarget {
    /// An alias from `~/.ssh/config`, `user@host` or a host name.
    pub host: String,
    /// `-p`; None uses ssh's default (22, or the config's Port).
    pub port: Option<u16>,
    /// `-i`, with `IdentitiesOnly=yes` so ssh offers only this key.
    pub identity_file: Option<PathBuf>,
    /// Signs in with this password, supplied by jojod as ssh's password
    /// helper (see `jojod::askpass`), instead of with a key.
    pub password: Option<String>,
}

impl SshTarget {
    /// A target that is only a host: an alias, `user@host` or a host name.
    pub fn host(host: impl Into<String>) -> Self {
        SshTarget { host: host.into(), port: None, identity_file: None, password: None }
    }
}

/// Debug output never includes the password.
impl std::fmt::Debug for SshTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshTarget")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("identity_file", &self.identity_file)
            .field("password", &self.password.as_ref().map(|_| "<hidden>"))
            .finish()
    }
}

/// The program ssh runs as its password helper: the app's bundled jojod
/// (set at startup with `set_askpass_program`). The bundled copy, not the
/// one the daemon runs from, because on Windows the running daemon's file
/// cannot be replaced, so it can be an older version without the helper.
static ASKPASS_PROGRAM: OnceLock<PathBuf> = OnceLock::new();

pub fn set_askpass_program(path: PathBuf) {
    let _ = ASKPASS_PROGRAM.set(path);
}

/// How long a non-interactive command (`serve --daemonize`, a check) may run.
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// The most stderr kept from one `ssh` process, for error messages.
const STDERR_LIMIT: usize = 8 * 1024;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Windows' own OpenSSH client when present (it uses the "OpenSSH
/// Authentication Agent" service, spec 8.2 step 2); otherwise `ssh` on PATH.
pub fn ssh_program() -> PathBuf {
    let system = std::env::var_os("SystemRoot").map(|root| PathBuf::from(root).join("System32").join("OpenSSH").join("ssh.exe"));
    system.filter(|p| p.is_file()).unwrap_or_else(|| PathBuf::from("ssh"))
}

/// `ssh -T -o ConnectTimeout=10 [sign-in options] [extra options] <host> <remote command>`.
///
/// With a key (no password), `BatchMode=yes` makes ssh fail instead of
/// prompting for a password, passphrase or host key confirmation, because no
/// terminal is attached. With a password, ssh asks jojod for it instead (see
/// `password_sign_in`).
pub fn ssh_command(target: &SshTarget, options: &[&str], remote: &str) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(ssh_program());
    // -T and RequestTTY=no: a PTY would rewrite the binary protocol's bytes
    // (a `RequestTTY force` in ~/.ssh/config would otherwise apply).
    // RemoteCommand=none: ssh refuses a command line when ~/.ssh/config sets
    // a RemoteCommand for the host.
    cmd.args(["-T", "-o", "RequestTTY=no", "-o", "RemoteCommand=none", "-o", "ConnectTimeout=10"]);
    for arg in sign_in_args(target) {
        cmd.arg(arg);
    }
    if let Some(password) = &target.password {
        password_sign_in(&mut cmd, password);
    }
    for option in options {
        cmd.args(["-o", option]);
    }
    cmd.arg("--").arg(&target.host).arg(remote);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// The arguments that select how ssh signs in to `target`: the port, the key
/// file, and either `BatchMode=yes` (keys only) or the password settings.
pub fn sign_in_args(target: &SshTarget) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(port) = target.port {
        args.extend(["-p".to_owned(), port.to_string()]);
    }
    if let Some(file) = &target.identity_file {
        args.extend(["-i".to_owned(), file.display().to_string(), "-o".to_owned(), "IdentitiesOnly=yes".to_owned()]);
    }
    if target.password.is_some() {
        // One attempt, password methods only (offering agent keys first can
        // use up the server's attempt limit), and a new host key accepted
        // and recorded in known_hosts instead of asked about.
        for option in [
            "BatchMode=no",
            "NumberOfPasswordPrompts=1",
            "PreferredAuthentications=keyboard-interactive,password",
            "PubkeyAuthentication=no",
            "StrictHostKeyChecking=accept-new",
        ] {
            args.extend(["-o".to_owned(), option.to_owned()]);
        }
    } else {
        args.extend(["-o".to_owned(), "BatchMode=yes".to_owned()]);
    }
    args
}

/// Makes ssh get the password from jojod: `SSH_ASKPASS` names the bundled
/// jojod, `SSH_ASKPASS_REQUIRE=force` makes ssh use it although no terminal
/// is attached (OpenSSH 8.4 and later; `DISPLAY` is set for older clients,
/// which use the helper only when it is), and the password travels in
/// jojod's environment, set on this ssh process only.
fn password_sign_in(cmd: &mut tokio::process::Command, password: &str) {
    if let Some(program) = ASKPASS_PROGRAM.get() {
        cmd.env("SSH_ASKPASS", program);
    }
    cmd.env("SSH_ASKPASS_REQUIRE", "force")
        .env("DISPLAY", std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_owned()))
        .env(jojod::askpass::ASKPASS_VAR, "1")
        .env(jojod::askpass::PASSWORD_VAR, password);
}

/// The output of a finished remote command.
#[derive(Debug)]
pub struct CommandOutput {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Runs `remote` on `target` with `input` on its stdin and waits up to
/// `COMMAND_TIMEOUT` for it to finish.
pub async fn run(target: &SshTarget, remote: &str, input: &[u8]) -> io::Result<CommandOutput> {
    run_with_timeout(target, remote, input, COMMAND_TIMEOUT).await
}

/// `run` with its own time limit (the daemon upload scales it with size).
pub async fn run_with_timeout(target: &SshTarget, remote: &str, input: &[u8], timeout: Duration) -> io::Result<CommandOutput> {
    let host = &target.host;
    let mut child = ssh_command(target, &[], &with_exit_marker(remote)).spawn()?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    let input = input.to_vec();
    let writer = tokio::spawn(async move {
        let _ = tokio::io::AsyncWriteExt::write_all(&mut stdin, &input).await;
        // Dropping stdin closes it, so `cat > file` on the remote side ends.
    });
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, format!("ssh {host} did not finish within {} s", timeout.as_secs())))??;
    let _ = writer.await;
    let (stderr, remote_code) = take_exit_marker(&String::from_utf8_lossy(&output.stderr));
    Ok(CommandOutput {
        code: remote_code.or(output.status.code()),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr,
    })
}

/// The stderr line on which the remote shell reports `remote`'s exit status.
const EXIT_MARKER: &str = "JOJO_EXIT_STATUS=";

/// Runs `remote` in a subshell and then prints its exit status to stderr.
/// Some hosts do not pass the remote exit status back to ssh: on RunCloud
/// servers (SHELL=/bin/rc-shell) ssh exits with 0 even after `exit 3`, so a
/// missing `jojod` (status 127) would look installed. The subshell keeps an
/// `exec` inside `remote` from skipping the report.
fn with_exit_marker(remote: &str) -> String {
    format!("({remote}); echo \"{EXIT_MARKER}$?\" >&2")
}

/// Removes the exit-status line that `with_exit_marker` adds and returns the
/// remaining stderr and the status. The status is None when the line is
/// missing, for example when ssh failed before the command ran (code 255);
/// the caller then uses ssh's own exit code.
fn take_exit_marker(stderr: &str) -> (String, Option<i32>) {
    let mut code = None;
    let mut kept = String::with_capacity(stderr.len());
    for line in stderr.split_inclusive('\n') {
        match line.trim_end().strip_prefix(EXIT_MARKER).map(str::parse::<i32>) {
            Some(Ok(status)) => code = Some(status),
            _ => kept.push_str(line),
        }
    }
    (kept, code)
}

/// True for ssh's own authentication failure (spec 8.3 rule 5):
/// `user@host: Permission denied (publickey).`, with exit code 255.
pub fn is_auth_failure(code: Option<i32>, stderr: &str) -> bool {
    code == Some(255) && is_ssh_permission_denied(stderr)
}

/// ssh's "Permission denied (<methods>)", as opposed to a remote file's
/// "Permission denied" (`bash: .../jojod: Permission denied`, or jojod's
/// `Permission denied (os error 13)`), which no key can fix.
fn is_ssh_permission_denied(stderr: &str) -> bool {
    stderr.lines().any(|line| {
        line.split("Permission denied (").nth(1).is_some_and(|rest| {
            ["publickey", "password", "keyboard-interactive", "gssapi", "hostbased"].iter().any(|method| rest.starts_with(method))
        })
    })
}

/// The reading half of a connection: ssh's stdout, which owns the `ssh`
/// process so that dropping the connection ends it.
struct ChildReader {
    stdout: ChildStdout,
    _child: Child,
}

impl AsyncRead for ChildReader {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stdout).poll_read(cx, buf)
    }
}

/// Why one `attach` attempt failed.
enum AttachFailure {
    Connect(ConnectError),
    /// The remote `jojod attach` could not reach a daemon (exit code 1).
    NoDaemon,
}

async fn attach(target: &SshTarget) -> Result<Client, AttachFailure> {
    let host = &target.host;
    let options = ["ServerAliveInterval=15", "ServerAliveCountMax=3"];
    let mut child = ssh_command(target, &options, &format!("{REMOTE_JOJOD} attach --stdio"))
        .spawn()
        .map_err(|e| AttachFailure::Connect(ConnectError::Io(io::Error::new(e.kind(), format!("could not run ssh: {e}")))))?;
    let stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    // Collected in the background so a chatty ssh never blocks on a full
    // stderr pipe; read when the attempt fails.
    let collected = Arc::new(Mutex::new(String::new()));
    let sink = collected.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        while let Ok(n) = stderr.read(&mut buf).await {
            if n == 0 {
                break;
            }
            let mut text = sink.lock().unwrap();
            if text.len() < STDERR_LIMIT {
                text.push_str(&String::from_utf8_lossy(&buf[..n]));
            }
        }
    });
    let reader = ChildReader { stdout, _child: child };
    // ssh's ConnectTimeout bounds the network part; the Hello timeout then
    // bounds the exchange itself.
    match Client::handshake(Box::new(reader), Box::new(stdin), Duration::from_secs(10) + HELLO_TIMEOUT).await {
        Ok(client) => Ok(client),
        Err(e) => {
            if let Some(daemon) = is_protocol_mismatch(&e) {
                return Err(AttachFailure::Connect(ConnectError::Mismatch { daemon }));
            }
            // The handshake ends when ssh exits; give its stderr a moment to arrive.
            tokio::time::sleep(Duration::from_millis(200)).await;
            let stderr = collected.lock().unwrap().trim().to_owned();
            Err(classify(host, &stderr, e))
        }
    }
}

/// Sorts a failed attach by what ssh and the remote shell printed. The exit
/// code is not available here (the reader owned the process), so the
/// messages decide: ssh's "Permission denied", the remote shell's report of
/// a missing jojod, or jojod's own failure to reach its socket.
fn classify(host: &str, stderr: &str, handshake: io::Error) -> AttachFailure {
    if is_ssh_permission_denied(stderr) {
        return AttachFailure::Connect(ConnectError::AuthFailed(stderr.to_owned()));
    }
    let lower = stderr.to_lowercase();
    if lower.contains(".local/bin/jojod") && (lower.contains("no such file") || lower.contains("not found")) {
        return AttachFailure::Connect(ConnectError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            format!("jojod is not installed on {host}; remove the remote location and add it again"),
        )));
    }
    // jojod reports a missing or refusing socket as "Error: ... (os error 2)"
    // or "(os error 111)"; ssh's own connection errors start with "ssh:".
    if !stderr.contains("ssh:") && (stderr.contains("(os error 2)") || stderr.contains("(os error 111)")) {
        return AttachFailure::NoDaemon;
    }
    let detail = if stderr.is_empty() { handshake.to_string() } else { stderr.to_owned() };
    AttachFailure::Connect(ConnectError::Io(io::Error::other(format!("ssh {host}: {detail}"))))
}

/// The connect step of a remote machine's connection loop: attaches, and
/// when no daemon is running (the server rebooted, or it was shut down)
/// starts one with `serve --daemonize` and attaches again.
pub async fn connect(target: &SshTarget) -> Result<Client, ConnectError> {
    let host = &target.host;
    match attach(target).await {
        Ok(client) => return Ok(client),
        Err(AttachFailure::Connect(e)) => return Err(e),
        Err(AttachFailure::NoDaemon) => {}
    }
    let started = run(target, &start_daemon_command(), b"").await.map_err(ConnectError::Io)?;
    if is_auth_failure(started.code, &started.stderr) {
        return Err(ConnectError::AuthFailed(started.stderr.trim().to_owned()));
    }
    if started.code != Some(0) {
        return Err(ConnectError::Io(io::Error::other(format!("could not start jojod on {host}: {}", started.stderr.trim()))));
    }
    match attach(target).await {
        Ok(client) => Ok(client),
        Err(AttachFailure::Connect(e)) => Err(e),
        // `serve --daemonize` reported a running daemon, yet this new
        // connection finds none: the daemon ended with the connection that
        // started it.
        Err(AttachFailure::NoDaemon) => Err(ConnectError::Unsupported(daemon_does_not_persist(target).await)),
    }
}

/// Reports what the server looks like to a new connection: whether
/// `/.dockerenv` exists, the name of process 1 (Linux only) and the user's
/// login shell, each on its own `key=value` line.
const SERVER_PROBE: &str = "[ -f /.dockerenv ] && echo dockerenv=yes; [ -r /proc/1/comm ] && echo \"pid1=$(cat /proc/1/comm)\"; echo \"shell=$SHELL\"";

/// Whether the probe's output shows a container: a `/.dockerenv` file, or a
/// process 1 that is not a system's init (systemd, init, launchd, runit,
/// openrc-init). A container started for one connection has `bash` or
/// `sshd` as process 1.
pub fn probe_shows_container(stdout: &str) -> bool {
    stdout.lines().any(|line| match line.trim().split_once('=') {
        Some(("dockerenv", "yes")) => true,
        Some(("pid1", name)) => !["systemd", "init", "launchd", "runit", "openrc-init"].contains(&name.trim()),
        _ => false,
    })
}

/// The login shell the probe reported, when there is one.
fn probe_shell(stdout: &str) -> Option<&str> {
    stdout.lines().find_map(|line| line.trim().strip_prefix("shell=")).filter(|s| !s.is_empty())
}

/// The explanation shown when a daemon started on `host` is gone by the
/// next SSH connection. It names the cause when the probe shows a container
/// and the RunCloud fix when the login shell is RunCloud's jailed shell.
pub fn unsupported_message(host: &str, probe: &str) -> String {
    let fix = match probe_shell(probe) {
        Some(s) if s.ends_with("rc-shell") => format!(
            "The login shell is {s}, RunCloud's jailed shell. In RunCloud, open the server's System Users and change this user's shell to /bin/bash, then Reconnect."
        ),
        Some(s) => format!("Change this user's login shell (now {s}) to /bin/bash or another ordinary shell, then Reconnect."),
        None => "Change this user's login shell to /bin/bash or another ordinary shell, then Reconnect.".to_owned(),
    };
    if probe_shows_container(probe) {
        format!("{host} runs each SSH connection in its own temporary container, so the Jojo daemon and its sessions end when the connection that started them closes. {fix}")
    } else {
        format!("jojod on {host} ends when the SSH connection that started it closes, so sessions cannot keep running there. {fix}")
    }
}

/// Probes `target` over a new connection and returns `unsupported_message`.
/// A probe that cannot run gives the message without the container detail.
pub async fn daemon_does_not_persist(target: &SshTarget) -> String {
    let probe = run(target, SERVER_PROBE, b"").await.map(|out| out.stdout).unwrap_or_default();
    unsupported_message(&target.host, &probe)
}

/// Whether a daemon is reachable over a new connection. `jojod attach
/// --stdio` with stdin closed connects, reaches the end of its input and
/// exits 0, or exits 1 with jojod's "os error 2" (no socket) or "os error
/// 111" (nothing listening). Some(false) only for those two errors; None
/// when the check itself failed (an ssh error, a timeout).
pub async fn daemon_reachable(target: &SshTarget) -> Option<bool> {
    let out = run(target, &format!("{REMOTE_JOJOD} attach --stdio"), b"").await.ok()?;
    if out.code == Some(0) {
        return Some(true);
    }
    let no_daemon = !out.stderr.contains("ssh:") && (out.stderr.contains("(os error 2)") || out.stderr.contains("(os error 111)"));
    no_daemon.then_some(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(failure: AttachFailure) -> &'static str {
        match failure {
            AttachFailure::Connect(ConnectError::AuthFailed(_)) => "auth",
            AttachFailure::Connect(ConnectError::Io(e)) if e.kind() == io::ErrorKind::NotFound => "not installed",
            AttachFailure::Connect(_) => "other",
            AttachFailure::NoDaemon => "no daemon",
        }
    }

    #[test]
    fn exit_marker_gives_the_remote_status_and_is_removed_from_stderr() {
        assert_eq!(with_exit_marker("jojod --version"), "(jojod --version); echo \"JOJO_EXIT_STATUS=$?\" >&2");
        let (stderr, code) = take_exit_marker("bash: jojod: No such file or directory\nJOJO_EXIT_STATUS=127\n");
        assert_eq!((stderr.as_str(), code), ("bash: jojod: No such file or directory\n", Some(127)));
        // ssh failed before the command ran: no marker, ssh's own code is used.
        let (stderr, code) = take_exit_marker("user@host: Permission denied (publickey).\r\n");
        assert_eq!((stderr.as_str(), code), ("user@host: Permission denied (publickey).\r\n", None));
    }

    #[test]
    fn login_shell_command_falls_back_when_shell_is_missing() {
        let cmd = start_daemon_command();
        // $SHELL is used only when it is an executable file (RunCloud sets it to /bin/rc-shell, which does not exist).
        assert!(cmd.starts_with("s=\"$SHELL\"; [ -x \"$s\" ] || s=$(command -v bash) || s=/bin/sh;"), "{cmd}");
        assert!(cmd.ends_with("exec \"$s\" -lc '~/.local/bin/jojod serve --daemonize'"), "{cmd}");
    }

    #[test]
    fn probe_output_shows_a_container_by_dockerenv_or_process_1() {
        assert!(probe_shows_container("dockerenv=yes\npid1=bash\nshell=/bin/rc-shell\n"));
        assert!(probe_shows_container("pid1=bash\nshell=/bin/bash\n"));
        assert!(!probe_shows_container("pid1=systemd\nshell=/bin/bash\n"));
        // macOS has no /proc: only the shell line.
        assert!(!probe_shows_container("shell=/bin/zsh\n"));
        assert!(!probe_shows_container(""));
    }

    #[test]
    fn unsupported_message_names_the_container_and_the_runcloud_fix() {
        let runcloud = unsupported_message("web1", "dockerenv=yes\npid1=bash\nshell=/bin/rc-shell\n");
        assert!(runcloud.starts_with("web1 runs each SSH connection in its own temporary container"), "{runcloud}");
        assert!(runcloud.contains("RunCloud's jailed shell"), "{runcloud}");
        let other = unsupported_message("box", "pid1=systemd\nshell=/usr/bin/fish\n");
        assert!(other.starts_with("jojod on box ends when the SSH connection"), "{other}");
        assert!(other.contains("(now /usr/bin/fish)"), "{other}");
        let unknown = unsupported_message("box", "");
        assert!(unknown.contains("Change this user's login shell to /bin/bash"), "{unknown}");
    }

    #[test]
    fn sorts_attach_failures_by_their_messages() {
        let eof = || io::Error::new(io::ErrorKind::UnexpectedEof, "eof");
        assert_eq!(kind(classify("h", "deploy@192.0.2.5: Permission denied (publickey).", eof())), "auth");
        assert_eq!(kind(classify("h", "bash: line 1: /home/j/.local/bin/jojod: No such file or directory", eof())), "not installed");
        assert_eq!(kind(classify("h", "Error: No such file or directory (os error 2)", eof())), "no daemon");
        assert_eq!(kind(classify("h", "Error: Connection refused (os error 111)", eof())), "no daemon");
        assert_eq!(kind(classify("h", "ssh: connect to host 10.0.0.1 port 22: Connection timed out", eof())), "other");
        assert_eq!(kind(classify("h", "bash: line 1: /home/u/.local/bin/jojod: Permission denied", eof())), "other");
        assert_eq!(kind(classify("h", "Error: Permission denied (os error 13)", eof())), "other");
        assert_eq!(kind(classify("h", "zsh:1: no such file or directory: /home/u/.local/bin/jojod", eof())), "not installed");
        assert_eq!(kind(classify("h", "ssh: connect to host 10.0.0.1 port 22: Connection refused", eof())), "other");
    }

    #[test]
    fn sign_in_arguments_follow_the_target() {
        let alias = SshTarget::host("build-box");
        assert_eq!(sign_in_args(&alias), ["-o", "BatchMode=yes"]);
        let key = SshTarget { host: "deploy@192.0.2.5".into(), port: Some(2222), identity_file: Some(PathBuf::from("/k/jojo_ed25519")), password: None };
        let args = sign_in_args(&key).join(" ");
        assert!(args.contains("-p 2222") && args.contains("IdentitiesOnly=yes") && args.contains("BatchMode=yes"), "{args}");
        let password = SshTarget { password: Some("s3cret".into()), identity_file: None, ..key };
        let args = sign_in_args(&password).join(" ");
        assert!(args.contains("BatchMode=no") && args.contains("PubkeyAuthentication=no") && args.contains("StrictHostKeyChecking=accept-new"), "{args}");
        assert!(!args.contains("s3cret"), "the password is never an argument: {args}");
        assert!(!format!("{password:?}").contains("s3cret"));
    }

    #[test]
    fn recognizes_ssh_auth_failures() {
        assert!(is_auth_failure(Some(255), "user@host: Permission denied (publickey)."));
        assert!(!is_auth_failure(Some(1), "Permission denied"));
        assert!(!is_auth_failure(Some(255), "Connection timed out"));
        assert!(!is_auth_failure(Some(255), "Error: Permission denied (os error 13)"));
    }
}
