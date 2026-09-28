//! A project's Toolkit: the buttons of the plugins that apply to it.

use super::detect::Conditions;
use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::OnceLock;

/// An `[[action]]` as written in a manifest
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ActionDef {
    pub name: String,
    pub command: String,
    pub description: Option<String>,
    #[serde(default)]
    pub confirm: bool,
    pub when: Option<Conditions>,
    pub tmux: Option<Tmux>,
}

/// `tmux = true` (window named after the action) or `tmux = { window = "name" }`
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Tmux {
    On(bool),
    Window { window: String },
}

impl Tmux {
    /// The window to run in, if any
    pub fn window(&self, action_name: &str) -> Option<String> {
        match self {
            Tmux::On(true) => Some(action_name.to_string()),
            Tmux::On(false) => None,
            Tmux::Window { window } => Some(window.clone()),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Generate {
    pub source: Source,
    #[serde(default)]
    pub skip: Vec<String>,
    /// A command source's result is kept until one of these files changes
    #[serde(default)]
    pub watch: Vec<String>,
    pub action: ActionDef,
}

/// Where a [[generate]] gets its items: exactly one of json / file / command.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub json: Option<String>,
    pub keys: Option<String>,
    pub values: Option<String>,
    pub file: Option<String>,
    pub regex: Option<String>,
    pub command: Option<String>,
    /// How a command's output is cut into items (by lines when not given)
    pub split: Option<String>,
}

impl Generate {
    /// What's wrong with it, in plain sentences
    pub fn problems(&self) -> Vec<String> {
        let s = &self.source;
        let mut out = Vec::new();
        let kinds = [s.json.is_some(), s.file.is_some(), s.command.is_some()].iter().filter(|k| **k).count();
        if kinds != 1 {
            out.push("a [[generate]] source needs exactly one of json, file or command".into());
        }
        if s.json.is_some() && s.keys.is_some() == s.values.is_some() {
            out.push("a json source needs either keys or values".into());
        }
        if s.file.is_some() {
            match s.regex.as_deref() {
                None => out.push("a file source needs a regex".into()),
                Some(re) => {
                    if let Err(e) = Regex::new(re) {
                        out.push(format!("bad regex: {e}"));
                    }
                }
            }
        }
        out
    }
}

/// What a plugin brings to the Toolkit
#[derive(Debug, Clone)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub priority: i32,
    pub requires: Vec<String>,
    /// None: applies to every project
    pub detect: Option<Conditions>,
    /// The icon of projects it applies to
    pub icon: Option<String>,
    pub vars: BTreeMap<String, String>,
    pub actions: Vec<ActionDef>,
    pub generate: Vec<Generate>,
    /// Its backend answers `actions`: asked again when these files change
    pub backend_actions: Option<Vec<String>>,
}

pub const DEFAULT_PRIORITY: i32 = 50;

#[derive(Serialize, Clone, Debug)]
pub struct Action {
    /// Stable within a project, e.g. "npm:dev"
    pub id: String,
    pub label: String,
    pub command: String,
    /// Where the action came from: the plugin id (npm, django, ...) or custom (added by you)
    pub source: String,
    /// Group title in the Toolkit: the plugin's name, or "yours"
    pub group: String,
    pub description: Option<String>,
    /// Ask before running
    pub confirm: bool,
    /// Run in this window of the project's tmux session instead of inside thumbdeck
    pub tmux: Option<String>,
}

pub struct Toolkit {
    pub actions: Vec<Action>,
    /// Plugins whose buttons couldn't be worked out, and why
    pub problems: Vec<String>,
}

/// Drop the providers whose requirements are circular or missing; says which and why.
pub fn resolve(mut providers: Vec<Provider>) -> (Vec<Provider>, Vec<String>) {
    let mut problems = Vec::new();
    // Circular requirements: every provider on a cycle is dropped
    let cyclic: Vec<String> = providers
        .iter()
        .filter(|p| reaches(&providers, &p.requires, &p.id, &mut vec![]))
        .map(|p| p.id.clone())
        .collect();
    if !cyclic.is_empty() {
        problems.push(format!("{} require each other in a circle; they're off", cyclic.join(", ")));
        providers.retain(|p| !cyclic.contains(&p.id));
    }
    // Missing requirements, followed until nothing changes (a needs b, b needs missing c)
    loop {
        let ids: Vec<String> = providers.iter().map(|p| p.id.clone()).collect();
        let Some(i) = providers.iter().position(|p| p.requires.iter().any(|r| !ids.contains(r))) else { break };
        let p = providers.remove(i);
        let missing: Vec<&str> = p.requires.iter().filter(|r| !ids.contains(r)).map(|r| r.as_str()).collect();
        problems.push(format!("{} is off: it requires {}, which isn't installed or is off", p.id, missing.join(", ")));
    }
    (providers, problems)
}

/// Whether `target` can be reached by following requirements from `from`
fn reaches(providers: &[Provider], from: &[String], target: &str, seen: &mut Vec<String>) -> bool {
    from.iter().any(|id| {
        if id == target {
            return true;
        }
        if seen.contains(id) {
            return false;
        }
        seen.push(id.clone());
        providers.iter().find(|p| &p.id == id).is_some_and(|p| reaches(providers, &p.requires, target, seen))
    })
}

/// The providers that apply to a project, with the ones they require, lowest priority first.
pub fn active<'a>(providers: &'a [Provider], dir: &Path) -> Vec<&'a Provider> {
    let find = |id: &str| providers.iter().find(|p| p.id == id);
    let mut ids: Vec<&str> =
        providers.iter().filter(|p| p.detect.as_ref().is_none_or(|d| d.matches(dir))).map(|p| p.id.as_str()).collect();
    let mut i = 0;
    while i < ids.len() {
        for r in find(ids[i]).map(|p| p.requires.as_slice()).unwrap_or_default() {
            if !ids.contains(&r.as_str()) {
                ids.push(r);
            }
        }
        i += 1;
    }
    let mut list: Vec<&Provider> = providers.iter().filter(|p| ids.contains(&p.id.as_str())).collect();
    list.sort_by(|a, b| (a.priority, &a.id).cmp(&(b.priority, &b.id)));
    list
}

