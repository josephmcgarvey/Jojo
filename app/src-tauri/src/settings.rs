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
    /// Show the first prompt Claude Code received as a session's name, until
    /// the user renames it (spec 7.2; off by default, user request).
    pub prompt_labels: bool,
    /// Play a chime when a session finishes or needs a response.
    pub sound: bool,
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
            prompt_labels: false,
            sound: true,
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

/// Writes settings: the known keys are merged into the file's existing JSON
/// object, so keys this version does not know (a newer Jojo's, or a hand
/// edit) are kept. Writes `settings.json.tmp`, then renames it over
/// `settings.json`.
pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    let mut object = std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    if let serde_json::Value::Object(known) = serde_json::to_value(settings).map_err(std::io::Error::other)? {
        object.extend(known);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut text = serde_json::to_string_pretty(&serde_json::Value::Object(object)).map_err(std::io::Error::other)?;
    text.push('\n');
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_round_trips_and_keeps_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{ "futureKey": [1, 2], "terminalFontSize": 15 }"#).unwrap();
        let mut settings = load(&path);
        settings.terminal_font_size = 16;
        settings.copy_on_select = false;
        save(&path, &settings).unwrap();
        assert_eq!(load(&path), settings);
        let raw: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(raw["futureKey"], serde_json::json!([1, 2]));
        assert!(!dir.path().join("settings.json.tmp").exists());
    }

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
