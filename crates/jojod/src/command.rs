//! Which program a session runs (spec section 5.1).

use jojo_proto::SessionKind;
use std::path::{Path, PathBuf};

/// Searches each directory on PATH for a file named exactly `name`.
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|dir| dir.join(name)).find(|p| p.is_file())
}

/// Program and arguments for a new session. `resume` is a
/// `claude_session_id` to continue (`claude --resume <id>`, spec 5.4).
pub fn command_for(kind: SessionKind, shell: Option<&str>, resume: Option<&str>) -> (String, Vec<String>) {
    match kind {
        SessionKind::Claude => {
            let (program, mut args) = claude_command();
            if let Some(id) = resume {
                args.extend(["--resume".to_owned(), id.to_owned()]);
            }
            (program, args)
        }
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

/// `claude` when it is on PATH; otherwise the path the user's login shell
/// resolves it to, then the first of the places Claude Code's installers
/// use; otherwise `claude`, so the spawn fails with a "not found" error that
/// names the program.
///
/// The daemon starts through the user's login shell and inherits its PATH,
/// but Claude Code's older local install makes `claude` an alias in
/// ~/.zshrc (`alias claude=~/.claude/local/claude`), which a program cannot
/// find on PATH: on macOS that failed with "Unable to spawn claude because:
/// No viable candidates found in PATH".
#[cfg(unix)]
fn claude_command() -> (String, Vec<String>) {
    if find_on_path("claude").is_some() {
        return ("claude".into(), vec![]);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let resolved = home.as_deref().and_then(|home| claude_from_login_shell(home).or_else(|| claude_in_install_places(home)));
    match resolved {
        Some(path) => (path.to_string_lossy().into_owned(), vec![]),
        None => ("claude".into(), vec![]),
    }
}

/// How long the login shell may take to answer `command -v claude`.
#[cfg(unix)]
const LOGIN_SHELL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Asks the user's interactive login shell what `claude` is (`command -v
/// claude`, which reads ~/.zprofile and ~/.zshrc and so sees aliases and
/// PATH additions made there). The shell is ended after
/// LOGIN_SHELL_TIMEOUT, so a startup file that waits does not block the
/// session's creation for long.
#[cfg(unix)]
fn claude_from_login_shell(home: &Path) -> Option<PathBuf> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let shell = std::env::var("SHELL").ok().filter(|s| Path::new(s).is_file()).unwrap_or_else(|| "/bin/sh".into());
    let mut child = Command::new(shell)
        .args(["-l", "-i", "-c", "command -v claude"])
        .current_dir(home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + LOGIN_SHELL_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(std::time::Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let mut output = String::new();
    child.stdout.take()?.read_to_string(&mut output).ok()?;
    parse_command_v(&output, home, |p| p.is_file())
}

/// The program path in a shell's `command -v claude` output: a path
/// (`/opt/homebrew/bin/claude`) or an alias (zsh `alias
/// claude=/Users/me/.claude/local/claude`, bash `alias
/// claude='~/.claude/local/claude'`), whose first word is taken and whose
/// leading `~/` is expanded to `home`. Startup files may print other lines,
/// so the last line that names claude is used. None when that program does
/// not exist (`exists`).
pub fn parse_command_v(output: &str, home: &Path, exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let line = output.lines().map(str::trim).rfind(|l| l.contains("claude"))?;
    let value = line.strip_prefix("alias claude=").unwrap_or(line).trim_matches(|c| c == '\'' || c == '"');
    let program = value.split_whitespace().next()?;
    let path = match program.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(program),
    };
    // A Unix absolute path, checked as text: on Windows, where this parser
    // is tested, `Path::is_absolute` wants a drive letter.
    let absolute = program.starts_with('/') || program.starts_with("~/");
    (absolute && exists(&path)).then_some(path)
}

/// The first existing `claude` in the places Claude Code's installers put
/// it: the native installer's ~/.local/bin, the older local install's
/// ~/.claude/local, Homebrew (Apple Silicon and Intel), npm's global prefix
/// and the newest nvm-managed Node.
#[cfg(unix)]
fn claude_in_install_places(home: &Path) -> Option<PathBuf> {
    let mut places = vec![
        home.join(".local/bin/claude"),
        home.join(".claude/local/claude"),
        PathBuf::from("/opt/homebrew/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
        home.join(".npm-global/bin/claude"),
    ];
    if let Ok(versions) = std::fs::read_dir(home.join(".nvm/versions/node")) {
        let mut nodes: Vec<PathBuf> = versions.filter_map(|e| e.ok().map(|e| e.path().join("bin/claude"))).collect();
        nodes.sort();
        places.extend(nodes.into_iter().rev());
    }
    places.into_iter().find(|p| p.is_file())
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
        assert_eq!(command_for(SessionKind::Shell, Some("bash"), None), ("bash".to_string(), vec![]));
    }

    #[test]
    fn resume_appends_the_claude_session_id() {
        let (_, args) = command_for(SessionKind::Claude, None, Some("abc-123"));
        assert!(args.ends_with(&["--resume".to_string(), "abc-123".to_string()]), "{args:?}");
        assert_eq!(command_for(SessionKind::Shell, Some("bash"), Some("ignored")).1, Vec::<String>::new());
    }

    #[test]
    fn reads_the_program_from_command_v_output() {
        let home = Path::new("/Users/me");
        let exists = |p: &Path| p == Path::new("/Users/me/.claude/local/claude") || p == Path::new("/opt/homebrew/bin/claude");
        // zsh prints an alias definition, bash quotes it, and a plain path is a path.
        assert_eq!(parse_command_v("alias claude=/Users/me/.claude/local/claude\n", home, exists), Some(PathBuf::from("/Users/me/.claude/local/claude")));
        assert_eq!(parse_command_v("alias claude='~/.claude/local/claude'\n", home, exists), Some(PathBuf::from("/Users/me/.claude/local/claude")));
        assert_eq!(parse_command_v("/opt/homebrew/bin/claude\n", home, exists), Some(PathBuf::from("/opt/homebrew/bin/claude")));
        // Lines printed by startup files are skipped; an alias with options keeps its program.
        assert_eq!(
            parse_command_v("Welcome back!\nalias claude='~/.claude/local/claude --verbose'\n", home, exists),
            Some(PathBuf::from("/Users/me/.claude/local/claude"))
        );
        // Nothing found, a program that does not exist, or a relative name.
        assert_eq!(parse_command_v("", home, exists), None);
        assert_eq!(parse_command_v("/usr/bin/claude\n", home, exists), None);
        assert_eq!(parse_command_v("alias claude=claude-code\n", home, |_| true), None);
    }

    #[test]
    fn labels() {
        assert_eq!(default_label(SessionKind::Claude, "claude.exe"), "claude");
        assert_eq!(default_label(SessionKind::Shell, "C:\\Program Files\\PowerShell\\7\\pwsh.exe"), "pwsh");
        assert_eq!(default_label(SessionKind::Shell, "/bin/bash"), "bash");
    }
}