/// The project's icon: from the first provider that applies and has one.
pub fn icon(providers: &[Provider], dir: &Path) -> Option<String> {
    active(providers, dir).into_iter().find_map(|p| p.icon.clone())
}

pub fn detect(providers: &[Provider], dir: &Path) -> Toolkit {
    let mut toolkit = Toolkit { actions: Vec::new(), problems: Vec::new() };
    for p in active(providers, dir) {
        let vars = vars(providers, p);
        let mut add = |a: &ActionDef, item: Option<&str>| {
            if a.when.as_ref().is_none_or(|w| w.matches(dir)) {
                let action = make(p, a, dir, &vars, item);
                if !toolkit.actions.iter().any(|x| x.id == action.id) {
                    toolkit.actions.push(action);
                }
            }
        };
        for a in &p.actions {
            add(a, None);
        }
        for g in &p.generate {
            match items(g, dir) {
                Ok(items) => items.iter().filter(|i| !g.skip.contains(i)).for_each(|i| add(&g.action, Some(i))),
                Err(e) => toolkit.problems.push(format!("{}: {e}", p.id)),
            }
        }
    }
    toolkit
}

fn make(p: &Provider, a: &ActionDef, dir: &Path, vars: &HashMap<String, String>, item: Option<&str>) -> Action {
    let name = expand(&a.name, dir, vars, item);
    Action {
        id: format!("{}:{name}", p.id),
        command: expand(&a.command, dir, vars, item),
        description: a.description.as_ref().map(|d| expand(d, dir, vars, item)),
        confirm: a.confirm,
        tmux: a.tmux.as_ref().and_then(|t| t.window(&name)),
        source: p.id.clone(),
        group: p.name.clone(),
        label: name,
    }
}

/// A provider's [vars] with those of the providers it requires; its own win.
fn vars(providers: &[Provider], p: &Provider) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for id in &p.requires {
        if let Some(required) = providers.iter().find(|x| &x.id == id) {
            out.extend(vars(providers, required));
        }
    }
    out.extend(p.vars.iter().map(|(k, v)| (k.clone(), v.clone())));
    out
}

