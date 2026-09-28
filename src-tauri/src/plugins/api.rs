//! The part of the page API (`window.thumbdeck`) that runs here: files, commands, storage,
//! settings, notifications. The page forwards a frame's call with the frame's plugin and
//! project attached (never taken from the frame itself).

use super::install::data_dir;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, UNIX_EPOCH};

/// Text read or captured above this is cut
const MAX_READ: u64 = 50 * 1024 * 1024;
const MAX_LIST: usize = 20_000;

/// Who's calling: the plugin, and the project of the frame (None for app-wide frames)
pub struct Caller<'a> {
    pub plugin: &'a str,
    pub project: Option<&'a Path>,
    /// The plugin's settings, every field filled in
    pub settings: Value,
}

/// The plugin's own folder for data (created when first needed)
pub fn data_folder(plugin: &str) -> Option<PathBuf> {
    Some(data_dir()?.join("plugin-data").join(plugin))
}

impl Caller<'_> {
    /// A path from the frame: absolute, ~/…, or relative to the project (or to the data folder)
    fn path(&self, p: &str) -> Result<PathBuf, String> {
        if p.is_empty() {
            return Err("an empty path".into());
        }
        if let Some(rest) = p.strip_prefix("~/").or(if p == "~" { Some("") } else { None }) {
            return Ok(crate::projects::dirs_home().ok_or("no home folder")?.join(rest));
        }
        let p = Path::new(p);
        if p.is_absolute() {
            return Ok(p.to_path_buf());
        }
        match self.project {
            Some(dir) => Ok(dir.join(p)),
            None => Ok(data_folder(self.plugin).ok_or("no home folder")?.join(p)),
        }
    }

    /// Where commands run by default
    fn cwd(&self) -> Result<PathBuf, String> {
        match self.project {
            Some(dir) => Ok(dir.to_path_buf()),
            None => {
                let d = data_folder(self.plugin).ok_or("no home folder")?;
                std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
                Ok(d)
            }
        }
    }
}

fn args<T: for<'de> Deserialize<'de>>(params: Value) -> Result<T, String> {
    serde_json::from_value(params).map_err(|e| format!("wrong arguments: {e}"))
}

/// Run one call; its result as JSON, or a plain-sentence error
pub fn call(c: &Caller, method: &str, params: Value) -> Result<Value, String> {
    match method {
        "fs.read" => read(c, args(params)?),
        "fs.write" => {
            #[derive(Deserialize)]
            struct A {
                path: String,
                text: String,
            }
            let a: A = args(params)?;
            let path = c.path(&a.path)?;
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("can't write {}: {e}", a.path))?;
            }
            std::fs::write(&path, a.text).map_err(|e| format!("can't write {}: {e}", a.path))?;
            Ok(Value::Null)
        }
        "fs.stat" => {
            #[derive(Deserialize)]
            struct A {
                path: String,
            }
            let a: A = args(params)?;
            Ok(stat(&c.path(&a.path)?))
        }
        "fs.list" => list(c, args(params)?),
        "exec" => exec(c, args(params)?),
        "storage.get" | "storage.set" | "storage.remove" => storage(c, method, args(params)?),
        "settings.get" => Ok(c.settings.clone()),
        _ => Err(format!("there's no {method} in the plugin API")),
    }
}

// ------------------------------------------------------------ files

#[derive(Deserialize)]
struct ReadArgs {
    path: String,
    #[serde(default)]
    encoding: Option<String>,
    start: Option<u64>,
    end: Option<u64>,
}

fn read(c: &Caller, a: ReadArgs) -> Result<Value, String> {
    let path = c.path(&a.path)?;
    let mut f = std::fs::File::open(&path).map_err(|_| format!("can't read {}: it isn't there", a.path))?;
    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = a.start.unwrap_or(0).min(size);
    let end = a.end.unwrap_or(size).clamp(start, size);
    if end - start > MAX_READ {
        return Err(format!("{} is too big to read at once ({} MB); read it in parts with start and end", a.path, (end - start) / 1_048_576));
    }
    use std::io::Seek;
    f.seek(std::io::SeekFrom::Start(start)).map_err(|e| e.to_string())?;
    let mut bytes = vec![0; (end - start) as usize];
    f.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    Ok(match a.encoding.as_deref() {
        Some("base64") => {
            use base64::Engine;
            Value::from(base64::engine::general_purpose::STANDARD.encode(bytes))
        }
        None | Some("utf8") => Value::from(String::from_utf8_lossy(&bytes).into_owned()),
        Some(other) => return Err(format!("encoding is utf8 or base64, not {other}")),
    })
}

