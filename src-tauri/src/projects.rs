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
}

/// Folders searched for projects when nothing else is configured.
pub fn default_roots() -> Vec<PathBuf> {
    let Some(home) = dirs_home() else { return vec![] };
    vec![home.join("dev"), home.join("projects"), home]
}

pub fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Git repositories one level below each root, sorted by name.
pub fn scan(roots: &[PathBuf]) -> Vec<Project> {
    let mut found: Vec<Project> = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let hidden = entry.file_name().to_string_lossy().starts_with('.');
            if hidden || !path.is_dir() || !path.join(".git").exists() {
                continue;
            }
            let path_str = path.to_string_lossy().to_string();
            if found.iter().any(|p| p.path == path_str) {
                continue;
            }
            found.push(Project {
                name: entry.file_name().to_string_lossy().to_string(),
                branch: git(&path, &["rev-parse", "--abbrev-ref", "HEAD"]),
                dirty: git(&path, &["status", "--porcelain"]).is_some_and(|s| !s.is_empty()),
                path: path_str,
            });
        }
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
