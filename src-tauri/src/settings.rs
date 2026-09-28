//! App settings, stored as JSON in the app's config folder (never in the projects).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    /// Project selected last, reopened on start
    pub last: Option<String>,
    /// Your own toolkit actions, per project path
    pub custom: HashMap<String, Vec<CustomAction>>,
    /// Detected actions you hid, per project path (action ids like "django:shell")
    pub hidden_actions: HashMap<String, Vec<String>>,
    /// Plugin tabs turned on, per project path
    pub tabs: HashMap<String, Vec<crate::plugins::PluginTab>>,
    /// The character in the top bar ("octopus", "crab", ...; empty: the octopus; "none": none)
    pub avatar: String,
    /// Installed plugins, in the order they were installed
    pub plugins: Vec<crate::plugins::install::Installed>,
    /// Each plugin's settings, by plugin id
    pub plugin_settings: HashMap<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CustomAction {
    pub id: String,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub confirm: bool,
    /// Run in this window of the project's tmux session instead of inside thumbdeck
    #[serde(default)]
    pub tmux: Option<String>,
}

/// Add a custom action, or replace the one with the same id. An empty id means a new action.
pub fn save_action(s: &mut Settings, project: String, mut action: CustomAction) {
    let list = s.custom.entry(project).or_default();
    if action.id.is_empty() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        action.id = format!("{nanos:x}");
    }
    match list.iter_mut().find(|a| a.id == action.id) {
        Some(existing) => *existing = action,
        None => list.push(action),
    }
}

/// Add a plugin's tab to a project (the same tab twice is allowed, with other setups), save its
/// setup, or remove it. `index` is the tab's position among the project's tabs.
pub fn edit_tab(s: &mut Settings, project: &str, change: &str, index: usize, tab: Option<crate::plugins::PluginTab>) {
    let tabs = s.tabs.entry(project.to_string()).or_default();
    match (change, tab) {
        ("add", Some(tab)) => tabs.push(tab),
        ("save", Some(tab)) if index < tabs.len() => tabs[index] = tab,
        ("remove", _) if index < tabs.len() => {
            tabs.remove(index);
        }
        _ => {}
    }
    if tabs.is_empty() {
        s.tabs.remove(project);
    }
}

/// Add an installed plugin, or replace the one with the same id (keeping its place)
pub fn add_plugin(s: &mut Settings, plugin: crate::plugins::install::Installed) {
    match s.plugins.iter_mut().find(|p| p.id == plugin.id) {
        Some(existing) => *existing = plugin,
        None => s.plugins.push(plugin),
    }
}

/// Forget a plugin, its settings and its tabs
pub fn remove_plugin(s: &mut Settings, id: &str) {
    s.plugins.retain(|p| p.id != id);
    s.plugin_settings.remove(id);
    for tabs in s.tabs.values_mut() {
        tabs.retain(|t| t.plugin != id);
    }
    s.tabs.retain(|_, tabs| !tabs.is_empty());
}

pub fn delete_action(s: &mut Settings, project: &str, id: &str) {
    if let Some(list) = s.custom.get_mut(project) {
        list.retain(|a| a.id != id);
        if list.is_empty() {
            s.custom.remove(project);
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        let roots = crate::projects::default_roots()
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        Settings { roots, added: vec![], hidden: vec![], pinned: vec![], last: None, custom: HashMap::new(), hidden_actions: HashMap::new(), tabs: HashMap::new(), avatar: String::new(), plugins: vec![], plugin_settings: HashMap::new() }
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
        Settings { roots: vec![], added: vec![], hidden: vec![], pinned: vec![], last: None, custom: HashMap::new(), hidden_actions: HashMap::new(), tabs: HashMap::new(), avatar: String::new(), plugins: vec![], plugin_settings: HashMap::new() }
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
    fn custom_actions_are_added_edited_and_deleted() {
        let mut s = empty();
        let new = |id: &str, name: &str| CustomAction { id: id.into(), name: name.into(), command: "echo".into(), confirm: false, tmux: None };
        save_action(&mut s, "/p".into(), new("", "Backup"));
        let id = s.custom["/p"][0].id.clone();
        assert!(!id.is_empty(), "a new action gets an id");
        save_action(&mut s, "/p".into(), new(&id, "Backup DB"));
        assert_eq!(s.custom["/p"].len(), 1, "same id replaces");
        assert_eq!(s.custom["/p"][0].name, "Backup DB");
        delete_action(&mut s, "/p", &id);
        assert!(!s.custom.contains_key("/p"), "empty projects are dropped");
    }

    #[test]
    fn custom_actions_saved_before_tmux_still_load() {
        let s: Settings = serde_json::from_str(r#"{"custom": {"/p": [{"id": "1", "name": "a", "command": "b"}]}}"#).unwrap();
        assert_eq!(s.custom["/p"][0].tmux, None);
    }

    #[test]
    fn plugin_tabs_are_added_saved_and_removed() {
        let mut s = empty();
        let tab = crate::plugins::PluginTab { plugin: "logs".into(), tab: "logs".into(), setup: serde_json::json!({ "title": "Logs" }) };
        edit_tab(&mut s, "/p", "add", 0, Some(tab.clone()));
        let mut changed = tab.clone();
        changed.setup["title"] = "Runs".into();
        edit_tab(&mut s, "/p", "save", 0, Some(changed));
        assert_eq!(s.tabs["/p"][0].setup["title"], "Runs");
        edit_tab(&mut s, "/p", "save", 5, Some(tab.clone()));
        edit_tab(&mut s, "/p", "remove", 3, None);
        assert_eq!(s.tabs["/p"].len(), 1, "out of range changes do nothing");
        edit_tab(&mut s, "/p", "remove", 0, None);
        assert!(!s.tabs.contains_key("/p"), "empty projects are dropped");
        edit_tab(&mut s, "/q", "add", 0, Some(tab));
        remove_plugin(&mut s, "logs");
        assert!(s.tabs.is_empty(), "removing a plugin takes its tabs");
    }

    #[test]
    fn tabs_saved_by_older_thumbdecks_are_dropped() {
        let s: Settings = serde_json::from_str(r#"{"extensions": {"/p": [{"extension": "logs", "setup": {}}]}}"#).unwrap();
        assert!(s.tabs.is_empty());
    }

    #[test]
    fn plugins_are_added_replaced_and_removed_with_their_settings() {
        let mut s = empty();
        let p = |id: &str, tag: &str| crate::plugins::install::Installed {
            id: id.into(), source: format!("https://x/{id}"), tag: Some(tag.into()), enabled: true, linked: false,
        };
        add_plugin(&mut s, p("a", "v1.0.0"));
        add_plugin(&mut s, p("b", "v1.0.0"));
        add_plugin(&mut s, p("a", "v2.0.0"));
        assert_eq!(s.plugins.iter().map(|p| (p.id.as_str(), p.tag.as_deref().unwrap())).collect::<Vec<_>>(), [("a", "v2.0.0"), ("b", "v1.0.0")]);
        s.plugin_settings.insert("a".into(), serde_json::json!({ "n": 1 }));
        remove_plugin(&mut s, "a");
        assert_eq!(s.plugins.len(), 1);
        assert!(s.plugin_settings.is_empty());
    }

    #[test]
    fn missing_fields_in_the_file_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"hidden": ["/p/a"]}"#).unwrap();
        assert_eq!(s.hidden, vec!["/p/a"]);
        assert!(!s.roots.is_empty(), "default scan folders are kept");
    }
}
