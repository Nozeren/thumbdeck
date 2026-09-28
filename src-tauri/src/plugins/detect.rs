//! `[detect]` tables (also an action's `when`): whether a plugin applies to a project.

use serde::Deserialize;
use std::path::Path;

/// Every key written must match.
#[derive(Deserialize, Debug, Default, Clone)]
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

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
struct JsonCondition {
    file: String,
    key: String,
    value: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
struct TextCondition {
    file: String,
    text: String,
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

    /// What's wrong with it (a bad regular expression), in plain sentences
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(re) = &self.git_remote {
            if let Err(e) = regex::Regex::new(re) {
                out.push(format!("git_remote isn't a valid regular expression: {e}"));
            }
        }
        for c in &self.any {
            out.extend(c.problems());
        }
        out
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
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn conditions(toml_text: &str) -> Conditions {
        toml::from_str(toml_text).unwrap()
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
    fn a_bad_remote_pattern_is_a_problem() {
        assert!(conditions(r#"git_remote = "(""#).problems()[0].contains("regular expression"));
        assert!(conditions(r#"any = [{ git_remote = "[" }]"#).problems().len() == 1);
        assert!(conditions(r#"git_remote = "github.com[:/]x/""#).problems().is_empty());
    }
}
