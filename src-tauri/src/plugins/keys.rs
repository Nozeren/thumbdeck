//! A plugin's keys: declared in its manifest, checked against the rules every place follows
//! (the same as RULES in src/lib/keys/keys.ts; a test keeps them equal).

use serde::{Deserialize, Serialize};

/// Rule 1 and the shared Space letters (plugin API 2): key -> the action it must run wherever
/// it's bound. A vim key means what it means in vim; the rest is behind Space.
pub const RULES: &[(&str, &str)] = &[
    ("j", "down"), ("k", "up"), ("gg", "first"), ("G", "last"),
    ("Ctrl+d", "page-down"), ("Ctrl+u", "page-up"), ("Ctrl+f", "full-down"), ("Ctrl+b", "full-up"), ("zz", "center"),
    ("l", "open"), ("Tab", "next-list"), ("Shift+Tab", "previous-list"),
    ("/", "search"), ("n", "next-match"), ("N", "previous-match"),
    ("f{x}", "find-letter"), ("F{x}", "find-letter-back"),
    ("m{x}", "mark"), ("'{x}", "jump"),
    ("v", "visual"), ("V", "visual-line"), ("yy", "yank"), ("Y", "yank"), ("p", "paste"), ("P", "paste-before"), ("x", "tick"),
    ("dd", "delete"), ("cc", "change"), ("o", "new"), ("O", "new-above"), ("i", "edit"), ("a", "edit"), ("r", "replace"),
    ("u", "undo"), ("Ctrl+r", "redo"),
    ("za", "fold-toggle"), ("zo", "fold-open"), ("zc", "fold-close"), ("zR", "unfold-all"), ("zM", "fold-all"),
    ("]c", "next-change"), ("[c", "previous-change"), ("]t", "next-thread"), ("[t", "previous-thread"),
    ("]f", "next-file"), ("[f", "previous-file"), ("]e", "next-error"), ("[e", "previous-error"),
    ("]r", "next-run"), ("[r", "previous-run"),
    ("gd", "diff"), ("gx", "outside"), ("R", "refresh"),
    ("Ctrl+w v", "split"),
    ("q", "leave"), ("Escape", "leave"), ("?", "help"),
    ("Space s", "stop"), ("Space p", "pin"), ("Space f", "filter"), ("Space o", "order"), ("Space h", "hidden"),
    ("Space i", "info"), ("Space e", "editor"), ("Space t", "terminal"), ("Space w", "watch"), ("Space /", "search-wide"),
    ("Space a", "add"), ("Space S", "setup"),
];

/// The rules plugins written for plugin API 1 were checked against (they keep their keys)
pub const RULES_API1: &[(&str, &str)] = &[
    ("j", "down"), ("k", "up"), ("g", "first"), ("G", "last"),
    ("d", "page-down"), ("u", "page-up"),
    ("l", "open"),
    ("Tab", "next-list"), ("Shift+Tab", "previous-list"),
    ("v", "review"), ("o", "outside"), ("r", "refresh"),
    ("q", "leave"), ("Escape", "leave"),
    ("S", "setup"), ("?", "help"),
];

/// Keys thumbdeck keeps even while a plugin has the keyboard (plugin API 2): the Space menu's
/// own entries, pages, panes, the : line, repeating, and digits (counts)
pub const RESERVED: &[&str] = &[
    "Space", "Space Space", "Space ,", "Space 1", "Space 2", "Space 3", "Space 4", "Space 5", "Space 6", "Space 7", "Space 8", "Space 9",
    "gt", "gT", ":", ".", ";", ",", "Ctrl+p", "Ctrl+h", "Ctrl+j", "Ctrl+k", "Ctrl+l",
    "Ctrl+w o", "Ctrl+w h", "Ctrl+w j", "Ctrl+w k", "Ctrl+w l",
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9",
];

/// What plugin API 1 reserved
pub const RESERVED_API1: &[&str] = &["1", "2", "3", "4", "5", "6", "7", "8", "9", "z", "Ctrl+p", "Ctrl+h", "Ctrl+j", "Ctrl+k", "Ctrl+l", "Ctrl+b"];

/// A `[keys.<name>]` table: one place's (or mode's) keys
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Keymap {
    /// Shown in the status line while it has the keyboard
    pub name: String,
    /// tab:<id>, panel:<id>, page:<id> or view
    pub surface: String,
    pub bindings: Vec<Binding>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub keys: Vec<String>,
    pub action: String,
    pub does: String,
}

