//! The Claude Code sessions running right now, in any project: Claude Code writes one file
//! per running session (~/.claude/sessions/<pid>.json) with its folder and status ("busy",
//! "idle", "waiting" for you). Undocumented, so read defensively: a file whose process is gone
//! (a crash leaves it behind) or that can't be read is skipped.

use serde::Serialize;
use serde_json::Value;
use std::path::Path;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Live {
    pub session_id: String,
    /// The folder it runs in
    pub cwd: String,
    /// "busy", "idle", "waiting", or whatever a newer Claude Code writes
    pub status: String,
}

fn alive(pid: i32) -> bool {
    // Signal 0 only checks: EPERM means it's there but someone else's
    pid > 0 && (unsafe { libc::kill(pid, 0) } == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM))
}

pub fn list(claude_home: &Path) -> Vec<Live> {
    let Ok(entries) = std::fs::read_dir(claude_home.join("sessions")) else { return vec![] };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .filter_map(|p| {
            let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).ok()?).ok()?;
            let pid = i32::try_from(v.get("pid")?.as_i64()?).ok()?;
            let text = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            alive(pid).then(|| Live { session_id: text("sessionId"), cwd: text("cwd"), status: text("status") })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_sessions_whose_process_is_alive() {
        let home = std::env::temp_dir().join(format!("thumbdeck-live-test-{}", std::process::id()));
        let dir = home.join("sessions");
        std::fs::create_dir_all(&dir).unwrap();
        let me = std::process::id();
        std::fs::write(dir.join(format!("{me}.json")), format!(r#"{{"pid":{me},"sessionId":"s1","cwd":"/p","status":"busy"}}"#)).unwrap();
        // A pid that can't exist (above Linux's and macOS's limits): a leftover of a crash
        std::fs::write(dir.join("99999999.json"), r#"{"pid":99999999,"sessionId":"s2","cwd":"/p","status":"idle"}"#).unwrap();
        std::fs::write(dir.join("1.key"), "not a session").unwrap();
        std::fs::write(dir.join("bad.json"), "{").unwrap();
        assert_eq!(list(&home), [Live { session_id: "s1".into(), cwd: "/p".into(), status: "busy".into() }]);
        assert!(list(Path::new("/missing")).is_empty());
        std::fs::remove_dir_all(home).unwrap();
    }
}
