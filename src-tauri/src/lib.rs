mod actions;
mod extensions;
mod packs;
mod projects;
mod runner;
mod settings;
mod terminal;
mod updater;

use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, Emitter, State};

#[derive(Serialize)]
struct Details {
    /// Your actions first, then the detected ones you haven't hidden
    actions: Vec<actions::Action>,
    /// Detected actions you hid, so they can be brought back
    hidden: Vec<actions::Action>,
    /// Toolkit packs that couldn't be used, and why
    problems: Vec<String>,
    readme: Option<String>,
    /// Extension tabs turned on for the project
    tabs: Vec<TabInfo>,
}

#[derive(Serialize)]
struct TabInfo {
    title: String,
    #[serde(flatten)]
    tab: extensions::Tab,
}

fn details(app: &AppHandle, path: &str) -> Details {
    let s = settings::load(app);
    let hidden_ids: Vec<&str> = s.hidden_actions.get(path).into_iter().flatten().map(|id| legacy_id(id)).collect();
    let custom = s.custom.get(path).cloned().unwrap_or_default().into_iter().map(|c| actions::Action {
        id: format!("custom:{}", c.id),
        label: c.name,
        command: c.command,
        source: "custom".into(),
        group: "yours".into(),
        description: None,
        confirm: c.confirm,
        tmux: c.tmux,
    });
    let toolkit = actions::detect(&packs::load(), Path::new(path));
    let (hidden, shown): (Vec<_>, Vec<_>) =
        toolkit.actions.into_iter().partition(|a| hidden_ids.contains(&a.id.as_str()));
    Details {
        actions: custom.chain(shown).collect(),
        hidden,
        problems: toolkit.problems,
        readme: projects::readme(Path::new(path)),
        tabs: s.extensions.get(path).into_iter().flatten().map(|t| TabInfo { title: t.title(), tab: t.complete() }).collect(),
    }
}

/// Ids of hidden actions from before toolkit packs, where they changed
fn legacy_id(id: &str) -> &str {
    if id == "pytest:pytest" { "python:pytest" } else { id }
}

#[derive(Serialize)]
struct ProjectList {
    projects: Vec<projects::Project>,
    roots: Vec<String>,
    last: Option<String>,
    avatar: String,
}

fn project_list(s: &settings::Settings) -> ProjectList {
    ProjectList {
        projects: projects::list(&s.roots, &s.added, &s.hidden, &s.pinned, &packs::load().packs),
        roots: s.roots.clone(),
        last: s.last.clone(),
        avatar: s.avatar.clone(),
    }
}

#[tauri::command]
async fn list_projects(app: AppHandle) -> ProjectList {
    project_list(&settings::load(&app))
}

/// Change the project list settings; returns the new list.
#[tauri::command]
async fn edit_projects(app: AppHandle, change: String, path: String) -> Result<ProjectList, String> {
    let s = settings::update(&app, |s| match change.as_str() {
        "add" => settings::add_project(s, path),
        "remove" => settings::remove_project(s, path),
        "unhide" => settings::unhide_project(s, &path),
        "add-root" => settings::add_root(s, path),
        "remove-root" => settings::remove_root(s, &path),
        "pin" => settings::toggle(&mut s.pinned, path),
        "last" => s.last = Some(path),
        "avatar" => s.avatar = path, // "path" is the character here
        _ => {}
    })?;
    Ok(project_list(&s))
}

/// An image a README shows, from the project's folder, as a data URL
#[tauri::command]
async fn readme_image(path: String, src: String) -> Result<String, String> {
    projects::readme_image(Path::new(&path), &src)
}

#[tauri::command]
async fn project_details(app: AppHandle, path: String) -> Details {
    details(&app, &path)
}

/// Change a project's toolkit; returns the new details.
/// change: "save" (add/edit a custom action), "delete", "hide" or "unhide" (detected actions)
#[tauri::command]
async fn edit_actions(
    app: AppHandle,
    path: String,
    change: String,
    id: String,
    action: Option<settings::CustomAction>,
) -> Result<Details, String> {
    settings::update(&app, |s| match change.as_str() {
        "save" => {
            if let Some(a) = action {
                settings::save_action(s, path.clone(), a);
            }
        }
        "delete" => settings::delete_action(s, &path, id.trim_start_matches("custom:")),
        "hide" => settings::toggle(s.hidden_actions.entry(path.clone()).or_default(), id),
        "unhide" => {
            if let Some(list) = s.hidden_actions.get_mut(&path) {
                list.retain(|h| h != &id);
            }
        }
        _ => {}
    })?;
    Ok(details(&app, &path))
}

