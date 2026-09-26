//! Agents: the Claude Code sessions run in a project, with the subagents they started, and the
//! subagents the project (and you) define. Sessions are read from Claude Code's own files
//! (~/.claude/projects/<folder>/<session>.jsonl, its subagents in <session>/subagents/). That
//! format isn't documented, so everything is read defensively: records it doesn't know are
//! skipped, and a file it can't read is reported, not guessed at.

pub mod defined;
pub mod live;
pub mod sessions;
pub mod skills;
pub mod transcript;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A project's setup for the tab (saved in thumbdeck's settings).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Setup {
    /// Tab title
    pub title: String,
    /// Also list your own subagents and skills, for all projects (~/.claude/agents, ~/.claude/skills)
    pub user_agents: bool,
}

impl Default for Setup {
    fn default() -> Self {
        Setup { title: "Agents".into(), user_agents: true }
    }
}

/// Claude Code's folder: $CLAUDE_CONFIG_DIR, or ~/.claude
pub fn claude_home() -> Option<PathBuf> {
    match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
        _ => Some(crate::projects::dirs_home()?.join(".claude")),
    }
}

/// Where Claude Code keeps a folder's sessions: its path with everything but letters and digits
/// turned into "-" ("/home/me/dev/app" -> "-home-me-dev-app")
pub fn sessions_dir(claude_home: &Path, project: &Path) -> PathBuf {
    let name: String = project.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    claude_home.join("projects").join(name)
}

/// Quoted for the shell, in a form bash, zsh and fish all read the same way
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// The command that starts Claude Code on a task: as one of the defined agents ("agent"), or
/// with a skill ("skill", the task after "/skill"). The task is one line: typed into a shell,
/// a newline would end the command early.
pub fn start_command(kind: &str, name: &str, task: &str) -> Result<String, String> {
    let task = task.split_whitespace().collect::<Vec<_>>().join(" ");
    match kind {
        "agent" if task.is_empty() => Ok(format!("claude --agent {}", quote(name))),
        "agent" => Ok(format!("claude --agent {} {}", quote(name), quote(&task))),
        "skill" => Ok(format!("claude {}", quote(format!("/{name} {task}").trim_end()))),
        _ => Err(format!("can't start a {kind}")),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The test data: a real (short) session whose "counter" subagent counted files
    pub fn testdata() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/extensions/agents/testdata")
    }

    pub const SESSION: &str = "5651334b-4fd2-4111-8f2b-8efe1d44f2f0";

    #[test]
    fn session_folders_are_named_after_the_project_path() {
        let home = Path::new("/h/.claude");
        assert_eq!(sessions_dir(home, Path::new("/home/me/dev/thumbdeck")), home.join("projects/-home-me-dev-thumbdeck"));
        assert_eq!(sessions_dir(home, Path::new("/tmp/a-b/c.d_e")), home.join("projects/-tmp-a-b-c-d-e"));
    }

    #[test]
    fn start_commands() {
        assert_eq!(start_command("agent", "counter", "").unwrap(), "claude --agent 'counter'");
        assert_eq!(start_command("agent", "counter", "count\n the  files").unwrap(), "claude --agent 'counter' 'count the files'");
        assert_eq!(start_command("skill", "tidy", "src/it's.rs").unwrap(), "claude '/tidy src/it'\\''s.rs'");
        assert_eq!(start_command("skill", "tidy", " ").unwrap(), "claude '/tidy'");
        assert!(start_command("x", "y", "").is_err());
    }

    #[test]
    fn setup_defaults() {
        let s: Setup = serde_json::from_str("{}").unwrap();
        assert_eq!(s, Setup::default());
        assert!(s.user_agents);
    }
}
