//! Pull requests: the open PRs of the project's GitHub repo that concern you, in the order
//! to deal with them: asked to review again, asked to review, reviewed, yours. Read with the
//! GitHub CLI (`gh`), so it uses your gh login.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Stdio};

/// A project's setup for the tab (saved in thumbdeck's settings).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Setup {
    /// Tab title
    pub title: String,
    /// "owner/name"; empty: from the project's git remote (origin); "demo": sample PRs
    pub repo: String,
    /// Not updated for this many days: stale
    pub stale_days: u32,
    /// Reviewers left out (bots)
    pub bots: Vec<String>,
    /// Cut from the end of user names ("ann_corp" -> "ann" with "_corp")
    pub trim_suffix: String,
    /// Minutes between refreshes while the tab is shown
    pub refresh_minutes: u32,
}

impl Default for Setup {
    fn default() -> Self {
        Setup {
            title: "Pull requests".into(),
            repo: String::new(),
            stale_days: 7,
            bots: vec!["copilot-pull-request-reviewer".into()],
            trim_suffix: String::new(),
            refresh_minutes: 5,
        }
    }
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    /// Asked to review, and you reviewed it before
    ReReview,
    /// Asked to review
    Review,
    /// You reviewed it, and aren't asked again
    Reviewed,
    /// You opened it
    Mine,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Reviewer {
    pub name: String,
    /// APPROVED, CHANGES_REQUESTED, COMMENTED, ... or PENDING (asked, no review yet)
    pub state: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct LastComment {
    pub author: String,
    pub updated: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Pr {
    pub category: Category,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub author: String,
    pub updated: String,
    pub draft: bool,
    /// SUCCESS, FAILURE, PENDING, ... or "" (no checks)
    pub checks: String,
    /// MERGEABLE, CONFLICTING or UNKNOWN
    pub mergeable: String,
    pub labels: Vec<String>,
    pub comments: u64,
    pub last_comment: Option<LastComment>,
    pub reviewers: Vec<Reviewer>,
    /// The unread GitHub notification about it (marked read when you open it)
    pub notification: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct PrList {
    pub repo: String,
    /// Sample PRs, not from GitHub (repo "demo")
    pub demo: bool,
    pub user: String,
    pub prs: Vec<Pr>,
}

const QUERY: &str = "query($owner: String!, $name: String!) {
  repository(owner: $owner, name: $name) {
    pullRequests(first: 50, states: OPEN, orderBy: {field: UPDATED_AT, direction: DESC}) {
      nodes {
        number title isDraft url updatedAt mergeable
        author { login }
        labels(first: 10) { nodes { name } }
        commits(last: 1) { nodes { commit { statusCheckRollup { state } } } }
        reviewRequests(first: 10) { nodes { requestedReviewer { ... on User { login } ... on Team { name } } } }
        reviews(last: 50) { nodes { author { login } state } }
        comments(last: 1) { totalCount nodes { author { login } updatedAt } }
      }
    }
  }
}";

/// "owner/name" from a GitHub remote URL (https, ssh or scp-like), if it's one
pub fn repo_from_url(url: &str) -> Option<String> {
    let rest = url.trim().split_once("github.com").map(|(_, r)| r)?;
    let rest = rest.trim_start_matches([':', '/']).trim_end_matches('/').trim_end_matches(".git");
    let mut parts = rest.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(owner), Some(name), None) if !owner.is_empty() && !name.is_empty() => Some(format!("{owner}/{name}")),
        _ => None,
    }
}

/// The setup's repo, or the one the project's origin remote points at
pub fn repo(path: &Path, setup: &Setup) -> Result<String, String> {
    if !setup.repo.trim().is_empty() {
        return Ok(setup.repo.trim().to_string());
    }
    let out = Command::new("git").args(["remote", "get-url", "origin"]).current_dir(path).stdin(Stdio::null()).output();
    let url = out.ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).to_string());
    match url {
        Some(url) => repo_from_url(&url).ok_or_else(|| format!("origin isn't a GitHub repo ({}): set the repo in the tab setup", url.trim())),
        None => Err("no git remote 'origin': set the repo in the tab setup".into()),
    }
}

/// Run the GitHub CLI; its error message when it fails
fn gh(args: &[&str]) -> Result<String, String> {
    let out = Command::new("gh")
        .args(args)
        .env_clear()
        .envs(crate::runner::shell_env().iter().cloned())
        .stdin(Stdio::null())
        .output()
        .map_err(|_| "the GitHub CLI (gh) isn't installed".to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!("gh: {}", err.lines().find(|l| !l.trim().is_empty()).unwrap_or("failed")))
    }
}

