//! Each plugin's log: its pages' console output and errors, and (later) its backend's stderr.
//! Kept in memory, the last lines only; Settings › Plugins shows it.

use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

const KEEP: usize = 500;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Line {
    /// Milliseconds since 1970
    pub at: u64,
    /// log, info, warn, error, debug (from a page), or stderr (from the backend)
    pub level: String,
    pub text: String,
}

#[derive(Default)]
pub struct Logs(Mutex<HashMap<String, VecDeque<Line>>>);

impl Logs {
    pub fn add(&self, plugin: &str, level: &str, text: &str) {
        let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        if cfg!(debug_assertions) {
            eprintln!("plugin {plugin} [{level}] {text}");
        }
        let mut all = self.0.lock().unwrap();
        let lines = all.entry(plugin.to_string()).or_default();
        lines.push_back(Line { at, level: level.to_string(), text: text.chars().take(4000).collect() });
        while lines.len() > KEEP {
            lines.pop_front();
        }
    }

    pub fn lines(&self, plugin: &str) -> Vec<Line> {
        self.0.lock().unwrap().get(plugin).map(|l| l.iter().cloned().collect()).unwrap_or_default()
    }

    pub fn clear(&self, plugin: &str) {
        self.0.lock().unwrap().remove(plugin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_last_lines_per_plugin() {
        let logs = Logs::default();
        for i in 0..KEEP + 10 {
            logs.add("a", "log", &i.to_string());
        }
        logs.add("b", "error", &"x".repeat(10_000));
        let a = logs.lines("a");
        assert_eq!((a.len(), a[0].text.as_str()), (KEEP, "10"));
        assert_eq!(logs.lines("b")[0].text.len(), 4000, "long lines are cut");
        logs.clear("a");
        assert!(logs.lines("a").is_empty() && logs.lines("nope").is_empty());
    }
}
