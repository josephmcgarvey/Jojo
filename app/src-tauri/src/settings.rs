//! %APPDATA%\Jojo\settings.json (spec section 9.10).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: String,
    pub terminal_font_family: String,
    pub terminal_font_size: u16,
    pub terminal_line_height: f32,
    pub scrollback_lines: u32,
    pub copy_on_select: bool,
    pub toasts: bool,
    pub default_shell: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "neon-glass".into(),
            terminal_font_family: "Geist Mono".into(),
            terminal_font_size: 13,
            terminal_line_height: 1.25,
            scrollback_lines: 10_000,
            copy_on_select: true,
            toasts: true,
            default_shell: "pwsh".into(),
        }
    }
}

pub fn default_path() -> PathBuf {
    let roaming = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    roaming.join("Jojo").join("settings.json")
}

/// Reads settings. A missing or unreadable file gives the defaults; keys
/// absent from the file keep their defaults.
pub fn load(path: &Path) -> Settings {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(&dir.path().join("settings.json")), Settings::default());
    }

    #[test]
    fn partial_file_keeps_defaults_for_missing_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{ "terminalFontSize": 15 }"#).unwrap();
        let s = load(&path);
        assert_eq!(s.terminal_font_size, 15);
        assert_eq!(s.scrollback_lines, 10_000);
        assert_eq!(s.default_shell, "pwsh");
    }

    #[test]
    fn invalid_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ nope").unwrap();
        assert_eq!(load(&path), Settings::default());
    }
}