/// The repo's PRs that concern you, with their notifications (all fetched at once)
pub fn fetch(repo: &str, setup: &Setup) -> Result<PrList, String> {
    if repo == "demo" {
        return Ok(demo(setup));
    }
    let (owner, name) = repo.split_once('/').ok_or_else(|| format!("'{repo}' isn't owner/name"))?;
    let (user, prs, notifications) = std::thread::scope(|s| {
        let user = s.spawn(|| gh(&["api", "user", "--jq", ".login"]));
        let prs = s.spawn(|| gh(&["api", "graphql", "-f", &format!("query={QUERY}"), "-f", &format!("owner={owner}"), "-f", &format!("name={name}")]));
        // Unread only; not knowing them just leaves the bells off
        let notifications = s.spawn(|| gh(&["api", &format!("repos/{repo}/notifications")]).unwrap_or_default());
        (user.join().unwrap(), prs.join().unwrap(), notifications.join().unwrap())
    });
    let user = user?.trim().to_string();
    let prs: Value = serde_json::from_str(&prs?).map_err(|e| format!("couldn't read gh's answer: {e}"))?;
    if prs.pointer("/data/repository").is_none_or(Value::is_null) {
        return Err(format!("no repo {repo} on GitHub (or no access)"));
    }
    let notifications = serde_json::from_str(&notifications).unwrap_or(Value::Null);
    Ok(PrList { repo: repo.to_string(), demo: false, prs: sort(&prs, &notifications, &user, setup), user })
}

/// Sample PRs (the test data) to see the tab without any PRs of your own
fn demo(setup: &Setup) -> PrList {
    let prs = serde_json::from_str(include_str!("testdata/repo.json")).unwrap_or(Value::Null);
    let notifications = serde_json::from_str(include_str!("testdata/notifications.json")).unwrap_or(Value::Null);
    let setup = Setup { trim_suffix: "_corp".into(), ..setup.clone() };
    PrList { repo: "demo".into(), demo: true, prs: sort(&prs, &notifications, "me_corp", &setup), user: "me".into() }
}

/// Mark a GitHub notification read
pub fn mark_read(thread: &str) -> Result<(), String> {
    gh(&["api", "-X", "PATCH", &format!("notifications/threads/{thread}")]).map(|_| ())
}

fn str_at<'a>(v: &'a Value, pointer: &str) -> &'a str {
    v.pointer(pointer).and_then(Value::as_str).unwrap_or("")
}

fn nodes<'a>(v: &'a Value, pointer: &str) -> &'a [Value] {
    v.pointer(pointer).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// The PRs that concern `user`, in the order to deal with them (newest first within each)
