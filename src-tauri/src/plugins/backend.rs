//! Plugin backends: a program per plugin that reads JSON requests on stdin and answers on
//! stdout, one per line (docs/plugin-spec.md, The backend). Started when first needed, kept
//! for every project and frame, started again after a crash (not after three in a minute),
//! told to shut down when thumbdeck quits. Its stderr goes to the plugin's log.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// What a backend can reach in thumbdeck
pub trait Host: Send + Sync + 'static {
    /// A line for the plugin's log (its stderr, or stdout that isn't a message)
    fn log(&self, plugin: &str, text: &str);
    /// A request (or event) from the backend: `event`, `ui.*`, `storage.*`, `actions.refresh`
    fn from_backend(&self, plugin: &str, method: &str, params: Value) -> Result<Value, String>;
}

/// How to start a plugin's backend
#[derive(Clone)]
pub struct Spec {
    pub plugin: String,
    pub folder: PathBuf,
    pub command: String,
    /// The `initialize` request's params (api, thumbdeck, folders, settings)
    pub init: Value,
}

struct Running {
    stdin: Mutex<ChildStdin>,
    child: Mutex<Child>,
    pending: Mutex<HashMap<u64, mpsc::Sender<Result<Value, String>>>>,
    next: AtomicU64,
    alive: AtomicBool,
    /// Asked to stop: its end isn't a crash
    stopping: AtomicBool,
}

impl Running {
    fn send(&self, message: &Value) -> Result<(), String> {
        let mut stdin = self.stdin.lock().unwrap();
        writeln!(stdin, "{message}").and_then(|_| stdin.flush()).map_err(|_| "the backend isn't running".to_string())
    }

    fn request(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap().insert(id, tx);
        if let Err(e) = self.send(&json!({ "id": id, "method": method, "params": params })) {
            self.pending.lock().unwrap().remove(&id);
            return Err(e);
        }
        match rx.recv_timeout(timeout) {
            Ok(answer) => answer,
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err(format!("the backend didn't answer {method} within {}s", timeout.as_secs()))
            }
        }
    }
}

/// Every plugin's backend (shared with the threads reading them)
#[derive(Default, Clone)]
pub struct Backends(Arc<Inner>);

#[derive(Default)]
struct Inner {
    running: Mutex<HashMap<String, Arc<Running>>>,
    /// When each plugin's backend ended by itself lately
    crashes: Mutex<HashMap<String, Vec<Instant>>>,
    /// Serialises starting, so two calls don't start two backends
    starting: Mutex<()>,
}

const CRASHES: usize = 3;
const CRASH_WINDOW: Duration = Duration::from_secs(60);

impl Backends {
    /// Call a method; starts the backend when it isn't running
    pub fn call(&self, host: Arc<dyn Host>, spec: &Spec, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let backend = self.get_or_start(host, spec)?;
        backend.request(method, params, timeout)
    }

    /// Tell a running backend something (no answer); nothing when it isn't running
    pub fn event(&self, plugin: &str, method: &str, params: Value) {
        if let Some(b) = self.0.running.lock().unwrap().get(plugin) {
            let _ = b.send(&json!({ "method": method, "params": params }));
        }
    }

    #[cfg(test)]
    pub fn is_running(&self, plugin: &str) -> bool {
        self.0.running.lock().unwrap().contains_key(plugin)
    }

    fn get_or_start(&self, host: Arc<dyn Host>, spec: &Spec) -> Result<Arc<Running>, String> {
        let _one_at_a_time = self.0.starting.lock().unwrap();
        if let Some(b) = self.0.running.lock().unwrap().get(&spec.plugin) {
            return Ok(b.clone());
        }
        let recent = {
            let mut crashes = self.0.crashes.lock().unwrap();
            let list = crashes.entry(spec.plugin.clone()).or_default();
            list.retain(|t| t.elapsed() < CRASH_WINDOW);
            list.len()
        };
        if recent >= CRASHES {
            return Err("its backend stopped three times in a minute, so it isn't started again for now (its log is in Settings › Plugins)".into());
        }
        let backend = start(host, spec, self.clone())?;
        self.0.running.lock().unwrap().insert(spec.plugin.clone(), backend.clone());
        if let Err(e) = backend.request("initialize", spec.init.clone(), Duration::from_secs(10)) {
            self.stop(&spec.plugin);
            return Err(format!("its backend didn't start: {e}"));
        }
        Ok(backend)
    }

