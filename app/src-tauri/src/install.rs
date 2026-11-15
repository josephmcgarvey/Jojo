//! Adding a Linux server (spec 8.2): the install steps, run over SSH and
//! reported one by one.

use crate::ssh::{is_auth_failure, run, run_with_timeout, start_daemon_command, SshTarget, REMOTE_JOJOD};
use serde::Serialize;
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// One step's outcome, sent to the Add machine dialog as it happens.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
pub struct InstallStep {
    /// "connect", "key", "platform", "install", "start", "linger" or "hooks".
    pub step: String,
    /// "running", "ok", "warning" or "failed".
    pub status: String,
    pub message: String,
}

fn step(step: &str, status: &str, message: impl Into<String>) -> InstallStep {
    InstallStep { step: step.into(), status: status.into(), message: message.into() }
}

/// The bundled daemon for a `uname -sm` result (spec 8.2 step 3).
pub fn binary_for_platform(uname: &str) -> Option<&'static str> {
    match uname.trim() {
        "Linux x86_64" => Some("jojod-linux-x86_64"),
        "Linux aarch64" | "Linux arm64" => Some("jojod-linux-aarch64"),
        _ => None,
    }
}

/// The version line jojod prints for `--version`: `jojod 0.1.0 (protocol 3)`.
pub fn bundled_version_line() -> String {
    format!("jojod {} (protocol {})", jojod::VERSION, jojo_proto::PROTOCOL_VERSION)
}

/// The host aliases in `~/.ssh/config` (spec 8.2 step 1): every name on a
/// `Host` line, except patterns containing `*`, `?` or `!`.
pub fn ssh_config_hosts(config: &str) -> Vec<String> {
    let mut hosts = Vec::new();
    for line in config.lines() {
        let line = line.trim();
        let mut words = line.split_whitespace();
        if !words.next().is_some_and(|w| w.eq_ignore_ascii_case("host")) {
            continue;
        }
        for name in words {
            if !name.contains(['*', '?', '!']) && !hosts.iter().any(|h| h == name) {
                hosts.push(name.to_owned());
            }
        }
    }
    hosts
}

pub fn default_ssh_config() -> Option<PathBuf> {
    crate::platform::home_dir().map(|home| home.join(".ssh").join("config"))
}

/// The protocol number in a `jojod --version` line, `jojod 0.1.0 (protocol 3)`.
pub fn protocol_of(version_line: &str) -> Option<u32> {
    version_line.rsplit_once("(protocol ")?.1.trim().strip_suffix(')')?.parse().ok()
}

/// The message returned when installing would end a running daemon of
/// another protocol and the caller did not allow it. The Add machine dialog
/// recognizes the prefix and offers to install anyway.
pub const RESTART_NEEDED: &str = "RESTART_NEEDED: ";