#[tauri::command]
fn run_action(app: AppHandle, runs: State<'_, runner::Runs>, path: String, command: String, label: String) -> Result<u64, String> {
    runner::start(app, &runs, &path, &command, &label)
}

#[tauri::command]
async fn open_in_tmux(path: String, name: String) -> Result<String, String> {
    terminal::open(&path, &name)
}

/// Clone or pull the toolkit packs repository; returns a short message
#[tauri::command]
async fn update_packs() -> Result<String, String> {
    packs::update()
}

/// Run a toolkit action in a window of the project's tmux session
#[tauri::command]
async fn run_in_tmux(path: String, name: String, window: String, command: String) -> Result<String, String> {
    terminal::run_in_window(&path, &name, &window, &command)
}

#[tauri::command]
fn extensions_available() -> &'static [extensions::Extension] {
    extensions::AVAILABLE
}

/// A new tab of an extension, with its default setup (not saved yet)
#[tauri::command]
fn new_tab(extension: String) -> Result<extensions::Tab, String> {
    extensions::Tab::new(&extension).ok_or_else(|| format!("no extension {extension}"))
}

/// Turn an extension on for a project ("add"), save a tab's setup ("save") or turn it off
/// ("remove"); returns the new details.
#[tauri::command]
async fn edit_tab(app: AppHandle, path: String, change: String, index: usize, tab: Option<extensions::Tab>) -> Result<Details, String> {
    settings::update(&app, |s| settings::edit_tab(s, &path, &change, index, tab))?;
    Ok(details(&app, &path))
}

// ------------------------------------------------------------ logs extension

use extensions::logs;

/// What's wrong with a setup (empty when it's fine)
#[tauri::command]
fn logs_check(setup: logs::Setup) -> Vec<String> {
    setup.problems()
}

#[derive(Serialize)]
struct LogList {
    files: Vec<logs::files::LogFile>,
    /// Log folders that don't exist
    missing: Vec<String>,
}

#[tauri::command]
async fn logs_list(path: String, setup: logs::Setup) -> LogList {
    let (files, missing) = logs::files::list(Path::new(&path), &setup);
    LogList { files, missing }
}

#[tauri::command]
async fn logs_open(file: String, setup: logs::Setup) -> Result<logs::files::Log, String> {
    logs::files::open(Path::new(&file), &setup)
}

#[tauri::command]
async fn logs_summary(file: String, setup: logs::Setup) -> Result<logs::files::Summary, String> {
    logs::files::summary(Path::new(&file), &setup)
}

/// A log's size, to notice it growing (live tail)
#[tauri::command]
fn logs_size(file: String) -> Option<u64> {
    std::fs::metadata(file).ok().map(|m| m.len())
}

// ------------------------------------------------------------ agents extension

use extensions::agents;

#[derive(Serialize)]
struct AgentList {
    sessions: Vec<agents::sessions::Session>,
    /// The project's subagents, then yours (when the setup says so)
    defined: Vec<agents::defined::Defined>,
    /// The project's skills, then yours (when the setup says so)
    skills: Vec<agents::skills::Skill>,
    /// Where the sessions are read from
    dir: String,
}

#[tauri::command]
async fn agents_list(path: String, setup: agents::Setup) -> Result<AgentList, String> {
    let home = agents::claude_home().ok_or("no home folder")?;
    let dir = agents::sessions_dir(&home, Path::new(&path));
    let mut defined = agents::defined::list(&Path::new(&path).join(".claude/agents"), "project");
    if setup.user_agents {
        defined.extend(agents::defined::list(&home.join("agents"), "user"));
    }
    let mut skills = agents::skills::list(&Path::new(&path).join(".claude/skills"), "project");
    if setup.user_agents {
        skills.extend(agents::skills::list(&home.join("skills"), "user"));
    }
    Ok(AgentList { sessions: agents::sessions::list(&dir), defined, skills, dir: dir.to_string_lossy().to_string() })
}

/// Start Claude Code as one of the agents or with a skill, in the project's "claude" tmux
/// window, then show that window. A window already running Claude is left alone (one session
/// at a time) and shown instead.
#[tauri::command]
async fn agents_start(path: String, name: String, kind: String, agent: String, task: String) -> Result<String, String> {
    let command = agents::start_command(&kind, &agent, &task)?;
    let message = terminal::run_in_window(&path, &name, "claude", &command)?;
    terminal::open_window(&path, &name, "claude")?;
    Ok(message)
}

