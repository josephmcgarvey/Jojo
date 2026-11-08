//! Claude Code agent state (spec section 7).
//!
//! The hook relay forwards each Claude Code hook payload as raw JSON. This
//! crate is the only place that interprets it: `HookEvent::parse` reads the
//! fields Jojo uses and ignores everything else, and `apply` maps an event to
//! the session's next state (spec 7.2 and 7.3). Unknown event names and
//! unknown `notification_type` values change nothing, so a Claude Code
//! release that adds hooks or fields does not break the daemon (spec 14,
//! risk 3). Everything here is pure; the clock is passed in.

use jojo_proto::AgentState;
use serde_json::Value;

/// How long a `working` session may produce no PTY output before it is
/// treated as `idle` (spec 7.3 rule 3). Pressing Esc in Claude Code ends the
/// turn without firing a hook; while Claude Code works, its spinner redraws
/// several times per second.
pub const SILENCE_MS: u64 = 10_000;

/// The longest label taken from a first prompt (spec 7.2).
pub const PROMPT_LABEL_CHARS: usize = 40;

/// The longest `tool_input` summary shown in a needs-you detail.
const TOOL_INPUT_CHARS: usize = 60;

/// `notification_type` values that mean Claude Code is waiting for the user.
const NEEDS_YOU_NOTIFICATIONS: [&str; 4] = ["permission_prompt", "elicitation_dialog", "elicitation_url_dialog", "agent_needs_input"];

/// The parts of a Claude Code hook payload that Jojo uses.
#[derive(Clone, Debug, PartialEq)]
pub enum HookEvent {
    /// `source` is `startup`, `resume`, `clear` or `compact`.
    SessionStart { claude_session_id: Option<String>, source: Option<String> },
    UserPromptSubmit { prompt: Option<String> },
    PreToolUse,
    PostToolUse,
    PermissionRequest { tool: Option<String> },
    Notification { notification_type: Option<String> },
    Elicitation,
    Stop,
    StopFailure { error_type: Option<String> },
    SessionEnd,
    /// An event name this version does not interpret.
    Other,
}

impl HookEvent {
    /// Parses a hook payload. Returns `None` when the text is not a JSON
    /// object with a string `hook_event_name`.
    pub fn parse(json: &str) -> Option<HookEvent> {
        let value: Value = serde_json::from_str(json).ok()?;
        let name = value.get("hook_event_name")?.as_str()?;
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        Some(match name {
            "SessionStart" => HookEvent::SessionStart { claude_session_id: text("session_id"), source: text("source") },
            "UserPromptSubmit" => HookEvent::UserPromptSubmit { prompt: text("prompt") },
            "PreToolUse" => HookEvent::PreToolUse,
            "PostToolUse" => HookEvent::PostToolUse,
            "PermissionRequest" => HookEvent::PermissionRequest { tool: tool_summary(&value) },
            "Notification" => HookEvent::Notification { notification_type: text("notification_type") },
            "Elicitation" => HookEvent::Elicitation,
            "Stop" => HookEvent::Stop,
            "StopFailure" => HookEvent::StopFailure { error_type: text("error_type") },
            "SessionEnd" => HookEvent::SessionEnd,
            _ => HookEvent::Other,
        })
    }
}

/// `Bash(git push origin main)`: the tool name and an abbreviated
/// `tool_input`, preferring its `command`, `file_path`, `url` or `pattern`
/// field and falling back to the compact JSON.
fn tool_summary(value: &Value) -> Option<String> {
    let name = value.get("tool_name")?.as_str()?;
    let input = value.get("tool_input");
    let preferred = input.and_then(|i| ["command", "file_path", "url", "pattern"].iter().find_map(|k| i.get(k).and_then(Value::as_str)));
    let detail = match (preferred, input) {
        (Some(text), _) => text.to_owned(),
        (None, Some(other)) if !other.is_null() => other.to_string(),
        _ => String::new(),
    };
    Some(format!("{name}({})", abbreviate(&detail, TOOL_INPUT_CHARS)))
}

/// The first `max` characters of `text` on one line, with "…" when cut.
fn abbreviate(text: &str, max: usize) -> String {
    let single: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if single.chars().count() <= max {
        single
    } else {
        let cut: String = single.chars().take(max).collect();
        format!("{}…", cut.trim_end())
    }
}

/// A state change caused by one event.
#[derive(Clone, Debug, PartialEq)]
pub struct Transition {
    pub state: AgentState,
    /// Shown in the badge tooltip and the toast body: the pending tool for
    /// `needs_you`, the error type for `error`. `None` clears it.
    pub detail: Option<String>,
}

fn to(state: AgentState) -> Option<Transition> {
    Some(Transition { state, detail: None })
}

