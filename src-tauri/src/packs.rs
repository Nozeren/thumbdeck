//! Toolkit packs: TOML files that say when they apply to a project and which buttons they add
//! (the format is SPEC.md in thumbdeck-toolkits).

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/bundled_packs.rs"));

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Pack {
    /// From the file name: django.toml -> "django"
    #[serde(skip)]
    pub id: String,
    pub schema: u32,
    pub name: String,
    #[serde(default)]
    #[allow(dead_code)] // part of the format, not shown yet
    pub description: String,
    pub icon: Option<String>,
    #[serde(default = "default_priority")]
    pub priority: i32,
    #[serde(default)]
    pub requires: Vec<String>,
    pub detect: Option<Conditions>,
    #[serde(default)]
    pub vars: BTreeMap<String, String>,
    #[serde(default, rename = "action")]
    pub actions: Vec<PackAction>,
    #[serde(default)]
    pub generate: Vec<Generate>,
}

fn default_priority() -> i32 {
    50
}

/// A [detect] table (also an action's `when`): every key written must match.
#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct Conditions {
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    all_files: Vec<String>,
    #[serde(default)]
    not_files: Vec<String>,
    #[serde(default)]
    json: Vec<JsonCondition>,
    #[serde(default)]
    contains: Vec<TextCondition>,
    #[serde(default)]
    any: Vec<Conditions>,
    git_remote: Option<String>,
    path: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct JsonCondition {
    file: String,
    key: String,
    value: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct TextCondition {
    file: String,
    text: String,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct PackAction {
    pub name: String,
    pub command: String,
    pub description: Option<String>,
    #[serde(default)]
    pub confirm: bool,
    pub when: Option<Conditions>,
    pub tmux: Option<Tmux>,
}

/// `tmux = true` (window named after the action) or `tmux = { window = "name" }`
#[derive(Deserialize, Debug)]
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

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Generate {
    pub source: Source,
    #[serde(default)]
    pub skip: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)] // for command sources, which aren't supported yet
    pub watch: Vec<String>,
    pub action: PackAction,
}

/// Where a [[generate]] gets its items: exactly one of json / file / command.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub json: Option<String>,
    pub keys: Option<String>,
    pub values: Option<String>,
    pub file: Option<String>,
    pub regex: Option<String>,
    pub command: Option<String>,
    #[allow(dead_code)] // for command sources, which aren't supported yet
    pub split: Option<String>,
}

/// All packs, bundled then from disk; plus what went wrong loading them (for the Toolkit).
pub struct Packs {
    pub packs: Vec<Pack>,
    pub problems: Vec<String>,
}

pub fn load() -> Packs {
    let mut sources: Vec<(String, String, String)> =
        BUNDLED.iter().map(|(id, text)| (id.to_string(), text.to_string(), "built in".to_string())).collect();
    if let Some(home) = crate::projects::dirs_home() {
        for dir in [home.join(".local/share/thumbdeck/toolkits/packs"), home.join(".config/thumbdeck/toolkits")] {
            sources.extend(read_dir(&dir));
        }
    }
    from_sources(sources)
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
    let mut packs: Vec<Pack> = Vec::new();
    let mut problems = Vec::new();
    for (id, text, origin) in sources {
        packs.retain(|p| p.id != id);
        match parse(&id, &text) {
            Ok(pack) => packs.push(pack),
            Err(e) => problems.push(format!("pack {id} ({origin}): {e}")),
        }
    }

    // Circular requirements: every pack on a cycle is dropped
    let cyclic: Vec<String> =
        packs.iter().filter(|p| reaches(&packs, &p.requires, &p.id, &mut vec![])).map(|p| p.id.clone()).collect();
    if !cyclic.is_empty() {
        problems.push(format!("packs {} require each other in a circle; they're off", cyclic.join(", ")));
        packs.retain(|p| !cyclic.contains(&p.id));
    }

    // Missing requirements, followed until nothing changes (a needs b, b needs missing c)
    loop {
        let ids: Vec<String> = packs.iter().map(|p| p.id.clone()).collect();
        let Some(i) = packs.iter().position(|p| p.requires.iter().any(|r| !ids.contains(r))) else { break };
        let pack = packs.remove(i);
        let missing: Vec<&str> = pack.requires.iter().filter(|r| !ids.contains(r)).map(|r| r.as_str()).collect();
        problems.push(format!("pack {} is off: it requires {}, which isn't installed or is off", pack.id, missing.join(", ")));
    }
    Packs { packs, problems }
}

