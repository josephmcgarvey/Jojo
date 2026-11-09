//! %APPDATA%\Jojo\layout.json (spec section 4, "Layout" row): the ordered
//! tabs of every project, each tab's split tree of session ids, its focused
//! pane, and the active tab. Keys are `"<machine id>:<project id>"`, because a
//! project id is unique only within one daemon.
//!
//! The backend only stores and loads the document. Every semantic rule (at
//! most 4 panes per tab, split sizes that sum to 1, session ids that still
//! exist on the daemon, no session in two tabs) is applied by the frontend in
//! app/src/lib/layout/reconcile.ts, because only the frontend has the
//! daemon's session list.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use ts_rs::TS;

/// Format version written to `version`. A file with any other version is
/// set aside like an unreadable one.
pub const LAYOUT_VERSION: u32 = 1;

/// `Row` places children side by side, left to right; `Column` stacks them,
/// top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SplitDirection {
    Row,
    Column,
}

/// One node of a tab's split tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LayoutNode {
    Pane {
        session_id: String,
    },
    /// `sizes` has one positive fraction per child and sums to 1.
    Split { direction: SplitDirection, children: Vec<LayoutNode>, sizes: Vec<f64> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct TabLayout {
    pub id: String,
    /// A name the user gave the tab; `None` shows the focused session's label.
    #[serde(default)]
    pub name: Option<String>,
    pub root: LayoutNode,
    /// Session id of the focused pane.
    pub focused: String,
    /// Session id of the zoomed pane, which then fills the tab.
    #[serde(default)]
    pub zoomed: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct ProjectLayout {
    pub tabs: Vec<TabLayout>,
    #[serde(default)]
    pub active_tab: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct LayoutFile {
    pub version: u32,
    pub projects: BTreeMap<String, ProjectLayout>,
}

impl Default for LayoutFile {
    fn default() -> Self {
        LayoutFile { version: LAYOUT_VERSION, projects: BTreeMap::new() }
    }
}

pub fn default_path() -> PathBuf {
    crate::platform::config_dir().join("layout.json")
}

/// Reads the layout. A missing file gives an empty layout. A file that does
/// not parse, or that has a version other than `LAYOUT_VERSION`, is renamed
/// to `layout.json.corrupt-<unix milliseconds>` (so the next save does not
/// overwrite it) and an empty layout is returned. An empty layout is safe:
/// reconciliation then opens one tab per daemon session.
pub fn load(path: &Path) -> LayoutFile {
    let Ok(bytes) = std::fs::read(path) else {
        return LayoutFile::default();
    };
    match serde_json::from_slice::<LayoutFile>(&bytes) {
        Ok(layout) if layout.version == LAYOUT_VERSION => layout,
        _ => {
            set_aside(path);
            LayoutFile::default()
        }
    }
}

fn set_aside(path: &Path) {
    let mut stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    loop {
        let mut name = path.as_os_str().to_owned();
        name.push(format!(".corrupt-{stamp}"));
        let candidate = PathBuf::from(name);
        if !candidate.exists() {
            // A failed rename leaves the file in place; the next save then
            // replaces it, which is the only remaining option.
            let _ = std::fs::rename(path, candidate);
            return;
        }
        stamp += 1;
    }
}

/// Writes the layout atomically: the JSON goes to `layout.json.tmp`, which is
/// then renamed over `layout.json` (`std::fs::rename` replaces an existing
/// file on Windows), so a crash mid-write never leaves a truncated file.
pub fn save(path: &Path, layout: &LayoutFile) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let bytes = serde_json::to_vec_pretty(layout).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn sample() -> LayoutFile {
        let tab = TabLayout {
            id: "T1".into(),
            name: Some("work".into()),
            root: LayoutNode::Split {
                direction: SplitDirection::Row,
                children: vec![
                    LayoutNode::Pane { session_id: "S1".into() },
                    LayoutNode::Pane { session_id: "S2".into() },
                ],
                sizes: vec![0.6, 0.4],
            },
            focused: "S2".into(),
            zoomed: None,
        };
        let mut projects = BTreeMap::new();
        projects.insert("local:P1".to_string(), ProjectLayout { tabs: vec![tab], active_tab: Some("T1".into()) });
        LayoutFile { version: LAYOUT_VERSION, projects }
    }

    #[test]
    fn missing_file_gives_an_empty_layout() {
        let dir = tempfile::tempdir().unwrap();
        let layout = load(&dir.path().join("layout.json"));
        assert_eq!(layout, LayoutFile::default());
        assert_eq!(layout.version, LAYOUT_VERSION);
        assert!(layout.projects.is_empty());
    }

    #[test]
    fn save_then_load_roundtrips_and_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Jojo").join("layout.json");
        save(&path, &sample()).unwrap();
        assert!(!dir.path().join("Jojo").join("layout.json.tmp").exists());
        assert_eq!(load(&path), sample());
    }

    #[test]
    fn save_replaces_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("layout.json");
        save(&path, &LayoutFile::default()).unwrap();
        save(&path, &sample()).unwrap();
        assert_eq!(load(&path), sample());
    }

    #[test]
    fn unreadable_file_is_set_aside_and_gives_an_empty_layout() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("layout.json");
        std::fs::write(&path, "{ nope").unwrap();
        assert_eq!(load(&path), LayoutFile::default());
        assert!(!path.exists(), "the unreadable file must be renamed, not left for the next save to overwrite");
        let names: Vec<String> =
            std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().any(|n| n.starts_with("layout.json.corrupt-")), "{names:?}");
    }

    #[test]
    fn a_file_with_another_version_is_set_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("layout.json");
        std::fs::write(&path, r#"{ "version": 99, "projects": {} }"#).unwrap();
        assert_eq!(load(&path), LayoutFile::default());
        assert!(!path.exists());
    }

    #[test]
    fn json_shape_matches_the_frontend_types() {
        let json = serde_json::to_value(sample()).unwrap();
        let root = &json["projects"]["local:P1"]["tabs"][0]["root"];
        assert_eq!(root["type"], "split");
        assert_eq!(root["direction"], "row");
        assert_eq!(root["children"][0], serde_json::json!({ "type": "pane", "session_id": "S1" }));
        assert_eq!(json["projects"]["local:P1"]["active_tab"], "T1");
        assert_eq!(json["projects"]["local:P1"]["tabs"][0]["zoomed"], serde_json::Value::Null);
    }
}
