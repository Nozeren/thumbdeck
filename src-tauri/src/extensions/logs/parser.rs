//! Reading a log into entries (JSON lines or plain text, from any tool), and finding its
//! sections. Sections follow start and end markers: every line whose message contains a
//! start text opens a section, which ends
//! at the first later line whose message contains an end text (that line included), or runs
//! to the end of the file.

use super::{Alias, BlockConfig, Kind, Setup};
use regex::Regex;
use serde::Serialize;
use serde_json::{Map, Value};
use std::sync::OnceLock;

#[derive(Serialize, Clone, Debug)]
pub struct Line {
    /// DEBUG, INFO, WARNING, ERROR or UNKNOWN (see level())
    pub level: String,
    pub time: String,
    pub message: String,
    /// The whole entry, for the detail view
    pub data: Value,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Block {
    pub kind: Kind,
    pub name: String,
    pub first_line: usize,
    /// None: still open at the end of the file
    pub last_line: Option<usize>,
}

pub fn parse(text: &str, setup: &Setup) -> Vec<Line> {
    let raw: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let json_lines = raw.first().is_some_and(|l| l.trim_start().starts_with('{') && serde_json::from_str::<Value>(l).is_ok());
    if json_lines {
        raw.iter().map(|l| json_line(l, setup)).collect()
    } else if !setup.line_pattern.is_empty() {
        pattern_lines(&raw, setup)
    } else {
        plain_lines(&raw)
    }
}

/// Every tool names its levels differently: they're put in the groups the viewer filters by.
/// Numbers are pino / bunyan levels (30 info, 50 error, ...).
pub fn level(text: &str) -> &'static str {
    let t = text.trim().to_ascii_lowercase();
    if let Ok(n) = t.parse::<u32>() {
        return match n {
            0..=29 => "DEBUG",
            30..=39 => "INFO",
            40..=49 => "WARNING",
            _ => "ERROR",
        };
    }
    match t.as_str() {
        "trace" | "debug" | "verbose" | "fine" | "finer" | "finest" | "dbg" | "d" | "v" => "DEBUG",
        "info" | "information" | "notice" | "inf" | "i" => "INFO",
        "warn" | "warning" | "wrn" | "w" => "WARNING",
        "error" | "err" | "fatal" | "critical" | "crit" | "panic" | "severe" | "alert" | "emerg" | "emergency" | "e"
        | "f" => "ERROR",
        _ => "UNKNOWN",
    }
}

/// A JSON entry; a line that isn't a JSON object (in a JSON log) is kept as text
fn json_line(raw: &str, setup: &Setup) -> Line {
    let data = match serde_json::from_str::<Value>(raw) {
        Ok(Value::Object(map)) => Value::Object(map),
        Ok(other) => Value::Object(Map::from_iter([("message".to_string(), other)])),
        Err(_) => Value::Object(Map::from_iter([("message".to_string(), Value::String(raw.to_string()))])),
    };
    let field = |names: &[String]| names.iter().find_map(|n| data.get(n).filter(|v| !v.is_null()));
    let text = |v: &Value| match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    Line {
        level: field(&setup.fields.level).map(|v| level(&text(v))).unwrap_or("UNKNOWN").to_string(),
        time: field(&setup.fields.time)
            .map(|v| match v.as_f64() {
                Some(n) => epoch(n),
                None => text(v),
            })
            .unwrap_or_default(),
        message: field(&setup.fields.message).map(text).unwrap_or_default(),
        data,
    }
}

/// Seconds or milliseconds since 1970 as "2026-07-31 13:00:00,001", in local time like the
/// times plain-text logs are written in
fn epoch(n: f64) -> String {
    let ms = if n > 1e11 { n } else { n * 1000.0 } as i64;
    date_text(ms + utc_offset(ms.div_euclid(1000)) * 1000)
}

/// Seconds east of UTC here, at that moment
fn utc_offset(secs: i64) -> i64 {
    let t = secs as libc::time_t;
    // SAFETY: localtime_r only writes the tm it's given
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() { 0 } else { tm.tm_gmtoff as i64 }
    }
}

/// Milliseconds since 1970 as a date and time (no time zone applied)
fn date_text(ms: i64) -> String {
    let (secs, ms) = (ms.div_euclid(1000), ms.rem_euclid(1000));
    let (days, rest) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // Howard Hinnant's civil_from_days
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02},{ms:03}", rest / 3600, rest % 3600 / 60, rest % 60)
}

fn regex(cell: &'static OnceLock<Regex>, re: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(re).unwrap())
}

