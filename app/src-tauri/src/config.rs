//! Export and import of the configuration (Settings ▸ Export configuration…
//! and Import configuration…): one `jojo-config.json` file that moves a
//! setup from one computer to another.
//!
//! The file holds:
//!
//! 1. The settings from settings.json, without `defaultShell`, which names a
//!    program on the exporting computer.
//! 2. The remote locations from machines.toml. A location records whether it
//!    signs in with a saved password and whether it uses the key Jojo set up
//!    (`~/.ssh/jojo_ed25519`); the password and the key itself are never
//!    written to the file.
//! 3. The snippets from snippets.json.
//! 4. The tab layouts and sidebar order of remote projects from layout.json.
//!    This PC's projects are processes and folders on the exporting
//!    computer, so their layouts are left out.
//!
//! Remote projects and sessions are not in the file: they live in each
//! server's jojod, so a computer that adds the same location sees them.
//!
//! Import merges instead of replacing:
//!
//! 1. A location whose `ssh_host` and `port` match one already listed is
//!    not added again; the listed one is used for its snippets and layouts.
//! 2. A new location that signs in with a password is added only when the
//!    import dialog supplies its password, which is saved in the system
//!    credential store.
//! 3. A new location that uses Jojo's key points at this computer's
//!    `~/.ssh/jojo_ed25519`; the preview reports when that file is missing.
//! 4. Snippets are matched by id, and imported ones replace older copies.
//!    A snippet listed for one location is moved to that location's id on
//!    this computer.
//! 5. A layout is added only for a project that has no tabs on this
//!    computer yet (the window applies this rule, since it owns the layout).
//! 6. Settings other than `defaultShell` replace this computer's.

use crate::bridge::LOCAL_MACHINE;
use crate::layout::{LayoutFile, ProjectLayout};
use crate::machines::{Machine, MachineKind};
use crate::settings::Settings;
use crate::snippets::{Snippet, SnippetScope};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use ts_rs::TS;

/// The value of the file's `format` key, which identifies a Jojo configuration file.
pub const FORMAT: &str = "jojo-config";
/// The file format version. A file with another version is refused.
pub const VERSION: u32 = 1;
/// The settings key that is never exported or imported.
const DEFAULT_SHELL_KEY: &str = "defaultShell";

/// A remote location as written to the file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExportedLocation {
    /// The location's id on the exporting computer; snippet scopes and
    /// layout keys in the same file refer to it.
    pub id: String,
    pub label: String,
    /// An alias from `~/.ssh/config`, or `user@host`.
    pub ssh_host: String,
    #[serde(default)]
    pub port: Option<u16>,
    /// The location signs in with `~/.ssh/jojo_ed25519`, the key Jojo set up.
    #[serde(default)]
    pub jojo_key: bool,
    /// The location signs in with a password saved in the exporting
    /// computer's credential store.
    #[serde(default)]
    pub password: bool,
}

/// The whole `jojo-config.json` file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfigFile {
    pub format: String,
    pub version: u32,
    /// The Jojo version that wrote the file.
    pub app_version: String,
    /// Unix milliseconds.
    pub exported_at: u64,
    /// settings.json's keys, without `defaultShell`.
    pub settings: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub locations: Vec<ExportedLocation>,
    #[serde(default)]
    pub snippets: Vec<Snippet>,
    /// Keyed `<exported location id>:<project id>`, as in layout.json.
    #[serde(default)]
    pub layouts: BTreeMap<String, ProjectLayout>,
    /// layout.json's `project_order` entries of remote projects.
    #[serde(default)]
    pub project_order: Vec<String>,
}