/// Fill in {variables}: the built-in ones, then the plugin's. Anything else in braces stays
/// (shell `${HOME}`, awk's `{print $1}`).
fn expand(text: &str, dir: &Path, vars: &HashMap<String, String>, item: Option<&str>) -> String {
    static VAR: OnceLock<Regex> = OnceLock::new();
    let re = VAR.get_or_init(|| Regex::new(r"\{([A-Za-z_][A-Za-z0-9_]*)\}").unwrap());
    re.replace_all(text, |c: &Captures| {
        let name = &c[1];
        if let Some(value) = builtin(name, dir, item) {
            value
        } else if let Some(value) = vars.get(name) {
            expand(value, dir, &HashMap::new(), item)
        } else {
            c[0].to_string()
        }
    })
    .into_owned()
}

fn builtin(name: &str, dir: &Path, item: Option<&str>) -> Option<String> {
    Some(match name {
        "project" => dir.file_name()?.to_string_lossy().to_string(),
        "path" => dir.to_string_lossy().to_string(),
        "python" => python(dir),
        "pm" => package_manager(dir).to_string(),
        "item" => item?.to_string(),
        _ => return None,
    })
}

/// Python in the project's own .venv when there is one.
fn python(dir: &Path) -> String {
    for venv in [".venv", "venv"] {
        if dir.join(venv).join("bin").join("python").exists() {
            return format!("{venv}/bin/python");
        }
    }
    "python3".to_string()
}

/// The JavaScript package manager, from the lockfile.
fn package_manager(dir: &Path) -> &'static str {
    if dir.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if dir.join("yarn.lock").exists() {
        "yarn"
    } else if dir.join("bun.lockb").exists() || dir.join("bun.lock").exists() {
        "bun"
    } else {
        "npm"
    }
}

/// The items of a [[generate]]; none when its file isn't there.
fn items(g: &Generate, dir: &Path) -> Result<Vec<String>, String> {
    let source = &g.source;
    if let Some(file) = &source.json {
        let at = source.keys.as_deref().or(source.values.as_deref()).unwrap_or_default();
        let value = super::detect::json_at(dir, file, at);
        return Ok(match value {
            Some(serde_json::Value::Object(map)) if source.keys.is_some() => map.keys().cloned().collect(),
            Some(serde_json::Value::Array(list)) if source.values.is_some() => {
                list.iter().filter_map(|v| v.as_str().map(String::from)).collect()
            }
            _ => vec![],
        });
    }
    if let (Some(file), Some(re)) = (&source.file, &source.regex) {
        let re = Regex::new(re).map_err(|e| e.to_string())?; // checked when loading
        let text = std::fs::read_to_string(dir.join(file)).unwrap_or_default();
        return Ok(text.lines().filter_map(|l| Some(re.captures(l)?.get(1)?.as_str().to_string())).collect());
    }
    let command = source.command.as_deref().unwrap_or_default();
    command_items(dir, command, source.split.as_deref(), &g.watch)
}

