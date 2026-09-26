//! The log files of a project, and what the tab needs to show one.

use super::outline::{self, Node};
use super::parser::{self, Line};
use super::Setup;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

#[derive(Serialize, Clone, Debug)]
pub struct LogFile {
    pub name: String,
    pub path: String,
    /// Seconds since 1970
    pub mtime: f64,
    pub size: u64,
    /// Has an ERROR line
    pub error: bool,
}

/// Log files in the setup's folders, newest first. Folders that don't exist are listed in
/// the second value.
pub fn list(project: &Path, setup: &Setup) -> (Vec<LogFile>, Vec<String>) {
    let mut files = Vec::new();
    let mut missing = Vec::new();
    for folder in setup.folders.iter().map(|f| f.trim()).filter(|f| !f.is_empty()) {
        let dir = project.join(folder);
        if !dir.is_dir() {
            missing.push(folder.to_string());
            continue;
        }
        let pattern = format!("{}/{}", glob::Pattern::escape(&dir.to_string_lossy()), setup.pattern);
        for path in glob::glob(&pattern).into_iter().flatten().flatten().filter(|p| p.is_file()) {
            if files.iter().any(|f: &LogFile| Path::new(&f.path) == path) {
                continue;
            }
            let Ok(meta) = path.metadata() else { continue };
            let mtime = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs_f64()).unwrap_or(0.0);
            files.push(LogFile {
                name: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                path: path.to_string_lossy().to_string(),
                mtime,
                size: meta.len(),
                error: has_error(&path, mtime, meta.len(), setup),
            });
        }
    }
    files.sort_by(|a, b| b.mtime.total_cmp(&a.mtime));
    (files, missing)
}

/// Whether a file has an ERROR line; remembered until the file changes (the list is
/// refreshed every few seconds, and logs can be big)
fn has_error(path: &Path, mtime: f64, size: u64, setup: &Setup) -> bool {
    static SEEN: Mutex<Option<HashMap<PathBuf, (f64, u64, bool)>>> = Mutex::new(None);
    let mut seen = SEEN.lock().unwrap();
    let seen = seen.get_or_insert_with(HashMap::new);
    if let Some((m, s, error)) = seen.get(path) {
        if *m == mtime && *s == size {
            return *error;
        }
    }
    let error = read(path).is_ok_and(|text| parser::parse(&text, setup).iter().any(|l| l.level == "ERROR"));
    seen.insert(path.to_path_buf(), (mtime, size, error));
    error
}

