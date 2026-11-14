//! `%APPDATA%\Jojo\snippets.json` (`~/Library/Application Support/Jojo` on
//! macOS): the snippets library, commands the user saved to insert into a
//! terminal or run there.
//!
//! The window owns the file; jojod never reads it, so snippets belong to one
//! computer. A missing file means no snippets. A file that cannot be parsed
//! is renamed to `snippets.json.corrupt-<unix ms>` and the library starts
//! empty, so a hand edit gone wrong never stops the app from starting and
//! the old file can still be recovered by hand.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// Where a snippet is listed: everywhere, only for terminals on one
/// location (machine id from machines.toml), or only for terminals in one
/// project (project id, a ULID unique across locations).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SnippetScope {
    #[default]
    All,
    Machine { machine_id: String },
    Project { project_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Snippet {
    /// A ULID made by the window.
    pub id: String,
    pub name: String,
    /// The text inserted into the terminal; may span several lines.
    pub command: String,
    /// A group name shown as a heading in the Snippets list; empty for none.
    #[serde(default)]
    pub folder: String,
    #[serde(default)]
    pub scope: SnippetScope,
    /// The values last typed for the command's `{{name}}` placeholders, by
    /// placeholder name, offered the next time the snippet is used.
    #[serde(default)]
    pub last_values: BTreeMap<String, String>,
}

/// The file's top level: `{ "snippets": [...] }`, leaving room for other keys.
#[derive(Debug, Default, Serialize, Deserialize)]
struct SnippetsFile {
    #[serde(default)]
    snippets: Vec<Snippet>,
}

pub fn default_path() -> PathBuf {
    crate::platform::config_dir().join("snippets.json")
}

pub fn load(path: &Path) -> Vec<Snippet> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => return Vec::new(),
    };
    match serde_json::from_slice::<SnippetsFile>(&bytes) {
        Ok(file) => file.snippets,
        Err(_) => {
            let mut aside = path.as_os_str().to_owned();
            aside.push(format!(".corrupt-{}", jojod::store::now_ms()));
            let _ = std::fs::rename(path, PathBuf::from(aside));
            Vec::new()
        }
    }
}

/// Writes `snippets.json.tmp`, then renames it over `snippets.json`.
pub fn save(path: &Path, snippets: &[Snippet]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(&SnippetsFile { snippets: snippets.to_vec() }).map_err(std::io::Error::other)?;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snippet(id: &str, scope: SnippetScope) -> Snippet {
        Snippet {
            id: id.into(),
            name: format!("name {id}"),
            command: "git checkout {{branch}}\ngit log -1".into(),
            folder: "Git".into(),
            scope,
            last_values: BTreeMap::from([("branch".to_owned(), "main".to_owned())]),
        }
    }

    #[test]
    fn a_missing_file_is_an_empty_library() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load(&dir.path().join("snippets.json")).is_empty());
    }

    #[test]
    fn round_trips_every_scope() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Jojo").join("snippets.json");
        let all = vec![
            snippet("01A", SnippetScope::All),
            snippet("01B", SnippetScope::Machine { machine_id: "01M".into() }),
            snippet("01C", SnippetScope::Project { project_id: "01P".into() }),
        ];
        save(&path, &all).unwrap();
        assert_eq!(load(&path), all);
        assert!(!dir.path().join("Jojo").join("snippets.json.tmp").exists());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"kind\": \"machine\"") && text.contains("\"machine_id\": \"01M\""), "{text}");
    }

    #[test]
    fn missing_folder_and_scope_default_to_none_and_all() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snippets.json");
        std::fs::write(&path, r#"{"snippets":[{"id":"01A","name":"Status","command":"git status"}]}"#).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded[0].folder, "");
        assert_eq!(loaded[0].scope, SnippetScope::All);
        assert!(loaded[0].last_values.is_empty());
    }

    #[test]
    fn a_corrupt_file_is_set_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snippets.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert!(load(&path).is_empty());
        assert!(!path.exists());
        let names: Vec<String> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().any(|n| n.starts_with("snippets.json.corrupt-")), "{names:?}");
    }
}
