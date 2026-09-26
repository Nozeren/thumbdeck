//! Opening a project in tmux: one session per project, in the terminal you already use.

use std::process::{Command, Output, Stdio};

/// tmux doesn't allow "." or ":" in session names
pub fn session_name(project: &str) -> String {
    project.chars().map(|c| if c == '.' || c == ':' { '_' } else { c }).collect()
}

fn tmux(args: &[&str]) -> std::io::Result<Output> {
    Command::new("tmux")
        .args(args)
        .env_clear()
        .envs(crate::runner::shell_env().iter().cloned())
        .stdin(Stdio::null())
        .output()
}

fn ok(out: std::io::Result<Output>) -> bool {
    out.is_ok_and(|o| o.status.success())
}

/// Switch the terminal to the project's tmux session, creating it first if needed
/// (Neovim in the first window, a shell in the second), then bring the terminal forward.
/// Returns a short message for the app to show.
pub fn open(path: &str, name: &str) -> Result<String, String> {
    let session = session_name(name);
    let target = format!("={session}"); // "=": exact name, not a prefix match

    if !ok(tmux(&["has-session", "-t", &target])) {
        if !ok(tmux(&["new-session", "-d", "-s", &session, "-c", path])) {
            return Err("couldn't start tmux (is it installed?)".into());
        }
        // Neovim runs inside the shell, so quitting it leaves you in the project folder
        let _ = tmux(&["send-keys", "-t", &format!("{target}:^"), "nvim", "Enter"]);
        let _ = tmux(&["new-window", "-d", "-t", &format!("{target}:"), "-c", path]);
    }

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
    fn session_names_have_no_dots_or_colons() {
        assert_eq!(super::session_name("steplink.nvim"), "steplink_nvim");
        assert_eq!(super::session_name("a:b"), "a_b");
        assert_eq!(super::session_name("DuoBudget-Django"), "DuoBudget-Django");
    }
}
