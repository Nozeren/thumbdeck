//! The official plugins (the plugins repo, checked out in toolkits/plugins) give the Toolkit
//! buttons and icons the built-in packs gave before them.

use super::toolkit::{detect, icon, resolve, Provider};
use crate::testutil::TempDir;
use std::path::PathBuf;

fn folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("toolkits/plugins")
}

fn providers() -> Vec<Provider> {
    let mut all = Vec::new();
    for entry in std::fs::read_dir(folder()).expect("the plugins repo isn't checked out: run `git submodule update --init`") {
        let dir = entry.unwrap().path();
        let m = super::manifest::read(&dir).unwrap_or_else(|p| panic!("{}: {p:?}", dir.display()));
        all.extend(m.provider(&dir));
    }
    let (all, problems) = resolve(all);
    assert!(problems.is_empty(), "{problems:?}");
    all
}

fn toolkit(d: &TempDir) -> Vec<(String, String)> {
    detect(&providers(), &d.0).actions.into_iter().map(|a| (a.id, a.command)).collect()
}

fn ids(d: &TempDir) -> Vec<String> {
    toolkit(d).into_iter().map(|(id, _)| id).collect()
}

#[test]
fn every_official_plugin_checks_out() {
    for entry in std::fs::read_dir(folder()).unwrap() {
        let dir = entry.unwrap().path();
        let m = super::manifest::read(&dir).unwrap_or_else(|p| panic!("{}: {p:?}", dir.display()));
        assert_eq!(dir.file_name().unwrap().to_string_lossy(), m.id, "a plugin's folder is named after its id");
        assert!(dir.join("README.md").is_file(), "{} has a README", m.id);
    }
}

#[test]
fn npm_scripts_with_the_projects_package_manager() {
    let d = TempDir::new("o-npm");
    d.file("package.json", r#"{"scripts": {"dev": "vite", "build": "vite build", "prepare": "x", "postinstall": "y"}}"#);
    assert_eq!(toolkit(&d), [("npm:build".into(), "npm run build".into()), ("npm:dev".into(), "npm run dev".into())]);
    d.file("pnpm-lock.yaml", "");
    assert_eq!(toolkit(&d)[0].1, "pnpm run build");
}

#[test]
fn makefile_targets() {
    let d = TempDir::new("o-make");
    d.file("Makefile", ".PHONY: build\nCC := gcc\nX:=1\nbuild: dep\n\tcc -o a: b\ndist/app:\ntest:\nbuild:\n%.o: %.c\na b: c\nclean::\n");
    assert_eq!(ids(&d), ["make:build", "make:dist/app", "make:test", "make:clean"]);
    assert_eq!(toolkit(&d)[0].1, "make build");
}

#[test]
fn django_with_python_and_the_venv() {
    let d = TempDir::new("o-django");
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
    let d = TempDir::new("o-django-tmux");
    d.file("manage.py", "");
    let t = detect(&providers(), &d.0);
    let window = |id: &str| t.actions.iter().find(|a| a.id == id).and_then(|a| a.tmux.clone());
    assert_eq!(window("django:runserver").as_deref(), Some("server"));
    assert_eq!(window("django:shell").as_deref(), Some("shell"));
    assert_eq!(window("django:migrate"), None);
}

#[test]
fn pytest_when_configured() {
    let d = TempDir::new("o-pytest");
    d.file("pyproject.toml", "[project]\nname = \"x\"");
    assert!(!ids(&d).contains(&"python:pytest".to_string()));
    d.file("pyproject.toml", "[tool.pytest.ini_options]\n");
    assert!(ids(&d).contains(&"python:pytest".to_string()), "configured in pyproject.toml");
    let only_ini = TempDir::new("o-pytest-ini");
    only_ini.file("pytest.ini", "");
    assert!(toolkit(&only_ini).contains(&("python:pytest".into(), "python3 -m pytest".into())));
}

#[test]
fn compose_gradle_cargo_go() {
    let d = TempDir::new("o-misc");
    d.file("docker-compose.yml", "").file("Cargo.toml", "").file("go.mod", "").file("gradlew", "");
    assert_eq!(ids(&d), ["gradle:assembleDebug", "gradle:installDebug", "gradle:test", "cargo:run", "cargo:build",
                         "cargo:test", "go:run", "go:test", "compose:up", "compose:down", "compose:logs", "compose:ps"]);
    assert!(toolkit(&d).contains(&("compose:logs".into(), "docker compose logs -f --tail=100".into())));
    assert!(toolkit(&d).contains(&("go:test".into(), "go test ./...".into())));
    let gradle_only = TempDir::new("o-gradle-only");
    gradle_only.file("build.gradle.kts", "");
    assert!(ids(&gradle_only).is_empty(), "no wrapper, no buttons");
}

#[test]
fn icons_in_the_old_order() {
    let p = providers();
    let d = TempDir::new("o-icons");
    assert_eq!(icon(&p, &d.0), None);
    for (file, want) in [("requirements.txt", "python"), ("go.mod", "go"), ("Cargo.toml", "rust"),
                         ("package.json", "node"), ("build.gradle", "android"), ("manage.py", "django")] {
        d.file(file, "{}");
        assert_eq!(icon(&p, &d.0).as_deref(), Some(want), "after adding {file}");
    }
}
