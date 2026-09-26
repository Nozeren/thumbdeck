mod actions;
mod packs;
mod projects;
mod runner;
mod settings;
mod terminal;

use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, State};

#[derive(Serialize)]
struct Details {
    /// Your actions first, then the detected ones you haven't hidden
    actions: Vec<actions::Action>,
    /// Detected actions you hid, so they can be brought back
    hidden: Vec<actions::Action>,
    /// Toolkit packs that couldn't be used, and why
    problems: Vec<String>,
    readme: Option<String>,
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
}

fn project_list(s: &settings::Settings) -> ProjectList {
    ProjectList {
        projects: projects::list(&s.roots, &s.added, &s.hidden, &s.pinned, &packs::load().packs),
        roots: s.roots.clone(),
        last: s.last.clone(),
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
        _ => {}
    })?;
    Ok(project_list(&s))
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
        .invoke_handler(tauri::generate_handler![list_projects, edit_projects, project_details, edit_actions, run_action, stop_run, open_in_tmux, run_in_tmux, update_packs])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