/// Builds the file from this computer's settings, locations, snippets and layout.
pub fn build_export(settings: &Settings, machines: &[Machine], snippets: &[Snippet], layout: &LayoutFile, app_version: &str, now_ms: u64) -> ConfigFile {
    let mut settings_object = match serde_json::to_value(settings) {
        Ok(serde_json::Value::Object(object)) => object,
        _ => serde_json::Map::new(),
    };
    settings_object.remove(DEFAULT_SHELL_KEY);
    let locations: Vec<ExportedLocation> = machines
        .iter()
        .filter(|m| m.kind == MachineKind::Ssh)
        .filter_map(|m| {
            Some(ExportedLocation {
                id: m.id.clone(),
                label: m.label.clone(),
                ssh_host: m.ssh_host.clone()?,
                port: m.port,
                jojo_key: m.identity_file.is_some(),
                password: m.password,
            })
        })
        .collect();
    let remote = |key: &str| key.split_once(':').is_some_and(|(machine, _)| locations.iter().any(|l| l.id == machine));
    ConfigFile {
        format: FORMAT.into(),
        version: VERSION,
        app_version: app_version.into(),
        exported_at: now_ms,
        settings: settings_object,
        layouts: layout.projects.iter().filter(|(key, _)| remote(key)).map(|(k, v)| (k.clone(), v.clone())).collect(),
        project_order: layout.project_order.iter().filter(|key| remote(key)).cloned().collect(),
        locations,
        snippets: snippets.to_vec(),
    }
}

/// Reads a file, refusing one that is not a Jojo configuration file of this format version.
pub fn parse(bytes: &[u8]) -> Result<ConfigFile, String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| format!("the file is not valid JSON: {e}"))?;
    if value.get("format").and_then(|f| f.as_str()) != Some(FORMAT) {
        return Err("the file is not a Jojo configuration file (its \"format\" is not \"jojo-config\")".into());
    }
    let version = value.get("version").and_then(|v| v.as_u64());
    if version != Some(u64::from(VERSION)) {
        return Err(format!(
            "the file has format version {}, and this Jojo reads version {VERSION}; install the Jojo version that exported it or a newer one",
            version.map_or("(none)".to_owned(), |v| v.to_string())
        ));
    }
    serde_json::from_value(value).map_err(|e| format!("the file could not be read: {e}"))
}

/// What this computer already has, against which an import is planned.
pub struct Current<'a> {
    pub settings: &'a Settings,
    pub machines: &'a [Machine],
    pub snippets: &'a [Snippet],
    pub layout: &'a LayoutFile,
    /// Host aliases in this computer's `~/.ssh/config`.
    pub ssh_aliases: &'a [String],
    /// Whether this computer has `~/.ssh/jojo_ed25519`.
    pub jojo_key_exists: bool,
}

/// What importing does with one location.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum LocationAction {
    /// The location is added to machines.toml.
    Add,
    /// A location with the same `ssh_host` and port is already listed.
    Exists,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
pub struct LocationPlan {
    /// The id in the file, which the import call's passwords are keyed by.
    pub id: String,
    pub label: String,
    pub ssh_host: String,
    pub port: Option<u16>,
    pub action: LocationAction,
    /// A new location that signs in with a password: it is added only when
    /// a password is supplied.
    pub needs_password: bool,
    /// A new location whose `ssh_host` is an alias this computer's
    /// `~/.ssh/config` lacks; it cannot connect until the alias is added.
    pub missing_alias: bool,
    /// A new location that uses Jojo's key, which this computer lacks.
    pub missing_key: bool,
}

/// What importing the file would change, shown before anything is written.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
pub struct ImportPreview {
    pub app_version: String,
    #[ts(type = "number")]
    pub exported_at: u64,
    pub locations: Vec<LocationPlan>,
    /// Snippets whose id this computer does not have.
    pub snippets_added: u32,
    /// Snippets whose id this computer has with different contents.
    pub snippets_replaced: u32,
    /// Settings keys whose value would change.
    pub settings_changed: Vec<String>,
    /// Remote projects whose tabs would be added (projects that have no
    /// tabs here yet, on locations that are added or already listed).
    pub layouts_added: u32,
}

