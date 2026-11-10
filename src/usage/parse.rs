//! One line of a session log to an [`Entry`].
//!
//! Claude Code writes an `assistant` line per content block of a response,
//! each carrying the message's usage, so a response is keyed by its message
//! and request ids. Codex writes `event_msg` lines of type `token_count`
//! with the session's running total and the last response's usage; the
//! model comes from the `turn_context` line before them.

use std::hash::{DefaultHasher, Hash, Hasher};

use serde_json::Value;

use super::{Entry, Source, Tokens};

fn num(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn key_of(parts: &[&str]) -> u64 {
    let mut h = DefaultHasher::new();
    parts.hash(&mut h);
    h.finish()
}

/// A Claude Code log line's usage; `None` for every other line.
pub(crate) fn claude_line(line: &str) -> Option<Entry> {
    // Most lines are prompts and tool output, some very long: only the
    // lines that can be an assistant's usage are parsed.
    if !line.contains("\"assistant\"") || !line.contains("\"usage\"") {
        return None;
    }
    let v: Value = serde_json::from_str(line).ok()?;
    if v.get("type")?.as_str()? != "assistant" {
        return None;
    }
    let msg = v.get("message")?;
    let usage = msg.get("usage")?;
    let model = msg.get("model")?.as_str()?;
    // Claude Code's own placeholders (errors, interruptions) cost nothing.
    if model.starts_with('<') {
        return None;
    }
    let tokens = Tokens {
        input: num(usage, "input_tokens"),
        output: num(usage, "output_tokens"),
        cache_read: num(usage, "cache_read_input_tokens"),
        cache_write: num(usage, "cache_creation_input_tokens"),
    };
    if tokens.total() == 0 {
        return None;
    }
    let at = rfc3339(v.get("timestamp")?.as_str()?)?;
    let key = msg
        .get("id")
        .and_then(Value::as_str)
        .map(|id| key_of(&["claude", id, v.get("requestId").and_then(Value::as_str).unwrap_or("")]));
    Some(Entry { at, source: Source::Claude, model: model.to_owned(), tokens, key })
}

/// What a Codex log has said so far that later lines need.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CodexState {
    model: Option<String>,
    last_total: Option<u64>,
}

/// A Codex log line's usage; `None` for every other line, and for a
/// `token_count` that repeats the running total (Codex writes one again
/// when only the rate limits changed).
pub(crate) fn codex_line(state: &mut CodexState, line: &str) -> Option<Entry> {
    if !line.contains("\"token_count\"") && !line.contains("\"turn_context\"") {
        return None;
    }
    let v: Value = serde_json::from_str(line).ok()?;
    let payload = v.get("payload")?;
    match v.get("type")?.as_str()? {
        "turn_context" => {
            if let Some(m) = payload.get("model").and_then(Value::as_str) {
                state.model = Some(m.to_owned());
            }
            None
        }
        "event_msg" if payload.get("type").and_then(Value::as_str) == Some("token_count") => {
            let info = payload.get("info").filter(|i| !i.is_null())?;
            let total = info.pointer("/total_token_usage/total_tokens").and_then(Value::as_u64);
            if total.is_some() && total == state.last_total {
                return None;
            }
            state.last_total = total;
            let last = info.get("last_token_usage")?;
            // OpenAI counts cached prompt tokens inside `input_tokens`.
            let (cached, written) = (num(last, "cached_input_tokens"), num(last, "cache_write_input_tokens"));
            let tokens = Tokens {
                input: num(last, "input_tokens").saturating_sub(cached + written),
                output: num(last, "output_tokens"),
                cache_read: cached,
                cache_write: written,
            };
            if tokens.total() == 0 {
                return None;
            }
            let stamp = v.get("timestamp")?.as_str()?;
            let at = rfc3339(stamp)?;
            // A forked or resumed session copies these lines into its new
            // file: the time and the running total name the same response.
            let key = Some(key_of(&["codex", stamp, &total.unwrap_or(0).to_string()]));
            let model = state.model.clone().unwrap_or_else(|| "unknown".into());
            Some(Entry { at, source: Source::Codex, model, tokens, key })
        }
        _ => None,
    }
}

