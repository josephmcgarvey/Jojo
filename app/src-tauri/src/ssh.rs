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
use std::sync::{Arc, Mutex};
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

/// `ssh -o BatchMode=yes -o ConnectTimeout=10 [extra options] <host> <remote command>`.
/// BatchMode makes ssh fail instead of prompting for a password, passphrase
/// or host key confirmation, because no terminal is attached.
pub fn ssh_command(host: &str, options: &[&str], remote: &str) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(ssh_program());
    // -T and RequestTTY=no: a PTY would rewrite the binary protocol's bytes
    // (a `RequestTTY force` in ~/.ssh/config would otherwise apply).
    // RemoteCommand=none: ssh refuses a command line when ~/.ssh/config sets
    // a RemoteCommand for the host.
    cmd.args(["-T", "-o", "RequestTTY=no", "-o", "RemoteCommand=none", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10"]);
    for option in options {
        cmd.args(["-o", option]);
    }
    cmd.arg("--").arg(host).arg(remote);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// The output of a finished remote command.
#[derive(Debug)]
pub struct CommandOutput {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Runs `remote` on `host` with `input` on its stdin and waits up to
/// `COMMAND_TIMEOUT` for it to finish.
pub async fn run(host: &str, remote: &str, input: &[u8]) -> io::Result<CommandOutput> {
    run_with_timeout(host, remote, input, COMMAND_TIMEOUT).await
}

/// `run` with its own time limit (the daemon upload scales it with size).
pub async fn run_with_timeout(host: &str, remote: &str, input: &[u8], timeout: Duration) -> io::Result<CommandOutput> {
    let mut child = ssh_command(host, &[], &with_exit_marker(remote)).spawn()?;
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
    NoDaemon(String),
}

async fn attach(host: &str) -> Result<Client, AttachFailure> {
    let options = ["ServerAliveInterval=15", "ServerAliveCountMax=3"];
    let mut child = ssh_command(host, &options, &format!("{REMOTE_JOJOD} attach --stdio"))
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
            format!("jojod is not installed on {host}; remove the machine and add it again"),
        )));
    }
    // jojod reports a missing or refusing socket as "Error: ... (os error 2)"
    // or "(os error 111)"; ssh's own connection errors start with "ssh:".
    if !stderr.contains("ssh:") && (stderr.contains("(os error 2)") || stderr.contains("(os error 111)")) {
        return AttachFailure::NoDaemon(stderr.to_owned());
    }
    let detail = if stderr.is_empty() { handshake.to_string() } else { stderr.to_owned() };
    AttachFailure::Connect(ConnectError::Io(io::Error::other(format!("ssh {host}: {detail}"))))
}

/// The connect step of a remote machine's connection loop: attaches, and
/// when no daemon is running (the server rebooted, or it was shut down)
/// starts one with `serve --daemonize` and attaches again.
pub async fn connect(host: &str) -> Result<Client, ConnectError> {
    match attach(host).await {
        Ok(client) => return Ok(client),
        Err(AttachFailure::Connect(e)) => return Err(e),
        Err(AttachFailure::NoDaemon(_)) => {}
    }
    let started = run(host, &start_daemon_command(), b"").await.map_err(ConnectError::Io)?;
    if is_auth_failure(started.code, &started.stderr) {
        return Err(ConnectError::AuthFailed(started.stderr.trim().to_owned()));
    }
    if started.code != Some(0) {
        return Err(ConnectError::Io(io::Error::other(format!("could not start jojod on {host}: {}", started.stderr.trim()))));
    }
    match attach(host).await {
        Ok(client) => Ok(client),
        Err(AttachFailure::Connect(e)) => Err(e),
        Err(AttachFailure::NoDaemon(stderr)) => Err(ConnectError::Io(io::Error::other(format!("jojod on {host} is not accepting connections: {stderr}")))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(failure: AttachFailure) -> &'static str {
        match failure {
            AttachFailure::Connect(ConnectError::AuthFailed(_)) => "auth",
            AttachFailure::Connect(ConnectError::Io(e)) if e.kind() == io::ErrorKind::NotFound => "not installed",
            AttachFailure::Connect(_) => "other",
            AttachFailure::NoDaemon(_) => "no daemon",
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
    fn sorts_attach_failures_by_their_messages() {
        let eof = || io::Error::new(io::ErrorKind::UnexpectedEof, "eof");
        assert_eq!(kind(classify("h", "jmcgarvey@100.100.0.5: Permission denied (publickey).", eof())), "auth");
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
    fn recognizes_ssh_auth_failures() {
        assert!(is_auth_failure(Some(255), "user@host: Permission denied (publickey)."));
        assert!(!is_auth_failure(Some(1), "Permission denied"));
        assert!(!is_auth_failure(Some(255), "Connection timed out"));
        assert!(!is_auth_failure(Some(255), "Error: Permission denied (os error 13)"));
    }
}
