//! Opening a project in tmux: one session per project, in the terminal you already use.

use std::process::{Command, Output, Stdio};

/// The project's Python virtual environment folder, if it has one (".venv" or "venv")
fn venv(path: &str) -> Option<&'static str> {
    [".venv", "venv"].into_iter().find(|v| std::path::Path::new(path).join(v).join("bin/activate").exists())
}

/// What to type in a new window before anything else: activate the venv, if there is one
fn activate_prefix(path: &str) -> String {
    let fish = std::env::var("SHELL").is_ok_and(|s| s.ends_with("fish"));
    match venv(path) {
        Some(v) if fish => format!("source {v}/bin/activate.fish; and "),
        Some(v) => format!("source {v}/bin/activate && "),
        None => String::new(),
    }
}

/// tmux doesn't allow "." or ":" in session names
pub fn session_name(project: &str) -> String {
    project.chars().map(|c| if c == '.' || c == ':' { '_' } else { c }).collect()
}

fn tmux(args: &[&str]) -> std::io::Result<Output> {
    // Tests use a tmux server of their own
    let server: &[&str] = if cfg!(test) { &["-L", "thumbdeck-test"] } else { &[] };
    Command::new("tmux")
        .args(server)
        .args(args)
        .env_clear()
        .envs(crate::runner::shell_env().iter().cloned())
        .stdin(Stdio::null())
        .output()
}

fn ok(out: std::io::Result<Output>) -> bool {
    out.is_ok_and(|o| o.status.success())
}

/// Start the project's tmux session unless it's there: Neovim on the project folder in the
/// first window, a shell in the second.
fn ensure_session(path: &str, session: &str) -> Result<(), String> {
    let target = format!("={session}"); // "=": exact name, not a prefix match
    if ok(tmux(&["has-session", "-t", &target])) {
        return Ok(());
    }
    if !ok(tmux(&["new-session", "-d", "-s", session, "-c", path])) {
        return Err("couldn't start tmux (is it installed?)".into());
    }
    // Window 1: Neovim, run from the shell so quitting it leaves a shell there. Both windows
    // activate the project's venv first, for Python projects.
    let prefix = activate_prefix(path);
    let _ = tmux(&["send-keys", "-t", &format!("{target}:^"), &format!("{prefix}nvim ."), "Enter"]);
    // Window 2: a shell
    let window = tmux(&["new-window", "-d", "-P", "-F", "#{window_id}", "-t", &format!("{target}:"), "-c", path])
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    if !prefix.is_empty() && !window.is_empty() {
        let activate = prefix.trim_end_matches(" && ").trim_end_matches("; and ");
        let _ = tmux(&["send-keys", "-t", &window, &format!("{activate} && clear"), "Enter"]);
    }
    Ok(())
}

/// Switch the terminal to the project's tmux session, creating it first if needed, then bring
/// the terminal forward. Returns a short message for the app to show.
pub fn open(path: &str, name: &str) -> Result<String, String> {
    let session = session_name(name);
    let target = format!("={session}");
    ensure_session(path, &session)?;

    let clients = tmux(&["list-clients", "-F", "#{client_name}"])
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(String::from).collect::<Vec<_>>())
        .unwrap_or_default();

    if let Some(client) = clients.first() {
        tmux(&["switch-client", "-c", client, "-t", &target]).map_err(|e| e.to_string())?;
        focus_terminal();
        Ok(format!("switched to tmux session {session}"))
    } else {
        launch_terminal(&session)?;
        Ok(format!("opened kitty on tmux session {session}"))
    }
}

/// Run a command in a window of the project's tmux session (made if needed), without leaving
/// thumbdeck. A window that's busy (not at a shell prompt) is left alone, so pressing
/// "runserver" twice doesn't type into the running server.
pub fn run_in_window(path: &str, name: &str, window: &str, command: &str) -> Result<String, String> {
    let session = session_name(name);
    let target = format!("={session}");
    ensure_session(path, &session)?;

    let list = tmux(&["list-windows", "-t", &target, "-F", "#{window_id}\t#{window_name}\t#{pane_current_command}"])
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let existing = list.lines().find_map(|l| match l.splitn(3, '\t').collect::<Vec<_>>()[..] {
        [id, n, cmd] if n == window => Some((id.to_string(), cmd.to_string())),
        _ => None,
    });
    let id = match existing {
        Some((_, cmd)) if !is_shell(&cmd) => {
            return Ok(format!("tmux window '{window}' is busy ({cmd}), left it alone"));
        }
        Some((id, _)) => id,
        None => {
            let out = tmux(&["new-window", "-d", "-P", "-F", "#{window_id}", "-n", window, "-t", &format!("{target}:"), "-c", path])
                .map_err(|e| e.to_string())?;
            let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !out.status.success() || id.is_empty() {
                return Err(format!("couldn't make tmux window '{window}'"));
            }
            id
        }
    };
    let keys = format!("{}{command}", activate_prefix(path));
    if !ok(tmux(&["send-keys", "-t", &id, &keys, "Enter"])) {
        return Err(format!("couldn't type into tmux window '{window}'"));
    }
    Ok(format!("started in tmux: {session} › {window}"))
}

