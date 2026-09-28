//! The selected project's branch for the status line: its name, how it stands against its
//! upstream, and how many files have uncommitted changes. (Everything else about git is the
//! Git plugin's.) Asks git not to take locks, so it can't get in your tools' way.

use serde::Serialize;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct Branch {
    /// None: not on a branch (a detached HEAD)
    pub name: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// The upstream branch was deleted
    pub gone: bool,
    /// Files with uncommitted changes (new ones too)
    pub changed: usize,
}

/// "## main...origin/main [ahead 2, behind 1]" and friends
fn parse_branch(line: &str) -> Branch {
    let line = line.trim_start_matches("## ");
    if let Some(name) = line.strip_prefix("No commits yet on ").or_else(|| line.strip_prefix("Initial commit on ")) {
        return Branch { name: Some(name.to_string()), ..Branch::default() };
    }
    if line.starts_with("HEAD (no branch)") {
        return Branch::default();
    }
    let (names, track) = match line.split_once(" [") {
        Some((n, t)) => (n, t.trim_end_matches(']')),
        None => (line, ""),
    };
    let name = names.split_once("...").map(|(n, _)| n).unwrap_or(names);
    let mut b = Branch { name: Some(name.to_string()), ..Branch::default() };
    for part in track.split(", ") {
        match part.split_once(' ') {
            Some(("ahead", n)) => b.ahead = n.parse().unwrap_or(0),
            Some(("behind", n)) => b.behind = n.parse().unwrap_or(0),
            _ if part == "gone" => b.gone = true,
            _ => {}
        }
    }
    b
}

/// `git status --porcelain=v1 -b -z`: the branch line, then one entry per file (a rename's is
/// followed by the path it came from)
fn parse_status(out: &str) -> Branch {
    let mut entries = out.split('\0').filter(|e| !e.is_empty());
    let mut branch = Branch::default();
    let mut changed = 0;
    while let Some(e) = entries.next() {
        if e.starts_with("## ") {
            branch = parse_branch(e);
            continue;
        }
        changed += 1;
        if matches!(e.chars().next(), Some('R' | 'C')) {
            entries.next();
        }
    }
    Branch { changed, ..branch }
}

pub fn status(path: &Path) -> Result<Branch, String> {
    let out = Command::new("git")
        .args(["-c", "core.quotepath=off", "status", "--porcelain=v1", "-b", "-z", "--untracked-files=all"])
        .current_dir(path)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .output()
        .map_err(|_| "git isn't installed".to_string())?;
    if !out.status.success() {
        return Err("not a git repository".into());
    }
    Ok(parse_status(&String::from_utf8_lossy(&out.stdout)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_lines() {
        let b = parse_branch("## main...origin/main [ahead 2, behind 1]");
        assert_eq!((b.name.as_deref(), b.ahead, b.behind, b.gone), (Some("main"), 2, 1, false));
        assert!(parse_branch("## feat...origin/feat [gone]").gone);
        assert_eq!(parse_branch("## main").name.as_deref(), Some("main"));
        assert_eq!(parse_branch("## No commits yet on main").name.as_deref(), Some("main"));
        assert_eq!(parse_branch("## HEAD (no branch)").name, None);
    }

    #[test]
    fn changed_files_are_counted_once() {
        let b = parse_status("## main\0M  a.rs\0 M b rs\0R  new.rs\0old.rs\0?? dir/c.txt\0");
        assert_eq!((b.name.as_deref(), b.changed), (Some("main"), 4));
    }
}
