//! Per-platform locations and user facts the rest of the app uses.

use std::path::PathBuf;

/// The user's home folder: `%USERPROFILE%` on Windows, `$HOME` elsewhere.
pub fn home_dir() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// Where settings.json, machines.toml and layout.json live:
/// `%APPDATA%\Jojo` on Windows, `~/Library/Application Support/Jojo` on
/// macOS, and `$XDG_CONFIG_HOME/jojo` (default `~/.config/jojo`) on Linux.
/// Falls back to the temporary folder when the base variable is missing.
pub fn config_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("Jojo")
    }
    #[cfg(target_os = "macos")]
    {
        home_dir().unwrap_or_else(std::env::temp_dir).join("Library").join("Application Support").join("Jojo")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|h| h.join(".config")))
            .unwrap_or_else(std::env::temp_dir)
            .join("jojo")
    }
}

/// The user's login shell from the user database (`pw_shell`, which
/// `chsh` changes). An app started from the Finder or the Dock does not
/// always have `$SHELL`, so the database is read instead.
#[cfg(unix)]
pub fn login_shell() -> Option<String> {
    // SAFETY: getpwuid returns a pointer to static storage or null; the
    // shell string is copied before any other call could overwrite it.
    unsafe {
        let entry = libc::getpwuid(libc::getuid());
        if entry.is_null() || (*entry).pw_shell.is_null() {
            return None;
        }
        let shell = std::ffi::CStr::from_ptr((*entry).pw_shell).to_string_lossy().into_owned();
        (!shell.is_empty()).then_some(shell)
    }
}

/// The local machine's name in the sidebar and messages.
pub const LOCAL_MACHINE_LABEL: &str = if cfg!(target_os = "macos") { "This Mac" } else { "This PC" };