/// Show a window of the project's tmux session in the terminal (switching to the session first)
pub fn open_window(path: &str, name: &str, window: &str) -> Result<String, String> {
    let message = open(path, name)?;
    let _ = tmux(&["select-window", "-t", &format!("={}:{window}", session_name(name))]);
    Ok(message)
}

/// Whether a pane's current command is a shell waiting at its prompt
fn is_shell(command: &str) -> bool {
    let command = command.trim_start_matches('-'); // login shells: "-zsh"
    let own = crate::runner::shell_env().iter().find(|(k, _)| k == "SHELL").map(|(_, v)| v.rsplit('/').next().unwrap_or(v));
    ["bash", "zsh", "fish", "sh", "dash", "ksh", "tcsh", "nu"].contains(&command) || own == Some(command)
}

/// Bring the terminal window to the front.
fn focus_terminal() {
    if cfg!(target_os = "macos") {
        let _ = Command::new("osascript").args(["-e", "tell application \"kitty\" to activate"]).status();
    } else if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        let _ = Command::new("hyprctl").args(["dispatch", "focuswindow", "class:kitty"]).output();
    }
}

/// No terminal attached to tmux: open kitty on the session.
fn launch_terminal(session: &str) -> Result<(), String> {
    let attach = ["tmux", "attach-session", "-t", session];
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.args(["-na", "kitty", "--args"]).args(attach);
        c
    } else {
        let mut c = Command::new("kitty");
        c.args(attach);
        c
    };
    cmd.env_clear()
        .envs(crate::runner::shell_env().iter().cloned())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("couldn't open kitty: {e}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn venv_is_activated_only_when_the_project_has_one() {
        let dir = std::env::temp_dir().join(format!("thumbdeck-venv-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".venv/bin")).unwrap();
        let path = dir.to_string_lossy().to_string();
        assert_eq!(super::activate_prefix(&path), String::new(), "no activate script yet");
        std::fs::write(dir.join(".venv/bin/activate"), "").unwrap();
        assert!(super::activate_prefix(&path).starts_with("source .venv/bin/activate"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Runs a real tmux (its own server, see tmux()); skipped when tmux isn't installed.
    #[test]
    fn commands_run_in_a_named_window_and_a_busy_window_is_left_alone() {
        if !super::ok(std::process::Command::new("tmux").arg("-V").output()) {
            return;
        }
        let dir = std::env::temp_dir().join(format!("thumbdeck-tmux-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.to_string_lossy().to_string();
        let windows = || {
            super::tmux(&["list-windows", "-t", "=proj_x", "-F", "#{window_name} #{pane_current_command}"])
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_default()
        };

        let first = super::run_in_window(&path, "proj.x", "server", "sleep 30");
        assert!(first.as_ref().is_ok_and(|m| m.contains("started")), "{first:?}");
        assert_eq!(windows().lines().count(), 3, "nvim, shell, server: {}", windows());
        // Wait for the shell to start sleep
        for _ in 0..50 {
            if windows().contains("server sleep") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let second = super::run_in_window(&path, "proj.x", "server", "sleep 30");
        assert!(second.as_ref().is_ok_and(|m| m.contains("busy")), "{second:?} / {}", windows());
        assert_eq!(windows().lines().count(), 3, "no second server window");

        let _ = super::tmux(&["kill-server"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn shells_are_told_from_running_commands() {
        assert!(super::is_shell("zsh") && super::is_shell("-bash") && super::is_shell("fish"));
        assert!(!super::is_shell("python3") && !super::is_shell("sleep") && !super::is_shell("nvim"));
    }

    #[test]
    fn session_names_have_no_dots_or_colons() {
        assert_eq!(super::session_name("steplink.nvim"), "steplink_nvim");
        assert_eq!(super::session_name("a:b"), "a_b");
        assert_eq!(super::session_name("DuoBudget-Django"), "DuoBudget-Django");
    }
}
