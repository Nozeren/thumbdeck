//! The subagents a project defines (.claude/agents/*.md) and yours (~/.claude/agents/*.md):
//! Markdown files whose frontmatter says what the agent is; the body is its instructions.

use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Defined {
    pub name: String,
    pub description: String,
    /// Empty: all tools
    pub tools: Vec<String>,
    pub model: Option<String>,
    pub color: Option<String>,
    /// "project" or "user"
    pub scope: String,
    pub file: String,
    /// Its instructions (the Markdown after the frontmatter)
    pub instructions: String,
}

/// The frontmatter's simple "key: value" lines (and "- item" lists), and the body after it.
/// Nested values (hooks, mcpServers) aren't needed here and are skipped.
pub fn frontmatter(text: &str) -> Option<(Vec<(String, String)>, String)> {
    let rest = text.strip_prefix("---")?.trim_start_matches(['\r', ' ']).strip_prefix('\n')?;
    let end = rest.find("\n---")?;
    let (head, body) = (&rest[..end], &rest[end + 4..]);
    let body = body.split_once('\n').map(|(_, b)| b).unwrap_or("").trim().to_string();
    let mut fields: Vec<(String, String)> = Vec::new();
    for line in head.lines() {
        if let Some(item) = line.trim_start().strip_prefix("- ") {
            // An item of the list started by the last key ("tools:\n  - Read")
            if line.starts_with([' ', '\t']) {
                if let Some((_, v)) = fields.last_mut() {
                    if !v.is_empty() {
                        v.push_str(", ");
                    }
                    v.push_str(item.trim());
                }
            }
            continue;
        }
        if line.starts_with([' ', '\t']) || line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let v = v.trim().trim_matches(['"', '\'']).to_string();
            fields.push((k.trim().to_string(), v));
        }
    }
    Some((fields, body))
}

fn parse(text: &str, scope: &str, file: &Path) -> Option<Defined> {
    let (fields, instructions) = frontmatter(text)?;
    let get = |k: &str| fields.iter().rev().find(|(key, _)| key == k).map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
    let name = get("name")?;
    Some(Defined {
        description: get("description").unwrap_or_default(),
        tools: get("tools")
            .map(|t| t.trim_matches(['[', ']']).split(',').map(|s| s.trim().trim_matches(['"', '\'']).to_string()).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default(),
        model: get("model"),
        color: get("color"),
        scope: scope.to_string(),
        file: file.to_string_lossy().to_string(),
        instructions,
        name,
    })
}

/// The agents in a folder of definitions (files that aren't one are skipped)
pub fn list(dir: &Path, scope: &str) -> Vec<Defined> {
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<Defined> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .filter_map(|p| parse(&std::fs::read_to_string(&p).ok()?, scope, &p))
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::agents::tests::testdata;

    #[test]
    fn the_test_projects_counter_agent() {
        let agents = list(&testdata().join("project/.claude/agents"), "project");
        assert_eq!(agents.len(), 1);
        let a = &agents[0];
        assert_eq!((a.name.as_str(), a.scope.as_str()), ("counter", "project"));
        assert!(a.description.starts_with("Counts the files"));
        assert_eq!(a.tools, ["Glob"]);
        assert_eq!((a.model.as_deref(), a.color.as_deref()), (Some("haiku"), Some("green")));
        assert!(a.instructions.starts_with("Count the files"));
    }

    #[test]
    fn frontmatter_forms() {
        let p = Path::new("/a.md");
        let list_form = "---\nname: \"reviewer\"\ntools:\n  - Read\n  - Grep\nhooks:\n  PreToolUse:\n    - matcher: Bash\n---\nReview.";
        let a = parse(list_form, "user", p).unwrap();
        assert_eq!(a.name, "reviewer");
        assert_eq!(a.tools, ["Read", "Grep"], "the nested hooks list isn't taken for tools");
        assert_eq!(a.instructions, "Review.");
        assert_eq!(parse("---\nname: x\ntools: [Read, 'Edit']\n---\n", "user", p).unwrap().tools, ["Read", "Edit"]);
        assert!(parse("---\nname: x\n---\n", "user", p).unwrap().tools.is_empty(), "no tools line: all tools");
        assert_eq!(parse("no frontmatter", "user", p), None);
        assert_eq!(parse("---\ndescription: no name\n---\n", "user", p), None);
        assert!(list(Path::new("/missing"), "user").is_empty());
    }
}
