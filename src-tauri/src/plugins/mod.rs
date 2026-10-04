//! Plugins (docs/plugin-spec.md): folders with a plugin.toml that add Toolkit buttons, tabs,
//! panels, pages and views. Installed ones are listed in the settings; this reads them.

pub mod api;
pub mod backend;
pub mod catalog;
pub mod detect;
pub mod frame;
pub mod install;
pub mod keys;
pub mod log;
pub mod manifest;
#[cfg(test)]
mod official;
pub mod toolkit;

use install::Installed;
use manifest::{Field, Manifest};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A plugin's tab turned on for a project, as saved in the settings
#[derive(Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct PluginTab {
    pub plugin: String,
    /// The tab's id in the plugin's manifest
    pub tab: String,
    /// Its setup (the fields the plugin declares, and its title)
    #[serde(default)]
    pub setup: Value,
}

/// An installed plugin with its manifest, when it could be read
pub struct Plugin {
    pub installed: Installed,
    pub folder: PathBuf,
    pub manifest: Option<Manifest>,
    /// Why it isn't loaded (empty when it is)
    pub problems: Vec<String>,
}

impl Plugin {
    /// Turned on and without problems
    pub fn works(&self) -> bool {
        self.installed.enabled && self.manifest.is_some()
    }
}

/// Every installed plugin, in the order they were installed
pub fn load(installed: &[Installed]) -> Vec<Plugin> {
    let dir = install::plugins_dir().unwrap_or_default();
    installed.iter().map(|i| read(i, &dir)).collect()
}

fn read(installed: &Installed, plugins_dir: &Path) -> Plugin {
    let folder = installed.folder(plugins_dir);
    let (manifest, mut problems) = match manifest::read(&folder) {
        Ok(m) => (Some(m), vec![]),
        Err(p) => (None, p),
    };
    if let Some(m) = &manifest {
        if m.id != installed.id {
            problems.push(format!("its plugin.toml now says it's {}, not {}", m.id, installed.id));
        }
    }
    let manifest = manifest.filter(|_| problems.is_empty());
    Plugin { installed: installed.clone(), folder, manifest, problems }
}

/// What the working plugins bring to the Toolkit (requirements not resolved yet)
pub fn providers(plugins: &[Plugin]) -> Vec<toolkit::Provider> {
    plugins.iter().filter(|p| p.works()).filter_map(|p| p.manifest.as_ref()?.provider(&p.folder)).collect()
}

/// A working plugin by id
pub fn find<'a>(plugins: &'a [Plugin], id: &str) -> Option<(&'a Plugin, &'a Manifest)> {
    plugins.iter().filter(|p| p.works()).find(|p| p.installed.id == id).and_then(|p| Some((p, p.manifest.as_ref()?)))
}

/// Whether a plugin's project features (tabs, project panels) apply to a project
pub fn applies(m: &Manifest, project: &Path) -> bool {
    m.detect.as_ref().is_none_or(|d| d.conditions.matches(project))
}

/// What a frame needs from its plugin: where its files are, its page and keys
#[derive(Serialize, Clone)]
pub struct FrameInfo {
    pub plugin: String,
    pub version: String,
    pub folder: String,
    pub data_folder: String,
    /// The plugin's name, and the surface's (a tab's default title)
    pub plugin_name: String,
    pub name: String,
    /// The HTML file, relative to the plugin's folder
    pub page: String,
    /// Its keymaps, the starting one first
    pub keymaps: Vec<(String, keys::Keymap)>,
    /// A tab's setup form (fields), or its own setup page
    pub fields: Vec<Field>,
    pub setup_page: Option<String>,
}

/// A plugin tab's frame info, with its setup completed; Err: why it can't show
/// A working plugin, or why it can't be used (in words)
fn working<'a>(plugins: &'a [Plugin], plugin: &str) -> Result<(&'a Plugin, &'a Manifest), String> {
    find(plugins, plugin).ok_or_else(|| {
        let found = plugins.iter().find(|p| p.installed.id == plugin);
        let name = found.and_then(|p| p.manifest.as_ref()).map(|m| m.name.as_str()).unwrap_or(plugin);
        match found {
            Some(p) if !p.installed.enabled => format!("The {name} plugin is turned off (Settings › Plugins)."),
            Some(_) => format!("The {name} plugin has problems, so it isn't loaded (Settings › Plugins)."),
            None => format!("The {name} plugin isn't installed (Settings › Plugins › Add a plugin)."),
        }
    })
}