/// When a file was last changed (None: it isn't there)
fn changed_at(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// A command's output as items: run in the project with a 2-second limit, and kept until one
/// of the `watch` files changes (a minute when there are none)
fn command_items(dir: &Path, command: &str, split: Option<&str>, watch: &[String]) -> Result<Vec<String>, String> {
    type Stamp = Vec<Option<std::time::SystemTime>>;
    static CACHE: OnceLock<std::sync::Mutex<HashMap<(std::path::PathBuf, String), (Stamp, std::time::Instant, Vec<String>)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = (dir.to_path_buf(), command.to_string());
    let stamp: Stamp = watch.iter().map(|f| changed_at(&dir.join(f))).collect();
    if let Some((s, at, items)) = cache.lock().unwrap().get(&key) {
        let fresh = if watch.is_empty() { at.elapsed() < std::time::Duration::from_secs(60) } else { *s == stamp };
        if fresh {
            return Ok(items.clone());
        }
    }
    let out = crate::plugins::api::shell_output(dir, command, 2000)?;
    let items: Vec<String> = match split {
        Some(sep) => out.split(sep).map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect(),
        None => out.lines().map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect(),
    };
    cache.lock().unwrap().insert(key, (stamp, std::time::Instant::now(), items.clone()));
    Ok(items)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::testutil::TempDir;

    /// Providers from manifest-style TOML: (id, text); name defaults to the id
    pub fn providers(list: &[(&str, &str)]) -> Vec<Provider> {
        #[derive(Deserialize)]
        struct T {
            name: Option<String>,
            #[serde(default)]
            priority: Option<i32>,
            #[serde(default)]
            requires: Vec<String>,
            detect: Option<Conditions>,
            icon: Option<String>,
            #[serde(default)]
            vars: BTreeMap<String, String>,
            #[serde(default, rename = "action")]
            actions: Vec<ActionDef>,
            #[serde(default)]
            generate: Vec<Generate>,
        }
        list.iter()
            .map(|(id, text)| {
                let t: T = toml::from_str(text).unwrap();
                Provider {
                    id: id.to_string(),
                    name: t.name.unwrap_or_else(|| id.to_string()),
                    priority: t.priority.unwrap_or(DEFAULT_PRIORITY),
                    requires: t.requires,
                    detect: t.detect,
                    icon: t.icon,
                    vars: t.vars,
                    actions: t.actions,
                    generate: t.generate,
                    backend_actions: None,
                }
            })
            .collect()
    }

    fn labels(t: &Toolkit) -> Vec<String> {
        t.actions.iter().map(|a| a.id.clone()).collect()
    }

    #[test]
    fn providers_apply_by_detection_and_requirement_in_priority_order() {
        let p = providers(&[
            ("base", "name = \"Base\"\npriority = 60\n[detect]\nfiles = [\"base\"]\n[[action]]\nname = \"b\"\ncommand = \"b\""),
            ("top", "name = \"Top\"\npriority = 10\nrequires = [\"base\"]\n[detect]\nfiles = [\"top\"]\n[[action]]\nname = \"t\"\ncommand = \"t\""),
            ("other", "name = \"Other\"\n[detect]\nfiles = [\"other\"]\n[[action]]\nname = \"o\"\ncommand = \"o\""),
        ]);
        let d = TempDir::new("requires");
        assert!(labels(&detect(&p, &d.0)).is_empty());
        d.file("top", "");
        assert_eq!(labels(&detect(&p, &d.0)), ["top:t", "base:b"], "base comes along with top");
        let t = detect(&p, &d.0);
        assert_eq!((t.actions[0].group.as_str(), t.actions[0].source.as_str()), ("Top", "top"));
    }

    #[test]
    fn missing_and_circular_requirements_turn_providers_off() {
        let (kept, problems) = resolve(providers(&[
            ("a", "requires = [\"b\"]"),
            ("b", "requires = [\"gone\"]"),
            ("x", "requires = [\"y\"]"),
            ("y", "requires = [\"x\"]"),
            ("z", "requires = [\"x\"]"),
            ("ok", ""),
        ]));
        let ids: Vec<&str> = kept.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["ok"], "a and b (missing), x and y (circle), z (needs x)");
        assert!(problems.iter().any(|p| p.contains("b is off") && p.contains("gone")));
        assert!(problems.iter().any(|p| p.contains("a is off")));
        assert!(problems.iter().any(|p| p.contains("circle")));
    }

    #[test]
    fn variables() {
        let p = providers(&[
            ("base", "[vars]\nrun = \"{python} base.py\"\nshared = \"base\""),
            ("top", r#"requires = ["base"]
[vars]
shared = "top"
project = "not allowed"
[[action]]
name = "go"
command = "{run} {shared} {project} ${HOME} awk '{print $1}' {nope} {item}"
"#),
        ]);
        let d = TempDir::new("vars");
        let t = detect(&p, &d.0);
        let name = d.0.file_name().unwrap().to_string_lossy();
        assert_eq!(t.actions[0].command, format!("python3 base.py top {name} ${{HOME}} awk '{{print $1}}' {{nope}} {{item}}"));
        d.file(".venv/bin/python", "");
        assert!(detect(&p, &d.0).actions[0].command.starts_with(".venv/bin/python base.py"));
    }

    #[test]
    fn when_confirm_description_and_tmux() {
        let p = providers(&[("p", r#"
[[action]]
name = "server"
command = "serve"
description = "Start {project}"
confirm = true
tmux = true
[[action]]
name = "shell"
command = "sh"
tmux = { window = "repl" }
[[action]]
name = "only-with-x"
command = "x"
when = { files = ["x"] }
"#)]);
        let d = TempDir::new("when");
        let t = detect(&p, &d.0);
        assert_eq!(labels(&t), ["p:server", "p:shell"]);
        assert_eq!(t.actions[0].tmux.as_deref(), Some("server"), "tmux = true: named after the action");
        assert_eq!(t.actions[1].tmux.as_deref(), Some("repl"));
        assert!(t.actions[0].confirm);
        assert!(t.actions[0].description.as_ref().unwrap().starts_with("Start thumbdeck-when"));
        d.file("x", "");
        assert_eq!(labels(&detect(&p, &d.0)).len(), 3);
    }

    #[test]
    fn generated_actions() {
        let p = providers(&[("g", r#"
[[generate]]
source = { json = "p.json", keys = "scripts" }
skip = ["prepare"]
action = { name = "{item}", command = "run {item}" }
[[generate]]
source = { json = "p.json", values = "list" }
action = { name = "v-{item}", command = "v {item}" }
[[generate]]
source = { file = "Makefile", regex = "^([a-z]+):" }
action = { name = "{item}", command = "make {item}" }
"#)]);
        let d = TempDir::new("generate");
        let t = detect(&p, &d.0);
        assert!(t.actions.is_empty(), "no files, no items");
        assert!(t.problems.is_empty());
        d.file("p.json", r#"{"scripts": {"dev": "x", "prepare": "y"}, "list": ["a", 1]}"#);
        d.file("Makefile", "build:\n\tcc\ndev: build\nbuild:\n");
        let t = detect(&p, &d.0);
        assert_eq!(labels(&t), ["g:dev", "g:v-a", "g:build"], "skip, strings only, first of duplicates wins");
        assert_eq!(t.actions[0].command, "run dev");
    }

    #[test]
    fn command_sources_are_run_and_kept_until_a_watched_file_changes() {
        let d = TempDir::new("generate-command");
        d.file("tasks.txt", "build test");
        let p = providers(&[("j", r#"
[[generate]]
source = { command = "cat tasks.txt; echo ran >> runs.log", split = " " }
watch = ["tasks.txt"]
action = { name = "{item}", command = "just {item}" }
[[generate]]
source = { command = "exit 3" }
action = { name = "{item}", command = "x" }
"#)]);
        let t = detect(&p, &d.0);
        assert_eq!(labels(&t), ["j:build", "j:test"]);
        assert!(t.problems[0].contains("failed (exit 3)"), "{:?}", t.problems);
        detect(&p, &d.0);
        assert_eq!(std::fs::read_to_string(d.0.join("runs.log")).unwrap().lines().count(), 1, "kept, not run again");
        std::thread::sleep(std::time::Duration::from_millis(20));
        d.file("tasks.txt", "lint");
        assert_eq!(labels(&detect(&p, &d.0)), ["j:lint"], "the watched file changed");
    }

    #[test]
    fn bad_generate_sources() {
        let g = |source: &str| -> Vec<String> {
            let text = format!("source = {source}\naction = {{ name = \"x\", command = \"x\" }}");
            toml::from_str::<Generate>(&text).unwrap().problems()
        };
        assert!(g(r#"{ json = "p.json" }"#)[0].contains("keys or values"));
        assert!(g(r#"{ json = "p.json", keys = "a", file = "b", regex = "x" }"#)[0].contains("exactly one"));
        assert!(g(r#"{ file = "Makefile" }"#)[0].contains("needs a regex"));
        assert!(g(r#"{ file = "Makefile", regex = "(" }"#)[0].contains("bad regex"));
        assert!(g(r#"{ json = "p.json", keys = "scripts" }"#).is_empty());
    }
}
