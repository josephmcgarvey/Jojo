//! Default locations (spec sections 4 and 8.1).

use std::path::PathBuf;

/// Where the daemon listens: a named pipe name on Windows, a socket path on
/// Linux.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint(pub String);

/// `%LOCALAPPDATA%\Jojo` on Windows, `~/.jojo` on Linux.
pub fn default_state_dir() -> PathBuf {
    #[cfg(windows)]
    {
        let base = std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA is set on Windows");
        PathBuf::from(base).join("Jojo")
    }
    #[cfg(unix)]
    {
        let home = std::env::var_os("HOME").expect("HOME is set");
        PathBuf::from(home).join(".jojo")
    }
}

/// `\\.\pipe\jojo-<username>` on Windows, `~/.jojo/jojod.sock` on Linux.
pub fn default_endpoint() -> Endpoint {
    #[cfg(windows)]
    {
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".to_string());
        let safe: String = user
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        Endpoint(format!(r"\\.\pipe\jojo-{safe}"))
    }
    #[cfg(unix)]
    {
        Endpoint(default_state_dir().join("jojod.sock").to_string_lossy().into_owned())
    }
}