/// What a frame of a plugin's surface needs: "tab", "panel", "card", "page" or "view" (its id
/// is ignored for a view); a tab's setup form comes with it
pub fn frame_info(plugins: &[Plugin], plugin: &str, surface: &str, id: &str) -> Result<FrameInfo, String> {
    let (p, m) = working(plugins, plugin)?;
    let missing = || format!("{} has no {surface} called {id}.", m.name);
    let (name, page, fields, setup_page, keys) = match surface {
        "tab" => {
            let t = m.tabs.iter().find(|t| t.id == id).ok_or_else(missing)?;
            (t.name.clone(), t.page.clone(), t.setup.clone(), t.setup_page.clone(), format!("tab:{id}"))
        }
        "panel" => {
            let x = m.panels.iter().find(|x| x.id == id).ok_or_else(missing)?;
            (x.name.clone(), x.page.clone(), vec![], None, format!("panel:{id}"))
        }
        "card" => {
            let x = m.cards.iter().find(|x| x.id == id).ok_or_else(missing)?;
            (x.name.clone(), x.page.clone(), vec![], None, format!("card:{id}"))
        }
        "page" => {
            let x = m.pages.iter().find(|x| x.id == id).ok_or_else(missing)?;
            (m.name.clone(), x.page.clone(), vec![], None, format!("page:{id}"))
        }
        "view" => {
            let v = m.view.as_ref().ok_or_else(|| format!("{} has no view.", m.name))?;
            (v.name.clone(), v.page.clone(), vec![], None, "view".to_string())
        }
        _ => return Err(format!("there's no surface {surface}")),
    };
    Ok(FrameInfo {
        plugin: plugin.to_string(),
        version: m.version.clone(),
        folder: p.folder.to_string_lossy().to_string(),
        data_folder: api::data_folder(plugin).map(|d| d.to_string_lossy().to_string()).unwrap_or_default(),
        plugin_name: m.name.clone(),
        name,
        page,
        keymaps: m.keymaps(&keys),
        fields,
        setup_page,
    })
}

/// A plugin tab's frame info, with its setup completed; Err: why it can't show
pub fn tab_frame(plugins: &[Plugin], plugin: &str, tab: &str, saved: &Value) -> Result<(FrameInfo, Value), String> {
    let (_, m) = working(plugins, plugin)?;
    let t = m.tabs.iter().find(|t| t.id == tab).ok_or_else(|| format!("{} no longer has a tab called {tab}.", m.name))?;
    let mut setup = manifest::complete(&t.setup, saved);
    if setup.get("title").and_then(|v| v.as_str()).is_none_or(|t| t.trim().is_empty()) {
        setup["title"] = Value::from(t.name.clone());
    }
    Ok((frame_info(plugins, plugin, "tab", tab)?, setup))
}

/// A plugin's view, for the Plugins pane
#[derive(Serialize)]
pub struct View {
    pub plugin: String,
    pub name: String,
    /// It shows a status next to its name
    pub status: bool,
    pub frame: FrameInfo,
}

/// The views of the working plugins, in the plugins' order
pub fn views(plugins: &[Plugin]) -> Vec<View> {
    plugins
        .iter()
        .filter(|p| p.works())
        .filter_map(|p| {
            let m = p.manifest.as_ref()?;
            let v = m.view.as_ref()?;
            Some(View { plugin: m.id.clone(), name: v.name.clone(), status: v.status, frame: frame_info(plugins, &m.id, "view", "").ok()? })
        })
        .collect()
}

/// A panel on the right
#[derive(Serialize)]
pub struct PanelInfo {
    pub plugin: String,
    pub id: String,
    pub name: String,
    /// "project" or "app"
    pub scope: String,
    /// null: its page's height (up to half the column); a number of lines
    pub lines: Option<u32>,
    pub frame: FrameInfo,
}

/// The panels a project shows: app-wide ones, and project ones of plugins that apply to it;
/// in the plugins' order
pub fn panels(plugins: &[Plugin], project: Option<&Path>) -> Vec<PanelInfo> {
    plugins
        .iter()
        .filter(|p| p.works())
        .filter_map(|p| p.manifest.as_ref())
        .flat_map(|m| {
            let applies = project.is_some_and(|dir| applies(m, dir));
            m.panels.iter().filter(move |x| x.scope == "app" || applies).filter_map(|x| {
                Some(PanelInfo {
                    plugin: m.id.clone(),
                    id: x.id.clone(),
                    name: x.name.clone(),
                    scope: x.scope.clone(),
                    lines: match x.height {
                        manifest::Height::Lines(n) => Some(n),
                        manifest::Height::Word(_) => None,
                    },
                    frame: frame_info(plugins, &m.id, "panel", &x.id).ok()?,
                })
            })
        })
        .collect()
}

/// A card in a project's Overview
#[derive(Serialize)]
pub struct CardInfo {
    pub plugin: String,
    pub id: String,
    pub name: String,
    /// null: its page's height; a number of lines
    pub lines: Option<u32>,
    /// The plugin's tab Enter shows (when the card's keys don't bind Enter)
    pub opens: Option<String>,
    pub frame: FrameInfo,
}

