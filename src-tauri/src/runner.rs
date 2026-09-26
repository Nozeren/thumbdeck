//! Running toolkit actions: output streams to the window line by line, Stop ends the whole
//! process group (so e.g. Django's auto-reloader goes too).

use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter};

#[derive(Default)]
pub struct Runs {
    next_id: AtomicU64,
    /// run id -> process id (also the process group id); shared with the waiting threads
    running: Arc<Mutex<HashMap<u64, u32>>>,
}

#[derive(Serialize, Clone)]
struct Output {
    id: u64,
    line: String,
    stderr: bool,
}

#[derive(Serialize, Clone)]
struct Exit {
    id: u64,
    code: Option<i32>,
}

/// The environment of the user's interactive login shell. Apps started from a desktop
/// launcher or the macOS Dock don't get the PATH your terminal has (Homebrew, ~/.local/bin,
/// fnm, ...), so it's read once from the shell itself and used for every command.
pub fn shell_env() -> &'static Vec<(String, String)> {
    static ENV: OnceLock<Vec<(String, String)>> = OnceLock::new();
    ENV.get_or_init(|| {
        const MARKER: &str = "__THUMBDECK_ENV__";
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let output = Command::new(shell)
            .args(["-ilc", &format!("printf {MARKER}; env -0")])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output();
        let Ok(output) = output else { return std::env::vars().collect() };
        let text = String::from_utf8_lossy(&output.stdout);
        let Some((_, env)) = text.split_once(MARKER) else { return std::env::vars().collect() };
        env.split('\0')
            .filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
            .collect()
    })
}

/// Warm the environment up in the background so the first click isn't slow.
pub fn preload_env() {
    std::thread::spawn(|| {
        shell_env();
    });
}

pub fn start(app: AppHandle, runs: &Runs, dir: &str, command: &str) -> Result<u64, String> {
    let mut child = Command::new("/bin/sh")
        .args(["-c", command])
        .current_dir(dir)
        .env_clear()
        .envs(shell_env().iter().cloned())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|e| format!("could not start `{command}`: {e}"))?;

    let id = runs.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    runs.running.lock().unwrap().insert(id, child.id());

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let readers = [(stdout.map(|s| Box::new(s) as Box<dyn Read + Send>), false),
                   (stderr.map(|s| Box::new(s) as Box<dyn Read + Send>), true)];
    let mut handles = Vec::new();
    for (stream, is_err) in readers {
        let Some(stream) = stream else { continue };
        let app = app.clone();
        handles.push(std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                let _ = app.emit("run-output", Output { id, line, stderr: is_err });
            }
        }));
    }

    let running = Arc::clone(&runs.running);
    std::thread::spawn(move || {
        let code = child.wait().ok().and_then(|s| s.code());
        for h in handles {
            let _ = h.join();
        }
        running.lock().unwrap().remove(&id);
        let _ = app.emit("run-exit", Exit { id, code });
    });
    Ok(id)
}

pub fn stop(runs: &Runs, id: u64) {
    if let Some(pid) = runs.running.lock().unwrap().get(&id) {
        // Negative pid: signal the whole process group
        unsafe {
            libc::kill(-(*pid as i32), libc::SIGTERM);
        }
    }
}
