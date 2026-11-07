//! The shell programs a new shell session can run (Plan 3 amendment B).
//!
//! Detection runs once per app run (`available`), because installing a shell
//! while Jojo is open is rare and the result is shown in every "New shell"
//! menu. A shell is offered only when its program exists:
//!
//! 1. PowerShell 7 (`pwsh.exe`), Windows PowerShell (`powershell.exe`) and
//!    Command Prompt (`cmd.exe`) are looked up in the directories of `PATH`.
//!    Their `program` is the bare name (`pwsh`, `powershell`, `cmd`), which
//!    jojod resolves through `PATH` and which matches the `defaultShell`
//!    setting's values.
//! 2. Git Bash is looked up at `Git\bin\bash.exe` under `%ProgramFiles%`,
//!    `%ProgramFiles(x86)%` and `%LOCALAPPDATA%\Programs`, and its `program`
//!    is that full path. `bash.exe` on `PATH` is not used, because
//!    `C:\Windows\System32\bash.exe` is the WSL launcher, not Git Bash.

use serde::Serialize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use ts_rs::TS;

/// One entry of the "New shell" submenu.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
pub struct ShellOption {
    /// The menu text, for example "PowerShell 7".
    pub label: String,
    /// The value sent as `CreateSession.shell`.
    pub program: String,
}

/// Shells found on `PATH`: (menu label, executable file name, program sent to jojod).
const PATH_SHELLS: [(&str, &str, &str); 3] = [
    ("PowerShell 7", "pwsh.exe", "pwsh"),
    ("Windows PowerShell", "powershell.exe", "powershell"),
    ("Command Prompt", "cmd.exe", "cmd"),
];

/// Returns the shells that exist, in menu order. `path` is the value of the
/// `PATH` variable, `git_roots` the directories that may contain `Git`, and
/// `exists` reports whether a file exists (injected so tests need no files).
pub fn detect(path: Option<OsString>, git_roots: &[PathBuf], exists: impl Fn(&Path) -> bool) -> Vec<ShellOption> {
    let dirs: Vec<PathBuf> = path.map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    let mut shells: Vec<ShellOption> = PATH_SHELLS
        .iter()
        .filter(|(_, file, _)| dirs.iter().any(|dir| exists(&dir.join(file))))
        .map(|(label, _, program)| ShellOption { label: (*label).into(), program: (*program).into() })
        .collect();
    if let Some(bash) = git_roots.iter().map(|root| root.join("Git").join("bin").join("bash.exe")).find(|p| exists(p)) {
        shells.push(ShellOption { label: "Git Bash".into(), program: bash.to_string_lossy().into_owned() });
    }
    shells
}

/// The shells on this PC, detected on first use and cached for the app run.
pub fn available() -> &'static [ShellOption] {
    static SHELLS: OnceLock<Vec<ShellOption>> = OnceLock::new();
    SHELLS.get_or_init(|| {
        let mut git_roots: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)"].iter().filter_map(std::env::var_os).map(PathBuf::from).collect();
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            git_roots.push(PathBuf::from(local).join("Programs"));
        }
        detect(std::env::var_os("PATH"), &git_roots, Path::is_file)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn files(paths: &[&str]) -> impl Fn(&Path) -> bool {
        let set: HashSet<PathBuf> = paths.iter().map(PathBuf::from).collect();
        move |p: &Path| set.contains(p)
    }

    fn path_var(dirs: &[&str]) -> Option<OsString> {
        Some(std::env::join_paths(dirs.iter().map(PathBuf::from)).unwrap())
    }

    #[test]
    fn offers_only_the_shells_found_on_path_in_menu_order() {
        let exists = files(&[r"C:\Windows\System32\cmd.exe", r"C:\Program Files\PowerShell\7\pwsh.exe"]);
        let shells = detect(path_var(&[r"C:\Windows\System32", r"C:\Program Files\PowerShell\7"]), &[], exists);
        assert_eq!(
            shells,
            vec![
                ShellOption { label: "PowerShell 7".into(), program: "pwsh".into() },
                ShellOption { label: "Command Prompt".into(), program: "cmd".into() },
            ]
        );
    }

    #[test]
    fn finds_git_bash_under_the_first_root_that_has_it() {
        let exists = files(&[r"C:\Users\me\AppData\Local\Programs\Git\bin\bash.exe"]);
        let roots = [PathBuf::from(r"C:\Program Files"), PathBuf::from(r"C:\Users\me\AppData\Local\Programs")];
        let shells = detect(None, &roots, exists);
        assert_eq!(shells, vec![ShellOption { label: "Git Bash".into(), program: r"C:\Users\me\AppData\Local\Programs\Git\bin\bash.exe".into() }]);
    }

    #[test]
    fn ignores_the_wsl_bash_launcher_on_path() {
        let exists = files(&[r"C:\Windows\System32\bash.exe"]);
        assert!(detect(path_var(&[r"C:\Windows\System32"]), &[], exists).is_empty());
    }

    #[test]
    fn serializes_as_label_and_program() {
        let json = serde_json::to_value(ShellOption { label: "Command Prompt".into(), program: "cmd".into() }).unwrap();
        assert_eq!(json, serde_json::json!({ "label": "Command Prompt", "program": "cmd" }));
    }
}
