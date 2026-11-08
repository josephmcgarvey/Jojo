//! Adding a Linux server (spec 8.2): the install steps, run over SSH and
//! reported one by one.

use crate::ssh::{is_auth_failure, run, REMOTE_JOJOD};
use serde::Serialize;
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// One step's outcome, sent to the Add machine dialog as it happens.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
pub struct InstallStep {
    /// "connect", "platform", "install", "start", "linger" or "hooks".
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
    std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join(".ssh").join("config"))
}

/// Runs spec 8.2 steps 2 to 7 against `host`, calling `report` before and
/// after each step. Returns an error message when a step fails; warnings
/// (lingering off, Claude Code missing) do not stop the install.
pub async fn install(host: &str, daemon_dir: &Path, mut report: impl FnMut(InstallStep)) -> Result<(), String> {
    // Step 2: connectivity, with BatchMode (no prompts).
    report(step("connect", "running", format!("ssh {host} uname -sm")));
    let uname = run(host, "uname -sm", b"").await.map_err(|e| fail(&mut report, "connect", e.to_string()))?;
    if uname.code != Some(0) {
        let message = if is_auth_failure(uname.code, &uname.stderr) {
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
    let installed = run(host, &format!("{REMOTE_JOJOD} --version"), b"").await.map_err(|e| fail(&mut report, "install", e.to_string()))?;
    let wanted = bundled_version_line();
    let current = installed.stdout.trim().to_owned();
    if installed.code == Some(0) && current == wanted {
        report(step("install", "ok", format!("{wanted} is installed")));
    } else {
        if installed.code == Some(0) {
            // A different jojod is installed; its daemon (if running) speaks
            // another protocol and must end before the new one starts.
            let _ = run(host, &format!("{REMOTE_JOJOD} shutdown"), b"").await;
        }
        let bytes = std::fs::read(&local_binary).map_err(|e| fail(&mut report, "install", format!("cannot read {binary}: {e}")))?;
        report(step("install", "running", format!("Uploading {binary} ({} KB)", bytes.len() / 1024)));
        // Streamed over stdin, so the server needs neither scp nor SFTP;
        // `mv` replaces the binary atomically.
        let script = "mkdir -p ~/.local/bin && cat > ~/.local/bin/jojod.new && chmod +x ~/.local/bin/jojod.new && mv ~/.local/bin/jojod.new ~/.local/bin/jojod";
        let upload = run(host, script, &bytes).await.map_err(|e| fail(&mut report, "install", e.to_string()))?;
        if upload.code != Some(0) {
            return Err(fail(&mut report, "install", format!("upload failed: {}", upload.stderr.trim())));
        }
        report(step("install", "ok", if current.is_empty() { format!("Installed {wanted}") } else { format!("Upgraded {current} to {wanted}") }));
    }

    // Step 5: start (a running daemon of the same version is kept).
    report(step("start", "running", "Starting jojod"));
    let started = run(host, &format!("{REMOTE_JOJOD} serve --daemonize"), b"").await.map_err(|e| fail(&mut report, "start", e.to_string()))?;
    if started.code != Some(0) {
        return Err(fail(&mut report, "start", format!("jojod did not start: {}", started.stderr.trim())));
    }
    report(step("start", "ok", "jojod is running"));

    // Step 6: logout survival.
    let linger = run(host, "loginctl show-user \"$USER\" -p Linger 2>/dev/null", b"").await;
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
    let hooks = run(host, &format!("{REMOTE_JOJOD} install-hooks >/dev/null && (command -v claude >/dev/null || echo NO_CLAUDE)"), b"")
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

fn fail(report: &mut impl FnMut(InstallStep), name: &str, message: String) -> String {
    report(step(name, "failed", message.clone()));
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_platforms_to_bundled_binaries() {
        assert_eq!(binary_for_platform("Linux x86_64\n"), Some("jojod-linux-x86_64"));
        assert_eq!(binary_for_platform("Linux aarch64"), Some("jojod-linux-aarch64"));
        assert_eq!(binary_for_platform("Darwin arm64"), None);
        assert_eq!(binary_for_platform("Linux armv7l"), None);
    }

    #[test]
    fn lists_concrete_host_aliases_from_ssh_config() {
        let config = "Host onprem-server 100.100.0.5\n  HostName 100.100.0.5\n  User j\n\nHost *\n  ServerAliveInterval 30\nhost dev !bad\nMatch host x\n";
        assert_eq!(ssh_config_hosts(config), vec!["onprem-server", "100.100.0.5", "dev"]);
    }

    #[test]
    fn the_version_line_matches_jojod_version_output() {
        assert_eq!(bundled_version_line(), format!("jojod {} (protocol {})", jojod::VERSION, jojo_proto::PROTOCOL_VERSION));
    }
}
