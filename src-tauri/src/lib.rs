mod branch;
mod extensions;
mod packs;
mod plugins;
mod projects;
mod runner;
mod settings;
mod terminal;
#[cfg(test)]
mod testutil;
mod updater;

use plugins::toolkit::{self, Action, Provider};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use tauri::{AppHandle, Emitter, State};

#[derive(Serialize)]
struct Details {
    /// Your actions first, then the detected ones you haven't hidden
    actions: Vec<Action>,
    /// Detected actions you hid, so they can be brought back
    hidden: Vec<Action>,
    /// Toolkit packs and plugins that couldn't be used, and why
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
    /// A plugin's tab: its frame
    #[serde(skip_serializing_if = "Option::is_none")]
    frame: Option<plugins::FrameInfo>,
    /// A plugin's tab that can't show, and why
    #[serde(skip_serializing_if = "Option::is_none")]
    missing: Option<String>,
}

fn tab_info(t: &extensions::Tab, loaded: &[plugins::Plugin]) -> TabInfo {
    let Some(plugin) = &t.plugin else {
        return TabInfo { title: t.title(), tab: t.complete(), frame: None, missing: None };
    };
    match plugins::tab_frame(loaded, plugin, &t.extension, &t.setup) {
        Ok((frame, setup)) => {
            let title = setup["title"].as_str().unwrap_or(&frame.name).to_string();
            TabInfo { title, tab: extensions::Tab { setup, ..t.clone() }, frame: Some(frame), missing: None }
        }
        Err(why) => {
            let title = t.setup.get("title").and_then(|v| v.as_str()).unwrap_or(&t.extension).to_string();
            TabInfo { title, tab: t.clone(), frame: None, missing: Some(why) }
        }
    }
}

fn details(app: &AppHandle, path: &str) -> Details {
    let s = settings::load(app);
    let hidden_ids: Vec<&str> = s.hidden_actions.get(path).into_iter().flatten().map(|id| legacy_id(id)).collect();
    let custom = s.custom.get(path).cloned().unwrap_or_default().into_iter().map(|c| Action {
        id: format!("custom:{}", c.id),
        label: c.name,
        command: c.command,
        source: "custom".into(),
        group: "yours".into(),
        description: None,
        confirm: c.confirm,
        tmux: c.tmux,
    });
    let (providers, mut problems) = providers(&s);
    let toolkit = toolkit::detect(&providers, Path::new(path));
    problems.extend(toolkit.problems);
    let (hidden, shown): (Vec<_>, Vec<_>) =
        toolkit.actions.into_iter().partition(|a| hidden_ids.contains(&a.id.as_str()));
    Details {
        actions: custom.chain(shown).collect(),
        hidden,
        problems,
        readme: projects::readme(Path::new(path)),
        tabs: {
            let loaded = plugins::load(&s.plugins);
            s.extensions.get(path).into_iter().flatten().map(|t| tab_info(t, &loaded)).collect()
        },
    }
}

