//! Default locations (spec sections 4 and 8.1).

use std::path::PathBuf;

/// Where the daemon listens: a named pipe name on Windows, a socket path on
/// Linux.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint(pub String);

/// The environment variable that selects a separate profile.
pub const PROFILE_VAR: &str = "JOJO_PROFILE";

/// A profile name, when `value` is a usable one: 1 to 32 ASCII letters,
/// digits, `-` or `_`. Anything else (unset, empty, a path) means the
/// default profile.
pub fn profile_from(value: Option<&str>) -> Option<String> {
    let value = value?;
    let usable = !value.is_empty() && value.len() <= 32 && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    usable.then(|| value.to_owned())
}

/// The profile selected by `JOJO_PROFILE`, or None for the default one.
///
/// A profile is a complete, separate Jojo: its own settings, locations,
/// layout and snippets (the app's folder), and its own daemon, sessions,
/// logs and pipe or socket (the locations below). It is used to record a
/// demo without showing the user's real projects and servers. The app
/// passes its environment to the jojod it starts, and jojod passes it to
/// every session, so `jojod hook` run by Claude Code in a session reaches
/// that profile's daemon.
pub fn profile() -> Option<String> {
    profile_from(std::env::var(PROFILE_VAR).ok().as_deref())
}

/// `%LOCALAPPDATA%\Jojo` on Windows, `~/.jojo` on Linux, with
/// `profiles/<name>` added for a profile.
pub fn default_state_dir() -> PathBuf {
    #[cfg(windows)]
    let base = PathBuf::from(std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA is set on Windows")).join("Jojo");
    #[cfg(unix)]
    let base = PathBuf::from(std::env::var_os("HOME").expect("HOME is set")).join(".jojo");
    match profile() {
        Some(name) => base.join("profiles").join(name),
        None => base,
    }
}

/// `\\.\pipe\jojo-<username>` on Windows (`jojo-<username>-<profile>` for a
/// profile), and `jojod.sock` in the state folder on Linux.
pub fn default_endpoint() -> Endpoint {
    #[cfg(windows)]
    {
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".to_string());
        let safe: String = user
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        match profile() {
            Some(name) => Endpoint(format!(r"\\.\pipe\jojo-{safe}-{name}")),
            None => Endpoint(format!(r"\\.\pipe\jojo-{safe}")),
        }
    }
    #[cfg(unix)]
    {
        Endpoint(default_state_dir().join("jojod.sock").to_string_lossy().into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::profile_from;

    #[test]
    fn accepts_short_plain_profile_names_only() {
        assert_eq!(profile_from(Some("demo")), Some("demo".to_owned()));
        assert_eq!(profile_from(Some("demo-2_b")), Some("demo-2_b".to_owned()));
        assert_eq!(profile_from(None), None);
        assert_eq!(profile_from(Some("")), None);
        assert_eq!(profile_from(Some("../evil")), None, "a path is not a profile name");
        assert_eq!(profile_from(Some("a b")), None);
        assert_eq!(profile_from(Some(&"x".repeat(33))), None);
    }
}
