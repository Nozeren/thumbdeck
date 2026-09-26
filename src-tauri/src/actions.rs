//! Detecting a project's toolkit from the files it contains.

use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone)]
pub struct Action {
    /// Stable within a project, e.g. "npm:dev"
    pub id: String,
    pub label: String,
    pub command: String,
    /// Where the action came from: custom (added by you), npm, make, django, pytest, compose,
    /// gradle, cargo, go
    pub source: String,
    /// Ask before running (custom actions only)
    #[serde(default)]
    pub confirm: bool,
}

fn action(source: &str, label: &str, command: impl Into<String>) -> Action {
    Action {
        id: format!("{source}:{label}"),
        label: label.to_string(),
        command: command.into(),
        source: source.to_string(),
        confirm: false,
    }
}

pub fn detect(dir: &Path) -> Vec<Action> {
    let mut actions = Vec::new();
    npm(dir, &mut actions);
    make(dir, &mut actions);
    django(dir, &mut actions);
    pytest(dir, &mut actions);
    compose(dir, &mut actions);
    gradle(dir, &mut actions);
    if dir.join("Cargo.toml").exists() {
        for (label, cmd) in [("run", "cargo run"), ("build", "cargo build"), ("test", "cargo test")] {
            actions.push(action("cargo", label, cmd));
        }
    }
    if dir.join("go.mod").exists() {
        for (label, cmd) in [("run", "go run ."), ("test", "go test ./...")] {
            actions.push(action("go", label, cmd));
        }
    }
    actions
}

/// One action per script in package.json, run with the project's package manager.
fn npm(dir: &Path, out: &mut Vec<Action>) {
    let Ok(text) = std::fs::read_to_string(dir.join("package.json")) else { return };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { return };
    let Some(scripts) = json.get("scripts").and_then(|s| s.as_object()) else { return };
    let manager = if dir.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if dir.join("yarn.lock").exists() {
        "yarn"
    } else if dir.join("bun.lockb").exists() || dir.join("bun.lock").exists() {
        "bun"
    } else {
        "npm"
    };
    for name in scripts.keys() {
        // Lifecycle hooks aren't things you run by hand
        if ["prepare", "postinstall", "preinstall", "install"].contains(&name.as_str()) {
            continue;
        }
        out.push(action("npm", name, format!("{manager} run {name}")));
    }
}

/// One action per target in the Makefile.
fn make(dir: &Path, out: &mut Vec<Action>) {
    let Ok(text) = std::fs::read_to_string(dir.join("Makefile")) else { return };
    for line in text.lines() {
        let Some((target, rest)) = line.split_once(':') else { continue };
        let simple = !target.is_empty()
            && target.chars().all(|c| c.is_ascii_alphanumeric() || "_-./".contains(c))
            && !target.starts_with('.')
            && !rest.starts_with('='); // "VAR := value" is an assignment, not a target
        if simple && !out.iter().any(|a| a.source == "make" && a.label == target) {
            out.push(action("make", target, format!("make {target}")));
        }
    }
}

/// Python in the project's own .venv when there is one.
fn python(dir: &Path) -> String {
    for venv in [".venv", "venv"] {
        let bin = dir.join(venv).join("bin").join("python");
        if bin.exists() {
            return format!("{venv}/bin/python");
        }
    }
    "python3".to_string()
}

fn django(dir: &Path, out: &mut Vec<Action>) {
    if !dir.join("manage.py").exists() {
        return;
    }
    let py = python(dir);
    for cmd in ["runserver", "migrate", "makemigrations", "test", "shell"] {
        out.push(action("django", cmd, format!("{py} manage.py {cmd}")));
    }
}

fn pytest(dir: &Path, out: &mut Vec<Action>) {
    let configured = dir.join("pytest.ini").exists()
        || dir.join("conftest.py").exists()
        || std::fs::read_to_string(dir.join("pyproject.toml"))
            .map(|t| t.contains("[tool.pytest"))
            .unwrap_or(false);
    if configured {
        out.push(action("pytest", "pytest", format!("{} -m pytest", python(dir))));
    }
}

fn compose(dir: &Path, out: &mut Vec<Action>) {
    let file = ["compose.yaml", "compose.yml", "docker-compose.yml", "docker-compose.yaml"]
        .into_iter()
        .find(|f| dir.join(f).exists());
    if file.is_none() {
        return;
    }
    for (label, cmd) in [
        ("up", "docker compose up -d"),
        ("down", "docker compose down"),
        ("logs", "docker compose logs -f --tail=100"),
        ("ps", "docker compose ps"),
    ] {
        out.push(action("compose", label, cmd));
    }
}

fn gradle(dir: &Path, out: &mut Vec<Action>) {
    if !dir.join("gradlew").exists() {
        return;
    }
    for task in ["assembleDebug", "installDebug", "test"] {
        out.push(action("gradle", task, format!("./gradlew {task}")));
    }
}