/// What brings buttons to the Toolkit: the packs, then the working plugins (a plugin replaces
/// the pack with its id); with what went wrong
fn providers(s: &settings::Settings) -> (Vec<Provider>, Vec<String>) {
    let packs = packs::load();
    let mine = plugins::providers(&plugins::load(&s.plugins));
    let mut all: Vec<Provider> = packs.packs.into_iter().filter(|p| !mine.iter().any(|m| m.id == p.id)).collect();
    all.extend(mine);
    let (all, more) = toolkit::resolve(all);
    (all, packs.problems.into_iter().chain(more).collect())
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
        projects: projects::list(&s.roots, &s.added, &s.hidden, &s.pinned, &providers(s).0),
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

/// The tabs a project can have: the built-in extensions, then the plugins' tabs
#[tauri::command]
async fn tabs_available(app: AppHandle, path: String) -> Vec<plugins::Addable> {
    let builtin = extensions::AVAILABLE.iter().map(|e| plugins::Addable { id: e.id.into(), name: e.name.into(), description: e.description.into() });
    let loaded = plugins::load(&settings::load(&app).plugins);
    builtin.chain(plugins::addable_tabs(&loaded, Path::new(&path))).collect()
}

/// A new tab, with its default setup (not saved yet): a built-in extension ("logs") or a
/// plugin's tab ("plugin:<plugin>:<tab>", with its frame info for the setup form)
#[tauri::command]
async fn new_tab(app: AppHandle, extension: String) -> Result<TabInfo, String> {
    let loaded = plugins::load(&settings::load(&app).plugins);
    let tab = match extension.strip_prefix("plugin:").and_then(|r| r.split_once(':')) {
        Some((plugin, tab)) => {
            let (_, setup) = plugins::tab_frame(&loaded, plugin, tab, &serde_json::Value::Null)?;
            extensions::Tab { extension: tab.into(), plugin: Some(plugin.into()), setup }
        }
        None => extensions::Tab::new(&extension).ok_or_else(|| format!("no extension {extension}"))?,
    };
    Ok(tab_info(&tab, &loaded))
}

/// Turn an extension on for a project ("add"), save a tab's setup ("save") or turn it off
/// ("remove"); returns the new details.
#[tauri::command]
async fn edit_tab(app: AppHandle, path: String, change: String, index: usize, tab: Option<extensions::Tab>) -> Result<Details, String> {
    settings::update(&app, |s| settings::edit_tab(s, &path, &change, index, tab))?;
    Ok(details(&app, &path))
}

// ------------------------------------------------------------ plugins

fn plugin_infos(s: &settings::Settings) -> Vec<plugins::Info> {
    plugins::load(&s.plugins).iter().map(|p| plugins::info(p, &s.plugin_settings)).collect()
}

fn plugins_dir() -> Result<std::path::PathBuf, String> {
    plugins::install::plugins_dir().ok_or_else(|| "no home folder".to_string())
}

#[tauri::command]
async fn plugins_list(app: AppHandle) -> Vec<plugins::Info> {
    plugin_infos(&settings::load(&app))
}

/// Install a plugin from a git URL (`url#folder` for one in a folder), or link a folder
#[tauri::command]
async fn plugin_add(app: AppHandle, source: String, link: bool) -> Result<Vec<plugins::Info>, String> {
    let s = settings::load(&app);
    let installed = if link {
        plugins::install::link(Path::new(source.trim()))?
    } else {
        plugins::install::install(&source, &plugins_dir()?)?
    };
    if s.plugins.iter().any(|p| p.id == installed.id) {
        if !link {
            let _ = plugins::install::remove(&installed, &plugins_dir()?);
        }
        return Err(format!("a plugin called {} is installed already", installed.id));
    }
    let s = settings::update(&app, |s| settings::add_plugin(s, installed))?;
    Ok(plugin_infos(&s))
}

/// Turn a plugin on ("enable") or off ("disable"), move it to its newest release ("update"),
/// or delete it with its settings ("remove")
#[tauri::command]
async fn plugin_edit(app: AppHandle, id: String, change: String) -> Result<Vec<plugins::Info>, String> {
    let s = settings::load(&app);
    let p = s.plugins.iter().find(|p| p.id == id).cloned().ok_or_else(|| format!("no plugin {id}"))?;
    let s = match change.as_str() {
        "enable" | "disable" => settings::update(&app, |s| settings::add_plugin(s, plugins::install::Installed { enabled: change == "enable", ..p }))?,
        "update" => {
            let updated = plugins::install::update(&p, &plugins_dir()?)?;
            settings::update(&app, |s| settings::add_plugin(s, updated))?
        }
        "remove" => {
            plugins::install::remove(&p, &plugins_dir()?)?;
            settings::update(&app, |s| settings::remove_plugin(s, &id))?
        }
        _ => return Err(format!("can't {change} a plugin")),
    };
    Ok(plugin_infos(&s))
}

#[tauri::command]
async fn plugin_save_settings(app: AppHandle, id: String, settings: serde_json::Value) -> Result<Vec<plugins::Info>, String> {
    let s = settings::update(&app, |s| {
        s.plugin_settings.insert(id, settings);
    })?;
    Ok(plugin_infos(&s))
}

/// A frame's call to the page API (the part that runs here: files, commands, storage,
/// settings, notifications). The page attaches the frame's plugin and project.
#[tauri::command]
async fn plugin_call(app: AppHandle, plugin: String, project: Option<String>, method: String, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let s = settings::load(&app);
    let loaded = plugins::load(&s.plugins);
    let (_, m) = plugins::find(&loaded, &plugin).ok_or_else(|| format!("the plugin {plugin} isn't on"))?;
    match method.as_str() {
        "ui.notify" => {
            use tauri_plugin_notification::NotificationExt;
            let title = params["title"].as_str().unwrap_or_default();
            let body = params["body"].as_str().unwrap_or_default();
            app.notification().builder().title(title).body(body).show().map_err(|e| e.to_string())?;
            Ok(serde_json::Value::Null)
        }
        "ui.openUrl" => {
            use tauri_plugin_opener::OpenerExt;
            let url = params["url"].as_str().unwrap_or_default();
            if !(url.starts_with("https://") || url.starts_with("http://") || url.starts_with("mailto:")) {
                return Err(format!("{url} isn't a web address"));
            }
            app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())?;
            Ok(serde_json::Value::Null)
        }
        _ => {
            let settings = plugins::manifest::complete(&m.settings, s.plugin_settings.get(&plugin).unwrap_or(&serde_json::Value::Null));
            tauri::async_runtime::spawn_blocking(move || {
                let caller = plugins::api::Caller { plugin: &plugin, project: project.as_deref().map(Path::new), settings };
                plugins::api::call(&caller, &method, params)
            })
            .await
            .map_err(|e| e.to_string())?
        }
    }
}

