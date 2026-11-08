#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod git;
mod graph;
mod metrics;
mod sample;
mod shot;
mod watch;

use actions::{Action, ActionResult, Engine, UndoInfo};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{Emitter, Manager, State};

/// Most commits read for the graph at first (the original's "Initial Commits in Graph" minimum).
const INITIAL_COMMITS: usize = 2000;

struct AppState {
    engine: Arc<Engine>,
    screenshot: Option<PathBuf>,
    watchers: Mutex<HashMap<String, watch::RepoWatch>>,
}

#[derive(Serialize)]
struct LaunchOptions {
    screenshot: bool,
    version: String,
    metrics: metrics::Metrics,
}

#[derive(Serialize)]
struct RepoView {
    path: String,
    name: String,
    #[serde(flatten)]
    data: git::RepoData,
    graph: graph::GraphLayout,
    undo: UndoInfo,
}

fn err<E: ToString>(e: E) -> String {
    e.to_string()
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(err)
}

#[tauri::command]
fn launch_options(state: State<'_, AppState>) -> LaunchOptions {
    LaunchOptions { screenshot: state.screenshot.is_some(), version: env!("CARGO_PKG_VERSION").to_string(), metrics: metrics::metrics() }
}

#[tauri::command]
async fn open_repo(path: String, state: State<'_, AppState>) -> Result<RepoView, String> {
    let engine = state.engine.clone();
    blocking(move || {
        let p = PathBuf::from(&path);
        if !p.exists() {
            return Err(format!("The folder {path} doesn't exist"));
        }
        let root = git::repo_root(&p).ok_or_else(|| format!("{path} is not a Git repository"))?;
        let data = git::load(&root, INITIAL_COMMITS);
        let graph = graph::layout(&data.commits);
        let root_s = root.to_string_lossy().replace('/', "\\");
        Ok(RepoView {
            name: git::repo_name(&root),
            undo: engine.undo_info(&root_s),
            path: root_s,
            data,
            graph,
        })
    })
    .await?
}

#[tauri::command]
async fn commit_files(path: String, sha: String) -> Result<Vec<git::FileChange>, String> {
    blocking(move || {
        let dir = PathBuf::from(&path);
        let o = git::run(&dir, &["rev-list", "--parents", "-n1", &sha]);
        let parents: Vec<String> = o.stdout.split_whitespace().skip(1).map(|s| s.to_string()).collect();
        let c = git::Commit {
            sha: sha.clone(),
            parents,
            author: String::new(),
            email: String::new(),
            time: 0,
            summary: String::new(),
            body: String::new(),
        };
        git::commit_files(&dir, &c)
    })
    .await
}

#[tauri::command]
async fn file_diff(
    path: String,
    source: git::DiffSource,
    file: String,
    untracked: bool,
    parent: Option<String>,
) -> Result<Vec<git::DiffLine>, String> {
    blocking(move || git::file_diff(Path::new(&path), &source, &file, untracked, parent.as_deref())).await
}

#[tauri::command]
async fn run_action(path: String, action: Action, state: State<'_, AppState>) -> Result<ActionResult, String> {
    let engine = state.engine.clone();
    blocking(move || engine.perform(&path, action)).await
}

#[tauri::command]
fn activity_log(state: State<'_, AppState>) -> Vec<git::LogEntry> {
    state.engine.log_entries()
}

#[tauri::command]
async fn sample_repo() -> Result<String, String> {
    blocking(|| sample::ensure_sample().map(|p| p.to_string_lossy().into_owned())).await?
}

#[tauri::command]
async fn last_commit_message(path: String) -> Result<(String, String), String> {
    blocking(move || git::last_commit_message(Path::new(&path))).await
}

#[tauri::command]
async fn pick_folder(title: String) -> Result<Option<String>, String> {
    blocking(move || rfd::FileDialog::new().set_title(&title).pick_folder().map(|p| p.to_string_lossy().into_owned())).await
}

#[tauri::command]
async fn init_repo(parent: String, name: String, state: State<'_, AppState>) -> Result<String, String> {
    let engine = state.engine.clone();
    blocking(move || {
        let dir = PathBuf::from(&parent).join(name.trim());
        std::fs::create_dir_all(&dir).map_err(err)?;
        let o = engine.git(&dir, &["init", "-b", "main"]);
        if o.ok {
            Ok(dir.to_string_lossy().into_owned())
        } else {
            Err(o.message())
        }
    })
    .await?
}

