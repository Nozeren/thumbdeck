//! A project's Claude Code sessions, summed up: title, status, model, tokens, cost, and the
//! subagents each one started. Files are read incrementally: a session that grows is only read
//! from where the last look stopped.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// Mid-turn with nothing written for this long: probably interrupted
const QUIET: Duration = Duration::from_secs(600);

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// In the middle of a turn, and writing
    Working,
    /// The turn is over: waiting for you (a session) or finished (a subagent)
    Idle,
    /// In the middle of a turn, but quiet for a long time: probably interrupted
    Stale,
}

#[derive(Serialize, Clone, Debug)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub first_prompt: String,
    pub started: String,
    pub last_activity: String,
    pub model: Option<String>,
    pub branch: Option<String>,
    pub tokens: Tokens,
    /// Claude Code's own total, when it has written one
    pub cost: Option<f64>,
    /// More answers came after that total was written (it's written now and then): it's at least that
    pub cost_behind: bool,
    /// Prompts you sent
    pub prompts: usize,
    pub status: Status,
    pub subagents: Vec<Subagent>,
    pub file: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Subagent {
    pub id: String,
    /// The subagent's name (e.g. "Explore", or one defined in .claude/agents)
    pub agent_type: String,
    pub description: String,
    pub model: Option<String>,
    pub tokens: Tokens,
    pub status: Status,
    pub started: String,
    pub last_activity: String,
    /// 1: started by the session, 2: by a subagent, ...
    pub depth: u64,
    pub file: String,
}

/// What's known about a transcript so far
#[derive(Clone, Debug, Default)]
struct Fold {
    title: Option<String>,
    first_prompt: Option<String>,
    started: Option<String>,
    last: Option<String>,
    model: Option<String>,
    branch: Option<String>,
    cost: Option<f64>,
    cost_behind: bool,
    prompts: usize,
    /// In the middle of a turn: a prompt or tool result is waiting for an answer
    turn_open: bool,
    /// Usage per message: a message's content blocks come as separate records with the same usage
    usage: HashMap<String, Tokens>,
}

/// A message Claude Code sends itself, like a subagent's "I'm done" (it starts a turn, but
/// you didn't type it)
pub fn injected(record: &Value) -> bool {
    record.get("promptSource").and_then(Value::as_str) == Some("system")
        || record.pointer("/origin/kind").and_then(Value::as_str).is_some_and(|k| k != "user")
}

/// The text of a user message, if it's one you typed (not a tool result or something injected)
pub fn prompt_text(record: &Value) -> Option<String> {
    if record.get("isMeta").and_then(Value::as_bool) == Some(true) || injected(record) {
        return None;
    }
    let content = record.get("message")?.get("content")?;
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => {
            if parts.iter().any(|p| p.get("type").and_then(Value::as_str) == Some("tool_result")) {
                return None;
            }
            parts.iter().filter_map(|p| p.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("\n")
        }
        _ => return None,
    };
    let text = text.trim();
    // Slash-command plumbing and interrupt markers aren't prompts
    if text.is_empty() || text.starts_with("<command-") || text.starts_with("<local-command") || text.starts_with("[Request interrupted") {
        return None;
    }
    Some(text.to_string())
}

impl Fold {
    fn apply(&mut self, r: &Value) {
        let time = r.get("timestamp").and_then(Value::as_str);
        if let Some(t) = time {
            self.started.get_or_insert_with(|| t.to_string());
            self.last = Some(t.to_string());
        }
        if let Some(b) = r.get("gitBranch").and_then(Value::as_str).filter(|b| !b.is_empty()) {
            self.branch = Some(b.to_string());
        }
        match r.get("type").and_then(Value::as_str) {
            Some("ai-title") => self.title = r.get("aiTitle").and_then(Value::as_str).map(String::from),
            Some("cost-state") => {
                self.cost = r.get("totalCostUSD").and_then(Value::as_f64).or(self.cost);
                self.cost_behind = false;
            }
            Some("queue-operation") if r.get("operation").and_then(Value::as_str) == Some("enqueue") => self.turn_open = true,
            Some("user") => {
                if let Some(text) = prompt_text(r) {
                    self.prompts += 1;
                    self.first_prompt.get_or_insert(text);
                    self.turn_open = true;
                } else if injected(r) {
                    self.turn_open = true;
                } else if r.pointer("/message/content").and_then(Value::as_array).is_some_and(|parts| {
                    parts.iter().any(|p| p.get("type").and_then(Value::as_str) == Some("tool_result"))
                }) {
                    self.turn_open = true;
                }
            }
            Some("assistant") => {
                let Some(m) = r.get("message") else { return };
                if let Some(model) = m.get("model").and_then(Value::as_str).filter(|m| !m.starts_with('<')) {
                    self.model = Some(model.to_string());
                }
                if let (Some(id), Some(u)) = (m.get("id").and_then(Value::as_str), m.get("usage")) {
                    let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
                    let tokens = Tokens {
                        input: n("input_tokens"),
                        output: n("output_tokens"),
                        cache_read: n("cache_read_input_tokens"),
                        cache_write: n("cache_creation_input_tokens"),
                    };
                    self.usage.insert(id.to_string(), tokens);
                    self.cost_behind = true;
                }
                match m.get("stop_reason").and_then(Value::as_str) {
                    Some("tool_use") => self.turn_open = true,
                    Some(_) => self.turn_open = false, // end_turn, max_tokens, stop_sequence, refusal
                    None => {}
                }
            }
            _ => {} // attachments, snapshots, modes, and whatever comes next
        }
    }

