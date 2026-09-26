//! Extensions: tabs next to the README that you turn on per project, each with its own
//! setup (saved in thumbdeck's settings). They're built into the app: an extension is a module
//! here (its setup and commands) and a folder in src/lib/extensions (its tab and setup form),
//! listed in AVAILABLE and in src/lib/extensions/index.ts.

pub mod logs;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize)]
pub struct Extension {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Its setup with every field filled in (from Null: the default setup); None if it isn't
    /// one of this extension's setups
    #[serde(skip)]
    setup: fn(&Value) -> Option<Value>,
}

/// Every extension this thumbdeck has
pub const AVAILABLE: &[Extension] = &[Extension {
    id: "logs",
    name: "Logs",
    description: "The project's log files: levels, search, errors, live tail, optional sections",
    setup: complete_setup::<logs::Setup>,
}];

/// An extension's setup type turns saved JSON into a complete setup (its serde defaults fill
/// in what's missing)
fn complete_setup<S: Serialize + DeserializeOwned + Default>(saved: &Value) -> Option<Value> {
    let setup: S = if saved.is_null() { S::default() } else { serde_json::from_value(saved.clone()).ok()? };
    serde_json::to_value(setup).ok()
}

fn find(id: &str) -> Option<&'static Extension> {
    AVAILABLE.iter().find(|e| e.id == id)
}

/// An extension turned on for a project
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Tab {
    pub extension: String,
    /// The extension's own setup
    pub setup: Value,
}

impl Tab {
    /// A new tab with the extension's default setup
    pub fn new(extension: &str) -> Option<Tab> {
        let setup = (find(extension)?.setup)(&Value::Null)?;
        Some(Tab { extension: extension.into(), setup })
    }

    /// The setup with every field filled in (a setup saved by an older thumbdeck may lack new
    /// ones; the tab relies on all of them)
    pub fn complete(&self) -> Tab {
        let setup = find(&self.extension).and_then(|e| (e.setup)(&self.setup));
        Tab { extension: self.extension.clone(), setup: setup.unwrap_or_else(|| self.setup.clone()) }
    }

    /// The tab's title, from its setup
    pub fn title(&self) -> String {
        self.setup.get("title").and_then(|t| t.as_str()).filter(|t| !t.is_empty()).map(String::from).unwrap_or_else(|| {
            find(&self.extension).map(|e| e.name.to_string()).unwrap_or_else(|| self.extension.clone())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_available_extension_can_be_turned_on() {
        for e in AVAILABLE {
            let tab = Tab::new(e.id).unwrap();
            assert_eq!(tab.title(), "Logs");
        }
        assert!(Tab::new("nope").is_none());
    }

    #[test]
    fn an_unknown_extension_keeps_its_setup_as_saved() {
        let tab = Tab { extension: "gone".into(), setup: serde_json::json!({ "x": 1 }) };
        assert_eq!(tab.complete(), tab, "e.g. a tab saved by a newer thumbdeck");
        assert_eq!(tab.title(), "gone");
    }

    #[test]
    fn a_partial_setup_is_completed_with_defaults() {
        let tab = Tab { extension: "logs".into(), setup: serde_json::json!({ "folders": ["out"] }) };
        let full = tab.complete();
        assert_eq!(full.setup["folders"], serde_json::json!(["out"]));
        assert_eq!(full.setup["pattern"], "*.log");
        assert!(full.setup["fields"]["message"].is_array());
    }
}
