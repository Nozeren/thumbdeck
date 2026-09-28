//! plugin.toml: what a plugin is and what it adds (docs/plugin-spec.md). Parsing and every
//! check, with problems in plain sentences; a plugin with problems isn't loaded.

use super::detect::Conditions;
use super::keys::Keymap;
use super::toolkit::{ActionDef, Generate, Provider, DEFAULT_PRIORITY};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// The plugin API versions this thumbdeck speaks
pub const API: std::ops::RangeInclusive<u32> = 1..=1;

/// Project icons thumbdeck has (an `icon` under [detect] is one of these, or an .svg file)
pub const ICONS: &[&str] = &["django", "python", "android", "node", "tauri", "rust", "go", "nvim", "folder"];

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct Raw {
    id: String,
    name: String,
    version: String,
    api: u32,
    #[serde(default)]
    description: String,
    homepage: Option<String>,
    icon: Option<String>,
    detect: Option<toml::Table>,
    #[serde(default)]
    vars: BTreeMap<String, String>,
    #[serde(default, rename = "action")]
    actions: Vec<ActionDef>,
    #[serde(default)]
    generate: Vec<Generate>,
    #[serde(default)]
    tab: Vec<TabDef>,
    #[serde(default)]
    panel: Vec<PanelDef>,
    #[serde(default)]
    page: Vec<PageDef>,
    view: Option<ViewDef>,
    #[serde(default)]
    settings: Vec<Field>,
    backend: Option<BackendDef>,
    /// Keymaps in the order written (the first of a surface is its starting one)
    #[serde(default)]
    keys: toml::Table,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // api and icon aren't used yet
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api: u32,
    pub description: String,
    pub homepage: Option<String>,
    pub icon: Option<String>,
    pub detect: Option<Detect>,
    pub vars: BTreeMap<String, String>,
    pub actions: Vec<ActionDef>,
    pub generate: Vec<Generate>,
    pub tabs: Vec<TabDef>,
    pub panels: Vec<PanelDef>,
    pub pages: Vec<PageDef>,
    pub view: Option<ViewDef>,
    pub settings: Vec<Field>,
    pub backend: Option<BackendDef>,
    /// Keymaps by name, in the order written
    pub keys: Vec<(String, Keymap)>,
}