    /// Ask a backend to shut down (killed after 2 seconds), and forget its crashes
    pub fn stop(&self, plugin: &str) {
        self.0.crashes.lock().unwrap().remove(plugin);
        let Some(b) = self.0.running.lock().unwrap().remove(plugin) else { return };
        b.stopping.store(true, Ordering::Relaxed);
        std::thread::spawn(move || {
            let _ = b.request("shutdown", Value::Null, Duration::from_secs(2));
            let started = Instant::now();
            while b.alive.load(Ordering::Relaxed) && started.elapsed() < Duration::from_secs(2) {
                std::thread::sleep(Duration::from_millis(20));
            }
            if b.alive.load(Ordering::Relaxed) {
                let pid = b.child.lock().unwrap().id() as i32;
                unsafe {
                    libc::kill(-pid, libc::SIGKILL);
                }
            }
        });
    }

    /// Stop them all (thumbdeck is quitting); waits up to 2 seconds
    pub fn stop_all(&self) {
        let ids: Vec<String> = self.0.running.lock().unwrap().keys().cloned().collect();
        for id in &ids {
            self.stop(id);
        }
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(2100) && !ids.is_empty() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// A backend ended: forget it, and count it when nobody asked it to stop
    fn ended(&self, plugin: &str, backend: &Arc<Running>) {
        let mut running = self.0.running.lock().unwrap();
        if running.get(plugin).is_some_and(|b| Arc::ptr_eq(b, backend)) {
            running.remove(plugin);
        }
        if !backend.stopping.load(Ordering::Relaxed) {
            self.0.crashes.lock().unwrap().entry(plugin.to_string()).or_default().push(Instant::now());
        }
    }
}

/// Start the program and the threads reading its output
fn start(host: Arc<dyn Host>, spec: &Spec, backends: Backends) -> Result<Arc<Running>, String> {
    let mut child = Command::new("/bin/sh")
        .args(["-c", &spec.command])
        .current_dir(&spec.folder)
        .env_clear()
        .envs(crate::runner::shell_env().iter().cloned())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|e| format!("couldn't start its backend ({}): {e}", spec.command))?;
    let stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let stderr = child.stderr.take().ok_or("no stderr")?;
    let backend = Arc::new(Running {
        stdin: Mutex::new(stdin),
        child: Mutex::new(child),
        pending: Mutex::new(HashMap::new()),
        next: AtomicU64::new(0),
        alive: AtomicBool::new(true),
        stopping: AtomicBool::new(false),
    });

    let (plugin, log_host) = (spec.plugin.clone(), host.clone());
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            log_host.log(&plugin, &line);
        }
    });

    let (plugin, b) = (spec.plugin.clone(), backend.clone());
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if line.trim().is_empty() {
                continue;
            }
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                host.log(&plugin, &line); // a stray print: into the log
                continue;
            };
            let id = message.get("id").cloned();
            match message.get("method").and_then(|m| m.as_str()) {
                // Its answer to one of ours
                None => {
                    let Some(id) = id.as_ref().and_then(|i| i.as_u64()) else { continue };
                    let answer = match message.get("error") {
                        Some(e) => Err(e.get("message").and_then(|m| m.as_str()).map(String::from).unwrap_or_else(|| e.to_string())),
                        None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
                    };
                    if let Some(tx) = b.pending.lock().unwrap().remove(&id) {
                        let _ = tx.send(answer);
                    }
                }
                // A request or an event of its own
                Some(method) => {
                    let params = message.get("params").cloned().unwrap_or(Value::Null);
                    let (host, b, plugin, method) = (host.clone(), b.clone(), plugin.clone(), method.to_string());
                    std::thread::spawn(move || {
                        let answer = host.from_backend(&plugin, &method, params);
                        if let Some(id) = id {
                            let reply = match answer {
                                Ok(result) => json!({ "id": id, "result": result }),
                                Err(e) => json!({ "id": id, "error": { "message": e } }),
                            };
                            let _ = b.send(&reply);
                        }
                    });
                }
            }
        }
        // Its output closed: it ended
        let _ = b.child.lock().unwrap().wait();
        b.alive.store(false, Ordering::Relaxed);
        for (_, tx) in b.pending.lock().unwrap().drain() {
            let _ = tx.send(Err("the backend stopped (its log is in Settings › Plugins)".into()));
        }
        if !b.stopping.load(Ordering::Relaxed) {
            host.log(&plugin, "the backend stopped");
        }
        backends.ended(&plugin, &b);
    });
    Ok(backend)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[derive(Default)]
    struct TestHost {
        log: Mutex<Vec<String>>,
        asked: Mutex<Vec<(String, Value)>>,
    }

    impl Host for TestHost {
        fn log(&self, _plugin: &str, text: &str) {
            self.log.lock().unwrap().push(text.to_string());
        }
        fn from_backend(&self, _plugin: &str, method: &str, params: Value) -> Result<Value, String> {
            self.asked.lock().unwrap().push((method.to_string(), params.clone()));
            match method {
                "storage.get" => Ok(json!("stored")),
                "event" => Ok(Value::Null),
                _ => Err(format!("no {method}")),
            }
        }
    }

    /// A backend written in sh (the tests need nothing else): it answers by the id it finds
    fn script(d: &TempDir, body: &str) -> Spec {
        d.file("backend.sh", body);
        Spec { plugin: "p".into(), folder: d.0.clone(), command: "sh backend.sh".into(), init: json!({ "api": 1 }) }
    }

    // Reads one request per line and answers with the id it found; `count` answers the number
    // of requests so far (so a restart shows), `boom` exits, `ask` makes a request of its own
    const ECHO: &str = r#"