/// Whether a location in the file is the same server as a listed one.
fn same_location(location: &ExportedLocation, machine: &Machine) -> bool {
    machine.kind == MachineKind::Ssh && machine.ssh_host.as_deref() == Some(location.ssh_host.as_str()) && machine.port == location.port
}

fn existing_id(location: &ExportedLocation, machines: &[Machine]) -> Option<String> {
    machines.iter().find(|m| same_location(location, m)).map(|m| m.id.clone())
}

/// An alias is an `ssh_host` without `user@`; `user@host` needs no config entry.
fn is_alias(host: &str) -> bool {
    !host.contains('@')
}

/// This computer's settings with the file's applied, keeping `defaultShell`.
/// Keys this version does not know are ignored; a value of the wrong type
/// leaves the settings unchanged.
fn merged_settings(current: &Settings, file: &ConfigFile) -> Settings {
    let Ok(serde_json::Value::Object(mut object)) = serde_json::to_value(current) else {
        return current.clone();
    };
    for (key, value) in &file.settings {
        if key != DEFAULT_SHELL_KEY && object.contains_key(key) {
            object.insert(key.clone(), value.clone());
        }
    }
    serde_json::from_value(serde_json::Value::Object(object)).unwrap_or_else(|_| current.clone())
}

fn changed_settings(current: &Settings, merged: &Settings) -> Vec<String> {
    let (Ok(serde_json::Value::Object(before)), Ok(serde_json::Value::Object(after))) = (serde_json::to_value(current), serde_json::to_value(merged)) else {
        return Vec::new();
    };
    after.iter().filter(|(key, value)| before.get(*key) != Some(value)).map(|(key, _)| key.clone()).collect()
}

/// The layout key on this computer for a key in the file, or None when its
/// location is not imported.
fn remapped_key(key: &str, ids: &HashMap<String, String>) -> Option<String> {
    let (machine, project) = key.split_once(':')?;
    Some(format!("{}:{project}", ids.get(machine)?))
}

/// The snippet with a location scope moved to that location's id here.
fn remapped_snippet(snippet: &Snippet, ids: &HashMap<String, String>) -> Snippet {
    let mut snippet = snippet.clone();
    if let SnippetScope::Machine { machine_id } = &mut snippet.scope {
        if let Some(id) = ids.get(machine_id.as_str()) {
            *machine_id = id.clone();
        }
    }
    snippet
}

/// Ids of the file's locations that are already listed here, and the local
/// machine's id, mapped to this computer's ids.
fn existing_ids(file: &ConfigFile, machines: &[Machine]) -> HashMap<String, String> {
    let mut ids: HashMap<String, String> = file.locations.iter().filter_map(|l| Some((l.id.clone(), existing_id(l, machines)?))).collect();
    ids.insert(LOCAL_MACHINE.into(), LOCAL_MACHINE.into());
    ids
}

pub fn preview(file: &ConfigFile, current: &Current) -> ImportPreview {
    let locations: Vec<LocationPlan> = file
        .locations
        .iter()
        .map(|l| {
            let add = existing_id(l, current.machines).is_none();
            LocationPlan {
                id: l.id.clone(),
                label: l.label.clone(),
                ssh_host: l.ssh_host.clone(),
                port: l.port,
                action: if add { LocationAction::Add } else { LocationAction::Exists },
                needs_password: add && l.password,
                missing_alias: add && !l.password && !l.jojo_key && is_alias(&l.ssh_host) && !current.ssh_aliases.iter().any(|a| a == &l.ssh_host),
                missing_key: add && l.jojo_key && !current.jojo_key_exists,
            }
        })
        .collect();
    // Existing locations keep their ids; added ones get new ids, which do
    // not change whether a project already has tabs here.
    let mut ids = existing_ids(file, current.machines);
    for location in &file.locations {
        ids.entry(location.id.clone()).or_insert_with(|| format!("new-{}", location.id));
    }
    let ids_before = existing_ids(file, current.machines);
    let mut snippets_added = 0;
    let mut snippets_replaced = 0;
    for snippet in &file.snippets {
        let incoming = remapped_snippet(snippet, &ids_before);
        match current.snippets.iter().find(|s| s.id == snippet.id) {
            None => snippets_added += 1,
            Some(existing) if *existing != incoming => snippets_replaced += 1,
            Some(_) => {}
        }
    }
    let merged = merged_settings(current.settings, file);
    let layouts_added = file.layouts.keys().filter_map(|key| remapped_key(key, &ids)).filter(|key| !current.layout.projects.contains_key(key)).count();
    ImportPreview {
        app_version: file.app_version.clone(),
        exported_at: file.exported_at,
        locations,
        snippets_added,
        snippets_replaced,
        settings_changed: changed_settings(current.settings, &merged),
        layouts_added: layouts_added as u32,
    }
}