fn sort(response: &Value, notifications: &Value, user: &str, setup: &Setup) -> Vec<Pr> {
    let short = |login: &str| -> String {
        let s = &setup.trim_suffix;
        login.strip_suffix(s.as_str()).filter(|_| !s.is_empty()).unwrap_or(login).to_string()
    };
    // PR web URL -> unread notification id
    let unread: Vec<(String, String)> = notifications
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or(&[])
        .iter()
        .filter(|n| str_at(n, "/subject/type") == "PullRequest")
        .map(|n| {
            let url = str_at(n, "/subject/url").replace("https://api.github.com/repos/", "https://github.com/").replace("/pulls/", "/pull/");
            (url, str_at(n, "/id").to_string())
        })
        .collect();

    let mut out = Vec::new();
    for pr in nodes(response, "/data/repository/pullRequests/nodes") {
        let author = str_at(pr, "/author/login");
        let requested = |login: &str| nodes(pr, "/reviewRequests/nodes").iter().any(|r| str_at(r, "/requestedReviewer/login") == login);
        let reviewed = nodes(pr, "/reviews/nodes").iter().any(|r| str_at(r, "/author/login") == user);
        let category = if author == user {
            Category::Mine
        } else if requested(user) && reviewed {
            Category::ReReview
        } else if requested(user) {
            Category::Review
        } else if reviewed {
            Category::Reviewed
        } else {
            continue;
        };

        // Each reviewer's latest review, then those asked who haven't reviewed
        let mut reviewers: Vec<Reviewer> = Vec::new();
        for r in nodes(pr, "/reviews/nodes") {
            let login = str_at(r, "/author/login");
            if login.is_empty() || setup.bots.iter().any(|b| b == login) {
                continue;
            }
            let state = str_at(r, "/state").to_string();
            match reviewers.iter_mut().find(|x| x.name == login) {
                Some(x) => x.state = state,
                None => reviewers.push(Reviewer { name: login.to_string(), state }),
            }
        }
        for r in nodes(pr, "/reviewRequests/nodes") {
            let who = Some(str_at(r, "/requestedReviewer/login")).filter(|l| !l.is_empty()).unwrap_or(str_at(r, "/requestedReviewer/name"));
            if !who.is_empty() && !setup.bots.iter().any(|b| b == who) {
                reviewers.retain(|x| x.name != who); // asked again: pending
                reviewers.push(Reviewer { name: who.to_string(), state: "PENDING".into() });
            }
        }
        reviewers.iter_mut().for_each(|r| r.name = short(&r.name));

        let url = str_at(pr, "/url").to_string();
        let last = nodes(pr, "/comments/nodes").first();
        out.push(Pr {
            category,
            number: pr.get("number").and_then(Value::as_u64).unwrap_or(0),
            title: str_at(pr, "/title").to_string(),
            author: if author.is_empty() { "unknown".into() } else { short(author) },
            updated: str_at(pr, "/updatedAt").to_string(),
            draft: pr.get("isDraft").and_then(Value::as_bool).unwrap_or(false),
            checks: str_at(pr, "/commits/nodes/0/commit/statusCheckRollup/state").to_string(),
            mergeable: str_at(pr, "/mergeable").to_string(),
            labels: nodes(pr, "/labels/nodes").iter().map(|l| str_at(l, "/name").to_string()).collect(),
            comments: pr.pointer("/comments/totalCount").and_then(Value::as_u64).unwrap_or(0),
            last_comment: last.map(|c| LastComment { author: short(str_at(c, "/author/login")), updated: str_at(c, "/updatedAt").to_string() }),
            notification: unread.iter().find(|(u, _)| *u == url).map(|(_, id)| id.clone()),
            reviewers,
            url,
        });
    }
    // Stable: newest first stays within each group
    out.sort_by_key(|p| p.category as u8);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn testdata(file: &str) -> Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/extensions/prs/testdata").join(file);
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn repos_from_remote_urls() {
        for url in ["git@github.com:acme/app.git", "https://github.com/acme/app", "https://github.com/acme/app.git\n", "ssh://git@github.com/acme/app.git"] {
            assert_eq!(repo_from_url(url).as_deref(), Some("acme/app"), "{url}");
        }
        assert_eq!(repo_from_url("git@gitlab.com:acme/app.git"), None);
        assert_eq!(repo_from_url("https://github.com/acme"), None);
    }

    #[test]
    fn prs_are_sorted_into_groups_in_the_order_to_deal_with_them() {
        let setup = Setup { trim_suffix: "_corp".into(), ..Setup::default() };
        let prs = sort(&testdata("repo.json"), &testdata("notifications.json"), "me_corp", &setup);
        let groups: Vec<_> = prs.iter().map(|p| (p.number, p.category)).collect();
        assert_eq!(groups, [(2, Category::ReReview), (3, Category::Review), (4, Category::Reviewed), (1, Category::Mine)], "5 isn't yours");
        assert_eq!(prs[0].notification.as_deref(), Some("111"));
        assert_eq!(prs[0].reviewers, [Reviewer { name: "me".into(), state: "PENDING".into() }], "asked again: pending");
        assert_eq!(prs[1].author, "unknown");

        let mine = &prs[3];
        assert_eq!((mine.author.as_str(), mine.draft, mine.checks.as_str(), mine.mergeable.as_str()), ("me", true, "FAILURE", "CONFLICTING"));
        assert_eq!(mine.labels, ["bug"]);
        assert_eq!(mine.comments, 3);
        assert_eq!(mine.last_comment.as_ref().unwrap().author, "bob");
        let reviewers: Vec<_> = mine.reviewers.iter().map(|r| (r.name.as_str(), r.state.as_str())).collect();
        assert_eq!(reviewers, [("bob", "APPROVED"), ("ann", "PENDING"), ("qa-team", "PENDING")], "latest review each, no bots");
        assert_eq!(mine.notification, None);
    }

    #[test]
    fn the_demo_shows_every_group() {
        let list = fetch("demo", &Setup::default()).unwrap();
        assert!(list.demo);
        assert_eq!(list.prs.len(), 4);
        assert_eq!(list.prs[3].author, "me", "names are trimmed");
    }

    #[test]
    fn setup_defaults() {
        let s: Setup = serde_json::from_str("{}").unwrap();
        assert_eq!(s, Setup::default());
        assert_eq!((s.stale_days, s.refresh_minutes), (7, 5));
    }
}
