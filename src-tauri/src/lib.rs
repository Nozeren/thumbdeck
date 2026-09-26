mod actions;
mod projects;
mod runner;
mod settings;

use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, State};

#[derive(Serialize)]
struct Details {
    actions: Vec<actions::Action>,
    readme: Option<String>,
}

#[derive(Serialize)]
struct ProjectList {
    projects: Vec<projects::Project>,
    roots: Vec<String>,
    last: Option<String>,
}

fn project_list(s: &settings::Settings) -> ProjectList {
    ProjectList {
        projects: projects::list(&s.roots, &s.added, &s.hidden, &s.pinned),
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
async fn project_details(path: String) -> Details {
    let dir = Path::new(&path);
    Details { actions: actions::detect(dir), readme: projects::readme(dir) }
}

#[tauri::command]
fn run_action(app: AppHandle, runs: State<'_, runner::Runs>, path: String, command: String) -> Result<u64, String> {
    runner::start(app, &runs, &path, &command)
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
        .manage(runner::Runs::default())
        .invoke_handler(tauri::generate_handler![list_projects, edit_projects, project_details, run_action, stop_run])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