/// The next state after `event`, or `None` when the event changes nothing
/// (spec 7.2 and 7.3). An `exited` session never changes: its process is
/// gone, and a late hook from it must not revive the badge.
pub fn apply(current: AgentState, event: &HookEvent) -> Option<Transition> {
    if current == AgentState::Exited {
        return None;
    }
    let next = match event {
        // Auto-compaction fires SessionStart with source "compact" in the
        // middle of a turn; the turn goes on, so the state stays.
        HookEvent::SessionStart { source: Some(source), .. } if source == "compact" => None,
        HookEvent::SessionStart { .. } => to(AgentState::Idle),
        // A pending permission prompt is resolved only by PostToolUse (the
        // tool ran) or Stop. PreToolUse while needs_you comes from another
        // tool call, for example a parallel subagent, and must not hide the
        // prompt behind working.
        HookEvent::PreToolUse if current == AgentState::NeedsYou => None,
        HookEvent::UserPromptSubmit { .. } | HookEvent::PreToolUse | HookEvent::PostToolUse => to(AgentState::Working),
        HookEvent::PermissionRequest { tool } => Some(Transition {
            state: AgentState::NeedsYou,
            detail: Some(match tool {
                Some(tool) => format!("Needs permission: {tool}"),
                None => "Needs permission".to_owned(),
            }),
        }),
        HookEvent::Elicitation => Some(Transition { state: AgentState::NeedsYou, detail: Some("Needs input".to_owned()) }),
        HookEvent::Notification { notification_type: Some(kind) } if NEEDS_YOU_NOTIFICATIONS.contains(&kind.as_str()) => {
            // A PermissionRequest for the same prompt usually arrives first
            // and carries the tool; keep its detail instead of replacing it.
            if current == AgentState::NeedsYou {
                return None;
            }
            Some(Transition { state: AgentState::NeedsYou, detail: Some("Needs input".to_owned()) })
        }
        HookEvent::Notification { .. } => None,
        HookEvent::Stop => to(AgentState::Done),
        HookEvent::StopFailure { error_type } => Some(Transition {
            state: AgentState::Error,
            detail: Some(format!("Error: {}", error_type.as_deref().unwrap_or("unknown"))),
        }),
        // Claude Code exited, but the PTY process (a shell the user started
        // claude from, or claude itself until the PTY reports its exit) may
        // still be running. The session is no longer an agent turn.
        HookEvent::SessionEnd => to(AgentState::Idle),
        HookEvent::Other => None,
    };
    next.filter(|t| t.state != current || current == AgentState::NeedsYou)
}

/// True for a Claude Code session id that is safe to pass on a command
/// line: 1 to 128 ASCII letters, digits and hyphens (Claude Code uses UUIDs).
/// Ids come from hook payloads, which any process in a pane can send, and
/// Resume runs `cmd.exe /d /c claude --resume <id>` for npm installs.
pub fn is_valid_claude_session_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// The label set from a first prompt: its first `PROMPT_LABEL_CHARS`
/// characters on one line, or `None` for an empty prompt.
pub fn label_from_prompt(prompt: &str) -> Option<String> {
    let label = abbreviate(prompt, PROMPT_LABEL_CHARS);
    (!label.is_empty()).then_some(label)
}

