//! Finding projects: git repositories directly inside the configured folders.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize, Clone)]
pub struct Project {
    pub name: String,
    pub path: String,
    pub branch: Option<String>,
    /// Uncommitted changes in the working tree
    pub dirty: bool,
    /// Scan folder it was found in; None when added by hand
    pub root: Option<String>,
    /// What kind of project it looks like (for its icon): django, android, tauri, node, rust,
    /// go, nvim, python or folder
    pub kind: String,
    pub hidden: bool,
    pub pinned: bool,
}

/// From the packs that apply (see actions::icon), except for Tauri apps and Neovim plugins,
/// which no pack is about.
fn kind(dir: &Path, packs: &[crate::packs::Pack]) -> String {
    let has = |f: &str| dir.join(f).exists();
    let name = dir.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    if has("src-tauri") {
        "tauri".into()
    } else if name.ends_with(".nvim") || (has("lua") && has("plugin")) {
        "nvim".into()
    } else {
        crate::actions::icon(packs, dir).unwrap_or_else(|| "folder".into())
    }
}

pub fn info(path: &Path, root: Option<String>, hidden: bool, pinned: bool, packs: &[crate::packs::Pack]) -> Project {
    let git_repo = path.join(".git").exists();
    Project {
        name: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        path: path.to_string_lossy().to_string(),
        branch: git_repo.then(|| git(path, &["rev-parse", "--abbrev-ref", "HEAD"])).flatten(),
        dirty: git_repo && git(path, &["status", "--porcelain"]).is_some_and(|s| !s.is_empty()),
        root,
        kind: kind(path, packs),
        hidden,
        pinned,
    }
}

/// Folders searched for projects when nothing else is configured.
pub fn default_roots() -> Vec<PathBuf> {
    let Some(home) = dirs_home() else { return vec![] };
    vec![home.join("dev"), home.join("projects"), home]
}

pub fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Git repositories one level below each root, plus the folders added by hand, sorted by name.
/// Hidden ones are included (marked), so the app can offer to show them again.
pub fn list(roots: &[String], added: &[String], hidden: &[String], pinned: &[String], packs: &[crate::packs::Pack]) -> Vec<Project> {
    let mut paths: Vec<(PathBuf, Option<String>)> = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let dotfolder = entry.file_name().to_string_lossy().starts_with('.');
            if !dotfolder && path.is_dir() && path.join(".git").exists() {
                paths.push((path, Some(root.clone())));
            }
        }
    }
    for path in added {
        let path = PathBuf::from(path);
        if path.is_dir() {
            paths.push((path, None));
        }
    }

    let mut found: Vec<Project> = Vec::new();
    for (path, root) in paths {
        let path_str = path.to_string_lossy().to_string();
        if found.iter().any(|p| p.path == path_str) {
            continue;
        }
        found.push(info(&path, root, hidden.contains(&path_str), pinned.contains(&path_str), packs));
    }
    found.sort_by_key(|p| p.name.to_lowercase());
    found
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The project's README, if it has one.
pub fn readme(dir: &Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut candidates: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().to_lowercase().starts_with("readme"))
                .unwrap_or(false)
        })
        .collect();
    // Prefer README.md over README.txt etc.
    candidates.sort_by_key(|p| !p.to_string_lossy().to_lowercase().ends_with(".md"));
    std::fs::read_to_string(candidates.first()?).ok()
}