fn millis(m: &std::fs::Metadata) -> u64 {
    m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn stat(path: &Path) -> Value {
    match std::fs::metadata(path) {
        Ok(m) => json!({ "size": m.len(), "modified": millis(&m), "dir": m.is_dir() }),
        Err(_) => Value::Null,
    }
}

#[derive(Deserialize)]
struct ListArgs {
    folder: String,
    pattern: Option<String>,
    #[serde(default)]
    recursive: bool,
}

fn list(c: &Caller, a: ListArgs) -> Result<Value, String> {
    let root = c.path(&a.folder)?;
    if !root.is_dir() {
        return Err(format!("there's no folder {}", a.folder));
    }
    let pattern = match &a.pattern {
        Some(p) => Some(glob::Pattern::new(p).map_err(|e| format!("pattern {p}: {e}"))?),
        None => None,
    };
    let mut out = Vec::new();
    let mut folders = vec![root];
    while let Some(dir) = folders.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let Ok(m) = e.metadata() else { continue };
            let name = e.file_name().to_string_lossy().to_string();
            if a.recursive && m.is_dir() && !name.starts_with('.') {
                folders.push(e.path());
            }
            if pattern.as_ref().is_some_and(|p| !p.matches(&name)) {
                continue;
            }
            out.push(json!({ "name": name, "path": e.path(), "size": m.len(), "modified": millis(&m), "dir": m.is_dir() }));
            if out.len() >= MAX_LIST {
                return Ok(Value::Array(out));
            }
        }
    }
    Ok(Value::Array(out))
}

// ------------------------------------------------------------ commands

#[derive(Deserialize)]
struct ExecArgs {
    command: Value,
    cwd: Option<String>,
    #[serde(default)]
    env: Map<String, Value>,
    input: Option<String>,
    /// Milliseconds; 30 seconds when not given
    timeout: Option<u64>,
}

/// Run a command and capture what it prints. A string runs through the shell (with your login
/// shell's environment, like Toolkit buttons); a list runs the program directly.
fn exec(c: &Caller, a: ExecArgs) -> Result<Value, String> {
    let mut cmd = match &a.command {
        Value::String(s) => {
            let mut cmd = Command::new("/bin/sh");
            cmd.args(["-c", s]);
            cmd
        }
        Value::Array(list) if !list.is_empty() => {
            let words: Vec<String> = list.iter().map(|v| v.as_str().map(String::from).ok_or("a command list holds text only")).collect::<Result<_, _>>()?;
            let mut cmd = Command::new(&words[0]);
            cmd.args(&words[1..]);
            cmd
        }
        _ => return Err("command is text, or a list of words".into()),
    };
    let cwd = match &a.cwd {
        Some(d) => c.path(d)?,
        None => c.cwd()?,
    };
    let shown = match &a.command {
        Value::String(s) => s.clone(),
        v => v.as_array().map(|l| l.iter().filter_map(|w| w.as_str()).collect::<Vec<_>>().join(" ")).unwrap_or_default(),
    };
    cmd.current_dir(&cwd)
        .env_clear()
        .envs(crate::runner::shell_env().iter().cloned())
        .envs(a.env.iter().map(|(k, v)| (k.clone(), v.as_str().map(String::from).unwrap_or_else(|| v.to_string()))))
        .stdin(if a.input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("couldn't run {shown}: {e}"))?;
    if let (Some(input), Some(mut stdin)) = (a.input, child.stdin.take()) {
        std::thread::spawn(move || {
            let _ = stdin.write_all(input.as_bytes());
        });
    }
    let capture = |stream: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(s) = stream {
                let _ = s.take(MAX_READ).read_to_end(&mut bytes);
            }
            String::from_utf8_lossy(&bytes).into_owned()
        })
    };
    let out = capture(child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>));
    let err = capture(child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>));
    let limit = Duration::from_millis(a.timeout.unwrap_or(30_000));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if started.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{shown} took longer than {}s, so it was stopped", limit.as_secs_f32()));
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    Ok(json!({
        "code": status.code().unwrap_or(-1),
        "stdout": out.join().unwrap_or_default(),
        "stderr": err.join().unwrap_or_default(),
    }))
}

// ------------------------------------------------------------ storage

#[derive(Deserialize)]
struct StorageArgs {
    key: String,
    value: Option<Value>,
    /// "app" or "project"; project when the frame has one
    scope: Option<String>,
}

/// A short, stable name for a project folder (for its storage file)
fn hash(path: &Path) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in path.to_string_lossy().bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn storage_file(c: &Caller, scope: Option<&str>) -> Result<PathBuf, String> {
    let base = data_folder(c.plugin).ok_or("no home folder")?;
    match (scope, c.project) {
        (Some("app"), _) | (None, None) => Ok(base.join("storage.json")),
        (Some("project") | None, Some(p)) => Ok(base.join("projects").join(format!("{}.json", hash(p)))),
        (Some("project"), None) => Err("this frame has no project; use scope \"app\"".into()),
        (Some(other), _) => Err(format!("scope is \"app\" or \"project\", not \"{other}\"")),
    }
}

