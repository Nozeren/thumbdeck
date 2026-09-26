//! A project's toolkit: the buttons of the packs that apply to it.

use crate::packs::{Pack, PackAction, Packs, Source};
use regex::{Captures, Regex};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

#[derive(Serialize, Clone)]
pub struct Action {
    /// Stable within a project, e.g. "npm:dev"
    pub id: String,
    pub label: String,
    pub command: String,
    /// Where the action came from: the pack id (npm, django, ...) or custom (added by you)
    pub source: String,
    /// Group title in the Toolkit: the pack's name, or "yours"
    pub group: String,
    pub description: Option<String>,
    /// Ask before running
    pub confirm: bool,
    /// Run in this window of the project's tmux session instead of inside thumbdeck
    pub tmux: Option<String>,
}

pub struct Toolkit {
    pub actions: Vec<Action>,
    /// Packs that couldn't be used, and why
    pub problems: Vec<String>,
}

/// The packs that apply to a project, with the packs they require, lowest priority first.
fn active<'a>(packs: &'a [Pack], dir: &Path) -> Vec<&'a Pack> {
    let find = |id: &str| packs.iter().find(|p| p.id == id);
    let mut ids: Vec<&str> =
        packs.iter().filter(|p| p.detect.as_ref().is_none_or(|d| d.matches(dir))).map(|p| p.id.as_str()).collect();
    let mut i = 0;
    while i < ids.len() {
        for r in find(ids[i]).map(|p| p.requires.as_slice()).unwrap_or_default() {
            if !ids.contains(&r.as_str()) {
                ids.push(r);
            }
        }
        i += 1;
    }
    let mut list: Vec<&Pack> = packs.iter().filter(|p| ids.contains(&p.id.as_str())).collect();
    list.sort_by(|a, b| (a.priority, &a.id).cmp(&(b.priority, &b.id)));
    list
}

/// The project's icon: from the first pack that applies and has one.
pub fn icon(packs: &[Pack], dir: &Path) -> Option<String> {
    active(packs, dir).into_iter().find_map(|p| p.icon.clone())
}

pub fn detect(packs: &Packs, dir: &Path) -> Toolkit {
    let mut toolkit = Toolkit { actions: Vec::new(), problems: packs.problems.clone() };
    for pack in active(&packs.packs, dir) {
        let vars = vars(&packs.packs, pack);
        let mut add = |a: &PackAction, item: Option<&str>| {
            if a.when.as_ref().is_none_or(|w| w.matches(dir)) {
                let action = make(pack, a, dir, &vars, item);
                if !toolkit.actions.iter().any(|x| x.id == action.id) {
                    toolkit.actions.push(action);
                }
            }
        };
        for a in &pack.actions {
            add(a, None);
        }
        for g in &pack.generate {
            match items(&g.source, dir) {
                Ok(items) => items.iter().filter(|i| !g.skip.contains(i)).for_each(|i| add(&g.action, Some(i))),
                Err(e) => toolkit.problems.push(format!("pack {}: {e}", pack.id)),
            }
        }
    }
    toolkit
}

fn make(pack: &Pack, a: &PackAction, dir: &Path, vars: &HashMap<String, String>, item: Option<&str>) -> Action {
    let name = expand(&a.name, dir, vars, item);
    Action {
        id: format!("{}:{name}", pack.id),
        command: expand(&a.command, dir, vars, item),
        description: a.description.as_ref().map(|d| expand(d, dir, vars, item)),
        confirm: a.confirm,
        tmux: a.tmux.as_ref().and_then(|t| t.window(&name)),
        source: pack.id.clone(),
        group: pack.name.clone(),
        label: name,
    }
}

/// A pack's [vars] with those of the packs it requires; its own win.
fn vars(packs: &[Pack], pack: &Pack) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for id in &pack.requires {
        if let Some(required) = packs.iter().find(|p| &p.id == id) {
            out.extend(vars(packs, required));
        }
    }
    out.extend(pack.vars.iter().map(|(k, v)| (k.clone(), v.clone())));
    out
}