fn read(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("couldn't read {}: {e}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// A log, ready to show
#[derive(Serialize)]
pub struct Log {
    pub lines: Vec<Line>,
    pub outline: Vec<Node>,
    pub size: u64,
}

pub fn open(path: &Path, setup: &Setup) -> Result<Log, String> {
    let text = read(path)?;
    let lines = parser::parse(&text, setup);
    let outline = outline::build(&lines, &parser::blocks(&lines, &setup.blocks));
    Ok(Log { lines, outline, size: text.len() as u64 })
}

/// At a glance, for the file list's preview
#[derive(Serialize, Debug)]
pub struct Summary {
    pub total: usize,
    /// DEBUG, INFO, WARNING, ERROR (and others seen), with counts
    pub levels: Vec<(String, usize)>,
    /// Top-level blocks (scenarios)
    pub blocks: usize,
    pub first_error: Option<String>,
    pub duration: Option<String>,
}

pub fn summary(path: &Path, setup: &Setup) -> Result<Summary, String> {
    let log = open(path, setup)?;
    let mut levels: Vec<(String, usize)> = ["DEBUG", "INFO", "WARNING", "ERROR"].iter().map(|l| (l.to_string(), 0)).collect();
    for line in &log.lines {
        match levels.iter_mut().find(|(l, _)| *l == line.level) {
            Some((_, n)) => *n += 1,
            None => levels.push((line.level.clone(), 1)),
        }
    }
    let seconds = |l: &Line| timestamp(&l.time);
    let duration = match (log.lines.first().and_then(seconds), log.lines.last().and_then(seconds)) {
        (Some(a), Some(b)) if b >= a => Some(format_duration(b - a)),
        _ => None,
    };
    Ok(Summary {
        total: log.lines.len(),
        levels,
        blocks: log.outline.iter().filter(|n| matches!(n, Node::Block { .. })).count(),
        first_error: log.lines.iter().find(|l| l.level == "ERROR").map(|l| l.message.clone()),
        duration,
    })
}

/// "2026-07-31 10:00:00,001" (or with T, or .001) as seconds
fn timestamp(text: &str) -> Option<f64> {
    let t = text.get(..23).unwrap_or(text);
    let num = |range: std::ops::Range<usize>| t.get(range)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, s) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let ms = num(20..23).unwrap_or(0);
    // Days since 1970 (Howard Hinnant's days_from_civil)
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let days = era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468;
    Some((days * 86400 + h * 3600 + mi * 60 + s) as f64 + ms as f64 / 1000.0)
}

fn format_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        return format!("{seconds:.1}s");
    }
    let (minutes, secs) = ((seconds as u64) / 60, (seconds as u64) % 60);
    if minutes < 60 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{}h {}m", minutes / 60, minutes % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::logs::parser::tests::{sections_setup, SAMPLE};
    use crate::packs::tests::TempDir;

    fn touch_later(path: &Path, secs: u64) {
        let f = std::fs::File::options().write(true).open(path).unwrap();
        f.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(secs)).unwrap();
    }

    #[test]
    fn logs_are_listed_newest_first_across_folders_with_their_status() {
        let d = TempDir::new("logs-list");
        d.file("logs/a.log", SAMPLE).file("reports/b.log", "{\"level\": \"INFO\", \"message\": \"ok\"}").file("logs/notes.txt", "");
        touch_later(&d.0.join("reports/b.log"), 10);
        let setup = Setup { folders: vec!["logs".into(), "reports".into(), "gone".into(), "logs".into()], ..sections_setup() };
        let (files, missing) = list(&d.0, &setup);
        let names: Vec<(&str, bool)> = files.iter().map(|f| (f.name.as_str(), f.error)).collect();
        assert_eq!(names, [("b.log", false), ("a.log", true)], "newest first, no duplicates, only *.log");
        assert_eq!(missing, ["gone"]);
    }

    #[test]
    fn the_error_flag_follows_changes() {
        let d = TempDir::new("logs-error-cache");
        d.file("logs/run.log", "{\"level\": \"INFO\", \"message\": \"ok\"}\n");
        let setup = sections_setup();
        assert!(!list(&d.0, &setup).0[0].error);
        d.file("logs/run.log", "{\"level\": \"INFO\", \"message\": \"ok\"}\n{\"level\": \"ERROR\", \"message\": \"no\"}\n");
        assert!(list(&d.0, &setup).0[0].error);
    }

    #[test]
    fn open_and_summarize() {
        let d = TempDir::new("logs-open");
        d.file("run.log", SAMPLE);
        let log = open(&d.0.join("run.log"), &sections_setup()).unwrap();
        assert_eq!(log.lines.len(), 28);
        let s = summary(&d.0.join("run.log"), &sections_setup()).unwrap();
        assert_eq!(s.total, 28);
        assert_eq!(s.blocks, 3, "sections: Environment setup, Login works, Logout works");
        assert_eq!(s.levels[3], ("ERROR".to_string(), 2));
        assert_eq!(s.first_error.as_deref(), Some("Element '#submit-button' not found after 10s timeout"));
        assert_eq!(s.duration.as_deref(), Some("2.7s"));
        assert!(open(&d.0.join("missing.log"), &sections_setup()).is_err());
    }

    #[test]
    fn timestamps_and_durations() {
        let a = timestamp("2026-07-31 23:59:59,500").unwrap();
        let b = timestamp("2026-08-01T00:01:00.000").unwrap();
        assert_eq!(b - a, 60.5);
        assert_eq!(timestamp("yesterday"), None);
        assert_eq!(format_duration(2.72), "2.7s");
        assert_eq!(format_duration(125.0), "2m 5s");
        assert_eq!(format_duration(7300.0), "2h 1m");
    }
}