/// A date and/or time at the start of a line: ISO ("2026-07-31 10:00:00,001", "...T...Z"),
/// time only ("10:00:00.123"), Apache/Django ("[31/Jul/2026:10:00:00 +0000]", "[31/Jul/2026 10:00:00]"),
/// syslog ("Jul 31 10:00:00")
fn leading_time(line: &str) -> Option<(String, usize)> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = regex(&RE, concat!(
        r"^\[?(\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:[.,]\d+)?(?:Z|[+-]\d{2}:?\d{2})?",
        r"|\d{2}:\d{2}:\d{2}(?:[.,]\d+)?",
        r"|\d{2}/[A-Za-z]{3}/\d{4}[: ]\d{2}:\d{2}:\d{2}(?: [+-]\d{4})?",
        r"|[A-Z][a-z]{2} [ \d]\d \d{2}:\d{2}:\d{2})\]?[ \t:|-]*",
    ));
    let c = re.captures(line)?;
    Some((c[1].to_string(), c.get(0)?.end()))
}

/// The level of a plain-text line: a level word in capitals (ERROR, WARN, ...), or level=error,
/// [error], <error>; else an HTTP status in an access log (5xx error, 4xx warning)
fn line_level(line: &str) -> &'static str {
    static WORD: OnceLock<Regex> = OnceLock::new();
    static TAGGED: OnceLock<Regex> = OnceLock::new();
    static HTTP: OnceLock<Regex> = OnceLock::new();
    let word = regex(&WORD, r"\b(TRACE|DEBUG|INFO|NOTICE|WARN|WARNING|ERROR|ERR|FATAL|CRITICAL|CRIT|PANIC|SEVERE)\b");
    let tagged = regex(&TAGGED, r#"(?i)(?:level[=:]"?|\[|<)(trace|debug|info|notice|warn|warning|error|err|fatal|critical|panic)\b"#);
    let found = [word.captures(line), tagged.captures(line)]
        .into_iter()
        .flatten()
        .min_by_key(|c| c.get(1).map(|m| m.start()).unwrap_or(usize::MAX));
    if let Some(c) = found {
        return level(&c[1]);
    }
    let http = regex(&HTTP, r#"HTTP/[\d.]+" (\d{3}) "#);
    match http.captures(line).and_then(|c| c[1].parse::<u16>().ok()) {
        Some(500..=599) => "ERROR",
        Some(400..=499) => "WARNING",
        Some(_) => "INFO",
        None => "UNKNOWN",
    }
}

/// A level word right at the start of a message ("ERROR    Job 7 failed"): the viewer shows the
/// level in its own column, so it's left out of the message
fn without_level(message: &str) -> &str {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = regex(&RE, r"^\[?(?:TRACE|DEBUG|INFO|NOTICE|WARN|WARNING|ERROR|ERR|FATAL|CRITICAL|CRIT|PANIC|SEVERE)\]?(?:[ \t]*[:|-])?[ \t]+");
    re.find(message).map(|m| &message[m.end()..]).unwrap_or(message)
}

fn plain_entry(time: String, level: &str, message: String) -> Line {
    let data = serde_json::json!({ "time": time, "level": level, "message": message });
    Line { level: level.to_string(), time, message, data }
}

fn add_to(prev: &mut Line, text: &str) {
    prev.message.push('\n');
    prev.message.push_str(text);
    prev.data["message"] = Value::String(prev.message.clone());
    // A traceback under an entry makes it an error
    if prev.level == "UNKNOWN" || prev.level == "INFO" {
        let l = line_level(text);
        if l == "ERROR" {
            prev.level = l.to_string();
            prev.data["level"] = l.into();
        }
    }
}

/// Plain text without a pattern. When the log's lines start with a time, a line without one
/// (a traceback, wrapped output) belongs to the entry above; otherwise each line is an entry,
/// except indented ones, which belong to the line above.
fn plain_lines(raw: &[&str]) -> Vec<Line> {
    let timestamped = raw.first().is_some_and(|l| leading_time(l).is_some());
    let mut out: Vec<Line> = Vec::new();
    for l in raw {
        let time = leading_time(l);
        let continues = if timestamped { time.is_none() } else { l.starts_with([' ', '\t']) };
        match out.last_mut() {
            Some(prev) if continues => add_to(prev, l),
            _ => {
                let (time, rest) = time.map(|(t, end)| (t, &l[end..])).unwrap_or_default();
                let rest = if time.is_empty() { l } else { without_level(rest) };
                out.push(plain_entry(time, line_level(l), rest.to_string()));
            }
        }
    }
    out
}

/// Plain text with the setup's pattern: a matching line starts an entry, the lines after it
/// that don't match belong to it.
fn pattern_lines(raw: &[&str], setup: &Setup) -> Vec<Line> {
    let re = Regex::new(&setup.line_pattern).ok();
    let mut out: Vec<Line> = Vec::new();
    for l in raw {
        match (re.as_ref().and_then(|re| re.captures(l)), out.last_mut()) {
            (Some(c), _) => {
                let group = |name| c.name(name).map(|m| m.as_str().to_string()).unwrap_or_default();
                let lvl = match group("level") {
                    g if g.is_empty() => line_level(l),
                    g => level(&g),
                };
                out.push(plain_entry(group("time"), lvl, group("message")));
            }
            (None, Some(prev)) => add_to(prev, l),
            (None, None) => out.push(plain_entry(String::new(), line_level(l), l.to_string())),
        }
    }
    out
}

fn contains_any(message: &str, texts: &[String]) -> bool {
    !message.is_empty() && texts.iter().any(|t| !t.is_empty() && message.contains(t.as_str()))
}

/// All sections, in order of their first line (and of the setup for sections starting together)
pub fn blocks(lines: &[Line], configs: &[BlockConfig]) -> Vec<Block> {
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        for c in configs {
            if !contains_any(&line.message, &c.start) {
                continue;
            }
            let last_line = (i + 1..lines.len()).find(|&j| contains_any(&lines[j].message, &c.end));
            out.push(Block { kind: c.kind, name: title(line.message.trim(), c.alias.as_ref()), first_line: i, last_line });
        }
    }
    out
}

