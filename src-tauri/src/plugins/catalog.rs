//! The catalog: the official plugins, listed in catalog.toml in the plugins repository. The first
//! start offers them, and Settings › Plugins › Add a plugin lists them.

use serde::{Deserialize, Serialize};

/// Where the catalog is; $THUMBDECK_CATALOG (a URL or a file) takes its place
const URL: &str = "https://raw.githubusercontent.com/Nozeren/thumbdeck-plugins/main/catalog.toml";

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub description: String,
    /// What Settings › Plugins › Add takes: a git URL, maybe with #folder
    pub source: String,
    /// Ticked in the first start's list
    #[serde(default)]
    pub recommended: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    #[serde(default)]
    plugin: Vec<Entry>,
}

pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    toml::from_str::<Catalog>(text).map(|c| c.plugin).map_err(|e| format!("the catalog can't be read: {}", e.message()))
}

pub fn load() -> Result<Vec<Entry>, String> {
    let from = std::env::var("THUMBDECK_CATALOG").ok().filter(|v| !v.is_empty()).unwrap_or_else(|| URL.to_string());
    let text = if from.starts_with("https://") || from.starts_with("http://") {
        let out = std::process::Command::new("curl")
            .args(["-fsSL", "--max-time", "20", &from])
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|e| format!("couldn't run curl: {e}"))?;
        if !out.status.success() {
            return Err("couldn't get the plugin catalog (are you online?)".into());
        }
        String::from_utf8_lossy(&out.stdout).into_owned()
    } else {
        std::fs::read_to_string(&from).map_err(|e| format!("couldn't read the catalog {from}: {e}"))?
    };
    parse(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_official_catalog_reads() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/toolkits/catalog.toml")).unwrap();
        let list = parse(&text).unwrap();
        assert!(list.iter().any(|e| e.id == "git" && e.recommended));
        for e in &list {
            let folder = e.source.split_once('#').map(|(_, f)| f).unwrap_or("");
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("toolkits").join(folder);
            let m = crate::plugins::manifest::read(&path).unwrap_or_else(|p| panic!("{}: {p:?}", e.id));
            assert_eq!((m.id.as_str(), m.name.as_str()), (e.id.as_str(), e.name.as_str()), "the catalog matches the plugin");
        }
        assert!(parse("[[plugin]]\nid = \"x\"\n").unwrap_err().contains("can't be read"));
    }
}