/// The result of planning an import, before anything is written.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportPlan {
    /// Locations to add to machines.toml, with this computer's new ids.
    pub new_machines: Vec<Machine>,
    /// Passwords to save in the credential store, by new location id.
    pub passwords: Vec<(String, String)>,
    pub settings: Settings,
    pub snippets: Vec<Snippet>,
    /// Layouts keyed by this computer's location ids; the window adds those
    /// whose project has no tabs yet.
    pub layouts: BTreeMap<String, ProjectLayout>,
    pub project_order: Vec<String>,
    /// Sentences describing locations that were not added or need a step.
    pub notes: Vec<String>,
}

/// Plans the import. `passwords` holds the passwords typed in the import
/// dialog, keyed by the location ids in the file; `new_id` makes the id of
/// each added location; `jojo_key` is this computer's
/// `~/.ssh/jojo_ed25519` path.
pub fn plan(file: &ConfigFile, current: &Current, passwords: &HashMap<String, String>, jojo_key: Option<&str>, mut new_id: impl FnMut() -> String) -> ImportPlan {
    let mut ids = existing_ids(file, current.machines);
    let mut new_machines = Vec::new();
    let mut saved_passwords = Vec::new();
    let mut notes = Vec::new();
    for location in &file.locations {
        if ids.contains_key(&location.id) {
            continue;
        }
        let password = passwords.get(&location.id).filter(|p| !p.is_empty());
        if location.password && password.is_none() {
            notes.push(format!("{} was not added because no password was entered for it.", location.label));
            continue;
        }
        if location.jojo_key && !current.jojo_key_exists {
            notes.push(format!(
                "{} signs in with ~/.ssh/jojo_ed25519, which this computer does not have. Copy jojo_ed25519 and jojo_ed25519.pub from the other computer's ~/.ssh folder, or remove the location and add it again with its username and password.",
                location.label
            ));
        }
        if !location.password && !location.jojo_key && is_alias(&location.ssh_host) && !current.ssh_aliases.iter().any(|a| a == &location.ssh_host) {
            notes.push(format!(
                "{} uses the SSH alias {}, which this computer's ~/.ssh/config does not list. Copy its Host entry and key from the other computer.",
                location.label, location.ssh_host
            ));
        }
        let id = new_id();
        ids.insert(location.id.clone(), id.clone());
        if let Some(password) = password.filter(|_| location.password) {
            saved_passwords.push((id.clone(), password.clone()));
        }
        new_machines.push(Machine {
            id,
            label: location.label.clone(),
            kind: MachineKind::Ssh,
            ssh_host: Some(location.ssh_host.clone()),
            enabled: true,
            port: location.port,
            identity_file: if location.jojo_key { jojo_key.map(str::to_owned) } else { None },
            password: location.password,
        });
    }
    let mut snippets = current.snippets.to_vec();
    for snippet in &file.snippets {
        let incoming = remapped_snippet(snippet, &ids);
        match snippets.iter_mut().find(|s| s.id == incoming.id) {
            Some(existing) => *existing = incoming,
            None => snippets.push(incoming),
        }
    }
    ImportPlan {
        new_machines,
        passwords: saved_passwords,
        settings: merged_settings(current.settings, file),
        snippets,
        layouts: file.layouts.iter().filter_map(|(key, layout)| Some((remapped_key(key, &ids)?, layout.clone()))).collect(),
        project_order: file.project_order.iter().filter_map(|key| remapped_key(key, &ids)).collect(),
        notes,
    }
}