fn storage(c: &Caller, method: &str, a: StorageArgs) -> Result<Value, String> {
    let file = storage_file(c, a.scope.as_deref())?;
    let mut all: Map<String, Value> = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    if method == "storage.get" {
        return Ok(all.get(&a.key).cloned().unwrap_or(Value::Null));
    }
    if method == "storage.set" {
        all.insert(a.key, a.value.unwrap_or(Value::Null));
    } else {
        all.remove(&a.key);
    }
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string(&all).map_err(|e| e.to_string())?;
    std::fs::write(&file, text).map_err(|e| format!("can't save: {e}"))?;
    Ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn caller<'a>(project: Option<&'a Path>) -> Caller<'a> {
        Caller { plugin: "test-plugin", project, settings: json!({ "n": 1 }) }
    }

    #[test]
    fn files() {
        let d = TempDir::new("api-files");
        d.file("logs/a.log", "0123456789").file("logs/b.txt", "x").file("logs/deep/c.log", "c");
        let c = caller(Some(&d.0));
        assert_eq!(call(&c, "fs.read", json!({ "path": "logs/a.log" })).unwrap(), "0123456789");
        assert_eq!(call(&c, "fs.read", json!({ "path": "logs/a.log", "start": 3, "end": 6 })).unwrap(), "345");
        assert_eq!(call(&c, "fs.read", json!({ "path": "logs/a.log", "start": 8 })).unwrap(), "89");
        assert_eq!(call(&c, "fs.read", json!({ "path": "logs/b.txt", "encoding": "base64" })).unwrap(), "eA==");
        assert!(call(&c, "fs.read", json!({ "path": "nope" })).unwrap_err().contains("isn't there"));

        call(&c, "fs.write", json!({ "path": "out/new.txt", "text": "hi" })).unwrap();
        assert_eq!(std::fs::read_to_string(d.0.join("out/new.txt")).unwrap(), "hi");
        let s = call(&c, "fs.stat", json!({ "path": "logs/a.log" })).unwrap();
        assert_eq!((s["size"].as_u64(), s["dir"].as_bool()), (Some(10), Some(false)));
        assert_eq!(call(&c, "fs.stat", json!({ "path": "nope" })).unwrap(), Value::Null);

        let names = |v: Value| v.as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(names(call(&c, "fs.list", json!({ "folder": "logs", "pattern": "*.log" })).unwrap()), ["a.log"]);
        let mut deep = names(call(&c, "fs.list", json!({ "folder": "logs", "pattern": "*.log", "recursive": true })).unwrap());
        deep.sort();
        assert_eq!(deep, ["a.log", "c.log"]);
        assert!(call(&c, "fs.list", json!({ "folder": "nope" })).is_err());
        assert!(call(&c, "fs.nope", json!({})).unwrap_err().contains("no fs.nope"));
    }

    #[test]
    fn commands() {
        let d = TempDir::new("api-exec");
        d.file("f.txt", "");
        let c = caller(Some(&d.0));
        let r = call(&c, "exec", json!({ "command": "ls; echo oops >&2; exit 3" })).unwrap();
        assert_eq!((r["code"].as_i64(), r["stdout"].as_str(), r["stderr"].as_str()), (Some(3), Some("f.txt\n"), Some("oops\n")));
        let r = call(&c, "exec", json!({ "command": ["printf", "%s|", "a b", "$HOME"] })).unwrap();
        assert_eq!(r["stdout"], "a b|$HOME|", "a list isn't shell-expanded");
        let r = call(&c, "exec", json!({ "command": "cat; echo $X", "input": "in\n", "env": { "X": "y" } })).unwrap();
        assert_eq!(r["stdout"], "in\ny\n");
        assert!(call(&c, "exec", json!({ "command": "sleep 5", "timeout": 100 })).unwrap_err().contains("longer than 0.1s"));
        assert!(call(&c, "exec", json!({ "command": ["no-such-program-x"] })).unwrap_err().starts_with("couldn't run"));
    }

    #[test]
    fn storage_by_scope() {
        let d = TempDir::new("api-storage");
        // The data folder follows XDG_DATA_HOME; other tests don't set it, so this one does
        // its checks through storage_file only
        let p = caller(Some(&d.0));
        let app = storage_file(&p, Some("app")).unwrap();
        let project = storage_file(&p, None).unwrap();
        assert!(app.ends_with("plugin-data/test-plugin/storage.json"));
        assert!(project.to_string_lossy().contains("plugin-data/test-plugin/projects/"));
        assert_ne!(storage_file(&caller(Some(Path::new("/a"))), None).unwrap(), storage_file(&caller(Some(Path::new("/b"))), None).unwrap());
        assert_eq!(storage_file(&caller(None), None).unwrap(), app, "no project: the app's");
        assert!(storage_file(&caller(None), Some("project")).is_err());
        assert!(storage_file(&p, Some("everywhere")).is_err());
        assert_eq!(call(&p, "settings.get", Value::Null).unwrap(), json!({ "n": 1 }));
    }
}
