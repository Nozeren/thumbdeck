mod actions;
mod projects;
mod runner;

use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, State};

#[derive(Serialize)]
struct Details {
    actions: Vec<actions::Action>,
    readme: Option<String>,
}

#[tauri::command]
async fn list_projects() -> Vec<projects::Project> {
    projects::scan(&projects::default_roots())
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
        .manage(runner::Runs::default())
        .invoke_handler(tauri::generate_handler![list_projects, project_details, run_action, stop_run])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
