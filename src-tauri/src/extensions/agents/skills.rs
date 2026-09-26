//! The skills a project has (.claude/skills/<name>/SKILL.md) and yours (~/.claude/skills):
//! a folder per skill, its SKILL.md frontmatter says what it is. Skills synced from claude.ai
//! live deeper (skills/synced/...) and aren't listed.

use super::defined::frontmatter;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Skill {
    pub name: String,
    pub description: String,
    /// "project" or "user"
    pub scope: String,
    pub file: String,
    /// Its instructions (the Markdown after the frontmatter)
    pub instructions: String,
}

fn parse(text: &str, folder: &str, scope: &str, file: &Path) -> Option<Skill> {
    let (fields, instructions) = frontmatter(text)?;
    let get = |k: &str| fields.iter().rev().find(|(key, _)| key == k).map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
    Some(Skill {
        // The folder's name when the frontmatter has none
        name: get("name").unwrap_or_else(|| folder.to_string()),
        description: get("description").unwrap_or_default(),
        scope: scope.to_string(),
        file: file.to_string_lossy().to_string(),
        instructions,
    })
}

/// The skills in a skills folder (folders without a readable SKILL.md are skipped)
pub fn list(dir: &Path, scope: &str) -> Vec<Skill> {
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<Skill> = entries
        .flatten()
        .filter_map(|e| {
            let file = e.path().join("SKILL.md");
            parse(&std::fs::read_to_string(&file).ok()?, &e.file_name().to_string_lossy(), scope, &file)
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::agents::tests::testdata;

    #[test]
    fn the_test_projects_tidy_skill() {
        let skills = list(&testdata().join("project/.claude/skills"), "project");
        assert_eq!(skills.len(), 1);
        assert_eq!((skills[0].name.as_str(), skills[0].scope.as_str()), ("tidy", "project"));
        assert!(skills[0].description.starts_with("Tidies up"));
        assert!(skills[0].instructions.starts_with("Sort the imports"));
        assert!(list(Path::new("/missing"), "user").is_empty());
    }

    #[test]
    fn a_skill_without_a_name_is_named_after_its_folder() {
        let s = parse("---\ndescription: d\n---\nbody", "fix-it", "user", Path::new("/s/SKILL.md")).unwrap();
        assert_eq!(s.name, "fix-it");
    }
}