/// Fill in {variables}: the built-in ones, then the pack's. Anything else in braces stays
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
fn items(source: &Source, dir: &Path) -> Result<Vec<String>, String> {
    if let Some(file) = &source.json {
        let at = source.keys.as_deref().or(source.values.as_deref()).unwrap_or_default();
        let value = crate::packs::json_at(dir, file, at);
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
    Err("command sources aren't supported yet, so its generated actions are left out".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packs::tests::{packs, TempDir};

    fn labels(t: &Toolkit) -> Vec<String> {
        t.actions.iter().map(|a| a.id.clone()).collect()
    }

    #[test]
    fn packs_apply_by_detection_and_requirement_in_priority_order() {
        let p = packs(&[
            ("base", "schema = 1\nname = \"Base\"\npriority = 60\n[detect]\nfiles = [\"base\"]\n[[action]]\nname = \"b\"\ncommand = \"b\""),
            ("top", "schema = 1\nname = \"Top\"\npriority = 10\nrequires = [\"base\"]\n[detect]\nfiles = [\"top\"]\n[[action]]\nname = \"t\"\ncommand = \"t\""),
            ("other", "schema = 1\nname = \"Other\"\n[detect]\nfiles = [\"other\"]\n[[action]]\nname = \"o\"\ncommand = \"o\""),
        ]);
        let d = TempDir::new("requires");
        assert!(labels(&detect(&p, &d.0)).is_empty());
        d.file("top", "");
        assert_eq!(labels(&detect(&p, &d.0)), ["top:t", "base:b"], "base comes along with top");
        let t = detect(&p, &d.0);
        assert_eq!((t.actions[0].group.as_str(), t.actions[0].source.as_str()), ("Top", "top"));
    }

    #[test]
    fn variables() {
        let p = packs(&[
            ("base", "schema = 1\nname = \"B\"\n[vars]\nrun = \"{python} base.py\"\nshared = \"base\""),
            ("top", r#"schema = 1
name = "T"
requires = ["base"]
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
        let p = packs(&[("p", r#"schema = 1
name = "P"
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
        let p = packs(&[("g", r#"schema = 1
name = "G"
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
[[generate]]
source = { command = "just --summary" }
action = { name = "{item}", command = "just {item}" }
"#)]);
        let d = TempDir::new("generate");
        let t = detect(&p, &d.0);
        assert!(t.actions.is_empty(), "no files, no items");
        assert!(t.problems[0].contains("command sources"));
        d.file("p.json", r#"{"scripts": {"dev": "x", "prepare": "y"}, "list": ["a", 1]}"#);
        d.file("Makefile", "build:\n\tcc\ndev: build\nbuild:\n");
        let t = detect(&p, &d.0);
        assert_eq!(labels(&t), ["g:dev", "g:v-a", "g:build"], "skip, strings only, first of duplicates wins");
        assert_eq!(t.actions[0].command, "run dev");
    }

    /// The bundled packs give what thumbdeck detected before it had packs
    mod bundled {
        use super::*;

        fn toolkit(d: &TempDir) -> Vec<(String, String)> {
            let p = crate::packs::from_sources(
                crate::packs::BUNDLED.iter().map(|(i, t)| (i.to_string(), t.to_string(), "b".into())).collect(),
            );
            detect(&p, &d.0).actions.into_iter().map(|a| (a.id, a.command)).collect()
        }

        fn ids(d: &TempDir) -> Vec<String> {
            toolkit(d).into_iter().map(|(id, _)| id).collect()
        }

        #[test]
        fn npm_scripts_with_the_projects_package_manager() {
            let d = TempDir::new("b-npm");
            d.file("package.json", r#"{"scripts": {"dev": "vite", "build": "vite build", "prepare": "x", "postinstall": "y"}}"#);
            assert_eq!(toolkit(&d), [("npm:build".into(), "npm run build".into()), ("npm:dev".into(), "npm run dev".into())]);
            d.file("pnpm-lock.yaml", "");
            assert_eq!(toolkit(&d)[0].1, "pnpm run build");
        }

        #[test]
        fn makefile_targets() {
            let d = TempDir::new("b-make");
            d.file("Makefile", ".PHONY: build\nCC := gcc\nX:=1\nbuild: dep\n\tcc -o a: b\ndist/app:\ntest:\nbuild:\n%.o: %.c\na b: c\nclean::\n");
            assert_eq!(ids(&d), ["make:build", "make:dist/app", "make:test", "make:clean"]);
            assert_eq!(toolkit(&d)[0].1, "make build");
        }

        #[test]
        fn django_with_python_and_the_venv() {
            let d = TempDir::new("b-django");
            d.file("manage.py", "").file("requirements.txt", "");
            let t = toolkit(&d);
            let got: Vec<&str> = t.iter().map(|(id, _)| id.as_str()).collect();
            assert_eq!(got, ["django:runserver", "django:migrate", "django:makemigrations", "django:test", "django:shell",
                             "python:create venv", "python:install"]);
            assert_eq!(t[1].1, "python3 manage.py migrate");
            d.file(".venv/bin/python", "");
            assert_eq!(toolkit(&d)[1].1, ".venv/bin/python manage.py migrate");
            assert!(!ids(&d).contains(&"python:create venv".to_string()), "has a venv already");
        }

        #[test]
        fn django_servers_and_shells_run_in_tmux() {
            let d = TempDir::new("b-django-tmux");
            d.file("manage.py", "");
            let p = crate::packs::from_sources(
                crate::packs::BUNDLED.iter().map(|(i, t)| (i.to_string(), t.to_string(), "b".into())).collect(),
            );
            let t = detect(&p, &d.0);
            let window = |id: &str| t.actions.iter().find(|a| a.id == id).and_then(|a| a.tmux.clone());
            assert_eq!(window("django:runserver").as_deref(), Some("server"));
            assert_eq!(window("django:shell").as_deref(), Some("shell"));
            assert_eq!(window("django:migrate"), None);
        }

        #[test]
        fn pytest_when_configured() {
            let d = TempDir::new("b-pytest");
            d.file("pyproject.toml", "[project]\nname = \"x\"");
            assert!(!ids(&d).contains(&"python:pytest".to_string()));
            d.file("pyproject.toml", "[tool.pytest.ini_options]\n");
            assert!(ids(&d).contains(&"python:pytest".to_string()), "configured in pyproject.toml");
            let only_ini = TempDir::new("b-pytest-ini");
            only_ini.file("pytest.ini", "");
            assert!(toolkit(&only_ini).contains(&("python:pytest".into(), "python3 -m pytest".into())));
        }

        #[test]
        fn compose_gradle_cargo_go() {
            let d = TempDir::new("b-misc");
            d.file("docker-compose.yml", "").file("Cargo.toml", "").file("go.mod", "").file("gradlew", "");
            assert_eq!(ids(&d), ["gradle:assembleDebug", "gradle:installDebug", "gradle:test", "cargo:run", "cargo:build",
                                 "cargo:test", "go:run", "go:test", "compose:up", "compose:down", "compose:logs", "compose:ps"]);
            assert!(toolkit(&d).contains(&("compose:logs".into(), "docker compose logs -f --tail=100".into())));
            assert!(toolkit(&d).contains(&("go:test".into(), "go test ./...".into())));
            let gradle_only = TempDir::new("b-gradle-only");
            gradle_only.file("build.gradle.kts", "");
            assert!(ids(&gradle_only).is_empty(), "no wrapper, no buttons");
        }

        #[test]
        fn icons_in_the_old_order() {
            let p = crate::packs::from_sources(
                crate::packs::BUNDLED.iter().map(|(i, t)| (i.to_string(), t.to_string(), "b".into())).collect(),
            );
            let d = TempDir::new("b-icons");
            assert_eq!(icon(&p.packs, &d.0), None);
            for (file, want) in [("requirements.txt", "python"), ("go.mod", "go"), ("Cargo.toml", "rust"),
                                 ("package.json", "node"), ("build.gradle", "android"), ("manage.py", "django")] {
                d.file(file, "{}");
                assert_eq!(icon(&p.packs, &d.0).as_deref(), Some(want), "after adding {file}");
            }
        }
    }
}