/// A line for a plugin's log (its page's console)
#[tauri::command]
fn plugin_log(logs: State<'_, plugins::log::Logs>, plugin: String, level: String, text: String) {
    logs.add(&plugin, &level, &text);
}

#[tauri::command]
fn plugin_log_lines(logs: State<'_, plugins::log::Logs>, plugin: String) -> Vec<plugins::log::Line> {
    logs.lines(&plugin)
}

#[tauri::command]
fn plugin_log_clear(logs: State<'_, plugins::log::Logs>, plugin: String) {
    logs.clear(&plugin);
}

/// Plugins with a newer release: id -> its tag ("the latest commit" for one without tags)
fn plugin_updates(s: &settings::Settings) -> HashMap<String, String> {
    let Ok(dir) = plugins_dir() else { return HashMap::new() };
    s.plugins
        .iter()
        .filter_map(|p| match plugins::install::newer(p, &dir) {
            Ok(next) => Some((p.id.clone(), next?)),
            Err(e) => {
                eprintln!("thumbdeck: checking {} for updates: {e}", p.id);
                None
            }
        })
        .collect()
}

#[tauri::command]
async fn plugins_check_updates(app: AppHandle) -> HashMap<String, String> {
    plugin_updates(&settings::load(&app))
}

/// Look for newer plugin releases a little after start
fn watch_plugin_updates(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(15));
        let updates = plugin_updates(&settings::load(&app));
        if !updates.is_empty() {
            let _ = app.emit("plugin-updates", updates);
        }
    });
}

/// `thumbdeck plugin check <folder>`: runs instead of the app; returns the exit code
pub fn cli() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1..).unwrap_or_default() {
        [plugin, check, rest @ ..] if plugin == "plugin" && check == "check" => {
            Some(plugins::check_command(Path::new(rest.first().map(String::as_str).unwrap_or("."))))
        }
        [plugin, ..] if plugin == "plugin" => {
            eprintln!("usage: thumbdeck plugin check <folder>");
            Some(2)
        }
        _ => None,
    }
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

// ------------------------------------------------------------ the status line

/// The project's branch and how many files changed, for the status line
#[tauri::command]
async fn branch_status(path: String) -> Result<branch::Branch, String> {
    branch::status(Path::new(&path))
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
        .manage(plugins::log::Logs::default())
        .register_uri_scheme_protocol("plugin", |ctx, request| {
            let loaded = plugins::load(&settings::load(ctx.app_handle()).plugins);
            plugins::frame::serve(&request, |id| plugins::find(&loaded, id).map(|(p, _)| p.folder.clone()))
        })
        .invoke_handler(tauri::generate_handler![list_projects, edit_projects, project_details, edit_actions, run_action, stop_run, open_in_tmux, run_in_tmux, update_packs,
            tabs_available, new_tab, plugin_call, plugin_log, plugin_log_lines, plugin_log_clear, edit_tab, logs_check, logs_list, logs_open,
            logs_summary, logs_size, check_update, install_update, restart, readme_image, agents_list, agents_transcript, agents_start, prs_list, prs_open, claude_live,
            branch_status, prs_diff,
            plugins_list, plugin_add, plugin_edit, plugin_save_settings, plugins_check_updates])
        .setup(|app| {
            watch_for_updates(app.handle().clone());
            watch_plugin_updates(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
