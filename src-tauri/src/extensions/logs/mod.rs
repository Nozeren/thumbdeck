//! Logs: a log viewer tab for any project.
//! Log files (JSON lines or plain text) with levels, search and live tail, optionally folded
//! into sections that start and end at marker texts.

pub mod files;
pub mod outline;
pub mod parser;

use serde::{Deserialize, Serialize};

/// A project's setup for the tab (saved in thumbdeck's settings).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Setup {
    /// Tab title
    pub title: String,
    /// Folders with logs, relative to the project (or absolute)
    pub folders: Vec<String>,
    /// Which files in them are logs (glob)
    pub pattern: String,
    /// Optional sections: what marks their start and end
    pub blocks: Vec<BlockConfig>,
    /// Names of the fields in JSON lines, first one found wins
    pub fields: Fields,
    /// Plain-text lines: a regex with the named groups time, level and message. Empty: read
    /// them automatically (see parser::plain_lines).
    pub line_pattern: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Fields {
    pub message: Vec<String>,
    pub level: Vec<String>,
    pub time: Vec<String>,
}

/// One kind of section. The start and end texts are plain text found anywhere in a line's
/// message (any of them); a section ends at the first line after its start with an end text,
/// or runs to the end of the file.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BlockConfig {
    pub name: String,
    /// bookmark: a top-level section (◆); index: a section inside one (▸)
    pub kind: Kind,
    pub start: Vec<String>,
    #[serde(default)]
    pub end: Vec<String>,
    /// How the section's title is made from its first line (default: the line itself)
    #[serde(default)]
    pub alias: Option<Alias>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Bookmark,
    Index,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "lowercase")]
pub enum Alias {
    /// The first group of this regex
    Regex(String),
    /// Always this title
    Rewrite(String),
    /// The line with one text replaced by another
    Replace { from: String, to: String },
    /// The line with this before it
    Prefix(String),
}

impl Default for Fields {
    fn default() -> Self {
        let v = |list: &[&str]| list.iter().map(|s| s.to_string()).collect();
        Fields {
            message: v(&["message", "msg", "event", "text", "log"]),
            level: v(&["level", "levelname", "severity", "lvl", "loglevel"]),
            time: v(&["timestamp", "time", "ts", "@timestamp", "asctime", "datetime", "date"]),
        }
    }
}

impl Default for Setup {
    fn default() -> Self {
        Setup {
            title: "Logs".into(),
            folders: vec![".".into(), "logs".into(), "log".into()],
            pattern: "*.log".into(),
            blocks: vec![],
            fields: Fields::default(),
            line_pattern: String::new(),
        }
    }
}

impl Setup {
    /// What's wrong with it, if anything (shown in the setup form)
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.folders.iter().all(|f| f.trim().is_empty()) {
            out.push("add at least one folder".into());
        }
        if glob::Pattern::new(&self.pattern).is_err() {
            out.push(format!("file pattern '{}' isn't a valid glob", self.pattern));
        }
        if !self.line_pattern.is_empty() {
            match regex::Regex::new(&self.line_pattern) {
                Err(e) => out.push(format!("line pattern: {e}")),
                Ok(re) if !re.capture_names().flatten().any(|n| n == "message") => {
                    out.push("the line pattern needs a (?P<message>...) group".into())
                }
                Ok(_) => {}
            }
        }
        for b in &self.blocks {
            if b.start.iter().all(|s| s.is_empty()) {
                out.push(format!("section '{}' needs a start text", b.name));
            }
            if let Some(Alias::Regex(re)) = &b.alias {
                match regex::Regex::new(re) {
                    Err(e) => out.push(format!("section '{}': {e}", b.name)),
                    Ok(r) if r.captures_len() < 2 => out.push(format!("section '{}': the title regex needs a (group)", b.name)),
                    Ok(_) => {}
                }
            }
        }
        out
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Sections for the sample test run (testdata/sample_run.log): its
    /// DEFAULT_CONFIG, which checks that the section rules work as intended
    pub fn sections() -> Vec<BlockConfig> {
        let block = |name: &str, kind, start: &[&str], end: &[&str], alias| BlockConfig {
            name: name.into(),
            kind,
            start: start.iter().map(|s| s.to_string()).collect(),
            end: end.iter().map(|s| s.to_string()).collect(),
            alias: Some(alias),
        };
        vec![
            block("scenario", Kind::Bookmark, &["# Scenario: "], &["# Scenario:", "short test summary info"],
                  Alias::Regex("################ (.*?) ################".into())),
            block("environment_setup", Kind::Index, &["configuration file."], &["# Scenario: "],
                  Alias::Rewrite("Environment setup".into())),
            block("scenario_setup", Kind::Index, &["# Scenario: "], &[">>>>>>>>>>>>>>>>"], Alias::Rewrite("Setup".into())),
            block("step", Kind::Index, &[">>>>>>>>>>>>>>>>"], &[">>>>>>>>>>>>>>>>", "# Scenario completed: "],
                  Alias::Regex(">>>>>>>>>>>>>>>>(.*?)<<<<<<<<<<<<<<<<".into())),
            block("teardown", Kind::Index, &["# Scenario completed: "], &["# Scenario:", "short test summary info"],
                  Alias::Rewrite("Teardown".into())),
        ]
    }

    #[test]
    fn the_default_setup_is_valid_and_so_are_the_test_sections() {
        assert!(Setup::default().problems().is_empty());
        assert!(Setup { blocks: sections(), ..Setup::default() }.problems().is_empty());
    }

    #[test]
    fn problems_are_found() {
        let s = Setup {
            folders: vec![" ".into()],
            pattern: "[".into(),
            line_pattern: "(?P<time>x)".into(),
            blocks: vec![BlockConfig {
                name: "b".into(),
                kind: Kind::Index,
                start: vec![],
                end: vec![],
                alias: Some(Alias::Regex("no group".into())),
            }],
            ..Setup::default()
        };
        assert_eq!(s.problems().len(), 5, "{:?}", s.problems());
    }

    #[test]
    fn setup_round_trips_through_json_and_fills_in_missing_fields() {
        let s = Setup { blocks: sections(), ..Setup::default() };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Setup>(&json).unwrap(), s);
        let partial: Setup = serde_json::from_str(r#"{"folders": ["out"]}"#).unwrap();
        assert_eq!(partial.folders, ["out"]);
        assert_eq!(partial.title, "Logs");
    }
}
