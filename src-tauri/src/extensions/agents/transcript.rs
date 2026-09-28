//! A session's (or subagent's) conversation, for reading: your prompts, the answers, and each
//! tool call with its result. A call that started a subagent points at it.

use super::sessions::{injected, prompt_text, shorten};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// Tool results and inputs longer than this are cut for the view
const LONG: usize = 4000;

#[derive(Serialize, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Entry {
    /// Something you asked
    Prompt { time: String, text: String },
    /// An answer
    Text { time: String, text: String },
    /// A tool call and, once it's back, its result. `agent` is the subagent it started.
    Tool { time: String, id: String, name: String, input: Value, result: Option<ToolResult>, agent: Option<String> },
    /// Interruptions and other things that happened
    Note { time: String, text: String },
}

#[derive(Serialize, Debug, PartialEq)]
pub struct ToolResult {
    pub text: String,
    pub error: bool,
    /// Length before it was cut
    pub length: usize,
}

fn cut(text: &str) -> String {
    match text.char_indices().nth(LONG) {
        Some((i, _)) => format!("{}…", &text[..i]),
        None => text.to_string(),
    }
}

/// Long strings in a tool's input, cut
fn cut_value(v: &Value) -> Value {
    match v {
        Value::String(s) => Value::String(cut(s)),
        Value::Array(a) => Value::Array(a.iter().map(cut_value).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), cut_value(v))).collect()),
        other => other.clone(),
    }
}

/// A tool result's content: text, or text blocks (images and the like are named)
fn result_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .map(|p| match p.get("type").and_then(Value::as_str) {
                Some("text") => p.get("text").and_then(Value::as_str).unwrap_or_default().to_string(),
                Some(other) => format!("[{other}]"),
                None => String::new(),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// The text between <name> and </name>
fn tag<'a>(body: &'a str, name: &str) -> Option<&'a str> {
    let start = body.find(&format!("<{name}>"))? + name.len() + 2;
    let end = start + body[start..].find(&format!("</{name}>"))?;
    Some(body[start..end].trim()).filter(|t| !t.is_empty())
}

/// A message Claude Code sent itself, in words: a subagent's notice says what finished and its result
fn notice(record: &Value, body: &str) -> String {
    if let Some(summary) = tag(body, "summary") {
        return match tag(body, "result") {
            Some(result) => format!("subagent: {summary} → {}", shorten(result, 160)),
            None => format!("subagent: {summary}"),
        };
    }
    let kind = record.pointer("/origin/kind").and_then(Value::as_str).unwrap_or("message").replace('-', " ");
    // Otherwise the words between the tags
    let words: Vec<&str> = body.split(['<', '>']).enumerate().filter(|(i, _)| i % 2 == 0).map(|(_, t)| t).collect();
    format!("{kind}: {}", shorten(&words.join(" "), 200))
}