/// Whether `target` can be reached by following requirements from `from`
fn reaches(packs: &[Pack], from: &[String], target: &str, seen: &mut Vec<String>) -> bool {
    from.iter().any(|id| {
        if id == target {
            return true;
        }
        if seen.contains(id) {
            return false;
        }
        seen.push(id.clone());
        packs.iter().find(|p| &p.id == id).is_some_and(|p| reaches(packs, &p.requires, target, seen))
    })
}

fn parse(id: &str, text: &str) -> Result<Pack, String> {
    let mut pack: Pack = toml::from_str(text).map_err(|e| e.message().to_string())?;
    if pack.schema != 1 {
        return Err(format!("schema {} needs a newer thumbdeck", pack.schema));
    }
    pack.id = id.to_string();
    for g in &pack.generate {
        let s = &g.source;
        let kinds = [s.json.is_some(), s.file.is_some(), s.command.is_some()].iter().filter(|k| **k).count();
        if kinds != 1 {
            return Err("a [[generate]] source needs exactly one of json, file or command".into());
        }
        if s.json.is_some() && s.keys.is_some() == s.values.is_some() {
            return Err("a json source needs either keys or values".into());
        }
        if s.file.is_some() {
            let re = s.regex.as_deref().ok_or("a file source needs a regex")?;
            regex::Regex::new(re).map_err(|e| format!("bad regex: {e}"))?;
        }
    }
    Ok(pack)
}

impl Conditions {
    pub fn matches(&self, dir: &Path) -> bool {
        let exists = |f: &String| exists(dir, f);
        (self.files.is_empty() || self.files.iter().any(exists))
            && self.all_files.iter().all(exists)
            && !self.not_files.iter().any(exists)
            && self.json.iter().all(|c| json_matches(dir, c))
            && self.contains.iter().all(|c| {
                std::fs::read_to_string(dir.join(&c.file)).is_ok_and(|t| t.contains(&c.text))
            })
            && (self.any.is_empty() || self.any.iter().any(|c| c.matches(dir)))
            && self.path.as_ref().is_none_or(|p| path_matches(dir, p))
            && self.git_remote.as_ref().is_none_or(|re| remote_matches(dir, re))
    }
}

/// A file or folder in the project, by name or glob
fn exists(dir: &Path, pattern: &str) -> bool {
    if !pattern.contains(['*', '?', '[']) {
        return dir.join(pattern).exists();
    }
    let full = format!("{}/{pattern}", glob::Pattern::escape(&dir.to_string_lossy()));
    glob::glob(&full).is_ok_and(|mut found| found.any(|p| p.is_ok()))
}

/// A value in a JSON file, by dotted path ("dependencies.react")
pub fn json_at(dir: &Path, file: &str, key: &str) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(dir.join(file)).ok()?;
    let mut value: serde_json::Value = serde_json::from_str(&text).ok()?;
    for part in key.split('.').filter(|p| !p.is_empty()) {
        value = value.get_mut(part)?.take();
    }
    Some(value)
}

fn json_matches(dir: &Path, c: &JsonCondition) -> bool {
    match (json_at(dir, &c.file, &c.key), &c.value) {
        (None, _) => false,
        (Some(_), None) => true,
        (Some(serde_json::Value::String(s)), Some(want)) => &s == want,
        (Some(v), Some(want)) => &v.to_string() == want,
    }
}

fn path_matches(dir: &Path, pattern: &str) -> bool {
    let pattern = match (pattern.strip_prefix("~/"), crate::projects::dirs_home()) {
        (Some(rest), Some(home)) => format!("{}/{rest}", home.display()),
        _ => pattern.to_string(),
    };
    glob::Pattern::new(&pattern).is_ok_and(|p| p.matches_path(dir))
}