    fn tokens(&self) -> Tokens {
        self.usage.values().fold(Tokens::default(), |a, t| Tokens {
            input: a.input + t.input,
            output: a.output + t.output,
            cache_read: a.cache_read + t.cache_read,
            cache_write: a.cache_write + t.cache_write,
        })
    }

    fn status(&self, modified: SystemTime) -> Status {
        let quiet = SystemTime::now().duration_since(modified).unwrap_or_default() > QUIET;
        match (self.turn_open, quiet) {
            (false, _) => Status::Idle,
            (true, false) => Status::Working,
            (true, true) => Status::Stale,
        }
    }
}

/// Read a transcript: from where the last look stopped when it only grew since
fn fold(path: &Path) -> Option<(Fold, SystemTime)> {
    static SEEN: Mutex<Option<HashMap<PathBuf, (u64, Fold)>>> = Mutex::new(None);
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    let mut seen = SEEN.lock().unwrap();
    let seen = seen.get_or_insert_with(HashMap::new);
    let (mut offset, mut fold) = match seen.get(path) {
        Some((offset, fold)) if *offset <= meta.len() => (*offset, fold.clone()),
        _ => (0, Fold::default()), // new, or rewritten shorter
    };
    if offset < meta.len() {
        let mut file = std::fs::File::open(path).ok()?;
        file.seek(SeekFrom::Start(offset)).ok()?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).ok()?;
        // Only whole lines: the last one may still be being written
        let end = bytes.iter().rposition(|b| *b == b'\n').map(|i| i + 1).unwrap_or(0);
        for line in bytes[..end].split(|b| *b == b'\n') {
            if let Ok(record) = serde_json::from_slice::<Value>(line) {
                fold.apply(&record);
            }
        }
        offset += end as u64;
    }
    seen.insert(path.to_path_buf(), (offset, fold.clone()));
    Some((fold, modified))
}

/// The sessions in a project's session folder, most recent first
pub fn list(dir: &Path) -> Vec<Session> {
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<Session> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
        .filter_map(|p| session(&p))
        .collect();
    out.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
    out
}

fn session(path: &Path) -> Option<Session> {
    let (f, modified) = fold(path)?;
    let id = path.file_stem()?.to_string_lossy().to_string();
    // A file with no conversation in it (only settings records) isn't worth listing
    f.started.as_ref()?;
    let first_prompt = f.first_prompt.clone().unwrap_or_default();
    Some(Session {
        title: f.title.clone().unwrap_or_else(|| shorten(&first_prompt, 80)),
        first_prompt: shorten(&first_prompt, 400),
        started: f.started.clone().unwrap_or_default(),
        last_activity: f.last.clone().unwrap_or_default(),
        model: f.model.clone(),
        branch: f.branch.clone(),
        tokens: f.tokens(),
        cost: f.cost,
        cost_behind: f.cost_behind && f.cost.is_some(),
        prompts: f.prompts,
        status: f.status(modified),
        subagents: subagents(&path.with_extension("").join("subagents")),
        file: path.to_string_lossy().to_string(),
        id,
    })
}

/// A session's subagents: agent-<id>.jsonl with agent-<id>.meta.json next to it
fn subagents(dir: &Path) -> Vec<Subagent> {
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<Subagent> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
        .filter_map(|p| {
            let (f, modified) = fold(&p)?;
            let stem = p.file_stem()?.to_string_lossy().to_string();
            let meta: Value = std::fs::read_to_string(p.with_extension("meta.json"))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or(Value::Null);
            let text = |k: &str| meta.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
            Some(Subagent {
                id: stem.strip_prefix("agent-").unwrap_or(&stem).to_string(),
                agent_type: Some(text("agentType")).filter(|t| !t.is_empty()).unwrap_or_else(|| "subagent".into()),
                description: text("description"),
                model: f.model.clone(),
                tokens: f.tokens(),
                status: f.status(modified),
                started: f.started.clone().unwrap_or_default(),
                last_activity: f.last.clone().unwrap_or_default(),
                depth: meta.get("spawnDepth").and_then(Value::as_u64).unwrap_or(1),
                file: p.to_string_lossy().to_string(),
            })
        })
        .collect();
    out.sort_by(|a, b| a.started.cmp(&b.started));
    out
}