#[tauri::command]
async fn clone_repo(url: String, parent: String, state: State<'_, AppState>) -> Result<String, String> {
    let engine = state.engine.clone();
    blocking(move || {
        let name = url.trim_end_matches('/').rsplit(['/', '\\', ':']).next().unwrap_or("repo").trim_end_matches(".git").to_string();
        let parent = PathBuf::from(&parent);
        std::fs::create_dir_all(&parent).map_err(err)?;
        let dest = parent.join(&name);
        let o = engine.git(&parent, &["clone", "--progress", url.trim(), dest.to_string_lossy().as_ref()]);
        if o.ok {
            Ok(dest.to_string_lossy().into_owned())
        } else {
            Err(o.message())
        }
    })
    .await?
}

#[tauri::command]
fn open_terminal(path: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd")
            .args(["/C", "start", "cmd"])
            .current_dir(&path)
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(err)?;
    }
    #[cfg(not(windows))]
    std::process::Command::new("x-terminal-emulator").current_dir(&path).spawn().map_err(err)?;
    Ok(())
}

#[tauri::command]
fn open_file_manager(path: String) -> Result<(), String> {
    #[cfg(windows)]
    std::process::Command::new("explorer").arg(&path).spawn().map_err(err)?;
    #[cfg(not(windows))]
    std::process::Command::new("xdg-open").arg(&path).spawn().map_err(err)?;
    Ok(())
}

fn settings_file() -> PathBuf {
    sample::data_dir().join("settings.json")
}

#[tauri::command]
fn load_settings(state: State<'_, AppState>) -> serde_json::Value {
    if state.screenshot.is_some() {
        return serde_json::Value::Null; // pictures always start from the same state
    }
    std::fs::read_to_string(settings_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null)
}

#[tauri::command]
fn save_settings(settings: serde_json::Value, state: State<'_, AppState>) -> Result<(), String> {
    if state.screenshot.is_some() {
        return Ok(());
    }
    let f = settings_file();
    if let Some(d) = f.parent() {
        std::fs::create_dir_all(d).map_err(err)?;
    }
    std::fs::write(f, serde_json::to_string_pretty(&settings).map_err(err)?).map_err(err)
}

/// Watch a repository and tell the window when something changed (rules in src/watch.rs).
#[tauri::command]
fn watch_repo(path: String, app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut w = state.watchers.lock().unwrap();
    if w.contains_key(&path) {
        return Ok(());
    }
    let key = path.clone();
    let watcher = watch::start(Path::new(&path), move || {
        let _ = app.emit("repo-changed", key.clone());
    })?;
    w.insert(path, watcher);
    Ok(())
}

#[tauri::command]
fn unwatch_repo(path: String, state: State<'_, AppState>) {
    state.watchers.lock().unwrap().remove(&path);
}

/// Called by the window once it has drawn about 120 frames in --screenshot mode.
#[tauri::command]
fn screenshot_ready(window: tauri::WebviewWindow, state: State<'_, AppState>, app: tauri::AppHandle) -> Result<(), String> {
    let Some(out) = state.screenshot.clone() else { return Ok(()) };
    take_and_exit(&window, &out, &app);
    Ok(())
}

fn take_and_exit(window: &tauri::WebviewWindow, out: &Path, app: &tauri::AppHandle) {
    #[cfg(windows)]
    let res = match window.hwnd() {
        Ok(h) => shot::capture_window(h.0 as isize, out),
        Err(e) => Err(e.to_string()),
    };
    #[cfg(not(windows))]
    let res = shot::capture_window(0, out);
    match res {
        Ok(()) => println!("saved {}", out.display()),
        Err(e) => eprintln!("screenshot failed: {e}"),
    }
    app.exit(0);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let screenshot = args.iter().position(|a| a == "--screenshot").and_then(|i| args.get(i + 1)).map(|p| {
        let p = PathBuf::from(p);
        if p.is_absolute() {
            p
        } else {
            std::env::current_dir().unwrap_or_default().join(p)
        }
    });

    tauri::Builder::default()
        .manage(AppState { engine: Arc::new(Engine::default()), screenshot: screenshot.clone(), watchers: Mutex::new(HashMap::new()) })
        .setup(move |app| {
            if let Some(out) = screenshot.clone() {
                // Safety net: take the picture anyway if the page never reports ready.
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(40));
                    if let Some(w) = handle.get_webview_window("main") {
                        let h2 = handle.clone();
                        let _ = handle.run_on_main_thread(move || take_and_exit(&w, &out, &h2));
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            launch_options,
            open_repo,
            commit_files,
            file_diff,
            run_action,
            activity_log,
            sample_repo,
            last_commit_message,
            pick_folder,
            init_repo,
            clone_repo,
            open_terminal,
            open_file_manager,
            load_settings,
            save_settings,
            watch_repo,
            unwatch_repo,
            screenshot_ready
        ])
        .run(tauri::generate_context!())
        .expect("error while running GitCraft");
}