n=0
echo "starting up" >&2
while IFS= read -r line; do
  n=$((n+1))
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  case "$line" in
    *'"initialize"'*) echo "{\"id\":$id,\"result\":{}}" ;;
    *'"count"'*) echo "not json, a stray print"; echo "{\"id\":$id,\"result\":$n}" ;;
    *'"fail"'*) echo "{\"id\":$id,\"error\":{\"message\":\"No such log\"}}" ;;
    *'"ask"'*) echo '{"method":"event","params":{"name":"tick","data":1}}'; echo '{"id":900,"method":"storage.get","params":{"key":"k"}}'; echo "{\"id\":$id,\"result\":\"asked\"}" ;;
    *'"shutdown"'*) echo "{\"id\":$id,\"result\":{}}"; exit 0 ;;
    *'"boom"'*) exit 3 ;;
    *'"slow"'*) sleep 2; echo "{\"id\":$id,\"result\":1}" ;;
    *'"id":900'*) echo "$line" >&2 ;;
  esac
done
"#;

    #[test]
    fn requests_answers_and_errors() {
        let d = TempDir::new("backend-calls");
        let spec = script(&d, ECHO);
        let host = Arc::new(TestHost::default());
        let b = Backends::default();
        let second = Duration::from_secs(5);
        assert_eq!(b.call(host.clone(), &spec, "count", Value::Null, second).unwrap(), json!(2), "initialize came first");
        assert_eq!(b.call(host.clone(), &spec, "count", Value::Null, second).unwrap(), json!(3), "the same backend");
        assert_eq!(b.call(host.clone(), &spec, "fail", Value::Null, second).unwrap_err(), "No such log");
        assert!(b.call(host.clone(), &spec, "slow", Value::Null, Duration::from_millis(200)).unwrap_err().contains("didn't answer slow"));
        assert_eq!(b.call(host.clone(), &spec, "ask", Value::Null, second).unwrap(), json!("asked"));
        std::thread::sleep(Duration::from_millis(300));
        let asked = host.asked.lock().unwrap().clone();
        assert_eq!(asked[0], ("event".to_string(), json!({ "name": "tick", "data": 1 })));
        assert_eq!(asked[1].0, "storage.get");
        let log = host.log.lock().unwrap().clone();
        assert!(log.contains(&"starting up".to_string()), "stderr goes to the log: {log:?}");
        assert!(log.contains(&"not json, a stray print".to_string()));
        assert!(log.iter().any(|l| l.contains(r#""result":"stored""#)), "our answer to its request: {log:?}");
        b.stop("p");
        std::thread::sleep(Duration::from_millis(300));
        assert!(!b.is_running("p"));
    }

    #[test]
    fn crashes_restart_until_three_in_a_minute() {
        let d = TempDir::new("backend-crash");
        let spec = script(&d, ECHO);
        let host = Arc::new(TestHost::default());
        let b = Backends::default();
        let t = Duration::from_secs(5);
        for _ in 0..CRASHES {
            assert!(b.call(host.clone(), &spec, "boom", Value::Null, t).unwrap_err().contains("stopped"));
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(b.call(host.clone(), &spec, "count", Value::Null, t).unwrap_err().contains("three times"));
        b.stop("p"); // forgets the crashes (as after an update)
        assert_eq!(b.call(host.clone(), &spec, "count", Value::Null, t).unwrap(), json!(2));
        b.stop_all();
    }

    #[test]
    fn a_backend_that_wont_start_says_why() {
        let d = TempDir::new("backend-nostart");
        let mut spec = script(&d, "exit 1\n");
        let host = Arc::new(TestHost::default());
        let b = Backends::default();
        assert!(b.call(host.clone(), &spec, "count", Value::Null, Duration::from_secs(5)).unwrap_err().starts_with("its backend didn't start"));
        spec.folder = d.0.join("missing");
        assert!(b.call(host, &spec, "count", Value::Null, Duration::from_secs(5)).unwrap_err().starts_with("couldn't start"));
    }
}