/// The cards a project's Overview shows: those of the plugins that apply to it, in the
/// plugins' order
pub fn cards(plugins: &[Plugin], project: &Path) -> Vec<CardInfo> {
    plugins
        .iter()
        .filter(|p| p.works())
        .filter_map(|p| p.manifest.as_ref())
        .filter(|m| applies(m, project))
        .flat_map(|m| {
            m.cards.iter().filter_map(|x| {
                Some(CardInfo {
                    plugin: m.id.clone(),
                    id: x.id.clone(),
                    name: x.name.clone(),
                    lines: match x.height {
                        manifest::Height::Lines(n) => Some(n),
                        manifest::Height::Word(_) => None,
                    },
                    opens: x.opens.clone(),
                    frame: frame_info(plugins, &m.id, "card", &x.id).ok()?,
                })
            })
        })
        .collect()
}

/// When a linked plugin's files last changed (the newest file in its folder; its node_modules
/// and .git aren't looked at)
pub fn changed_at(folder: &Path) -> Option<std::time::SystemTime> {
    let mut newest = None;
    let mut dirs = vec![folder.to_path_buf()];
    let mut seen = 0;
    while let Some(dir) = dirs.pop() {
        for e in std::fs::read_dir(&dir).ok()?.flatten() {
            let name = e.file_name();
            if name == "node_modules" || name == ".git" {
                continue;
            }
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                dirs.push(e.path());
            } else if let Ok(t) = meta.modified() {
                newest = newest.max(Some(t));
            }
            seen += 1;
            if seen > 5000 {
                return newest; // a huge folder: enough to go by
            }
        }
    }
    newest
}