/// Runs spec 8.2 steps 2 to 7 against `host`, calling `report` before and
/// after each step. Returns an error message when a step fails; warnings
/// (lingering off, Claude Code missing) do not stop the install.
///
/// When another version is installed: with the same protocol, the new
/// binary replaces the old file and the running daemon keeps running (and
/// its sessions with it); the new binary runs from the next start. With
/// another protocol, the running daemon must end, which ends every session
/// on the server, so that happens only with `allow_restart` (spec 8.4);
/// otherwise the install stops with `RESTART_NEEDED`.
pub async fn install(target: &SshTarget, daemon_dir: &Path, allow_restart: bool, mut report: impl FnMut(InstallStep)) -> Result<(), String> {
    let host = target.host.as_str();
    // Step 2: connectivity, with BatchMode (no prompts).
    report(step("connect", "running", format!("ssh {host} uname -sm")));
    let uname = run(target, "uname -sm", b"").await.map_err(|e| fail(&mut report, "connect", e.to_string()))?;
    if uname.code != Some(0) {
        let message = if is_auth_failure(uname.code, &uname.stderr) && target.password.is_some() {
            format!("The server refused the username or password: {}", uname.stderr.trim())
        } else if is_auth_failure(uname.code, &uname.stderr) {
            format!(
                "SSH refused the key: {}. Load the key into the OpenSSH Authentication Agent with ssh-add, or run `ssh {host}` once in a terminal to accept the host key, then try again.",
                uname.stderr.trim()
            )
        } else {
            format!("{}. Run `ssh {host}` once in a terminal to check the connection, then try again.", uname.stderr.trim())
        };
        return Err(fail(&mut report, "connect", message));
    }
    report(step("connect", "ok", "Connected"));

    // Step 3: platform.
    let platform = uname.stdout.trim().to_owned();
    let Some(binary) = binary_for_platform(&platform) else {
        return Err(fail(&mut report, "platform", format!("Unsupported platform: {platform}")));
    };
    let local_binary = daemon_dir.join(binary);
    if !local_binary.is_file() {
        return Err(fail(&mut report, "platform", format!("{binary} is not bundled with this Jojo build")));
    }
    report(step("platform", "ok", platform));

    // Step 4: install or upgrade.
    report(step("install", "running", format!("Checking {REMOTE_JOJOD}")));
    let installed = run(target, &format!("{REMOTE_JOJOD} --version"), b"").await.map_err(|e| fail(&mut report, "install", e.to_string()))?;
    let wanted = bundled_version_line();
    let current = installed.stdout.trim().to_owned();
    if installed.code == Some(0) && current == wanted {
        report(step("install", "ok", format!("{wanted} is installed")));
    } else {
        let other_protocol = installed.code == Some(0) && protocol_of(&current) != Some(jojo_proto::PROTOCOL_VERSION);
        if other_protocol {
            if !allow_restart {
                return Err(fail(
                    &mut report,
                    "install",
                    format!("{RESTART_NEEDED}{host} runs {current}. Installing {wanted} ends its daemon and every session running on {host}."),
                ));
            }
            // Its daemon (if running) speaks another protocol and must end
            // before the new one starts; `shutdown` with the old binary.
            let _ = run(target, &format!("{REMOTE_JOJOD} shutdown"), b"").await;
        }
        let bytes = std::fs::read(&local_binary).map_err(|e| fail(&mut report, "install", format!("cannot read {binary}: {e}")))?;
        report(step("install", "running", format!("Uploading {binary} ({} KB)", bytes.len() / 1024)));
        // Streamed over stdin, so the server needs neither scp nor SFTP;
        // `mv` replaces the binary atomically.
        let script = "mkdir -p ~/.local/bin && cat > ~/.local/bin/jojod.new && chmod +x ~/.local/bin/jojod.new && mv ~/.local/bin/jojod.new ~/.local/bin/jojod";
        // 30 s plus 32 KB/s: 4 MB gets about two and a half minutes, enough
        // for a slow uplink.
        let limit = std::time::Duration::from_secs(30 + bytes.len() as u64 / (32 * 1024));
        let upload = run_with_timeout(target, script, &bytes, limit).await.map_err(|e| fail(&mut report, "install", e.to_string()))?;
        if upload.code != Some(0) {
            return Err(fail(&mut report, "install", format!("upload failed: {}", upload.stderr.trim())));
        }
        report(step("install", "ok", if current.is_empty() { format!("Installed {wanted}") } else { format!("Upgraded {current} to {wanted}") }));
    }

    // Step 5: start (a running daemon of the same version is kept).
    report(step("start", "running", "Starting jojod"));
    let started = run(target, &start_daemon_command(), b"").await.map_err(|e| fail(&mut report, "start", e.to_string()))?;
    if started.code != Some(0) {
        return Err(fail(&mut report, "start", format!("jojod did not start: {}", started.stderr.trim())));
    }
    report(step("start", "ok", "jojod is running"));

    // Step 6: logout survival.
    let linger = run(target, "loginctl show-user \"$USER\" -p Linger 2>/dev/null", b"").await;
    match linger {
        Ok(out) if out.stdout.contains("Linger=yes") => report(step("linger", "ok", "Sessions survive logout")),
        Ok(out) if out.stdout.contains("Linger=no") => report(step(
            "linger",
            "warning",
            "Lingering is off. If this server ends a user's processes at logout, every session on it ends when your last SSH session closes. To keep them, run on the server: sudo loginctl enable-linger $USER",
        )),
        _ => report(step("linger", "ok", "No systemd login manager; processes are not ended at logout")),
    }

    // Step 7: hooks.
    // Through the login shell, like the daemon, so `command -v claude` sees
    // the PATH sessions will get.
    let check = format!("{REMOTE_JOJOD} install-hooks >/dev/null && (command -v claude >/dev/null || echo NO_CLAUDE)");
    let hooks = run(target, &crate::ssh::login_shell_command(&check), b"")
        .await
        .map_err(|e| fail(&mut report, "hooks", e.to_string()))?;
    if hooks.code != Some(0) {
        report(step("hooks", "warning", format!("Claude Code hooks were not registered: {}", hooks.stderr.trim())));
    } else if hooks.stdout.contains("NO_CLAUDE") {
        report(step("hooks", "warning", "Claude Code is not installed on this server; shell sessions work, and Claude sessions need it installed"));
    } else {
        report(step("hooks", "ok", "Claude Code hooks registered"));
    }
    Ok(())
}

/// Where Jojo keeps the key it installs on servers added with a username
/// and password: `~/.ssh/jojo_ed25519` (and `.pub`).
pub fn default_jojo_key() -> Option<PathBuf> {
    crate::platform::home_dir().map(|home| home.join(".ssh").join("jojo_ed25519"))
}

/// Windows' own ssh-keygen when present (next to the ssh that Jojo runs),
/// otherwise `ssh-keygen` on PATH.
fn ssh_keygen_program() -> PathBuf {
    let ssh = crate::ssh::ssh_program();
    match ssh.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        Some(dir) if dir.join("ssh-keygen.exe").is_file() => dir.join("ssh-keygen.exe"),
        _ => PathBuf::from("ssh-keygen"),
    }
}

