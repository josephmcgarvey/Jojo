//! Which program a session runs (spec section 5.1).

use jojo_proto::SessionKind;
use std::path::{Path, PathBuf};

/// Searches each directory on PATH for a file named exactly `name`.
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|dir| dir.join(name)).find(|p| p.is_file())
}

/// Program and arguments for a new session.
pub fn command_for(kind: SessionKind, shell: Option<&str>) -> (String, Vec<String>) {
    match kind {
        SessionKind::Claude => claude_command(),
        SessionKind::Shell => (shell.map(str::to_owned).unwrap_or_else(default_shell), vec![]),
    }
}

/// `claude.exe` when present; otherwise an npm-installed `claude.cmd` run
/// through `cmd.exe`, because ConPTY starts executables, not batch files.
/// When neither exists, `claude.exe` is returned so the spawn fails with a
/// "not found" error that names the program.
#[cfg(windows)]
fn claude_command() -> (String, Vec<String>) {
    if find_on_path("claude.exe").is_some() {
        ("claude.exe".into(), vec![])
    } else if find_on_path("claude.cmd").is_some() {
        ("cmd.exe".into(), vec!["/d".into(), "/c".into(), "claude".into()])
    } else {
        ("claude.exe".into(), vec![])
    }
}

#[cfg(unix)]
fn claude_command() -> (String, Vec<String>) {
    ("claude".into(), vec![])
}

#[cfg(windows)]
fn default_shell() -> String {
    if find_on_path("pwsh.exe").is_some() { "pwsh.exe".into() } else { "powershell.exe".into() }
}

#[cfg(unix)]
fn default_shell() -> String {
    std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/bash".into())
}

/// Initial sidebar label: `claude` for Claude sessions, otherwise the
/// program's file stem (`pwsh`, `bash`). Parses both `\` and `/` separators
/// so Windows paths work on any platform.
pub fn default_label(kind: SessionKind, program: &str) -> String {
    match kind {
        SessionKind::Claude => "claude".into(),
        SessionKind::Shell => {
            let last = program.rsplit(['\\', '/']).next().unwrap_or(program);
            Path::new(last).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| last.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jojo_proto::SessionKind;

    #[test]
    fn finds_a_program_known_to_exist() {
        #[cfg(windows)]
        assert!(find_on_path("cmd.exe").is_some());
        #[cfg(unix)]
        assert!(find_on_path("sh").is_some());
    }

    #[test]
    fn missing_program_is_none() {
        assert!(find_on_path("definitely-not-a-real-program-jojo").is_none());
    }

    #[test]
    fn shell_override_is_used() {
        assert_eq!(command_for(SessionKind::Shell, Some("bash")), ("bash".to_string(), vec![]));
    }

    #[test]
    fn labels() {
        assert_eq!(default_label(SessionKind::Claude, "claude.exe"), "claude");
        assert_eq!(default_label(SessionKind::Shell, "C:\\Program Files\\PowerShell\\7\\pwsh.exe"), "pwsh");
        assert_eq!(default_label(SessionKind::Shell, "/bin/bash"), "bash");
    }
}
