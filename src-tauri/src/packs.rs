//! Toolkit packs: TOML files that say when they apply to a project and which buttons they add
//! (the format is SPEC.md in thumbdeck-toolkits). They become plugins; until then they're read
//! into the same Toolkit providers as plugins.

use crate::plugins::detect::Conditions;
use crate::plugins::toolkit::{ActionDef, Generate, Provider};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/bundled_packs.rs"));

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct Pack {
    schema: u32,
    name: String,
    #[serde(default)]
    #[allow(dead_code)] // part of the format, not shown yet
    description: String,
    icon: Option<String>,
    #[serde(default = "default_priority")]
    priority: i32,
    #[serde(default)]
    requires: Vec<String>,
    detect: Option<Conditions>,
    #[serde(default)]
    vars: BTreeMap<String, String>,
    #[serde(default, rename = "action")]
    actions: Vec<ActionDef>,
    #[serde(default)]
    generate: Vec<Generate>,
}

fn default_priority() -> i32 {
    50
}

/// All packs, bundled then from disk, as Toolkit providers; plus what went wrong loading them.
pub struct Packs {
    pub packs: Vec<Provider>,
    pub problems: Vec<String>,
}

/// The packs repository: HTTPS for anyone when it's public, SSH (your GitHub key) otherwise
const REPOSITORY: &[&str] =
    &["https://github.com/Nozeren/thumbdeck-toolkits.git", "git@github.com:Nozeren/thumbdeck-toolkits.git"];

/// Where "Update toolkit packs" keeps its clone of the packs repository
fn clone_dir() -> Option<PathBuf> {
    Some(crate::projects::dirs_home()?.join(".local/share/thumbdeck/toolkits"))
}

pub fn load() -> Packs {
    let mut sources: Vec<(String, String, String)> =
        BUNDLED.iter().map(|(id, text)| (id.to_string(), text.to_string(), "built in".to_string())).collect();
    if let Some(clone) = clone_dir() {
        // An older clone than the bundled packs would bring back old versions of them
        if commit_time(&clone).is_some_and(|t| t >= BUNDLED_TIME) {
            sources.extend(read_dir(&clone.join("packs")));
        }
    }
    if let Some(home) = crate::projects::dirs_home() {
        sources.extend(read_dir(&home.join(".config/thumbdeck/toolkits")));
    }
    from_sources(sources)
}

fn git(args: &[&str]) -> std::io::Result<std::process::Output> {
    std::process::Command::new("git")
        .args(args)
        .env_clear()
        .envs(crate::runner::shell_env().iter().cloned())
        .env("GIT_TERMINAL_PROMPT", "0") // fail instead of asking for a password
        .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes") // or for a passphrase
        .stdin(std::process::Stdio::null())
        .output()
}

/// Seconds since 1970 of a repository's current commit
fn commit_time(repo: &Path) -> Option<u64> {
    if !repo.join(".git").exists() {
        return None;
    }
    let out = git(&["-C", &repo.to_string_lossy(), "log", "-1", "--format=%ct"]).ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// Get the latest packs without rebuilding thumbdeck: clone the packs repository, or pull it.
pub fn update() -> Result<String, String> {
    update_from(REPOSITORY, &clone_dir().ok_or("no home folder")?)
}

fn update_from(urls: &[&str], dir: &Path) -> Result<String, String> {
    let path = dir.to_string_lossy();
    let head = || git(&["-C", &path, "rev-parse", "--short", "HEAD"]).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let (before, out) = if dir.join(".git").exists() {
        (head().ok(), git(&["-C", &path, "pull", "--ff-only", "-q"]))
    } else if dir.exists() && dir.read_dir().is_ok_and(|mut d| d.next().is_some()) {
        return Err(format!("{path} is in the way (not a git clone); move it and try again"));
    } else {
        if let Some(parent) = dir.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = git(&["clone", "-q", urls[0], &path]);
        for url in &urls[1..] {
            if out.as_ref().is_ok_and(|o| o.status.success()) {
                break;
            }
            out = git(&["clone", "-q", url, &path]);
        }
        (None, out)
    };
    let out = out.map_err(|e| format!("couldn't run git: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("updating packs failed: {}", err.lines().last().unwrap_or("git failed").trim()));
    }
    let after = head().map_err(|e| e.to_string())?;
    let note = if commit_time(dir).is_some_and(|t| t < BUNDLED_TIME) { " (older than the built-in ones, so not used)" } else { "" };
    Ok(match before {
        Some(b) if b == after => format!("toolkit packs are up to date ({after}){note}"),
        _ => format!("toolkit packs updated to {after}{note}"),
    })
}

/// (id, text, where it came from) for every .toml in a folder
fn read_dir(dir: &Path) -> Vec<(String, String, String)> {
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    let mut files: Vec<PathBuf> =
        entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "toml")).collect();
    files.sort();
    files
        .into_iter()
        .filter_map(|p| {
            let id = p.file_stem()?.to_string_lossy().to_string();
            let text = std::fs::read_to_string(&p).ok()?;
            Some((id, text, p.display().to_string()))
        })
        .collect()
}

/// Parse packs (a later one with the same id replaces an earlier one), then drop the ones
/// whose requirements are circular or missing.
pub fn from_sources(sources: Vec<(String, String, String)>) -> Packs {
    let mut packs: Vec<Provider> = Vec::new();
    let mut problems = Vec::new();
    for (id, text, origin) in sources {
        packs.retain(|p| p.id != id);
        match parse(&id, &text) {
            Ok(pack) => packs.push(pack),
            Err(e) => problems.push(format!("pack {id} ({origin}): {e}")),
        }
    }
    let (packs, more) = crate::plugins::toolkit::resolve(packs);
    problems.extend(more.into_iter().map(|p| format!("pack {p}")));
    Packs { packs, problems }
}