/// Spec 7.3 rule 3: a `working` session whose PTY has produced no output for
/// `SILENCE_MS` becomes `idle`.
pub fn silence(current: AgentState, last_output_ms: u64, now_ms: u64) -> Option<AgentState> {
    (current == AgentState::Working && now_ms.saturating_sub(last_output_ms) >= SILENCE_MS).then_some(AgentState::Idle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use AgentState::*;

    fn ev(json: &str) -> HookEvent {
        HookEvent::parse(json).expect("parses")
    }

    fn state_after(current: AgentState, json: &str) -> Option<AgentState> {
        apply(current, &ev(json)).map(|t| t.state)
    }

    #[test]
    fn parses_the_fields_jojo_uses_and_ignores_the_rest() {
        assert_eq!(
            ev(r#"{"hook_event_name":"SessionStart","session_id":"abc","source":"startup","extra":1}"#),
            HookEvent::SessionStart { claude_session_id: Some("abc".into()), source: Some("startup".into()) }
        );
        assert_eq!(ev(r#"{"hook_event_name":"UserPromptSubmit","prompt":"fix it"}"#), HookEvent::UserPromptSubmit { prompt: Some("fix it".into()) });
        assert_eq!(ev(r#"{"hook_event_name":"SomethingNew"}"#), HookEvent::Other);
        assert_eq!(HookEvent::parse("not json"), None);
        assert_eq!(HookEvent::parse(r#"{"no_name":true}"#), None);
    }

    #[test]
    fn follows_the_state_table() {
        assert_eq!(state_after(Starting, r#"{"hook_event_name":"SessionStart"}"#), Some(Idle));
        for name in ["UserPromptSubmit", "PreToolUse", "PostToolUse"] {
            assert_eq!(state_after(Idle, &format!(r#"{{"hook_event_name":"{name}"}}"#)), Some(Working), "{name}");
        }
        assert_eq!(state_after(Working, r#"{"hook_event_name":"PermissionRequest","tool_name":"Bash"}"#), Some(NeedsYou));
        assert_eq!(state_after(Working, r#"{"hook_event_name":"Elicitation"}"#), Some(NeedsYou));
        assert_eq!(state_after(Working, r#"{"hook_event_name":"Stop"}"#), Some(Done));
        assert_eq!(state_after(Working, r#"{"hook_event_name":"StopFailure","error_type":"rate_limit"}"#), Some(Error));
        assert_eq!(state_after(Done, r#"{"hook_event_name":"SessionEnd"}"#), Some(Idle));
    }

    #[test]
    fn treats_only_waiting_notification_types_as_needs_you() {
        for kind in NEEDS_YOU_NOTIFICATIONS {
            assert_eq!(state_after(Working, &format!(r#"{{"hook_event_name":"Notification","notification_type":"{kind}"}}"#)), Some(NeedsYou), "{kind}");
        }
        assert_eq!(state_after(Working, r#"{"hook_event_name":"Notification","notification_type":"idle_prompt"}"#), None);
        assert_eq!(state_after(Working, r#"{"hook_event_name":"Notification"}"#), None);
    }

    #[test]
    fn keeps_the_permission_detail_when_the_notification_follows() {
        let ask = apply(Working, &ev(r#"{"hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"git push origin main"}}"#)).unwrap();
        assert_eq!(ask.detail.as_deref(), Some("Needs permission: Bash(git push origin main)"));
        assert_eq!(apply(NeedsYou, &ev(r#"{"hook_event_name":"Notification","notification_type":"permission_prompt"}"#)), None);
    }

    #[test]
    fn orders_a_permission_prompt_as_spec_7_3_rule_1() {
        // PreToolUse fires before the permission check, PermissionRequest after it.
        let mut state = Idle;
        for (json, expected) in [
            (r#"{"hook_event_name":"UserPromptSubmit","prompt":"push"}"#, Working),
            (r#"{"hook_event_name":"PreToolUse","tool_name":"Bash"}"#, Working),
            (r#"{"hook_event_name":"PermissionRequest","tool_name":"Bash"}"#, NeedsYou),
            (r#"{"hook_event_name":"PostToolUse","tool_name":"Bash"}"#, Working),
            (r#"{"hook_event_name":"Stop"}"#, Done),
        ] {
            if let Some(t) = apply(state, &ev(json)) {
                state = t.state;
            }
            assert_eq!(state, expected, "{json}");
        }
    }

    #[test]
    fn reports_the_error_type_and_summarizes_tool_input() {
        let failed = apply(Working, &ev(r#"{"hook_event_name":"StopFailure","error_type":"rate_limit"}"#)).unwrap();
        assert_eq!(failed.detail.as_deref(), Some("Error: rate_limit"));
        let long = format!(r#"{{"hook_event_name":"PermissionRequest","tool_name":"Write","tool_input":{{"file_path":"{}"}}}}"#, "a".repeat(80));
        let detail = apply(Working, &ev(&long)).unwrap().detail.unwrap();
        assert!(detail.starts_with("Needs permission: Write(aaaa") && detail.ends_with("…)"), "{detail}");
    }

    #[test]
    fn changes_nothing_for_exited_sessions_unknown_events_or_repeats() {
        assert_eq!(state_after(Exited, r#"{"hook_event_name":"UserPromptSubmit"}"#), None);
        assert_eq!(state_after(Working, r#"{"hook_event_name":"SomethingNew"}"#), None);
        assert_eq!(state_after(Working, r#"{"hook_event_name":"PreToolUse"}"#), None);
    }

    #[test]
    fn keeps_needs_you_through_other_tool_calls_and_compaction() {
        assert_eq!(state_after(NeedsYou, r#"{"hook_event_name":"PreToolUse","tool_name":"Read"}"#), None);
        assert_eq!(state_after(NeedsYou, r#"{"hook_event_name":"PostToolUse","tool_name":"Bash"}"#), Some(Working));
        assert_eq!(state_after(Working, r#"{"hook_event_name":"SessionStart","source":"compact"}"#), None);
        assert_eq!(state_after(Working, r#"{"hook_event_name":"SessionStart","source":"clear"}"#), Some(Idle));
    }

    #[test]
    fn accepts_only_plain_claude_session_ids() {
        assert!(is_valid_claude_session_id("0f3c2a9e-1b2c-4d5e-8f90-123456789abc"));
        assert!(!is_valid_claude_session_id("x&calc"));
        assert!(!is_valid_claude_session_id(""));
        assert!(!is_valid_claude_session_id(&"a".repeat(129)));
    }

    #[test]
    fn labels_from_the_first_forty_characters_of_a_prompt() {
        assert_eq!(label_from_prompt("Fix the\nlogin bug").as_deref(), Some("Fix the login bug"));
        let label = label_from_prompt(&"x".repeat(50)).unwrap();
        assert_eq!(label.chars().count(), PROMPT_LABEL_CHARS + 1);
        assert_eq!(label_from_prompt("   "), None);
    }

    #[test]
    fn falls_back_to_idle_after_ten_seconds_of_silence_while_working() {
        assert_eq!(silence(Working, 1_000, 1_000 + SILENCE_MS - 1), None);
        assert_eq!(silence(Working, 1_000, 1_000 + SILENCE_MS), Some(Idle));
        assert_eq!(silence(NeedsYou, 0, 60_000), None);
        assert_eq!(silence(Working, 5_000, 1_000), None);
    }
}