pub fn shorten(text: &str, max: usize) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match one_line.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", &one_line[..i]),
        None => one_line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::agents::tests::{testdata, SESSION};
    use crate::packs::tests::TempDir;

    #[test]
    fn a_real_session_with_a_subagent() {
        let list = list(&testdata().join("session"));
        assert_eq!(list.len(), 1);
        let s = &list[0];
        assert_eq!(s.id, SESSION);
        assert!(s.title.starts_with("Use the counter subagent"), "no ai-title: the first prompt");
        assert_eq!(s.prompts, 1);
        assert_eq!(s.model.as_deref(), Some("claude-haiku-4-5-20251001"));
        assert!(s.tokens.output > 0 && s.tokens.input + s.tokens.cache_read > 0, "{:?}", s.tokens);
        assert!(s.cost.is_some_and(|c| c > 0.0 && c < 0.1));
        assert!(!s.cost_behind, "the total was written after the last answer");
        assert_eq!(s.status, Status::Idle, "it answered");
        assert_eq!(s.subagents.len(), 1);
        let a = &s.subagents[0];
        assert_eq!((a.id.as_str(), a.agent_type.as_str()), ("aebb35f6fa412bbf6", "counter"));
        assert_eq!(a.description, "Count files in the current folder");
        assert_eq!(a.status, Status::Idle, "finished");
        assert_eq!(a.depth, 1);
        assert!(a.tokens.output > 0);
    }

    fn line(v: Value) -> String {
        v.to_string() + "\n"
    }

    fn user(text: &str) -> String {
        line(serde_json::json!({"type": "user", "timestamp": "2026-09-26T10:00:00Z", "message": {"role": "user", "content": text}}))
    }

    fn assistant(id: &str, stop: Option<&str>, output: u64) -> String {
        line(serde_json::json!({"type": "assistant", "timestamp": "2026-09-26T10:00:05Z", "message": {
            "id": id, "model": "claude-opus-5-5", "stop_reason": stop,
            "usage": {"input_tokens": 10, "output_tokens": output, "cache_read_input_tokens": 100}}}))
    }

    #[test]
    fn status_tokens_and_growing_files() {
        let d = TempDir::new("agents-fold");
        let path = d.0.join("s1.jsonl");
        // A prompt, then an answer that calls a tool: working
        std::fs::write(&path, user("fix the build") + &assistant("m1", None, 5) + &assistant("m1", Some("tool_use"), 7)).unwrap();
        let s = session(&path).unwrap();
        assert_eq!(s.status, Status::Working);
        assert_eq!(s.tokens, Tokens { input: 10, output: 7, cache_read: 100, cache_write: 0 }, "a message's usage counted once");
        assert_eq!(s.title, "fix the build");
        assert!(s.cost.is_none() && !s.cost_behind, "no total written: no cost");

        // It grows: the tool result, a final answer, a title; a half-written line at the end is left for later
        let more = line(serde_json::json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "t", "content": "ok"}]}}))
            + &assistant("m2", Some("end_turn"), 3)
            + &line(serde_json::json!({"type": "ai-title", "aiTitle": "Fix the build"}))
            + "{\"type\": \"assist";
        std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(more.as_bytes()).unwrap();
        let s = session(&path).unwrap();
        assert_eq!(s.status, Status::Idle);
        assert_eq!(s.title, "Fix the build");
        assert_eq!(s.tokens.output, 10);
        assert_eq!(s.prompts, 1, "a tool result isn't a prompt");

        // Rewritten shorter (not grown): read again from the start
        std::fs::write(&path, user("again")).unwrap();
        let s = session(&path).unwrap();
        assert_eq!((s.title.as_str(), s.status, s.tokens.output), ("again", Status::Working, 0));

        // Mid-turn but quiet for long: stale
        let old = SystemTime::now() - Duration::from_secs(3600);
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(old).unwrap();
        assert_eq!(session(&path).unwrap().status, Status::Stale);
    }

    #[test]
    fn what_counts_as_a_prompt() {
        let v = |c: Value| serde_json::json!({"type": "user", "message": {"content": c}});
        assert_eq!(prompt_text(&v("hi".into())).as_deref(), Some("hi"));
        assert_eq!(prompt_text(&v(serde_json::json!([{"type": "text", "text": "hi"}]))).as_deref(), Some("hi"));
        assert_eq!(prompt_text(&v(serde_json::json!([{"type": "tool_result", "content": "x"}]))), None);
        assert_eq!(prompt_text(&v("<command-name>/clear</command-name>".into())), None);
        assert_eq!(prompt_text(&v("[Request interrupted by user]".into())), None);
        let mut meta = v("injected".into());
        meta["isMeta"] = true.into();
        assert_eq!(prompt_text(&meta), None);
        let mut notice = v("<task-notification>done</task-notification>".into());
        notice["origin"] = serde_json::json!({"kind": "task-notification"});
        notice["promptSource"] = "system".into();
        assert_eq!(prompt_text(&notice), None, "a subagent's notice isn't your prompt");
        assert!(injected(&notice));
    }

    #[test]
    fn files_without_a_conversation_or_folder_are_skipped() {
        let d = TempDir::new("agents-empty");
        d.file("settings-only.jsonl", "{\"type\": \"mode\"}\nnot json\n");
        assert!(list(&d.0).is_empty());
        assert!(list(&d.0.join("missing")).is_empty());
        assert_eq!(shorten("a  b\nc", 10), "a b c");
        assert_eq!(shorten("abcdef", 3), "abc…");
    }

    use std::io::Write;
}