/// What the import command returns to the window, which applies the
/// settings, snippets and layouts to its own state and saves them.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
pub struct ImportResult {
    /// Locations added to machines.toml and connected.
    pub machines: Vec<Machine>,
    pub settings: Settings,
    pub snippets: Vec<Snippet>,
    pub layouts: BTreeMap<String, ProjectLayout>,
    pub project_order: Vec<String>,
    pub notes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{LayoutNode, TabLayout};

    fn remote(id: &str, host: &str) -> Machine {
        Machine { id: id.into(), label: format!("{host} label"), kind: MachineKind::Ssh, ssh_host: Some(host.into()), enabled: true, port: None, identity_file: None, password: false }
    }

    fn snippet(id: &str, scope: SnippetScope) -> Snippet {
        Snippet { id: id.into(), name: format!("snippet {id}"), command: "git status".into(), folder: String::new(), scope, last_values: BTreeMap::new() }
    }

    fn project_layout(session: &str) -> ProjectLayout {
        ProjectLayout {
            tabs: vec![TabLayout { id: "T1".into(), name: None, root: LayoutNode::Pane { session_id: session.into() }, focused: session.into(), zoomed: None }],
            active_tab: Some("T1".into()),
        }
    }

    /// The exporting computer: a key alias, a password location, a location
    /// with Jojo's key, a snippet for each kind of scope, and layouts for
    /// This PC and for a remote project.
    fn exported() -> ConfigFile {
        let settings = Settings { theme: "prism".into(), default_shell: "C:\\Program Files\\PowerShell\\7\\pwsh.exe".into(), ..Settings::default() };
        let password = Machine { password: true, port: Some(2222), ..remote("01PW", "deploy@203.0.113.7") };
        let keyed = Machine { identity_file: Some("C:\\Users\\a\\.ssh\\jojo_ed25519".into()), ..remote("01KEY", "deploy@198.51.100.4") };
        let machines = vec![crate::machines::this_pc(), remote("01ALIAS", "onprem-server"), password, keyed];
        let snippets = vec![
            snippet("S-ALL", SnippetScope::All),
            snippet("S-ALIAS", SnippetScope::Machine { machine_id: "01ALIAS".into() }),
            snippet("S-PROJECT", SnippetScope::Project { project_id: "P1".into() }),
        ];
        let mut layout = LayoutFile::default();
        layout.projects.insert("local:PL".into(), project_layout("S1"));
        layout.projects.insert("01ALIAS:P1".into(), project_layout("S2"));
        layout.project_order = vec!["local:PL".into(), "01ALIAS:P1".into()];
        build_export(&settings, &machines, &snippets, &layout, "0.2.9", 1_700_000_000_000)
    }