/// [detect]: the conditions, and what matching projects get
#[derive(Debug, Clone)]
pub struct Detect {
    pub conditions: Conditions,
    pub icon: Option<String>,
    pub priority: i32,
    pub requires: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct TabDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub page: String,
    /// The tab's setup form ([[tab.setup]])
    #[serde(default)]
    pub setup: Vec<Field>,
    /// Its own setup page instead of the form
    pub setup_page: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct PanelDef {
    pub id: String,
    pub name: String,
    pub page: String,
    #[serde(default = "project_scope")]
    pub scope: String,
    #[serde(default)]
    pub height: Height,
}

fn project_scope() -> String {
    "project".into()
}

/// "auto" (the page's height, up to half the column) or a number of lines
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum Height {
    Lines(u32),
    Word(String),
}

impl Default for Height {
    fn default() -> Self {
        Height::Word("auto".into())
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct PageDef {
    pub id: String,
    pub page: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ViewDef {
    pub name: String,
    pub page: String,
    /// Shows a short status next to its name in the Plugins pane
    #[serde(default)]
    pub status: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct BackendDef {
    pub command: String,
    pub install: Option<String>,
    #[serde(default)]
    pub actions: bool,
    #[serde(default)]
    pub watch: Vec<String>,
    /// Start with thumbdeck, not when first needed (for a backend that watches something)
    #[serde(default)]
    pub autostart: bool,
}

/// A settings or setup field, drawn by thumbdeck
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub default: Option<Value>,
    pub help: Option<String>,
    #[serde(default)]
    pub multiline: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
    #[serde(default)]
    pub choices: Vec<Choice>,
}

/// `"a"` or `{ value = "a", label = "A" }`; always sent to the page as the second
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(from = "ChoiceRaw")]
pub struct Choice {
    pub value: String,
    pub label: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ChoiceRaw {
    Plain(String),
    Full { value: String, label: String },
}

impl From<ChoiceRaw> for Choice {
    fn from(c: ChoiceRaw) -> Self {
        match c {
            ChoiceRaw::Plain(v) => Choice { label: v.clone(), value: v },
            ChoiceRaw::Full { value, label } => Choice { value, label },
        }
    }
}

const FIELD_TYPES: &[&str] = &["text", "number", "bool", "choice", "list", "folder", "file", "folders", "files"];

impl Field {
    /// The value it has when nothing was saved
    fn empty(&self) -> Value {
        match self.kind.as_str() {
            "number" => Value::from(self.min.unwrap_or(0.0).max(0.0)),
            "bool" => Value::Bool(false),
            "choice" => self.choices.first().map(|c| Value::from(c.value.clone())).unwrap_or(Value::Null),
            "list" | "folders" | "files" => Value::Array(vec![]),
            _ => Value::from(""),
        }
    }

    pub fn default_value(&self) -> Value {
        self.default.clone().unwrap_or_else(|| self.empty())
    }

    /// Whether a value fits the field's type
    fn fits(&self, v: &Value) -> bool {
        match self.kind.as_str() {
            "number" => v.is_number(),
            "bool" => v.is_boolean(),
            "list" | "folders" | "files" => v.as_array().is_some_and(|a| a.iter().all(|x| x.is_string())),
            "choice" => v.as_str().is_some_and(|s| self.choices.iter().any(|c| c.value == s)),
            _ => v.is_string(),
        }
    }

    fn problems(&self, place: &str) -> Vec<String> {
        let mut out = Vec::new();
        let name = if self.key.is_empty() { "a field" } else { &self.key };
        if !FIELD_TYPES.contains(&self.kind.as_str()) {
            out.push(format!("{place}: {name} has type \"{}\"; the types are {}", self.kind, FIELD_TYPES.join(", ")));
            return out;
        }
        if self.kind == "choice" && self.choices.is_empty() {
            out.push(format!("{place}: {name} is a choice but has no choices"));
        }
        if let Some(d) = &self.default {
            if !self.fits(d) {
                out.push(format!("{place}: {name}'s default doesn't fit its type ({})", self.kind));
            }
        }
        out
    }
}

/// Saved values with every field filled in: missing or wrong-typed ones get their default,
/// keys the fields don't know are kept (e.g. `title`, or a newer version's)
pub fn complete(fields: &[Field], saved: &Value) -> Value {
    let mut out = saved.as_object().cloned().unwrap_or_default();
    for f in fields {
        if !out.get(&f.key).is_some_and(|v| f.fits(v)) {
            out.insert(f.key.clone(), f.default_value());
        }
    }
    Value::Object(out)
}

fn check_fields(fields: &[Field], place: &str, reserved: &[&str], out: &mut Vec<String>) {
    let mut seen: Vec<&str> = Vec::new();
    for f in fields {
        if f.key.trim().is_empty() {
            out.push(format!("{place}: a field has no key"));
        } else if seen.contains(&f.key.as_str()) {
            out.push(format!("{place}: two fields are called {}", f.key));
        } else if reserved.contains(&f.key.as_str()) {
            out.push(format!("{place}: {} is thumbdeck's own field; call it something else", f.key));
        }
        seen.push(&f.key);
        out.extend(f.problems(place));
    }
}

/// Read and check a plugin folder's plugin.toml
pub fn read(folder: &Path) -> Result<Manifest, Vec<String>> {
    let file = folder.join("plugin.toml");
    let text = std::fs::read_to_string(&file).map_err(|_| vec![format!("there's no plugin.toml in {}", folder.display())])?;
    parse(&text, folder)
}

pub fn parse(text: &str, folder: &Path) -> Result<Manifest, Vec<String>> {
    let raw: Raw = toml::from_str(text).map_err(|e| vec![toml_problem(text, &e)])?;
    let mut out = Vec::new();

    // Who it is
    if !is_id(&raw.id) {
        out.push(format!("id \"{}\" should be lowercase letters, digits and dashes (like \"my-plugin\")", raw.id));
    }
    if raw.name.trim().is_empty() {
        out.push("name is empty".into());
    }
    if !is_version(&raw.version) {
        out.push(format!("version \"{}\" should look like 1.2.0", raw.version));
    }
    if raw.api > *API.end() {
        out.push(format!("it's made for plugin API {}, and this thumbdeck speaks up to {}: update thumbdeck", raw.api, API.end()));
    } else if !API.contains(&raw.api) {
        out.push(format!("api {} isn't a plugin API version (the first one is 1)", raw.api));
    }
    if let Some(icon) = &raw.icon {
        check_file(folder, icon, "icon", &mut out);
    }

    // [detect]
    let detect = match raw.detect {
        None => None,
        Some(mut table) => {
            let take_str = |t: &mut toml::Table, k: &str| t.remove(k).map(|v| v.as_str().map(String::from).ok_or(k.to_string()));
            let icon = take_str(&mut table, "icon").transpose().unwrap_or_else(|k| {
                out.push(format!("detect.{k} should be text"));
                None
            });
            let priority = match table.remove("priority") {
                None => DEFAULT_PRIORITY,
                Some(v) => v.as_integer().map(|n| n as i32).unwrap_or_else(|| {
                    out.push("detect.priority should be a whole number".into());
                    DEFAULT_PRIORITY
                }),
            };
            let requires = match table.remove("requires") {
                None => vec![],
                Some(v) => v.try_into::<Vec<String>>().unwrap_or_else(|_| {
                    out.push("detect.requires should be a list of plugin ids".into());
                    vec![]
                }),
            };
            if let Some(icon) = &icon {
                if icon.ends_with(".svg") {
                    check_file(folder, icon, "detect.icon", &mut out);
                } else if !ICONS.contains(&icon.as_str()) {
                    out.push(format!("detect.icon \"{icon}\" is neither an .svg file nor one of {}", ICONS.join(", ")));
                }
            }
            match toml::Value::Table(table).try_into::<Conditions>() {
                Ok(conditions) => {
                    out.extend(conditions.problems().into_iter().map(|p| format!("detect: {p}")));
                    Some(Detect { conditions, icon, priority, requires })
                }
                Err(e) => {
                    out.push(format!("detect: {}", plain(e.message())));
                    None
                }
            }
        }
    };

    // Toolkit
    let mut names: Vec<&str> = Vec::new();
    for a in &raw.actions {
        if a.name.trim().is_empty() {
            out.push("an [[action]] has no name".into());
        } else if names.contains(&a.name.as_str()) {
            out.push(format!("two actions are called {}", a.name));
        }
        names.push(&a.name);
        if a.command.trim().is_empty() {
            out.push(format!("action {} has no command", a.name));
        }
        if let Some(w) = &a.when {
            out.extend(w.problems().into_iter().map(|p| format!("action {}: {p}", a.name)));
        }
    }
    for g in &raw.generate {
        out.extend(g.problems());
    }

    // Where it shows up
    let ids = |kind: &str, list: Vec<(&str, &str)>, out: &mut Vec<String>| {
        let mut seen: Vec<&str> = Vec::new();
        for (id, page) in list {
            if !is_id(id) {
                out.push(format!("{kind} id \"{id}\" should be lowercase letters, digits and dashes"));
            } else if seen.contains(&id) {
                out.push(format!("two {kind}s have the id {id}"));
            }
            seen.push(id);
            check_file(folder, page, &format!("{kind} {id}'s page"), out);
        }
    };
    ids("tab", raw.tab.iter().map(|t| (t.id.as_str(), t.page.as_str())).collect(), &mut out);
    ids("panel", raw.panel.iter().map(|p| (p.id.as_str(), p.page.as_str())).collect(), &mut out);
    ids("page", raw.page.iter().map(|p| (p.id.as_str(), p.page.as_str())).collect(), &mut out);
    for t in &raw.tab {
        if t.name.trim().is_empty() {
            out.push(format!("tab {} has no name", t.id));
        }
        check_fields(&t.setup, &format!("tab {}'s setup", t.id), &["title"], &mut out);
        if let Some(p) = &t.setup_page {
            check_file(folder, p, &format!("tab {}'s setup_page", t.id), &mut out);
        }
    }
    for p in &raw.panel {
        if p.scope != "project" && p.scope != "app" {
            out.push(format!("panel {}: scope is \"project\" or \"app\", not \"{}\"", p.id, p.scope));
        }
        if matches!(&p.height, Height::Word(w) if w != "auto") {
            out.push(format!("panel {}: height is \"auto\" or a number of lines", p.id));
        }
    }
    if let Some(v) = &raw.view {
        check_file(folder, &v.page, "the view's page", &mut out);
    }
    check_fields(&raw.settings, "settings", &[], &mut out);

    // The backend
    if let Some(b) = &raw.backend {
        if b.command.trim().is_empty() {
            out.push("backend.command is empty".into());
        }
    }

    // Keys
    let mut keys: Vec<(String, Keymap)> = Vec::new();
    for (name, value) in raw.keys {
        match value.try_into::<Keymap>() {
            Ok(map) => keys.push((name, map)),
            Err(e) => out.push(format!("keys.{name}: {}", plain(e.message()))),
        }
    }
    for (name, map) in &keys {
        out.extend(map.problems(name));
        let surface_exists = match map.surface.split_once(':') {
            Some(("tab", id)) => raw.tab.iter().any(|t| t.id == id),
            Some(("panel", id)) => raw.panel.iter().any(|p| p.id == id),
            Some(("page", id)) => raw.page.iter().any(|p| p.id == id),
            None if map.surface == "view" => raw.view.is_some(),
            _ => false,
        };
        if !surface_exists {
            out.push(format!("keys.{name}: surface \"{}\" isn't one of the plugin's (tab:<id>, panel:<id>, page:<id> or view)", map.surface));
        }
    }

    if !out.is_empty() {
        return Err(out);
    }
    Ok(Manifest {
        id: raw.id,
        name: raw.name,
        version: raw.version,
        api: raw.api,
        description: raw.description,
        homepage: raw.homepage,
        icon: raw.icon,
        detect,
        vars: raw.vars,
        actions: raw.actions,
        generate: raw.generate,
        tabs: raw.tab,
        panels: raw.panel,
        pages: raw.page,
        view: raw.view,
        settings: raw.settings,
        backend: raw.backend,
        keys,
    })
}

impl Manifest {
    /// The keymaps of one of its surfaces ("tab:logs"), the starting one first
    pub fn keymaps(&self, surface: &str) -> Vec<(String, Keymap)> {
        self.keys.iter().filter(|(_, m)| m.surface == surface).cloned().collect()
    }

    /// What it brings to the Toolkit (None when it brings nothing: no detect, actions or icon).
    /// An .svg icon becomes its full path.
    pub fn provider(&self, folder: &Path) -> Option<Provider> {
        let d = self.detect.as_ref();
        let icon = d.and_then(|d| d.icon.clone()).map(|i| if i.ends_with(".svg") { folder.join(i).to_string_lossy().to_string() } else { i });
        let backend_actions = self.backend.as_ref().filter(|b| b.actions).map(|b| b.watch.clone());
        if self.actions.is_empty() && self.generate.is_empty() && icon.is_none() && d.is_none_or(|d| d.requires.is_empty()) && backend_actions.is_none() {
            return None;
        }
        Some(Provider {
            id: self.id.clone(),
            name: self.name.clone(),
            priority: d.map(|d| d.priority).unwrap_or(DEFAULT_PRIORITY),
            requires: d.map(|d| d.requires.clone()).unwrap_or_default(),
            detect: d.map(|d| d.conditions.clone()),
            icon,
            vars: self.vars.clone(),
            actions: self.actions.clone(),
            generate: self.generate.clone(),
            backend_actions,
        })
    }

    /// What it adds, in a few words each ("2 Toolkit buttons", "a tab: Logs")
    pub fn adds(&self) -> Vec<String> {
        let mut out = Vec::new();
        let n = self.actions.len();
        match (n, self.generate.is_empty()) {
            (0, true) => {}
            (0, false) => out.push("Toolkit buttons from the project's files".into()),
            (n, g) => out.push(format!("{n} Toolkit button{}{}", plural(n), if g { "" } else { ", and more from the project's files" })),
        }
        if self.backend.as_ref().is_some_and(|b| b.actions) {
            out.push("Toolkit buttons from its backend".into());
        }
        let names = |list: Vec<&str>| list.join(", ");
        if !self.tabs.is_empty() {
            out.push(format!("tab{}: {}", plural(self.tabs.len()), names(self.tabs.iter().map(|t| t.name.as_str()).collect())));
        }
        if !self.panels.is_empty() {
            out.push(format!("panel{} on the right: {}", plural(self.panels.len()), names(self.panels.iter().map(|p| p.name.as_str()).collect())));
        }
        if !self.pages.is_empty() {
            out.push(format!("{} full-window page{}", self.pages.len(), plural(self.pages.len())));
        }
        if let Some(v) = &self.view {
            out.push(format!("a view in the Plugins pane: {}", v.name));
        }
        if self.detect.as_ref().is_some_and(|d| d.icon.is_some()) {
            out.push("an icon for the projects it's about".into());
        }
        if self.backend.is_some() {
            out.push("a backend program".into());
        }
        out
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn is_id(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn is_version(s: &str) -> bool {
    let (core, _pre) = s.split_once('-').unwrap_or((s, ""));
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Versions compared as numbers (1.10.0 > 1.9.0); a pre-release comes before its release
pub fn version_key(s: &str) -> (Vec<u64>, bool) {
    let (core, pre) = s.split_once('-').map(|(c, p)| (c, !p.is_empty())).unwrap_or((s, false));
    (core.split('.').map(|p| p.parse().unwrap_or(0)).collect(), !pre)
}

/// A file the manifest names: relative, inside the plugin folder, and there
fn check_file(folder: &Path, path: &str, what: &str, out: &mut Vec<String>) {
    let p = Path::new(path);
    if p.is_absolute() || p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        out.push(format!("{what} ({path}) must be inside the plugin's folder"));
    } else if !folder.join(p).is_file() {
        out.push(format!("{what} ({path}) isn't there"));
    }
}

/// A TOML error with its line: "plugin.toml line 4: there's no key nmae (did you mean name?)"
fn toml_problem(text: &str, e: &toml::de::Error) -> String {
    let message = plain(e.message());
    match e.span() {
        Some(span) => format!("plugin.toml line {}: {message}", text[..span.start].matches('\n').count() + 1),
        None => format!("plugin.toml: {message}"),
    }
}

/// serde's "unknown field `nmae`, expected one of `id`, `name`, …" said shorter, with the
/// closest known key
fn plain(message: &str) -> String {
    let Some(rest) = message.strip_prefix("unknown field `") else {
        return message.replace("missing field", "it needs").replace('`', "");
    };
    let (key, expected) = rest.split_once('`').unwrap_or((rest, ""));
    let known: Vec<&str> = expected.split('`').skip(1).step_by(2).collect();
    let close = known.iter().map(|k| (distance(key, k), *k)).filter(|(d, _)| *d <= 2).min();
    match close {
        Some((_, k)) => format!("there's no key {key} (did you mean {k}?)"),
        None => format!("there's no key {key} here (the keys are {})", known.join(", ")),
    }
}

/// How many letters to change, add or remove to turn one word into the other
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let next = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(row[j + 1] + 1);
            prev = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    const MINIMAL: &str = "id = \"hello\"\nname = \"Hello\"\nversion = \"1.0.0\"\napi = 1\n";

    fn problems(d: &TempDir, text: &str) -> Vec<String> {
        parse(text, &d.0).err().unwrap_or_default()
    }

    #[test]
    fn a_minimal_manifest() {
        let d = TempDir::new("manifest-min");
        let m = parse(MINIMAL, &d.0).unwrap();
        assert_eq!((m.id.as_str(), m.version.as_str(), m.api), ("hello", "1.0.0", 1));
        assert!(m.provider(&d.0).is_none(), "adds nothing to the Toolkit");
        assert!(m.adds().is_empty());
    }

    #[test]
    fn a_pack_style_manifest_gives_a_provider() {
        let d = TempDir::new("manifest-pack");
        let m = parse(&format!("{MINIMAL}{}", r#"
[detect]
files = ["manage.py"]
icon = "django"
priority = 10
requires = ["python"]
[vars]
manage = "{python} manage.py"
[[action]]
name = "migrate"
command = "{manage} migrate"
[[generate]]
source = { json = "package.json", keys = "scripts" }
action = { name = "{item}", command = "npm run {item}" }
"#), &d.0).unwrap();
        let p = m.provider(&d.0).unwrap();
        assert_eq!((p.priority, p.requires.as_slice(), p.icon.as_deref()), (10, &["python".to_string()][..], Some("django")));
        assert_eq!(m.adds(), ["1 Toolkit button, and more from the project's files", "an icon for the projects it's about"]);
    }

    #[test]
    fn who_it_is_is_checked() {
        let d = TempDir::new("manifest-who");
        let p = problems(&d, "id = \"My Plugin\"\nname = \" \"\nversion = \"1.0\"\napi = 2\n");
        assert_eq!(p.len(), 4, "{p:?}");
        assert!(p[0].contains("lowercase"));
        assert!(p[1].contains("name is empty"));
        assert!(p[2].contains("1.2.0"));
        assert!(p[3].contains("update thumbdeck"));
        assert!(problems(&d, "id = \"a\"\nname = \"A\"\nversion = \"1.0.0\"\napi = 0\n")[0].contains("isn't a plugin API"));
        assert!(problems(&d, "id = \"a\"\nname = \"A\"\nversion = \"1.0.0-beta.1\"\napi = 1\n").is_empty());
    }

    #[test]
    fn typos_are_reported_with_their_line() {
        let d = TempDir::new("manifest-typo");
        let p = problems(&d, &format!("{MINIMAL}nmae = \"x\"\n"));
        assert_eq!(p, ["plugin.toml line 5: there's no key nmae (did you mean name?)"]);
        assert!(problems(&d, &format!("{MINIMAL}[detect]\nfile = [\"x\"]\n"))[0].contains("detect: there's no key file (did you mean files?)"));
        let far = problems(&d, &format!("{MINIMAL}colour = \"x\"\n"));
        assert!(far[0].contains("there's no key colour here (the keys are id, name,"), "{far:?}");
        assert_eq!(problems(&d, "id = \"x\"\n"), ["plugin.toml line 1: it needs name"]);
    }

    #[test]
    fn pages_and_files_must_be_there_and_inside() {
        let d = TempDir::new("manifest-files");
        d.file("tab.html", "").file("icon.svg", "");
        let text = format!("{MINIMAL}icon = \"icon.svg\"\n[[tab]]\nid = \"t\"\nname = \"T\"\npage = \"tab.html\"\n");
        assert!(problems(&d, &text).is_empty());
        let p = problems(&d, &format!("{MINIMAL}[[tab]]\nid = \"t\"\nname = \"T\"\npage = \"nope.html\"\n[[panel]]\nid = \"p\"\nname = \"P\"\npage = \"../x.html\"\n[view]\nname = \"V\"\npage = \"/etc/passwd\"\n"));
        assert_eq!(p.len(), 3, "{p:?}");
        assert!(p[0].contains("isn't there") && p[1].contains("inside") && p[2].contains("inside"));
        assert!(problems(&d, &format!("{MINIMAL}[detect]\nicon = \"whale\"\n"))[0].contains("neither an .svg"));
    }

    #[test]
    fn surfaces_ids_and_fields() {
        let d = TempDir::new("manifest-surfaces");
        d.file("a.html", "");
        let p = problems(&d, &format!("{MINIMAL}{}", r#"
[[tab]]
id = "t"
name = "T"
page = "a.html"
[[tab.setup]]
key = "title"
label = "Title"
type = "text"
[[tab.setup]]
key = "level"
label = "Level"
type = "choice"
[[tab]]
id = "t"
name = "Again"
page = "a.html"
[[panel]]
id = "p"
name = "P"
page = "a.html"
scope = "everywhere"
height = "tall"
[[settings]]
key = "n"
label = "N"
type = "number"
default = "five"
[[settings]]
key = "c"
label = "C"
type = "colour"
"#));
        let want = ["two tabs have the id t", "thumbdeck's own field", "has no choices", "scope is", "height is", "default doesn't fit", "the types are"];
        assert_eq!(p.len(), want.len(), "{p:?}");
        for (got, want) in p.iter().zip(want) {
            assert!(got.contains(want), "{got:?} should say {want:?}");
        }
    }

    #[test]
    fn keys_need_a_real_surface_and_follow_the_rules() {
        let d = TempDir::new("manifest-keys");
        d.file("a.html", "");
        let base = format!("{MINIMAL}[[tab]]\nid = \"t\"\nname = \"T\"\npage = \"a.html\"\n");
        let ok = format!("{base}[keys.list]\nname = \"T\"\nsurface = \"tab:t\"\nbindings = [{{ keys = [\"j\"], action = \"down\", does = \"down\" }}]\n[keys.all]\nname = \"A\"\nsurface = \"tab:t\"\nbindings = []\n");
        assert!(problems(&d, &ok).is_empty());
        let names: Vec<String> = parse(&ok, &d.0).unwrap().keymaps("tab:t").into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["list", "all"], "in the order written, not sorted");
        let bad = format!("{base}[keys.list]\nname = \"T\"\nsurface = \"tab:nope\"\nbindings = [{{ keys = [\"j\"], action = \"jump\", does = \"x\" }}]\n");
        let p = problems(&d, &bad);
        assert_eq!(p.len(), 2, "{p:?}");
        assert!(p[0].contains("means down") && p[1].contains("isn't one of the plugin's"));
    }

    #[test]
    fn complete_fills_in_defaults() {
        let d = TempDir::new("manifest-complete");
        let m = parse(&format!("{MINIMAL}{}", r#"
[[settings]]
key = "minutes"
label = "Minutes"
type = "number"
default = 25
[[settings]]
key = "folders"
label = "Folders"
type = "folders"
[[settings]]
key = "level"
label = "Level"
type = "choice"
choices = ["info", { value = "warn", label = "Warnings" }]
"#), &d.0).unwrap();
        assert_eq!(m.settings[2].choices[1], Choice { value: "warn".into(), label: "Warnings".into() });
        let full = complete(&m.settings, &Value::Null);
        assert_eq!(full, serde_json::json!({ "minutes": 25, "folders": [], "level": "info" }));
        let kept = complete(&m.settings, &serde_json::json!({ "minutes": 50, "level": "gone", "title": "Mine" }));
        assert_eq!(kept, serde_json::json!({ "minutes": 50, "folders": [], "level": "info", "title": "Mine" }));
    }

    #[test]
    fn versions_compare_as_numbers() {
        assert!(version_key("1.10.0") > version_key("1.9.0"));
        assert!(version_key("2.0.0") > version_key("2.0.0-beta"));
        assert!(version_key("0.1.0") < version_key("0.1.1"));
    }
}
