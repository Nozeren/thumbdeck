//! App settings, stored as JSON in the app's config folder (never in the projects).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    /// Folders whose git repositories show up as projects
    pub roots: Vec<String>,
    /// Folders added by hand (any folder, git or not)
    pub added: Vec<String>,
    /// Projects left out of the list
    pub hidden: Vec<String>,
    /// Projects shown in the Pinned section at the top
    pub pinned: Vec<String>,
    /// Folded sections of the project tree ("pinned", "added", "hidden" or a scan folder)
    pub collapsed: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        let roots = crate::projects::default_roots()
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        Settings { roots, added: vec![], hidden: vec![], pinned: vec![], collapsed: vec![] }
    }
}

fn file(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

pub fn load(app: &AppHandle) -> Settings {
    file(app)
        .ok()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = file(app)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// Load, change, save.
pub fn update(app: &AppHandle, change: impl FnOnce(&mut Settings)) -> Result<Settings, String> {
    let mut settings = load(app);
    change(&mut settings);
    save(app, &settings)?;
    Ok(settings)
}

fn push_unique(list: &mut Vec<String>, value: String) {
    if !list.contains(&value) {
        list.push(value);
    }
}

pub fn add_project(s: &mut Settings, path: String) {
    s.hidden.retain(|p| p != &path);
    push_unique(&mut s.added, path);
}

/// Added projects are removed; scanned ones are hidden (they'd come back on the next scan).
pub fn remove_project(s: &mut Settings, path: String) {
    s.pinned.retain(|p| p != &path);
    if s.added.contains(&path) {
        s.added.retain(|p| p != &path);
    } else {
        push_unique(&mut s.hidden, path);
    }
}

pub fn unhide_project(s: &mut Settings, path: &str) {
    s.hidden.retain(|p| p != path);
}

pub fn toggle(list: &mut Vec<String>, value: String) {
    if list.contains(&value) {
        list.retain(|v| v != &value);
    } else {
        list.push(value);
    }
}

pub fn add_root(s: &mut Settings, path: String) {
    push_unique(&mut s.roots, path);
}

pub fn remove_root(s: &mut Settings, path: &str) {
    s.roots.retain(|p| p != path);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> Settings {
        Settings { roots: vec![], added: vec![], hidden: vec![], pinned: vec![], collapsed: vec![] }
    }

    #[test]
    fn removing_a_scanned_project_hides_it_and_unhide_brings_it_back() {
        let mut s = empty();
        remove_project(&mut s, "/p/a".into());
        assert_eq!(s.hidden, vec!["/p/a"]);
        unhide_project(&mut s, "/p/a");
        assert!(s.hidden.is_empty());
    }

    #[test]
    fn removing_an_added_project_forgets_it_instead_of_hiding() {
        let mut s = empty();
        add_project(&mut s, "/x/b".into());
        remove_project(&mut s, "/x/b".into());
        assert!(s.added.is_empty() && s.hidden.is_empty());
    }

    #[test]
    fn adding_a_hidden_project_shows_it_again_and_nothing_is_duplicated() {
        let mut s = empty();
        remove_project(&mut s, "/p/a".into());
        add_project(&mut s, "/p/a".into());
        add_project(&mut s, "/p/a".into());
        add_root(&mut s, "/r".into());
        add_root(&mut s, "/r".into());
        assert!(s.hidden.is_empty());
        assert_eq!(s.added, vec!["/p/a"]);
        assert_eq!(s.roots, vec!["/r"]);
    }

    #[test]
    fn toggle_adds_then_removes() {
        let mut s = empty();
        toggle(&mut s.pinned, "/p/a".into());
        assert_eq!(s.pinned, vec!["/p/a"]);
        toggle(&mut s.pinned, "/p/a".into());
        assert!(s.pinned.is_empty());
    }

    #[test]
    fn missing_fields_in_the_file_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"hidden": ["/p/a"]}"#).unwrap();
        assert_eq!(s.hidden, vec!["/p/a"]);
        assert!(!s.roots.is_empty(), "default scan folders are kept");
    }
}
