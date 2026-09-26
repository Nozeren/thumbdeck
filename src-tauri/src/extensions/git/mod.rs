//! Git: a read-only look at the project's repo: uncommitted changes and their diffs, the
//! branch against its upstream, recent commits, branches and stashes. It never changes the
//! repo (that's for your own tools), and asks git not to take locks, so it can't get in their way.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Stdio};

/// A project's setup for the tab (saved in thumbdeck's settings).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Setup {
    /// Tab title
    pub title: String,
    /// How many recent commits to list
    pub commits: usize,
}

impl Default for Setup {
    fn default() -> Self {
        Setup { title: "Git".into(), commits: 100 }
    }
}

/// Diffs longer than this are cut (a huge generated file would freeze the page)
const MAX_DIFF: usize = 300_000;

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct Branch {
    /// None: not on a branch (a detached HEAD)
    pub name: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// The upstream branch was deleted
    pub gone: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Change {
    pub path: String,
    /// Where a renamed or copied file came from
    pub from: Option<String>,
    /// Staged (index) and unstaged (worktree) states: M modified, A added, D deleted,
    /// R renamed, C copied, U conflict, ? untracked, ' ' none
    pub staged: char,
    pub unstaged: char,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Status {
    pub branch: Branch,
    pub changes: Vec<Change>,
    /// When the repo last fetched (seconds since 1970), if it ever did
    pub fetched: Option<u64>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Commit {
    pub hash: String,
    pub short: String,
    pub author: String,
    pub date: String,
    pub subject: String,
    /// Branch and tag names pointing at it
    pub refs: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct LocalBranch {
    pub name: String,
    pub current: bool,
    pub upstream: Option<String>,
    /// "[ahead 1, behind 2]", "[gone]", or ""
    pub track: String,
    pub date: String,
    pub subject: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Stash {
    /// "stash@{0}"
    pub name: String,
    pub subject: String,
    pub date: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Branches {
    pub branches: Vec<LocalBranch>,
    pub stashes: Vec<Stash>,
}

/// Run git in the project; its output, or its error message
fn git(path: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(["-c", "core.quotepath=off", "-c", "color.ui=never", "--no-pager"])
        .args(args)
        .current_dir(path)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .output()
        .map_err(|_| "git isn't installed".to_string())?;
    // `git diff --no-index` exits 1 when the files differ
    if out.status.success() || (args.contains(&"--no-index") && out.status.code() == Some(1)) {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(err.lines().find(|l| !l.trim().is_empty()).unwrap_or("git failed").trim().to_string())
    }
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
    let (name, upstream) = match names.split_once("...") {
        Some((n, u)) => (n, Some(u.to_string())),
        None => (names, None),
    };
    let mut b = Branch { name: Some(name.to_string()), upstream, ..Branch::default() };
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

/// `git status --porcelain=v1 -b -z`: the branch line, then "XY path" entries (a rename's
/// entry is followed by the path it came from)
fn parse_status(out: &str) -> (Branch, Vec<Change>) {
    let mut entries = out.split('\0').filter(|e| !e.is_empty());
    let mut branch = Branch::default();
    let mut changes = Vec::new();
    while let Some(e) = entries.next() {
        if e.starts_with("## ") {
            branch = parse_branch(e);
            continue;
        }
        let mut chars = e.chars();
        let (Some(x), Some(y)) = (chars.next(), chars.next()) else { continue };
        let path = e.get(3..).unwrap_or("").to_string();
        let from = if matches!(x, 'R' | 'C') { entries.next().map(String::from) } else { None };
        // Conflicts show as U in either column, or AA / DD
        let conflict = x == 'U' || y == 'U' || (x == y && matches!(x, 'A' | 'D'));
        let (staged, unstaged) = if conflict { ('U', 'U') } else { (x, y) };
        changes.push(Change { path, from, staged, unstaged });
    }
    (branch, changes)
}

pub fn status(path: &Path) -> Result<Status, String> {
    let out = git(path, &["status", "--porcelain=v1", "-b", "-z", "--untracked-files=all"])?;
    let (branch, changes) = parse_status(&out);
    let git_dir = git(path, &["rev-parse", "--git-dir"]).map(|d| path.join(d.trim())).ok();
    let fetched = git_dir
        .and_then(|d| std::fs::metadata(d.join("FETCH_HEAD")).ok())
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs());
    Ok(Status { branch, changes, fetched })
}

fn cut(mut text: String) -> String {
    if text.len() > MAX_DIFF {
        let mut end = MAX_DIFF;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str("\n… (cut: the diff is too long to show)");
    }
    text
}

/// A file's diff: its staged or unstaged changes, or the whole file when it's untracked
pub fn diff(path: &Path, file: &str, staged: bool, untracked: bool) -> Result<String, String> {
    let out = if untracked {
        git(path, &["diff", "--no-index", "--", "/dev/null", file])?
    } else if staged {
        git(path, &["diff", "--cached", "--", file])?
    } else {
        git(path, &["diff", "--", file])?
    };
    Ok(cut(out))
}

/// Everything to review, as one diff: "changes" (all uncommitted changes, new files too), a
/// commit (by hash), or a stash ("stash@{0}")
pub fn review(path: &Path, what: &str) -> Result<String, String> {
    if what == "changes" {
        // Against the last commit; a repo without commits has only the staged and unstaged diffs
        let mut out = git(path, &["diff", "HEAD", "--find-renames"])
            .or_else(|_| Ok::<_, String>(git(path, &["diff", "--cached"])? + &git(path, &["diff"])?))?;
        let untracked = git(path, &["ls-files", "--others", "--exclude-standard", "-z"])?;
        for file in untracked.split('\0').filter(|f| !f.is_empty()) {
            out.push_str(&git(path, &["diff", "--no-index", "--", "/dev/null", file])?);
            if out.len() > MAX_DIFF {
                break;
            }
        }
        Ok(cut(out))
    } else if what.starts_with("stash@{") {
        Ok(cut(git(path, &["stash", "show", "--patch", "--find-renames", what])?))
    } else if !what.starts_with('-') && !what.is_empty() {
        Ok(cut(git(path, &["show", "--format=", "--patch", "--find-renames", what])?))
    } else {
        Err(format!("nothing to review called {what}"))
    }
}

/// Fields and records are split with the unit and record separators (they can't be in a message)
fn records(out: &str, fields: usize) -> impl Iterator<Item = Vec<&str>> {
    out.split('\u{1e}').map(|r| r.trim_start_matches('\n')).filter(|r| !r.is_empty()).map(|r| r.split('\u{1f}').collect::<Vec<_>>()).filter(move |f| f.len() == fields)
}

pub fn log(path: &Path, count: usize) -> Result<Vec<Commit>, String> {
    let n = format!("-n{}", count.max(1));
    let out = match git(path, &["log", &n, "--format=%H%x1f%h%x1f%an%x1f%aI%x1f%s%x1f%D%x1e"]) {
        Ok(out) => out,
        // A repo without commits yet
        Err(e) if e.contains("does not have any commits") => String::new(),
        Err(e) => return Err(e),
    };
    Ok(records(&out, 6)
        .map(|f| Commit { hash: f[0].into(), short: f[1].into(), author: f[2].into(), date: f[3].into(), subject: f[4].into(), refs: f[5].into() })
        .collect())
}

/// A commit's full message and diff
pub fn show(path: &Path, hash: &str) -> Result<String, String> {
    if hash.starts_with('-') {
        return Err("not a commit".into());
    }
    Ok(cut(git(path, &["show", "--format=%B", "--stat", "--patch", hash])?))
}

pub fn branches(path: &Path) -> Result<Branches, String> {
    let out = git(
        path,
        &["for-each-ref", "refs/heads", "--sort=-committerdate",
          "--format=%(refname:short)%1f%(HEAD)%1f%(upstream:short)%1f%(upstream:track)%1f%(committerdate:iso-strict)%1f%(contents:subject)%1e"],
    )?;
    let branches = records(&out, 6)
        .map(|f| LocalBranch {
            name: f[0].into(),
            current: f[1] == "*",
            upstream: Some(f[2].to_string()).filter(|u| !u.is_empty()),
            track: f[3].into(),
            date: f[4].into(),
            subject: f[5].into(),
        })
        .collect();
    let out = git(path, &["stash", "list", "--format=%gd%x1f%s%x1f%cI%x1e"])?;
    let stashes = records(&out, 3).map(|f| Stash { name: f[0].into(), subject: f[1].into(), date: f[2].into() }).collect();
    Ok(Branches { branches, stashes })
}

/// A stash's diff
pub fn stash(path: &Path, name: &str) -> Result<String, String> {
    if !name.starts_with("stash@{") {
        return Err("not a stash".into());
    }
    Ok(cut(git(path, &["stash", "show", "--stat", "--patch", name])?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_lines() {
        let b = parse_branch("## main...origin/main [ahead 2, behind 1]");
        assert_eq!((b.name.as_deref(), b.upstream.as_deref(), b.ahead, b.behind, b.gone), (Some("main"), Some("origin/main"), 2, 1, false));
        assert_eq!(parse_branch("## feat...origin/feat [gone]").gone, true);
        assert_eq!(parse_branch("## main"), Branch { name: Some("main".into()), ..Branch::default() });
        assert_eq!(parse_branch("## No commits yet on main").name.as_deref(), Some("main"));
        assert_eq!(parse_branch("## HEAD (no branch)").name, None);
    }

    #[test]
    fn status_entries() {
        let out = "## main\0M  a.rs\0 M b rs\0R  new.rs\0old.rs\0?? dir/c.txt\0UU d.rs\0AA e.rs\0";
        let (branch, changes) = parse_status(out);
        assert_eq!(branch.name.as_deref(), Some("main"));
        let short: Vec<_> = changes.iter().map(|c| (c.path.as_str(), c.from.as_deref(), c.staged, c.unstaged)).collect();
        assert_eq!(short, [
            ("a.rs", None, 'M', ' '),
            ("b rs", None, ' ', 'M'),
            ("new.rs", Some("old.rs"), 'R', ' '),
            ("dir/c.txt", None, '?', '?'),
            ("d.rs", None, 'U', 'U'),
            ("e.rs", None, 'U', 'U'),
        ]);
    }

    /// Runs a real git in a new repo; skipped when git isn't installed
    #[test]
    fn a_real_repo() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("thumbdeck-git-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            let ok = Command::new("git").args(["-c", "user.name=T", "-c", "user.email=t@example.test", "-c", "commit.gpgsign=false"]).args(args).current_dir(&dir).output().unwrap().status.success();
            assert!(ok, "git {args:?}");
        };
        run(&["init", "-q", "-b", "main"]);
        assert!(log(&dir, 10).unwrap().is_empty(), "no commits yet");
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        run(&["add", "a.txt"]);
        run(&["commit", "-q", "-m", "First"]);
        std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(dir.join("new.txt"), "hello\n").unwrap();

        let s = status(&dir).unwrap();
        assert_eq!(s.branch.name.as_deref(), Some("main"));
        assert_eq!(s.changes.len(), 2);
        assert!(diff(&dir, "a.txt", false, false).unwrap().contains("+two"));
        assert!(diff(&dir, "new.txt", false, true).unwrap().contains("+hello"));
        assert_eq!(diff(&dir, "a.txt", true, false).unwrap(), "", "nothing staged");

        let commits = log(&dir, 10).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].subject, "First");
        assert!(show(&dir, &commits[0].hash).unwrap().contains("+one"));
        assert!(show(&dir, "--output=/tmp/x").is_err(), "options aren't commits");

        let all = review(&dir, "changes").unwrap();
        assert!(all.contains("+two") && all.contains("+hello"), "changes and new files: {all}");
        assert!(review(&dir, &commits[0].hash).unwrap().contains("+one"));
        assert!(review(&dir, "--x").is_err());

        run(&["stash", "-q"]);
        let b = branches(&dir).unwrap();
        assert_eq!((b.branches.len(), b.branches[0].current), (1, true));
        assert_eq!(b.stashes.len(), 1);
        assert!(stash(&dir, &b.stashes[0].name).unwrap().contains("+two"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn long_diffs_are_cut() {
        let text = cut("é".repeat(MAX_DIFF));
        assert!(text.ends_with("too long to show)") && text.len() < MAX_DIFF + 60);
    }
}