fn title(message: &str, alias: Option<&Alias>) -> String {
    match alias {
        None => message.to_string(),
        Some(Alias::Regex(re)) => Regex::new(&format!("(?s){re}"))
            .ok()
            .and_then(|re| Some(re.captures(message)?.get(1)?.as_str().trim().to_string()))
            .unwrap_or_else(|| message.to_string()),
        Some(Alias::Rewrite(to)) => to.clone(),
        Some(Alias::Replace { from, to }) => message.replace(from.as_str(), to),
        Some(Alias::Prefix(p)) => format!("{p}{message}"),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::extensions::logs::tests::sections;

    /// A sample test run: JSON lines, two test scenarios
    pub const SAMPLE: &str = include_str!("testdata/sample_run.log");

    pub fn sections_setup() -> Setup {
        Setup { blocks: sections(), ..Setup::default() }
    }

    fn levels(text: &str) -> Vec<(String, String)> {
        parse(text, &Setup::default()).into_iter().map(|l| (l.level, l.message)).collect()
    }

    #[test]
    fn json_lines() {
        let lines = parse(SAMPLE, &Setup::default());
        assert_eq!(lines.len(), 28);
        assert_eq!(lines[9].level, "ERROR");
        assert_eq!(lines[0].time, "2026-07-31 10:00:00,001");
        assert_eq!(lines[0].data["message"], "Read 'ppe.yml' configuration file.");
    }

    #[test]
    fn json_from_other_tools() {
        // pino: numeric levels, epoch milliseconds, msg
        let pino = parse(r#"{"level":50,"time":1785492000123,"msg":"db down"}"#, &Setup::default());
        assert_eq!((pino[0].level.as_str(), pino[0].message.as_str()), ("ERROR", "db down"));
        assert_eq!(pino[0].time, date_text(1785492000123 + utc_offset(1785492000) * 1000), "local time");
        assert_eq!(date_text(1785492000123), "2026-07-31 10:00:00,123");
        // structlog / zap
        let s = parse("{\"event\": \"started\", \"level\": \"warning\", \"timestamp\": \"2026-07-31T10:00:00Z\"}\n\
                       {\"severity\": \"CRITICAL\", \"message\": \"x\"}\nnot json\n42", &Setup::default());
        assert_eq!((s[0].level.as_str(), s[0].message.as_str()), ("WARNING", "started"));
        assert_eq!(s[1].level, "ERROR");
        assert_eq!((s[2].level.as_str(), s[2].message.as_str()), ("UNKNOWN", "not json"));
        assert_eq!(s[3].message, "42");
        let mut setup = Setup::default();
        setup.fields.level = vec!["lvl".into()];
        assert_eq!(parse(r#"{"lvl": "dbg", "message": "m"}"#, &setup)[0].level, "DEBUG");
    }

    #[test]
    fn plain_python_logging_with_a_traceback() {
        let l = levels("2026-07-31 10:00:00,000 INFO    started\n\
                        2026-07-31 10:00:00,100 ERROR   something broke\n\
                        Traceback (most recent call last):\n  File \"x.py\", line 1\n\
                        2026-07-31 10:00:01,000 WARNING slow\n");
        assert_eq!(l.len(), 3, "the traceback belongs to the error");
        assert_eq!(l[1].0, "ERROR");
        assert!(l[1].1.starts_with("something broke\nTraceback"), "{}", l[1].1);
        assert_eq!(l[0].1, "started", "the level isn't repeated in the message");
        assert_eq!(l[2].0, "WARNING");
        let t = &parse("2026-07-31 10:00:00,000 INFO x", &Setup::default())[0];
        assert_eq!(t.time, "2026-07-31 10:00:00,000");
    }

    #[test]
    fn plain_logs_from_other_tools() {
        // Rust (env_logger / tracing), Go (logfmt), Django runserver, nginx access, syslog
        let l = levels("[2026-07-31T10:00:00Z ERROR app] failed\n\
                        2026-07-31T10:00:01Z  WARN app: slow\n");
        assert_eq!((l[0].0.as_str(), l[1].0.as_str()), ("ERROR", "WARNING"));
        assert_eq!(levels("time=2026-07-31T10:00:00Z level=error msg=\"no db\"")[0].0, "ERROR");
        let django = levels("[31/Jul/2026 10:00:00] \"GET / HTTP/1.1\" 200 512\n\
                             [31/Jul/2026 10:00:01] \"GET /x HTTP/1.1\" 404 10\n\
                             [31/Jul/2026 10:00:02] \"POST /y HTTP/1.1\" 500 99\n");
        assert_eq!(django.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["INFO", "WARNING", "ERROR"]);
        assert_eq!(django[0].1, "\"GET / HTTP/1.1\" 200 512", "the time isn't repeated in the message");
        assert_eq!(levels("Jul 31 10:00:00 host sshd[1]: error: bad key")[0].0, "UNKNOWN", "lowercase words aren't levels");
        assert_eq!(levels("Jul 31 10:00:00 host kernel: <error> disk")[0].0, "ERROR");
    }

    #[test]
    fn plain_output_without_times() {
        // e.g. a build or test run: every line an entry, indented ones belong to the line above
        let l = levels("Compiling app\nerror[E0425]: cannot find value `x`\n  --> src/main.rs:2:5\nFAILED tests::a\n");
        assert_eq!(l.len(), 3);
        assert!(l[1].1.ends_with("--> src/main.rs:2:5"));
        assert_eq!(l[0].0, "UNKNOWN");
    }

    #[test]
    fn a_line_pattern_from_the_setup() {
        let setup = Setup { line_pattern: r"^(?P<level>\w) (?P<message>.*)$".into(), ..Setup::default() };
        let lines = parse("E boom\n  more\nI fine\n", &setup);
        assert_eq!(lines.iter().map(|l| l.level.as_str()).collect::<Vec<_>>(), ["ERROR", "INFO"]);
        assert_eq!(lines[0].message, "boom\n  more");
    }

    #[test]
    fn levels_from_every_tool_land_in_four_groups() {
        for (text, want) in [("trace", "DEBUG"), ("Debug", "DEBUG"), ("notice", "INFO"), ("WARN", "WARNING"),
                             ("fatal", "ERROR"), ("CRITICAL", "ERROR"), ("20", "DEBUG"), ("30", "INFO"),
                             ("40", "WARNING"), ("60", "ERROR"), ("chatty", "UNKNOWN")] {
            assert_eq!(level(text), want, "{text}");
        }
    }

    #[test]
    fn sections_like_qa_log_tui() {
        let setup = sections_setup();
        let lines = parse(SAMPLE, &setup);
        let all = blocks(&lines, &setup.blocks);
        let names = |kind| all.iter().filter(|b| b.kind == kind).map(|b| b.name.as_str()).collect::<Vec<_>>();
        assert_eq!(names(Kind::Bookmark), ["Login works", "Logout works"]);
        let inner = names(Kind::Index);
        assert!(inner.contains(&"Environment setup"));
        assert_eq!(inner.iter().filter(|n| **n == "Setup").count(), 2);
        assert!(inner.contains(&"When I submit valid credentials"), "titles from the regex");
        let login = all.iter().find(|b| b.name == "Login works").unwrap();
        assert_eq!((login.first_line, login.last_line), (2, Some(16)), "ends at the next start");
        let open = blocks(&lines[..20], &setup.blocks);
        assert_eq!(open.iter().find(|b| b.name == "Logout works").unwrap().last_line, None, "runs to the end");
    }

    #[test]
    fn titles() {
        assert_eq!(title("# A #", Some(&Alias::Regex("# (.*?) #".into()))), "A");
        assert_eq!(title("line\nmore", Some(&Alias::Regex("line.(more)".into()))), "more", "regex sees newlines");
        assert_eq!(title("no match", Some(&Alias::Regex("x(y)".into()))), "no match");
        assert_eq!(title("a-b", Some(&Alias::Replace { from: "-".into(), to: " to ".into() })), "a to b");
        assert_eq!(title("b", Some(&Alias::Prefix("a ".into()))), "a b");
        assert_eq!(title("b", Some(&Alias::Rewrite("Setup".into()))), "Setup");
        assert_eq!(title("b", None), "b");
    }
}