fn parse(id: &str, text: &str) -> Result<Provider, String> {
    let pack: Pack = toml::from_str(text).map_err(|e| e.message().to_string())?;
    if pack.schema != 1 {
        return Err(format!("schema {} needs a newer thumbdeck", pack.schema));
    }
    if let Some(problem) = pack.generate.iter().flat_map(|g| g.problems()).next() {
        return Err(problem);
    }
    Ok(Provider {
        id: id.to_string(),
        name: pack.name,
        priority: pack.priority,
        requires: pack.requires,
        detect: pack.detect,
        icon: pack.icon,
        vars: pack.vars,
        actions: pack.actions,
        generate: pack.generate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::toolkit::{detect, icon};
    use crate::testutil::TempDir;

    fn packs(list: &[(&str, &str)]) -> Packs {
        from_sources(list.iter().map(|(id, t)| (id.to_string(), t.to_string(), "test".to_string())).collect())
    }

    #[test]
    fn bundled_packs_all_parse() {
        let loaded = from_sources(BUNDLED.iter().map(|(i, t)| (i.to_string(), t.to_string(), "b".into())).collect());
        assert!(loaded.problems.is_empty(), "{:?}", loaded.problems);
        assert_eq!(loaded.packs.len(), BUNDLED.len());
    }

    #[test]
    fn packs_are_read_from_a_folder_by_file_name() {
        let d = TempDir::new("pack-dir");
        d.file("work.toml", "schema = 1\nname = \"Work\"").file("notes.txt", "").file("a.toml", "x");
        let found = read_dir(&d.0);
        let ids: Vec<&str> = found.iter().map(|(id, _, _)| id.as_str()).collect();
        assert_eq!(ids, ["a", "work"], "only .toml files, sorted");
        assert!(found[1].2.ends_with("work.toml"), "problems name the file");
        assert!(read_dir(&d.0.join("missing")).is_empty());
    }

    #[test]
    fn update_clones_then_pulls() {
        let d = TempDir::new("update");
        let upstream = d.0.join("upstream");
        let clone = d.0.join("share/toolkits");
        let run = |args: &[&str]| assert!(git(args).unwrap().status.success(), "git {args:?}");
        let up = upstream.to_string_lossy().to_string();
        run(&["init", "-q", &up]);
        std::fs::create_dir_all(upstream.join("packs")).unwrap();
        std::fs::write(upstream.join("packs/a.toml"), "schema = 1\nname = \"A\"").unwrap();
        let commit = |msg: &str| {
            run(&["-C", &up, "add", "-A"]);
            run(&["-C", &up, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", msg]);
        };
        commit("one");

        let first = update_from(&["/nowhere/a", &up], &clone);
        assert!(first.as_ref().is_ok_and(|m| m.contains("updated to")), "falls back to the next url: {first:?}");
        assert_eq!(read_dir(&clone.join("packs")).len(), 1);
        assert!(update_from(&[&up], &clone).is_ok_and(|m| m.contains("up to date")));

        std::fs::write(upstream.join("packs/b.toml"), "schema = 1\nname = \"B\"").unwrap();
        commit("two");
        assert!(update_from(&[&up], &clone).is_ok_and(|m| m.contains("updated to")));
        assert_eq!(read_dir(&clone.join("packs")).len(), 2, "pulled the new pack");
        assert!(commit_time(&clone).is_some());

        let in_the_way = d.0.join("other");
        std::fs::create_dir_all(&in_the_way).unwrap();
        std::fs::write(in_the_way.join("x"), "").unwrap();
        assert!(update_from(&[&up], &in_the_way).is_err_and(|e| e.contains("in the way")));
        assert!(update_from(&["/nowhere/a", "/nowhere/b"], &d.0.join("c2")).is_err());
    }

    #[test]
    fn a_later_pack_replaces_an_earlier_one_with_the_same_id() {
        let loaded = packs(&[("a", "schema = 1\nname = \"First\""), ("a", "schema = 1\nname = \"Mine\"")]);
        assert_eq!(loaded.packs.len(), 1);
        assert_eq!(loaded.packs[0].name, "Mine");
    }

    #[test]
    fn broken_packs_are_reported_not_loaded() {
        let loaded = packs(&[
            ("bad", "schema = 1\nname = "),
            ("future", "schema = 2\nname = \"F\""),
            ("typo", "schema = 1\nname = \"T\"\n[detect]\nfile = [\"x\"]"),
            ("gen", "schema = 1\nname = \"G\"\n[[generate]]\nsource = { json = \"p.json\" }\naction = { name = \"{item}\", command = \"x\" }"),
            ("ok", "schema = 1\nname = \"OK\""),
        ]);
        assert_eq!(loaded.packs.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["ok"]);
        assert_eq!(loaded.problems.len(), 4, "{:?}", loaded.problems);
        assert!(loaded.problems[1].contains("newer thumbdeck"));
    }

    /// The bundled packs give what thumbdeck detected before it had packs
    mod bundled {
        use super::*;

        fn toolkit(d: &TempDir) -> Vec<(String, String)> {
            let p = from_sources(
                BUNDLED.iter().map(|(i, t)| (i.to_string(), t.to_string(), "b".into())).collect(),
            );
            detect(&p.packs, &d.0).actions.into_iter().map(|a| (a.id, a.command)).collect()
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
            let p = from_sources(
                BUNDLED.iter().map(|(i, t)| (i.to_string(), t.to_string(), "b".into())).collect(),
            );
            let t = detect(&p.packs, &d.0);
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
            let p = from_sources(
                BUNDLED.iter().map(|(i, t)| (i.to_string(), t.to_string(), "b".into())).collect(),
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
