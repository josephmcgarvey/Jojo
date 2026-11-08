//! Claude Code hook registration and the hook relay (spec section 7.1).
//!
//! `install` and `uninstall` edit `~/.claude/settings.json` as a JSON value:
//!
//! 1. Before writing, the existing file is copied to `settings.json.jojo-bak`.
//! 2. Hook entries that are not Jojo's are never removed or reordered; Jojo's
//!    entries are appended to each event's list.
//! 3. Jojo's entries are those whose command, with quotes removed, ends in
//!    `jojod hook` or `jojod.exe hook`. Installing again first removes them,
//!    so repeating `install-hooks` changes nothing else.
//! 4. Each entry has `"timeout": 5` and no `"async"`: Claude Code waits for
//!    synchronous hooks, so events reach the daemon in order.
//!
//! The command path uses forward slashes. Claude Code on Windows can run hook
//! commands through Git Bash, where unquoted backslashes are escape
//! characters; `C:/Users/...` works in both Git Bash and cmd.exe.

use jojo_proto::{ControlMsg, Frame, SessionId};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The hook events Jojo registers (spec 7.1).
pub const EVENTS: [&str; 10] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PermissionRequest",
    "Notification",
    "Elicitation",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

/// Seconds Claude Code waits for the relay before giving up on it.
const HOOK_TIMEOUT_SECONDS: u64 = 5;

/// The relay's whole budget: connect, Hello and the HookEvent (spec 7.1).
pub const RELAY_DEADLINE: Duration = Duration::from_millis(500);

/// `~/.claude/settings.json`.
pub fn default_settings_path() -> Option<PathBuf> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
    Some(PathBuf::from(home).join(".claude").join("settings.json"))
}

/// The hook command for the jojod at `jojod`: `"C:/.../jojod.exe" hook`.
pub fn hook_command(jojod: &Path) -> String {
    format!("\"{}\" hook", jojod.to_string_lossy().replace('\\', "/"))
}

/// True for a command string that runs a jojod hook relay.
fn is_jojo_command(command: &str) -> bool {
    let unquoted: String = command.chars().filter(|c| *c != '"' && *c != '\'').collect();
    let trimmed = unquoted.trim_end();
    trimmed.ends_with("jojod hook") || trimmed.ends_with("jojod.exe hook")
}

/// True for a hook group (`{ "matcher"?, "hooks": [...] }`) that contains a
/// Jojo command.
fn is_jojo_group(group: &Value) -> bool {
    group.get("hooks").and_then(Value::as_array).is_some_and(|hooks| {
        hooks.iter().any(|h| h.get("command").and_then(Value::as_str).is_some_and(is_jojo_command))
    })
}

/// Removes Jojo's hook groups from every event. With `prune`, events and the
/// `hooks` object that become empty are dropped too (uninstall); install
/// keeps them, so the keys stay where they were. Returns whether anything
/// changed.
fn remove_jojo(settings: &mut Value, prune: bool) -> bool {
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else { return false };
    let mut changed = false;
    for groups in hooks.values_mut() {
        if let Some(list) = groups.as_array_mut() {
            let before = list.len();
            list.retain(|g| !is_jojo_group(g));
            changed |= list.len() != before;
        }
    }
    if !prune {
        return changed;
    }
    hooks.retain(|_, groups| groups.as_array().is_none_or(|l| !l.is_empty()));
    if hooks.is_empty() {
        settings.as_object_mut().map(|o| o.remove("hooks"));
    }
    changed
}

/// Returns `settings` with Jojo's hooks registered for every event in
/// `EVENTS`, running `command`.
pub fn with_hooks(mut settings: Value, command: &str) -> Value {
    if !settings.is_object() {
        settings = Value::Object(Map::new());
    }
    remove_jojo(&mut settings, false);
    let object = settings.as_object_mut().expect("settings is an object");
    let hooks = object.entry("hooks").or_insert_with(|| Value::Object(Map::new()));
    if !hooks.is_object() {
        *hooks = Value::Object(Map::new());
    }
    let hooks = hooks.as_object_mut().expect("hooks is an object");
    for event in EVENTS {
        let groups = hooks.entry(event).or_insert_with(|| Value::Array(vec![]));
        if !groups.is_array() {
            *groups = Value::Array(vec![]);
        }
        groups
            .as_array_mut()
            .expect("event hooks are an array")
            .push(json!({ "hooks": [{ "type": "command", "command": command, "timeout": HOOK_TIMEOUT_SECONDS }] }));
    }
    settings
}

/// Returns `settings` without Jojo's hooks.
pub fn without_hooks(mut settings: Value) -> Value {
    remove_jojo(&mut settings, true);
    settings
}

/// Reads `path` as JSON; a missing file is an empty object. Malformed JSON is
/// an error, so a file the user is editing is never overwritten.
fn read_settings(path: &Path) -> anyhow::Result<Value> {
    match std::fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(Value::Object(Map::new())),
        Ok(text) => serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("{} is not valid JSON: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Object(Map::new())),
        Err(e) => Err(anyhow::anyhow!("cannot read {}: {e}", path.display())),
    }
}