/// Appends the public key read from stdin to `~/.ssh/authorized_keys`
/// unless that exact line is already there, creating the folder and file
/// with owner-only permissions (`umask 077`), as `ssh-copy-id` does.
pub const ADD_AUTHORIZED_KEY: &str = r#"umask 077; mkdir -p ~/.ssh && touch ~/.ssh/authorized_keys && k=$(cat) && { grep -qxF "$k" ~/.ssh/authorized_keys || printf '%s\n' "$k" >> ~/.ssh/authorized_keys; }"#;

/// For a server added with a username and password: makes sure Jojo's key
/// (`key`, created with ssh-keygen when missing, without a passphrase)
/// exists, adds its public key to the server's authorized_keys over the
/// password connection `with_password`, and signs in once with the key
/// alone. Returns the key target to use from then on, or why the key could
/// not be used (the caller then keeps signing in with the password).
pub async fn setup_key(with_password: &SshTarget, key: &Path, mut report: impl FnMut(InstallStep)) -> Result<SshTarget, String> {
    let public = PathBuf::from(format!("{}.pub", key.display()));
    if !key.is_file() || !public.is_file() {
        report(step("key", "running", format!("Creating {}", key.display())));
        if let Some(dir) = key.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
        }
        let mut keygen = tokio::process::Command::new(ssh_keygen_program());
        keygen.args(["-q", "-t", "ed25519", "-N", "", "-C", "jojo", "-f"]).arg(key);
        #[cfg(windows)]
        keygen.creation_flags(0x0800_0000);
        let made = keygen.output().await.map_err(|e| format!("could not run ssh-keygen: {e}"))?;
        if !made.status.success() {
            return Err(format!("ssh-keygen failed: {}", String::from_utf8_lossy(&made.stderr).trim()));
        }
    }
    let line = std::fs::read_to_string(&public).map_err(|e| format!("could not read {}: {e}", public.display()))?;
    report(step("key", "running", "Adding the key to ~/.ssh/authorized_keys on the server"));
    let added = run(with_password, ADD_AUTHORIZED_KEY, line.trim().as_bytes()).await.map_err(|e| e.to_string())?;
    if added.code != Some(0) {
        return Err(format!("the key could not be added to ~/.ssh/authorized_keys: {}", added.stderr.trim()));
    }
    let with_key = SshTarget { host: with_password.host.clone(), port: with_password.port, identity_file: Some(key.to_path_buf()), password: None };
    let check = run(&with_key, "true", b"").await.map_err(|e| e.to_string())?;
    if check.code != Some(0) {
        return Err(format!("the server did not accept the key: {}", check.stderr.trim()));
    }
    report(step("key", "ok", format!("Signed in with {}; the password is not saved", key.display())));
    Ok(with_key)
}

fn fail(report: &mut impl FnMut(InstallStep), name: &str, message: String) -> String {
    report(step(name, "failed", message.clone()));
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_authorized_keys_script_adds_a_line_once() {
        // The key arrives on stdin, so no key text is quoted into the script.
        assert!(ADD_AUTHORIZED_KEY.starts_with("umask 077;"));
        assert!(ADD_AUTHORIZED_KEY.contains("grep -qxF \"$k\" ~/.ssh/authorized_keys ||"));
        assert!(ADD_AUTHORIZED_KEY.contains(r"printf '%s\n' "));
    }

    #[test]
    fn maps_platforms_to_bundled_binaries() {
        assert_eq!(binary_for_platform("Linux x86_64\n"), Some("jojod-linux-x86_64"));
        assert_eq!(binary_for_platform("Linux aarch64"), Some("jojod-linux-aarch64"));
        assert_eq!(binary_for_platform("Darwin arm64"), None);
        assert_eq!(binary_for_platform("Linux armv7l"), None);
    }

    #[test]
    fn lists_concrete_host_aliases_from_ssh_config() {
        let config = "Host build-box 192.0.2.5\n  HostName 192.0.2.5\n  User deploy\n\nHost *\n  ServerAliveInterval 30\nhost dev !bad\nMatch host x\n";
        assert_eq!(ssh_config_hosts(config), vec!["build-box", "192.0.2.5", "dev"]);
    }

    #[test]
    fn reads_the_protocol_from_a_version_line() {
        assert_eq!(protocol_of("jojod 0.1.0 (protocol 3)"), Some(3));
        assert_eq!(protocol_of("jojod 0.2.0 (protocol 12)\n"), Some(12));
        assert_eq!(protocol_of("bash: jojod: not found"), None);
    }

    #[test]
    fn the_version_line_matches_jojod_version_output() {
        assert_eq!(bundled_version_line(), format!("jojod {} (protocol {})", jojod::VERSION, jojo_proto::PROTOCOL_VERSION));
    }
}