fn remote_matches(dir: &Path, re: &str) -> bool {
    let Ok(re) = regex::Regex::new(re) else { return false };
    let out = std::process::Command::new("git").arg("-C").arg(dir).args(["config", "--get", "remote.origin.url"]).output();
    out.is_ok_and(|o| o.status.success() && re.is_match(String::from_utf8_lossy(&o.stdout).trim()))
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// An empty temporary project folder, removed when dropped
    pub struct TempDir(pub PathBuf);
    impl TempDir {
        pub fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("thumbdeck-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }
        pub fn file(&self, name: &str, text: &str) -> &Self {
            let path = self.0.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
            self
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    pub fn packs(list: &[(&str, &str)]) -> Packs {
        from_sources(list.iter().map(|(id, t)| (id.to_string(), t.to_string(), "test".to_string())).collect())
    }

    fn conditions(toml_text: &str) -> Conditions {
        toml::from_str(toml_text).unwrap()
    }

    #[test]
    fn bundled_packs_all_parse() {
        let loaded = from_sources(BUNDLED.iter().map(|(i, t)| (i.to_string(), t.to_string(), "b".into())).collect());
        assert!(loaded.problems.is_empty(), "{:?}", loaded.problems);
        assert_eq!(loaded.packs.len(), BUNDLED.len());
    }

    #[test]
    fn detect_keys() {
        let d = TempDir::new("detect");
        d.file("manage.py", "").file("package.json", r#"{"dependencies": {"react": "18"}}"#);
        d.file("pyproject.toml", "[tool.pytest.ini_options]").file("src/app.py", "");
        assert!(conditions(r#"files = ["nope", "manage.py"]"#).matches(&d.0));
        assert!(!conditions(r#"all_files = ["nope", "manage.py"]"#).matches(&d.0));
        assert!(!conditions(r#"not_files = ["manage.py"]"#).matches(&d.0));
        assert!(conditions(r#"files = ["src/*.py"]"#).matches(&d.0), "globs");
        assert!(conditions(r#"files = ["**/app.py"]"#).matches(&d.0), "** globs");
        assert!(!conditions(r#"files = ["*.rs"]"#).matches(&d.0));
        assert!(conditions(r#"json = [{ file = "package.json", key = "dependencies.react" }]"#).matches(&d.0));
        assert!(conditions(r#"json = [{ file = "package.json", key = "dependencies.react", value = "18" }]"#).matches(&d.0));
        assert!(!conditions(r#"json = [{ file = "package.json", key = "dependencies.vue" }]"#).matches(&d.0));
        assert!(conditions(r#"contains = [{ file = "pyproject.toml", text = "[tool.pytest" }]"#).matches(&d.0));
        assert!(!conditions(r#"contains = [{ file = "setup.cfg", text = "[tool.pytest" }]"#).matches(&d.0));
        assert!(conditions(r#"any = [{ files = ["nope"] }, { files = ["manage.py"] }]"#).matches(&d.0));
        assert!(!conditions(r#"any = [{ files = ["nope"] }]"#).matches(&d.0));
        assert!(!conditions(r#"files = ["manage.py"]
            not_files = ["package.json"]"#).matches(&d.0), "every key must match");
        assert!(conditions("").matches(&d.0), "no conditions: always");
        let parent = d.0.parent().unwrap().display();
        assert!(conditions(&format!(r#"path = "{parent}/**""#)).matches(&d.0));
        assert!(!conditions(r#"path = "/elsewhere/**""#).matches(&d.0));
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

    #[test]
    fn missing_and_circular_requirements_turn_packs_off() {
        let loaded = packs(&[
            ("a", "schema = 1\nname = \"A\"\nrequires = [\"b\"]"),
            ("b", "schema = 1\nname = \"B\"\nrequires = [\"gone\"]"),
            ("x", "schema = 1\nname = \"X\"\nrequires = [\"y\"]"),
            ("y", "schema = 1\nname = \"Y\"\nrequires = [\"x\"]"),
            ("z", "schema = 1\nname = \"Z\"\nrequires = [\"x\"]"),
            ("ok", "schema = 1\nname = \"OK\""),
        ]);
        let ids: Vec<&str> = loaded.packs.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["ok"], "a and b (missing), x and y (circle), z (needs x)");
        assert!(loaded.problems.iter().any(|p| p.contains("b is off") && p.contains("gone")));
        assert!(loaded.problems.iter().any(|p| p.contains("a is off")));
        assert!(loaded.problems.iter().any(|p| p.contains("circle")));
    }
}