pub fn read(path: &Path) -> Result<Vec<Entry>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("couldn't read {}: {e}", path.display()))?;
    let mut out: Vec<Entry> = Vec::new();
    let mut tools: HashMap<String, usize> = HashMap::new(); // tool_use id -> entry
    for line in text.lines() {
        let Ok(r) = serde_json::from_str::<Value>(line) else { continue };
        let time = r.get("timestamp").and_then(Value::as_str).unwrap_or_default().to_string();
        let content = r.pointer("/message/content");
        match r.get("type").and_then(Value::as_str) {
            Some("user") => {
                if let Some(text) = prompt_text(&r) {
                    out.push(Entry::Prompt { time, text });
                    continue;
                }
                if injected(&r) {
                    let body = content.map(result_text).unwrap_or_default();
                    out.push(Entry::Note { time, text: notice(&r, &body) });
                    continue;
                }
                let parts = content.and_then(Value::as_array).cloned().unwrap_or_default();
                for part in parts.iter().filter(|p| p.get("type").and_then(Value::as_str) == Some("tool_result")) {
                    let id = part.get("tool_use_id").and_then(Value::as_str).unwrap_or_default();
                    let full = result_text(part.get("content").unwrap_or(&Value::Null));
                    let result = ToolResult {
                        text: cut(&full),
                        error: part.get("is_error").and_then(Value::as_bool).unwrap_or(false),
                        length: full.chars().count(),
                    };
                    let agent = r.pointer("/toolUseResult/agentId").and_then(Value::as_str).map(String::from);
                    match tools.get(id).and_then(|i| out.get_mut(*i)) {
                        Some(Entry::Tool { result: slot, agent: a, .. }) => {
                            *slot = Some(result);
                            if agent.is_some() {
                                *a = agent;
                            }
                        }
                        _ => out.push(Entry::Note { time: time.clone(), text: format!("result of an earlier call: {}", result.text) }),
                    }
                }
                let interrupted = content.and_then(Value::as_str).or_else(|| {
                    parts.iter().find_map(|p| p.get("text").and_then(Value::as_str))
                });
                if let Some(t) = interrupted.filter(|t| t.starts_with("[Request interrupted")) {
                    out.push(Entry::Note { time, text: t.trim_matches(['[', ']']).to_string() });
                }
            }
            Some("assistant") => {
                for part in content.and_then(Value::as_array).into_iter().flatten() {
                    match part.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            let text = part.get("text").and_then(Value::as_str).unwrap_or_default().trim().to_string();
                            if !text.is_empty() {
                                out.push(Entry::Text { time: time.clone(), text });
                            }
                        }
                        Some("tool_use") => {
                            let id = part.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
                            tools.insert(id.clone(), out.len());
                            out.push(Entry::Tool {
                                time: time.clone(),
                                name: part.get("name").and_then(Value::as_str).unwrap_or("tool").to_string(),
                                input: cut_value(part.get("input").unwrap_or(&Value::Null)),
                                result: None,
                                agent: None,
                                id,
                            });
                        }
                        _ => {} // thinking, and anything new
                    }
                }
            }
            Some("system") => {
                let text = r.get("content").and_then(Value::as_str).unwrap_or_default().trim();
                if !text.is_empty() && r.get("isMeta").and_then(Value::as_bool) != Some(true) {
                    out.push(Entry::Note { time, text: cut(text) });
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::agents::tests::{testdata, SESSION};

    #[test]
    fn the_real_session_reads_as_prompt_call_and_answer() {
        let entries = read(&testdata().join(format!("session/{SESSION}.jsonl"))).unwrap();
        assert!(matches!(&entries[0], Entry::Prompt { text, .. } if text.starts_with("Use the counter subagent")));
        let agent = entries.iter().find_map(|e| match e {
            Entry::Tool { name, input, agent, result, .. } if name == "Agent" => Some((input, agent, result)),
            _ => None,
        });
        let (input, agent, result) = agent.expect("the Agent call");
        assert_eq!(input["subagent_type"], "counter");
        assert_eq!(agent.as_deref(), Some("aebb35f6fa412bbf6"), "the call points at its subagent");
        assert!(result.is_some());
        assert!(matches!(entries.last(), Some(Entry::Text { text, .. }) if text.contains('3')), "{:?}", entries.last());
        let notes: Vec<&String> = entries.iter().filter_map(|e| if let Entry::Note { text, .. } = e { Some(text) } else { None }).collect();
        assert_eq!(notes.len(), 1, "the subagent's notice: {notes:?}");
        assert_eq!(notes[0], "subagent: Agent \"Count files in the current folder\" finished → 3");
        let other = serde_json::json!({"origin": {"kind": "scheduled-task"}});
        assert_eq!(notice(&other, "<a>wake</a> <b>up</b>"), "scheduled task: wake up");
    }

    #[test]
    fn the_subagent_transcript_reads_too() {
        let entries = read(&testdata().join(format!("session/{SESSION}/subagents/agent-aebb35f6fa412bbf6.jsonl"))).unwrap();
        assert!(entries.iter().any(|e| matches!(e, Entry::Tool { name, result: Some(_), .. } if name == "Glob")));
        assert!(read(Path::new("/nonexistent.jsonl")).is_err());
    }

    #[test]
    fn long_results_are_cut_and_errors_kept() {
        let d = crate::testutil::TempDir::new("agents-transcript");
        let long = "x".repeat(LONG + 10);
        let lines = [
            serde_json::json!({"type": "assistant", "message": {"content": [
                {"type": "thinking", "thinking": "hmm"},
                {"type": "tool_use", "id": "t1", "name": "Bash", "input": {"command": long}}]}}),
            serde_json::json!({"type": "user", "message": {"content": [
                {"type": "tool_result", "tool_use_id": "t1", "is_error": true, "content": [{"type": "text", "text": long}, {"type": "image"}]}]}}),
            serde_json::json!({"type": "user", "message": {"content": "[Request interrupted by user]"}}),
        ];
        d.file("t.jsonl", &lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n"));
        let entries = read(&d.0.join("t.jsonl")).unwrap();
        assert_eq!(entries.len(), 2, "thinking isn't shown: {entries:?}");
        let Entry::Tool { input, result: Some(r), .. } = &entries[0] else { panic!() };
        assert_eq!(input["command"].as_str().unwrap().chars().count(), LONG + 1, "cut, with …");
        assert!(r.error && r.length == LONG + 10 + "\n[image]".len());
        assert!(matches!(&entries[1], Entry::Note { text, .. } if text == "Request interrupted by user"));
    }
}