/// The Claude Code sessions running now, in any project (for the avatar)
#[tauri::command]
async fn claude_live() -> Vec<agents::live::Live> {
    agents::claude_home().map(|h| agents::live::list(&h)).unwrap_or_default()
}

/// A session's or subagent's conversation
#[tauri::command]
async fn agents_transcript(file: String) -> Result<Vec<agents::transcript::Entry>, String> {
    agents::transcript::read(Path::new(&file))
}

// ------------------------------------------------------------ pull requests extension

use extensions::prs;

#[tauri::command]
async fn prs_list(path: String, setup: prs::Setup) -> Result<prs::PrList, String> {
    let repo = prs::repo(Path::new(&path), &setup)?;
    prs::fetch(&repo, &setup)
}

/// A PR's diff for the review page (without checking it out)
#[tauri::command]
async fn prs_diff(path: String, setup: prs::Setup, number: u64) -> Result<String, String> {
    let repo = prs::repo(Path::new(&path), &setup)?;
    prs::diff(&repo, number)
}

/// Open a PR in the browser, marking its notification read
#[tauri::command]
async fn prs_open(app: AppHandle, url: String, notification: Option<String>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(&url, None::<&str>).map_err(|e| e.to_string())?;
    if let Some(thread) = notification {
        prs::mark_read(&thread)?;
    }
    Ok(())
}

// ------------------------------------------------------------ git extension (read-only)

use extensions::git;

#[tauri::command]
async fn git_status(path: String) -> Result<git::Status, String> {
    git::status(Path::new(&path))
}

#[tauri::command]
async fn git_diff(path: String, file: String, staged: bool, untracked: bool) -> Result<String, String> {
    git::diff(Path::new(&path), &file, staged, untracked)
}

#[tauri::command]
async fn git_log(path: String, count: usize) -> Result<Vec<git::Commit>, String> {
    git::log(Path::new(&path), count)
}

#[tauri::command]
async fn git_show(path: String, hash: String) -> Result<String, String> {
    git::show(Path::new(&path), &hash)
}

#[tauri::command]
async fn git_branches(path: String) -> Result<git::Branches, String> {
    git::branches(Path::new(&path))
}

/// A whole diff for the review page: "changes", a commit hash, or a stash name
#[tauri::command]
async fn git_review(path: String, what: String) -> Result<String, String> {
    git::review(Path::new(&path), &what)
}

#[tauri::command]
async fn git_stash(path: String, name: String) -> Result<String, String> {
    git::stash(Path::new(&path), &name)
}

// ------------------------------------------------------------ updates

fn version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

/// A newer release than this one, if there is one (none for dev builds)
#[tauri::command]
async fn check_update(app: AppHandle) -> Result<Option<updater::Update>, String> {
    updater::check(&version(&app))
}

/// Download, check and install the latest release; returns its version (runs after a restart)
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<String, String> {
    updater::install(&version(&app))
}

#[tauri::command]
fn restart(app: AppHandle) {
    app.restart();
}

/// Look for a new release a little after start, then every few hours
fn watch_for_updates(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(10));
        match updater::check(&version(&app)) {
            Ok(Some(update)) => {
                let _ = app.emit("update-available", update);
            }
            Ok(None) => {}
            Err(e) => eprintln!("thumbdeck: checking for updates: {e}"),
        }
        std::thread::sleep(std::time::Duration::from_secs(6 * 3600));
    });
}

#[tauri::command]
fn stop_run(runs: State<'_, runner::Runs>, id: u64) {
    runner::stop(&runs, id);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    runner::preload_env();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(runner::Runs::default())
        .invoke_handler(tauri::generate_handler![list_projects, edit_projects, project_details, edit_actions, run_action, stop_run, open_in_tmux, run_in_tmux, update_packs,
            extensions_available, new_tab, edit_tab, logs_check, logs_list, logs_open,
            logs_summary, logs_size, check_update, install_update, restart, readme_image, agents_list, agents_transcript, agents_start, prs_list, prs_open, claude_live,
            git_status, git_diff, git_log, git_show, git_branches, git_stash, git_review, prs_diff])
        .setup(|app| {
            watch_for_updates(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