/// Unix seconds of an RFC 3339 time (`2026-11-07T07:08:09.123Z`, or with
/// an offset).
pub(crate) fn rfc3339(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let num = |from: usize, len: usize| -> Option<i64> { s.get(from..from + len)?.parse().ok() };
    let (y, mo, d) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    let (h, mi, sec) = (num(11, 2)?, num(14, 2)?, num(17, 2)?);
    // Days from the civil date (Howard Hinnant's algorithm).
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let mut t = days * 86_400 + h * 3600 + mi * 60 + sec;
    if let Some(i) = s.get(19..)?.find(['+', '-']).map(|i| i + 19)
        && b.len() >= i + 6
    {
        let sign = if b[i] == b'+' { 1 } else { -1 };
        t -= sign * (num(i + 1, 2)? * 3600 + num(i + 4, 2)? * 60);
    }
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLAUDE: &str = r#"{"type":"user","message":{"role":"user","content":"hi"},"timestamp":"2026-11-10T02:34:50.000Z"}
{"type":"assistant","requestId":"req_1","timestamp":"2026-11-10T02:34:55.325Z","message":{"model":"claude-opus-5-5","id":"msg_1","content":[{"type":"thinking"}],"usage":{"input_tokens":2,"cache_creation_input_tokens":33712,"cache_read_input_tokens":0,"output_tokens":216}}}
{"type":"assistant","requestId":"req_1","timestamp":"2026-11-10T02:34:56.330Z","message":{"model":"claude-opus-5-5","id":"msg_1","content":[{"type":"tool_use"}],"usage":{"input_tokens":2,"cache_creation_input_tokens":33712,"cache_read_input_tokens":0,"output_tokens":216}}}
{"type":"assistant","requestId":"req_2","timestamp":"2026-11-10T02:34:58.669Z","message":{"model":"claude-opus-5-5","id":"msg_2","content":[],"usage":{"input_tokens":2,"cache_creation_input_tokens":4503,"cache_read_input_tokens":33712,"output_tokens":137}}}
{"type":"assistant","timestamp":"2026-11-10T02:35:00.000Z","message":{"model":"<synthetic>","id":"x","content":[],"usage":{"input_tokens":0,"output_tokens":0}}}
not json at all, "assistant" "usage"
"#;

    #[test]
    fn claude_lines_keep_streamed_copies_under_one_key() {
        let entries: Vec<Entry> = CLAUDE.lines().filter_map(claude_line).collect();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].key, entries[1].key);
        assert_ne!(entries[0].key, entries[2].key);
        assert_eq!(entries[0].tokens, Tokens { input: 2, output: 216, cache_read: 0, cache_write: 33712 });
        assert_eq!(entries[0].at, rfc3339("2026-11-10T02:34:55Z").unwrap());
        // Counted once after the sums dedupe them.
        let r = super::super::aggregate(entries.iter(), entries[2].at, &|_| 0);
        assert_eq!(r.today.totals.responses, 2);
        assert_eq!(r.today.totals.tokens.output, 216 + 137);
        assert_eq!(r.today.totals.tokens.cache_read, 33712);
    }

    const CODEX: &str = r#"{"timestamp":"2026-11-04T23:40:00.000Z","type":"session_meta","payload":{"id":"s"}}
{"timestamp":"2026-11-04T23:40:01.000Z","type":"turn_context","payload":{"model":"gpt-5-codex","cwd":"/x"}}
{"timestamp":"2026-11-04T23:40:10.452Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":28171,"cached_input_tokens":0,"output_tokens":265,"total_tokens":28436},"last_token_usage":{"input_tokens":28171,"cached_input_tokens":0,"cache_write_input_tokens":0,"output_tokens":265,"reasoning_output_tokens":48,"total_tokens":28436}}}}
{"timestamp":"2026-11-04T23:40:11.000Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":28171,"cached_input_tokens":0,"output_tokens":265,"total_tokens":28436},"last_token_usage":{"input_tokens":28171,"cached_input_tokens":0,"output_tokens":265,"total_tokens":28436}}}}
{"timestamp":"2026-11-04T23:40:12.000Z","type":"event_msg","payload":{"type":"token_count","info":null}}
{"timestamp":"2026-11-04T23:40:16.959Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":56669,"cached_input_tokens":28032,"output_tokens":483,"total_tokens":57152},"last_token_usage":{"input_tokens":28498,"cached_input_tokens":28032,"cache_write_input_tokens":0,"output_tokens":218,"total_tokens":28716}}}}
{"timestamp":"2026-11-04T23:40:20.000Z","type":"response_item","payload":{"type":"message","content":"token_count"}}
"#;

    #[test]
    fn codex_lines_skip_repeated_totals_and_split_cached_input() {
        let mut st = CodexState::default();
        let entries: Vec<Entry> = CODEX.lines().filter_map(|l| codex_line(&mut st, l)).collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].model, "gpt-5-codex");
        assert_eq!(entries[0].tokens, Tokens { input: 28171, output: 265, cache_read: 0, cache_write: 0 });
        assert_eq!(entries[1].tokens, Tokens { input: 466, output: 218, cache_read: 28032, cache_write: 0 });
        // A copy of the file (a resumed session) has the same keys.
        let mut again = CodexState::default();
        let copy: Vec<Entry> = CODEX.lines().filter_map(|l| codex_line(&mut again, l)).collect();
        assert_eq!(copy[1].key, entries[1].key);
    }

    #[test]
    fn times_parse_with_offsets() {
        assert_eq!(rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(rfc3339("2026-11-07T07:08:09.123Z"), Some(1_794_035_289));
        assert_eq!(rfc3339("2026-11-07T15:08:09+08:00"), Some(1_794_035_289));
        assert_eq!(rfc3339("nope"), None);
    }
}
