//! How full a Claude Code conversation's context window is, read from the
//! end of its transcript (the `transcript_path` every hook payload carries).
//!
//! Claude Code writes the conversation to a JSONL file, one record per line.
//! Each assistant reply records the API usage of the request that produced
//! it; the tokens sent to the model, which is what the context window
//! limits, are `input_tokens + cache_creation_input_tokens +
//! cache_read_input_tokens`. After a compaction (`/compact` or the automatic
//! one), a `system` record with subtype `compact_boundary` carries
//! `compactMetadata.postTokens`, the size of the context that remains, until
//! the next reply records a new usage.
//!
//! The window size is not in every transcript: Claude Code writes
//! `"contextWindow":<n>` only into the records of commands such as `/usage`.
//! `context_window` therefore uses that value when the tail holds one, and
//! otherwise infers the size from the model and the token count.

use serde_json::Value;

/// The default context window of Claude models.
pub const STANDARD_WINDOW: u64 = 200_000;
/// The extended window: models with a `[1m]` suffix, and the Claude 5 Opus
/// and Sonnet models, for which Claude Code reports `"contextWindow":1000000`
/// although their transcript records name the model without the suffix
/// (`claude-sonnet-5`, `claude-opus-5-5`).
pub const EXTENDED_WINDOW: u64 = 1_000_000;

/// How many bytes from the end of the transcript are read. The last reply's
/// record is usually a few kilobytes; a large tool result after it can push
/// it further back, so the read covers a generous tail.
pub const TAIL_BYTES: u64 = 512 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextUsage {
    /// Tokens in the context now.
    pub tokens: u64,
    /// The context window those tokens count against.
    pub window: u64,
}

/// Reads the context usage from the end of a transcript. `tail` is the last
/// TAIL_BYTES of the file; its first line may be cut and is skipped when it
/// does not parse. Returns None when the tail holds no main-conversation
/// reply with usage and no compaction record (a conversation that has not
/// received its first reply yet). Replies of subagents (`isSidechain`) are
/// ignored, because they run in their own context.
pub fn context_usage(tail: &str) -> Option<ContextUsage> {
    let mut tokens: Option<u64> = None;
    let mut model: Option<String> = None;
    let mut reported: Option<u64> = None;
    for line in tail.lines().rev() {
        if reported.is_none() {
            reported = reported_window(line);
        }
        if tokens.is_some() && model.is_some() {
            if reported.is_some() {
                break;
            }
            continue;
        }
        let Ok(record) = serde_json::from_str::<Value>(line) else { continue };
        if record["isSidechain"].as_bool() == Some(true) {
            continue;
        }
        match record["type"].as_str() {
            Some("assistant") => {
                let usage = &record["message"]["usage"];
                if usage.is_object() {
                    let sum = ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"]
                        .iter()
                        .map(|key| usage[*key].as_u64().unwrap_or(0))
                        .sum::<u64>();
                    if tokens.is_none() && sum > 0 {
                        tokens = Some(sum);
                    }
                    if model.is_none() {
                        model = record["message"]["model"].as_str().map(str::to_owned);
                    }
                }
            }
            Some("system") if record["subtype"] == "compact_boundary" && tokens.is_none() => {
                tokens = record["compactMetadata"]["postTokens"].as_u64();
            }
            _ => {}
        }
    }
    let tokens = tokens?;
    Some(ContextUsage { tokens, window: context_window(model.as_deref(), tokens, reported) })
}

/// The context window for a conversation: the window Claude Code reported
/// in the transcript when there is one; otherwise EXTENDED_WINDOW for a
/// `[1m]` model, a Claude 5 Opus or Sonnet model, or a conversation already
/// past the standard window; otherwise STANDARD_WINDOW.
pub fn context_window(model: Option<&str>, tokens: u64, reported: Option<u64>) -> u64 {
    if let Some(window) = reported.filter(|w| *w > 0) {
        return window.max(tokens);
    }
    let model = model.unwrap_or("");
    let extended_model = ["claude-opus-5", "claude-sonnet-5"].iter().any(|prefix| model.starts_with(prefix));
    if model.contains("[1m]") || extended_model || tokens > STANDARD_WINDOW {
        EXTENDED_WINDOW
    } else {
        STANDARD_WINDOW
    }
}