impl Keymap {
    /// What's wrong with its keys, in plain sentences (checked against its plugin API's rules)
    pub fn problems(&self, map: &str, api: u32) -> Vec<String> {
        let (rules, reserved) = if api >= 2 { (RULES, RESERVED) } else { (RULES_API1, RESERVED_API1) };
        let mut out = Vec::new();
        if self.name.trim().is_empty() {
            out.push(format!("keys.{map} needs a name (shown in the status line)"));
        }
        let mut seen: Vec<&str> = Vec::new();
        for b in &self.bindings {
            if b.action.trim().is_empty() {
                out.push(format!("keys.{map}: a binding for {} has no action", b.keys.join(", ")));
            }
            if b.keys.is_empty() {
                out.push(format!("keys.{map}: the binding for {} has no keys", b.action));
            }
            for k in &b.keys {
                if seen.contains(&k.as_str()) {
                    out.push(format!("keys.{map}: {k} is bound twice"));
                }
                seen.push(k);
                if reserved.contains(&k.as_str()) {
                    out.push(if api >= 2 {
                        format!("keys.{map}: {k} is thumbdeck's (Space, Space Space, Space , and Space 1–9 its menu, Toolkit, settings and pages; gt / gT pages; : the command line; . ; , repeating; digits counts; Ctrl+p, Ctrl+h/j/k/l and Ctrl+w panes)")
                    } else {
                        format!("keys.{map}: {k} is thumbdeck's (1–9 show tabs, z is wide, Ctrl+p the Plugins pane, Ctrl+h/j/k/l move between panes, Ctrl+b then n / p the next / previous tab)")
                    });
                }
                if let Some((_, rule)) = rules.iter().find(|(key, _)| key == k) {
                    if *rule != b.action {
                        out.push(format!("keys.{map}: {k} runs {} here, but {k} means {rule} everywhere", b.action));
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(bindings: &[(&[&str], &str)]) -> Keymap {
        Keymap {
            name: "X".into(),
            surface: "view".into(),
            bindings: bindings
                .iter()
                .map(|(keys, action)| Binding { keys: keys.iter().map(|k| k.to_string()).collect(), action: action.to_string(), does: "x".into() })
                .collect(),
        }
    }

    #[test]
    fn keys_follow_the_rules() {
        assert!(map(&[(&["j", "ArrowDown"], "down"), (&["/"], "search")]).problems("m", 2).is_empty());
        let p = map(&[(&["j"], "jump")]).problems("m", 2);
        assert_eq!(p, ["keys.m: j runs jump here, but j means down everywhere"]);
        assert!(map(&[(&["w"], "a"), (&["w"], "b")]).problems("m", 2)[0].contains("bound twice"));
        assert!(map(&[(&["3"], "three")]).problems("m", 2)[0].contains("thumbdeck's"));
        assert!(map(&[(&["Ctrl+p"], "x")]).problems("m", 2)[0].contains("thumbdeck's"));
        assert!(map(&[(&["Space Space"], "x")]).problems("m", 2)[0].contains("thumbdeck's"));
        assert!(map(&[(&[], "x")]).problems("m", 2)[0].contains("no keys"));
        // vim's meanings (API 2), and the old rules for plugins made for API 1
        assert!(map(&[(&["dd"], "delete"), (&["Space x"], "mine"), (&["gx"], "outside")]).problems("m", 2).is_empty());
        assert!(map(&[(&["d"], "page-down")]).problems("m", 2).is_empty(), "d alone isn't a rule any more");
        assert!(map(&[(&["R"], "reload")]).problems("m", 2)[0].contains("means refresh"));
        assert!(map(&[(&["d"], "page-down"), (&["z"], "x")]).problems("m", 1)[0].contains("thumbdeck's"));
        assert!(map(&[(&["d"], "delete")]).problems("m", 1)[0].contains("means page-down"));
    }

    /// The RULES object in a TypeScript or JavaScript file, as (key, action) pairs
    fn rules_in(ts: &str) -> Vec<(String, String)> {
        let start = ts.find("export const RULES").unwrap();
        let body = &ts[start..start + ts[start..].find("};").unwrap()];
        let body = &body[body.find('{').unwrap() + 1..];
        let re = regex::Regex::new(r#"("[^"]+"|[A-Za-z]+):\s*"([^"]+)""#).unwrap();
        let mut found: Vec<(String, String)> =
            re.captures_iter(body).map(|c| (c[1].trim_matches('"').to_string(), c[2].to_string())).collect();
        found.sort();
        found
    }

    /// The rules here are the ones the page follows, and the ones @thumbdeck/check checks
    #[test]
    fn rules_match_the_page_and_the_checker() {
        let mut here: Vec<(String, String)> = RULES.iter().map(|(k, a)| (k.to_string(), a.to_string())).collect();
        here.sort();
        assert_eq!(here, rules_in(include_str!("../../../src/lib/keys/keys.ts")), "RULES in keys.rs and keys.ts differ");
        let checker = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/toolkits/packages/check/index.js")).unwrap();
        assert_eq!(here, rules_in(&checker), "RULES in keys.rs and @thumbdeck/check differ");
        let reserved = checker.lines().find(|l| l.starts_with("export const RESERVED")).unwrap();
        for key in RESERVED {
            assert!(reserved.contains(&format!("\"{key}\"")), "{key} isn't reserved in @thumbdeck/check");
        }
    }
}
