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
    cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10"]);
    for option in options {
        cmd.args(["-o", option]);
    }
    cmd.arg(host).arg(remote);
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
    let mut child = ssh_command(host, &[], remote).spawn()?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    let input = input.to_vec();
    let writer = tokio::spawn(async move {
        let _ = tokio::io::AsyncWriteExt::write_all(&mut stdin, &input).await;
        // Dropping stdin closes it, so `cat > file` on the remote side ends.
    });
    let output = tokio::time::timeout(COMMAND_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, format!("ssh {host} did not finish within {} s", COMMAND_TIMEOUT.as_secs())))??;
    let _ = writer.await;
    Ok(CommandOutput {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// True for ssh's own authentication failure (spec 8.3 rule 5).
pub fn is_auth_failure(code: Option<i32>, stderr: &str) -> bool {
    code == Some(255) && stderr.contains("Permission denied")
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
    if stderr.contains("Permission denied") {
        return AttachFailure::Connect(ConnectError::AuthFailed(stderr.to_owned()));
    }
    if stderr.contains(".local/bin/jojod") && (stderr.contains("No such file") || stderr.contains("not found")) {
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
    let started = run(host, &format!("{REMOTE_JOJOD} serve --daemonize"), b"").await.map_err(ConnectError::Io)?;
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
    fn sorts_attach_failures_by_their_messages() {
        let eof = || io::Error::new(io::ErrorKind::UnexpectedEof, "eof");
        assert_eq!(kind(classify("h", "jmcgarvey@100.100.0.5: Permission denied (publickey).", eof())), "auth");
        assert_eq!(kind(classify("h", "bash: line 1: /home/j/.local/bin/jojod: No such file or directory", eof())), "not installed");
        assert_eq!(kind(classify("h", "Error: No such file or directory (os error 2)", eof())), "no daemon");
        assert_eq!(kind(classify("h", "Error: Connection refused (os error 111)", eof())), "no daemon");
        assert_eq!(kind(classify("h", "ssh: connect to host 10.0.0.1 port 22: Connection timed out", eof())), "other");
        assert_eq!(kind(classify("h", "ssh: connect to host 10.0.0.1 port 22: Connection refused", eof())), "other");
    }

    #[test]
    fn recognizes_ssh_auth_failures() {
        assert!(is_auth_failure(Some(255), "user@host: Permission denied (publickey)."));
        assert!(!is_auth_failure(Some(1), "Permission denied"));
        assert!(!is_auth_failure(Some(255), "Connection timed out"));
    }
}