/// Writes `settings` to `path` when it differs from `before`, after copying
/// the existing file to `<name>.jojo-bak`. Returns whether it wrote.
fn write_settings(path: &Path, before: &Value, settings: &Value) -> anyhow::Result<bool> {
    if settings == before {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if path.exists() {
        let mut backup = path.as_os_str().to_owned();
        backup.push(".jojo-bak");
        std::fs::copy(path, PathBuf::from(backup))?;
    }
    let mut text = serde_json::to_string_pretty(settings)?;
    text.push('\n');
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".jojo-tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    Ok(true)
}

/// `jojod install-hooks`: registers `command` in the settings file at `path`.
pub fn install(path: &Path, command: &str) -> anyhow::Result<bool> {
    let before = read_settings(path)?;
    write_settings(path, &before, &with_hooks(before.clone(), command))
}

/// `jojod uninstall-hooks`: removes Jojo's hooks from the settings file at `path`.
pub fn uninstall(path: &Path) -> anyhow::Result<bool> {
    let before = read_settings(path)?;
    write_settings(path, &before, &without_hooks(before.clone()))
}

/// `jojod hook`: forwards one hook payload to the local daemon (spec 7.1).
/// Returns immediately when `session` is `None` (Claude Code was started
/// outside Jojo). Every failure is ignored: the caller exits 0 and prints
/// nothing, so the relay never changes Claude Code's behavior.
pub async fn relay(endpoint: &crate::paths::Endpoint, session: Option<&str>, payload: String) {
    let Some(session_id) = session.and_then(|s| s.trim().parse::<ulid::Ulid>().ok()).map(SessionId) else { return };
    let _ = tokio::time::timeout(RELAY_DEADLINE, async {
        let (reader, writer) = crate::transport::connect_with_deadline(endpoint, RELAY_DEADLINE).await?;
        let mut client = crate::client::Client::handshake(reader, writer, RELAY_DEADLINE).await?;
        client.send(Frame::Control(ControlMsg::HookEvent { session_id, payload })).await
    })
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    const CMD: &str = "\"C:/Users/me/AppData/Local/Jojo/bin/jojod.exe\" hook";

    fn foreign() -> Value {
        json!({ "hooks": [{ "type": "command", "command": "npx notify-me" }] })
    }

    #[test]
    fn builds_a_quoted_forward_slash_command() {
        assert_eq!(hook_command(Path::new(r"C:\Users\me\AppData\Local\Jojo\bin\jojod.exe")), CMD);
        assert!(is_jojo_command(CMD));
        assert!(is_jojo_command("/home/me/.local/bin/jojod hook"));
        assert!(!is_jojo_command("npx notify-me"));
        assert!(!is_jojo_command("jojod hooks-list"));
    }

    #[test]
    fn appends_one_synchronous_entry_per_event_after_existing_hooks() {
        let settings = json!({ "model": "opus", "hooks": { "Stop": [foreign()] } });
        let installed = with_hooks(settings, CMD);
        assert_eq!(installed["model"], "opus");
        for event in EVENTS {
            let groups = installed["hooks"][event].as_array().unwrap();
            let entry = &groups.last().unwrap()["hooks"][0];
            assert_eq!(entry["command"], CMD, "{event}");
            assert_eq!(entry["timeout"], 5);
            assert!(entry.get("async").is_none());
        }
        let stop = installed["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2);
        assert_eq!(stop[0], foreign(), "existing entries stay first");
    }

    #[test]
    fn installing_twice_changes_nothing() {
        let once = with_hooks(json!({ "hooks": { "Stop": [foreign()] } }), CMD);
        assert_eq!(with_hooks(once.clone(), CMD), once);
    }

    #[test]
    fn a_new_jojod_path_replaces_the_old_entries() {
        let once = with_hooks(json!({}), "\"C:/old/jojod.exe\" hook");
        let twice = with_hooks(once, CMD);
        assert_eq!(twice["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert_eq!(twice["hooks"]["Stop"][0]["hooks"][0]["command"], CMD);
    }

    #[test]
    fn uninstall_removes_only_jojo_entries() {
        let installed = with_hooks(json!({ "theme": "dark", "hooks": { "Stop": [foreign()] } }), CMD);
        let removed = without_hooks(installed);
        assert_eq!(removed, json!({ "theme": "dark", "hooks": { "Stop": [foreign()] } }));
        assert_eq!(without_hooks(with_hooks(json!({}), CMD)), json!({}));
    }

    #[test]
    fn install_writes_a_backup_and_uninstall_restores_the_original_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert!(install(&path, CMD).unwrap(), "a missing file is created");
        assert!(!dir.path().join("settings.json.jojo-bak").exists(), "nothing to back up yet");
        assert!(!install(&path, CMD).unwrap(), "repeating changes nothing");

        std::fs::write(&path, serde_json::to_string(&json!({ "hooks": { "Stop": [foreign()] } })).unwrap()).unwrap();
        assert!(install(&path, CMD).unwrap());
        let backup: Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join("settings.json.jojo-bak")).unwrap()).unwrap();
        assert_eq!(backup, json!({ "hooks": { "Stop": [foreign()] } }));

        assert!(uninstall(&path).unwrap());
        let after: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(after, json!({ "hooks": { "Stop": [foreign()] } }));
    }

    #[test]
    fn keeps_the_order_of_existing_keys() {
        let settings: Value = serde_json::from_str(r#"{"zeta":1,"alpha":2,"hooks":{"Stop":[]},"middle":3}"#).unwrap();
        let installed = with_hooks(settings, CMD);
        let keys: Vec<&String> = installed.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["zeta", "alpha", "hooks", "middle"]);
    }

    #[test]
    fn refuses_to_overwrite_malformed_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert!(install(&path, CMD).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
    }
}