    fn current<'a>(settings: &'a Settings, machines: &'a [Machine], snippets: &'a [Snippet], layout: &'a LayoutFile, aliases: &'a [String], key: bool) -> Current<'a> {
        Current { settings, machines, snippets, layout, ssh_aliases: aliases, jojo_key_exists: key }
    }

    #[test]
    fn export_leaves_out_the_default_shell_secrets_and_this_pcs_layouts() {
        let file = exported();
        assert!(!file.settings.contains_key("defaultShell"));
        assert_eq!(file.settings["theme"], "prism");
        assert_eq!(file.locations.len(), 3, "This PC is not exported");
        let password = file.locations.iter().find(|l| l.id == "01PW").unwrap();
        assert!(password.password && !password.jojo_key && password.port == Some(2222));
        assert!(file.locations.iter().find(|l| l.id == "01KEY").unwrap().jojo_key);
        assert_eq!(file.layouts.keys().collect::<Vec<_>>(), vec!["01ALIAS:P1"]);
        assert_eq!(file.project_order, vec!["01ALIAS:P1"]);
        let text = serde_json::to_string(&file).unwrap();
        assert!(!text.contains("jojo_ed25519") && !text.contains("pwsh"), "{text}");
    }

    #[test]
    fn parse_round_trips_and_refuses_other_files() {
        let file = exported();
        assert_eq!(parse(&serde_json::to_vec(&file).unwrap()).unwrap(), file);
        assert!(parse(b"{ nope").unwrap_err().contains("not valid JSON"));
        assert!(parse(br#"{"snippets": []}"#).unwrap_err().contains("not a Jojo configuration file"));
        assert!(parse(br#"{"format": "jojo-config", "version": 2}"#).unwrap_err().contains("format version 2"));
    }

    #[test]
    fn preview_reports_what_each_location_needs() {
        let file = exported();
        let settings = Settings::default();
        let machines = vec![crate::machines::this_pc()];
        let layout = LayoutFile::default();
        let preview = preview(&file, &current(&settings, &machines, &[], &layout, &[], false));
        let by_id = |id: &str| preview.locations.iter().find(|l| l.id == id).unwrap().clone();
        assert!(by_id("01ALIAS").missing_alias && !by_id("01ALIAS").needs_password);
        assert!(by_id("01PW").needs_password && !by_id("01PW").missing_alias, "user@host needs no alias");
        assert!(by_id("01KEY").missing_key && !by_id("01KEY").missing_alias);
        assert!(preview.locations.iter().all(|l| l.action == LocationAction::Add));
        assert_eq!((preview.snippets_added, preview.snippets_replaced), (3, 0));
        assert_eq!(preview.settings_changed, vec!["theme"]);
        assert_eq!(preview.layouts_added, 1);
    }

    #[test]
    fn a_listed_location_is_matched_on_host_and_port() {
        let file = exported();
        let settings = Settings::default();
        let same_port = Machine { password: true, port: Some(2222), ..remote("LOCALPW", "deploy@203.0.113.7") };
        let machines = vec![crate::machines::this_pc(), remote("LOCALALIAS", "onprem-server"), same_port];
        let layout = LayoutFile::default();
        let aliases = vec!["onprem-server".to_owned()];
        let now = current(&settings, &machines, &[], &layout, &aliases, true);
        let preview = preview(&file, &now);
        let action = |id: &str| preview.locations.iter().find(|l| l.id == id).unwrap().action;
        assert_eq!(action("01ALIAS"), LocationAction::Exists);
        assert_eq!(action("01PW"), LocationAction::Exists);
        assert!(!preview.locations.iter().find(|l| l.id == "01PW").unwrap().needs_password);
        let plan = plan(&file, &now, &HashMap::new(), Some("/home/b/.ssh/jojo_ed25519"), || "NEW".into());
        assert_eq!(plan.new_machines.len(), 1, "only the keyed location is new");
        assert_eq!(plan.layouts.keys().collect::<Vec<_>>(), vec!["LOCALALIAS:P1"]);
        let alias_snippet = plan.snippets.iter().find(|s| s.id == "S-ALIAS").unwrap();
        assert_eq!(alias_snippet.scope, SnippetScope::Machine { machine_id: "LOCALALIAS".into() });
    }

    #[test]
    fn plan_adds_locations_with_new_ids_and_skips_a_password_location_without_one() {
        let file = exported();
        let settings = Settings { default_shell: "/bin/zsh".into(), ..Settings::default() };
        let machines = vec![crate::machines::this_pc()];
        let existing = vec![Snippet { command: "old".into(), ..snippet("S-ALL", SnippetScope::All) }, snippet("MINE", SnippetScope::All)];
        let layout = LayoutFile::default();
        let aliases = vec!["onprem-server".to_owned()];
        let now = current(&settings, &machines, &existing, &layout, &aliases, true);
        let mut counter = 0;
        let mut next = || {
            counter += 1;
            format!("N{counter}")
        };
        let skipped = plan(&file, &now, &HashMap::new(), Some("/Users/b/.ssh/jojo_ed25519"), &mut next);
        assert_eq!(skipped.new_machines.iter().map(|m| m.ssh_host.clone().unwrap()).collect::<Vec<_>>(), vec!["onprem-server", "deploy@198.51.100.4"]);
        assert!(skipped.notes.iter().any(|n| n.contains("no password")), "{:?}", skipped.notes);
        assert!(skipped.passwords.is_empty());

        let passwords = HashMap::from([("01PW".to_owned(), "hunter2".to_owned())]);
        let full = plan(&file, &now, &passwords, Some("/Users/b/.ssh/jojo_ed25519"), &mut next);
        assert_eq!(full.new_machines.len(), 3);
        let pw = full.new_machines.iter().find(|m| m.password).unwrap();
        assert_eq!(pw.port, Some(2222));
        assert_eq!(full.passwords, vec![(pw.id.clone(), "hunter2".to_owned())]);
        let keyed = full.new_machines.iter().find(|m| m.identity_file.is_some()).unwrap();
        assert_eq!(keyed.identity_file.as_deref(), Some("/Users/b/.ssh/jojo_ed25519"), "the key path is this computer's");
        assert!(full.notes.is_empty(), "{:?}", full.notes);
        assert_eq!(full.settings.theme, "prism");
        assert_eq!(full.settings.default_shell, "/bin/zsh", "defaultShell is kept");
        assert_eq!(full.snippets.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), vec!["S-ALL", "MINE", "S-ALIAS", "S-PROJECT"]);
        assert_eq!(full.snippets[0].command, "git status", "the imported copy replaces the older one");
        let alias_id = &full.new_machines.iter().find(|m| m.ssh_host.as_deref() == Some("onprem-server")).unwrap().id;
        assert_eq!(full.snippets[2].scope, SnippetScope::Machine { machine_id: alias_id.clone() });
        assert_eq!(full.layouts.keys().cloned().collect::<Vec<_>>(), vec![format!("{alias_id}:P1")]);
        assert_eq!(full.project_order, vec![format!("{alias_id}:P1")]);
    }

    #[test]
    fn plan_notes_a_missing_alias_and_a_missing_key() {
        let file = exported();
        let settings = Settings::default();
        let machines = vec![crate::machines::this_pc()];
        let layout = LayoutFile::default();
        let now = current(&settings, &machines, &[], &layout, &[], false);
        let plan = plan(&file, &now, &HashMap::new(), Some("/Users/b/.ssh/jojo_ed25519"), || "N".into());
        assert!(plan.notes.iter().any(|n| n.contains("onprem-server") && n.contains("~/.ssh/config")), "{:?}", plan.notes);
        assert!(plan.notes.iter().any(|n| n.contains("jojo_ed25519 and jojo_ed25519.pub")), "{:?}", plan.notes);
    }

    #[test]
    fn unknown_or_mistyped_settings_are_ignored() {
        let mut file = exported();
        file.settings.insert("futureKey".into(), serde_json::json!(true));
        file.settings.insert("defaultShell".into(), serde_json::json!("fish"));
        let current_settings = Settings::default();
        let merged = merged_settings(&current_settings, &file);
        assert_eq!(merged.theme, "prism");
        assert_eq!(merged.default_shell, current_settings.default_shell);
        file.settings.insert("terminalFontSize".into(), serde_json::json!("large"));
        assert_eq!(merged_settings(&current_settings, &file), current_settings, "a mistyped value leaves the settings unchanged");
    }
}