/// The last `"contextWindow":<n>` value on a line, written by Claude Code
/// into the records of commands such as `/usage`. The record may hold it as
/// JSON or inside a JSON string (`\"contextWindow\":<n>`), so the quote,
/// backslash and colon after the name are skipped.
fn reported_window(line: &str) -> Option<u64> {
    const NAME: &str = "contextWindow";
    let start = line.rfind(NAME)? + NAME.len();
    let rest = line[start..].trim_start_matches(['"', '\\', ':', ' ']);
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(model: &str, input: u64, created: u64, read: u64) -> String {
        format!(
            r#"{{"type":"assistant","isSidechain":false,"message":{{"model":"{model}","usage":{{"input_tokens":{input},"cache_creation_input_tokens":{created},"cache_read_input_tokens":{read},"output_tokens":900}}}}}}"#
        )
    }

    #[test]
    fn sums_the_input_tokens_of_the_last_reply() {
        let tail = [reply("claude-haiku-4-5", 10, 20, 30), r#"{"type":"user","message":{"content":"next"}}"#.into(), reply("claude-haiku-4-5", 4, 9_258, 120_000)].join("\n");
        assert_eq!(context_usage(&tail), Some(ContextUsage { tokens: 129_262, window: STANDARD_WINDOW }));
    }

    #[test]
    fn a_compaction_after_the_last_reply_gives_the_remaining_size() {
        let tail = [
            reply("claude-opus-5-5", 4, 9_258, 951_036),
            r#"{"type":"system","subtype":"compact_boundary","compactMetadata":{"trigger":"auto","preTokens":967272,"postTokens":21853}}"#.into(),
        ]
        .join("\n");
        assert_eq!(context_usage(&tail), Some(ContextUsage { tokens: 21_853, window: EXTENDED_WINDOW }));
    }

    #[test]
    fn skips_subagent_replies_a_cut_first_line_and_other_records() {
        let tail = [
            r#"partial line from the middle of a record"}"#.to_string(),
            reply("claude-sonnet-5", 1, 1, 50_000),
            r#"{"type":"assistant","isSidechain":true,"message":{"model":"claude-haiku-4-5","usage":{"input_tokens":3,"cache_read_input_tokens":9000}}}"#.into(),
            r#"{"type":"progress"}"#.into(),
        ]
        .join("\n");
        assert_eq!(context_usage(&tail).map(|u| u.tokens), Some(50_002));
    }

    #[test]
    fn uses_the_window_claude_code_reported() {
        let usage_record = r#"{"type":"system","subtype":"local_command","content":"{\"model_usage\":{\"claude-sonnet-5\":{\"contextWindow\":1000000}}}"}"#;
        let tail = [usage_record.to_string(), reply("claude-sonnet-5", 4, 0, 150_000)].join("\n");
        assert_eq!(context_usage(&tail).map(|u| u.window), Some(EXTENDED_WINDOW));
    }

    #[test]
    fn infers_the_window_from_the_model_and_the_token_count() {
        assert_eq!(context_window(Some("claude-haiku-4-5"), 90_000, None), STANDARD_WINDOW);
        assert_eq!(context_window(Some("claude-haiku-4-5[1m]"), 90_000, None), EXTENDED_WINDOW);
        assert_eq!(context_window(Some("claude-opus-5-5"), 90_000, None), EXTENDED_WINDOW);
        // Claude Code reports a 1M window for claude-sonnet-5 (a Time Tracker
        // session at 140,862 tokens showed 70% in Jojo and 14% in Claude).
        assert_eq!(context_window(Some("claude-sonnet-5"), 140_862, None), EXTENDED_WINDOW);
        // Past 200k, the conversation must have the larger window.
        assert_eq!(context_window(Some("claude-haiku-4-5"), 460_298, None), EXTENDED_WINDOW);
        assert_eq!(context_window(None, 10, Some(1_000_000)), EXTENDED_WINDOW);
    }

    #[test]
    fn no_reply_yet_gives_none() {
        assert_eq!(context_usage(r#"{"type":"user","message":{"content":"hi"}}"#), None);
        assert_eq!(context_usage(""), None);
    }
}