/// A plugin's tab that can be added to a project ("<plugin>:<tab>")
#[derive(Serialize)]
pub struct Addable {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// The plugin tabs a project can have (from working plugins that apply to it)
pub fn addable_tabs(plugins: &[Plugin], project: &Path) -> Vec<Addable> {
    plugins
        .iter()
        .filter(|p| p.works())
        .filter_map(|p| p.manifest.as_ref())
        .filter(|m| applies(m, project))
        .flat_map(|m| {
            m.tabs.iter().map(|t| Addable {
                id: format!("{}:{}", m.id, t.id),
                name: t.name.clone(),
                description: if t.description.is_empty() { m.description.clone() } else { t.description.clone() },
            })
        })
        .collect()
}

/// A backend's answer to `actions` as Toolkit buttons: `[{ name, command, description?,
/// confirm?, tmux? }]` (tmux: true or a window name)
pub fn backend_actions(plugin: &str, name: &str, answer: &Value) -> Result<Vec<toolkit::Action>, String> {
    let list = answer.as_array().ok_or("its answer to actions isn't a list")?;
    list.iter()
        .map(|a| {
            let label = a["name"].as_str().filter(|n| !n.trim().is_empty()).ok_or("an action without a name")?.to_string();
            let command = a["command"].as_str().ok_or_else(|| format!("action {label} has no command"))?.to_string();
            let tmux = match &a["tmux"] {
                Value::Bool(true) => Some(label.clone()),
                Value::String(w) if !w.is_empty() => Some(w.clone()),
                _ => None,
            };
            Ok(toolkit::Action {
                id: format!("{plugin}:{label}"),
                command,
                source: plugin.to_string(),
                group: name.to_string(),
                description: a["description"].as_str().map(String::from),
                confirm: a["confirm"].as_bool().unwrap_or(false),
                tmux,
                label,
            })
        })
        .collect()
}

/// A plugin as Settings › Plugins shows it
#[derive(Serialize)]
pub struct Info {
    id: String,
    name: String,
    version: Option<String>,
    description: String,
    homepage: Option<String>,
    /// Its icon file, in its folder
    icon: Option<String>,
    source: String,
    tag: Option<String>,
    linked: bool,
    enabled: bool,
    folder: String,
    readme: Option<String>,
    /// What it adds, in a few words each
    adds: Vec<String>,
    /// Why it isn't loaded
    problems: Vec<String>,
    /// Its settings form, and the values (every field filled in)
    fields: Vec<Field>,
    settings: Value,
}

pub fn info(p: &Plugin, saved: &HashMap<String, Value>) -> Info {
    let m = p.manifest.as_ref();
    let fields = m.map(|m| m.settings.clone()).unwrap_or_default();
    let settings = manifest::complete(&fields, saved.get(&p.installed.id).unwrap_or(&Value::Null));
    Info {
        id: p.installed.id.clone(),
        name: m.map(|m| m.name.clone()).unwrap_or_else(|| p.installed.id.clone()),
        version: m.map(|m| m.version.clone()),
        description: m.map(|m| m.description.clone()).unwrap_or_default(),
        homepage: m.and_then(|m| m.homepage.clone()),
        icon: m.and_then(|m| m.icon.clone()),
        source: p.installed.source.clone(),
        tag: p.installed.tag.clone(),
        linked: p.installed.linked,
        enabled: p.installed.enabled,
        folder: p.folder.to_string_lossy().to_string(),
        readme: crate::projects::readme(&p.folder),
        adds: m.map(|m| m.adds()).unwrap_or_default(),
        problems: p.problems.clone(),
        fields,
        settings,
    }
}

/// `thumbdeck plugin check <folder>`: the same checks as loading, in plain sentences.
/// Returns the exit code.
pub fn check_command(folder: &Path) -> i32 {
    match manifest::read(folder) {
        Ok(m) => {
            println!("✓ {} {} ({}) is fine.", m.name, m.version, m.id);
            for a in m.adds() {
                println!("  adds {a}");
            }
            0
        }
        Err(problems) => {
            println!("✗ {} has {} problem{}:", folder.display(), problems.len(), if problems.len() == 1 { "" } else { "s" });
            for p in problems {
                println!("  - {p}");
            }
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn broken_and_renamed_plugins_are_not_loaded() {
        let d = TempDir::new("plugins-load");
        d.file("ok/plugin.toml", "id = \"ok\"\nname = \"OK\"\nversion = \"1.0.0\"\napi = 1\n[[action]]\nname = \"a\"\ncommand = \"a\"\n");
        d.file("bad/plugin.toml", "id = \"bad\"\n");
        d.file("renamed/plugin.toml", "id = \"other\"\nname = \"X\"\nversion = \"1.0.0\"\napi = 1\n");
        let link = |id: &str, enabled: bool| Installed {
            id: id.into(), source: d.0.join(id).to_string_lossy().into(), tag: None, enabled, linked: true,
        };
        let plugins: Vec<Plugin> = [link("ok", true), link("bad", true), link("renamed", true), link("gone", true)]
            .iter()
            .map(|i| read(i, Path::new("/unused")))
            .collect();
        assert!(plugins[0].works());
        assert!(plugins[1].problems[0].contains("it needs name"));
        assert!(plugins[2].problems[0].contains("now says it's other"));
        assert!(plugins[3].problems[0].contains("no plugin.toml"));
        assert_eq!(providers(&plugins).len(), 1);

        let off = read(&link("ok", false), Path::new("/unused"));
        assert!(!off.works() && off.problems.is_empty(), "turned off isn't broken");
        assert!(providers(&[off]).is_empty());
    }

    #[test]
    fn info_fills_in_the_settings() {
        let d = TempDir::new("plugins-info");
        d.file("p/plugin.toml", "id = \"p\"\nname = \"P\"\nversion = \"1.0.0\"\napi = 1\n[[settings]]\nkey = \"n\"\nlabel = \"N\"\ntype = \"number\"\ndefault = 3\n");
        d.file("p/README.md", "# P");
        let i = Installed { id: "p".into(), source: d.0.join("p").to_string_lossy().into(), tag: None, enabled: true, linked: true };
        let info = info(&read(&i, Path::new("/unused")), &HashMap::new());
        assert_eq!(info.settings, serde_json::json!({ "n": 3 }));
        assert_eq!(info.readme.as_deref(), Some("# P"));
    }

    #[test]
    fn a_backends_actions() {
        let answer = serde_json::json!([
            { "name": "test", "command": "just test" },
            { "name": "serve", "command": "just serve", "tmux": true, "confirm": true, "description": "Serve" },
            { "name": "repl", "command": "just repl", "tmux": "shell" },
        ]);
        let list = backend_actions("tasks", "Tasks", &answer).unwrap();
        let short: Vec<_> = list.iter().map(|a| (a.id.as_str(), a.tmux.as_deref(), a.confirm)).collect();
        assert_eq!(short, [("tasks:test", None, false), ("tasks:serve", Some("serve"), true), ("tasks:repl", Some("shell"), false)]);
        assert_eq!(list[0].group, "Tasks");
        assert!(backend_actions("t", "T", &serde_json::json!({})).is_err());
        assert!(backend_actions("t", "T", &serde_json::json!([{ "command": "x" }])).unwrap_err().contains("without a name"));
    }

    #[test]
    fn check_command_exit_codes() {
        let d = TempDir::new("plugins-check");
        d.file("good/plugin.toml", "id = \"good\"\nname = \"G\"\nversion = \"1.0.0\"\napi = 1\n");
        d.file("bad/plugin.toml", "id = \"Bad\"\nname = \"G\"\nversion = \"1.0.0\"\napi = 1\n");
        assert_eq!(check_command(&d.0.join("good")), 0);
        assert_eq!(check_command(&d.0.join("bad")), 1);
        assert_eq!(check_command(&d.0.join("missing")), 1);
    }
}
