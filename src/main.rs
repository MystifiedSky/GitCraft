#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod accounts;
mod actions;
mod jobs;
mod files;
mod git;
mod graph;
mod ai;
mod ai_context;
mod operation;
mod pagination;
mod process_tree;
mod agents;
mod ai_cli;
mod ai_models;
mod extras;
mod external;
mod prefs;
mod hosting;
mod hosting_api;
mod manage;
mod rewrite;
mod metrics;
mod sample;
mod shell;
mod shot;
mod terminal;
mod watch;
mod watch_plan;
#[cfg(test)]
mod regressions;

use actions::{Action, ActionResult, Engine, UndoInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{Emitter, Manager, State};

use metrics::INITIAL_COMMITS;

struct AppState {
    engine: Arc<Engine>,
    screenshot: Option<PathBuf>,
    /// Test aid: which screen `--screenshot` pictures (`--shot-view diff|split|history|start`).
    shot_view: Option<String>,
    watchers: Mutex<HashMap<String, watch::RepoWatch>>,
    terminals: terminal::Terminals,
}

#[derive(Serialize)]
struct LaunchOptions {
    screenshot: bool,
    shot_view: Option<String>,
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
    history_cursor: Option<HistoryCursor>,
    undo: UndoInfo,
}

#[derive(Clone, Deserialize, Serialize)]
struct HistoryCursor {
    revs: Vec<String>,
    offset: usize,
    layout: graph::LayoutCursor,
}

#[derive(Serialize)]
struct HistoryPage {
    commits: Vec<git::Commit>,
    graph: graph::GraphLayout,
    has_more: bool,
    history_cursor: Option<HistoryCursor>,
}

#[tauri::command]
async fn history_page(path: String, cursor: HistoryCursor, max: Option<usize>) -> Result<HistoryPage, String> {
    blocking(move || {
        let mut cursor = cursor;
        let (commits, has_more) = git::history_page(Path::new(&path), &cursor.revs, cursor.offset, max.unwrap_or(2000).clamp(100, 5000))?;
        let graph = cursor.layout.append(&commits);
        cursor.offset += commits.len();
        Ok(HistoryPage { commits, graph, has_more, history_cursor: has_more.then_some(cursor) })
    }).await?
}

fn err<E: ToString>(e: E) -> String {
    e.to_string()
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(err)
}

#[tauri::command]
fn launch_options(state: State<'_, AppState>) -> LaunchOptions {
    LaunchOptions { screenshot: state.screenshot.is_some(), shot_view: state.shot_view.clone(), version: env!("CARGO_PKG_VERSION").to_string(), metrics: metrics::metrics() }
}

#[tauri::command]
async fn open_repo(path: String, max: Option<usize>, view: Option<git::RefView>, state: State<'_, AppState>) -> Result<RepoView, String> {
    let engine = state.engine.clone();
    blocking(move || {
        // Tabs saved by versions that turned /Users/x into \Users\x on macOS/Linux: restore the slashes.
        let path = if cfg!(not(windows)) && path.starts_with('\\') { path.replace('\\', "/") } else { path };
        let p = PathBuf::from(&path);
        if !p.exists() {
            return Err(format!("The folder {path} doesn't exist"));
        }
        let root = git::repo_root(&p).ok_or_else(|| format!("{path} is not a Git repository"))?;
        let view = view.unwrap_or_default();
        let data = git::load_view_checked(&root, max.unwrap_or(INITIAL_COMMITS).clamp(prefs::MIN_INITIAL_COMMITS, 5000), &view)?;
        let pinned_sha = view.pinned.as_ref().and_then(|p| data.refs.iter().find(|r| &r.full == p)).map(|r| r.sha.clone());
        let mut layout = graph::LayoutCursor::new(pinned_sha.as_deref());
        let graph = layout.append(&data.commits);
        let history_cursor = data.has_more.then(|| HistoryCursor { revs: data.history_revs.clone(), offset: data.commits.len(), layout });
        // Windows paths use backslashes; on macOS/Linux the path must stay as-is (a blanket replace turns /Users/x into \Users\x).
        let root_s = if cfg!(windows) { root.to_string_lossy().replace('/', "\\") } else { root.to_string_lossy().into_owned() };
        Ok(RepoView {
            name: git::repo_name(&root),
            undo: engine.undo_info(&root_s),
            path: root_s,
            data,
            graph,
            history_cursor,
        })
    })
    .await?
}

#[tauri::command]
async fn commit_details(path: String, sha: String) -> Result<git::Commit, String> {
    blocking(move || {
        let output = git::run(Path::new(&path), &["log", "--no-walk", "-1", "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%s%x00%b%x00", &sha, "--"]);
        if !output.ok { return Err(output.message()); }
        git::parse_commits(&output.stdout).into_iter().next().ok_or_else(|| "The selected commit is no longer available.".into())
    }).await?
}

#[tauri::command]
async fn commit_files(path: String, sha: String) -> Result<Vec<git::CommitFile>, String> {
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
        git::commit_file_entries(&dir, &c)
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
    ignore_ws: Option<bool>,
) -> Result<Vec<git::DiffLine>, String> {
    blocking(move || git::file_diff_opts(Path::new(&path), &source, &file, untracked, parent.as_deref(), ignore_ws.unwrap_or(false))).await
}

#[tauri::command]
async fn file_versions(path: String, source: git::DiffSource, file: String, untracked: bool, parent: Option<String>, allow_large: Option<bool>) -> Result<files::FileVersions, String> {
    blocking(move || files::versions_with_limit(Path::new(&path), &source, &file, parent.as_deref(), untracked, allow_large.unwrap_or(false))).await
}

#[tauri::command]
async fn save_file(path: String, file: String, content: String, expected: Option<String>, utf8: Option<bool>) -> Result<(), String> {
    blocking(move || {
        let operation = operation::lock_for(Path::new(&path));
        let _guard = operation.lock().unwrap_or_else(|e| e.into_inner());
        files::save_checked_mode(Path::new(&path), &file, &content, expected.as_deref(), utf8.unwrap_or(false))
    }).await?
}

#[tauri::command]
async fn file_history(path: String, file: String) -> Result<Vec<files::HistoryEntry>, String> {
    blocking(move || files::history(Path::new(&path), &file, metrics::FILE_HISTORY_MAX)).await
}

#[tauri::command]
async fn blame(path: String, file: String, rev: Option<String>) -> Result<Vec<files::BlameLine>, String> {
    blocking(move || files::blame(Path::new(&path), &file, rev.as_deref())).await?
}

#[tauri::command]
async fn commit_stats(path: String, shas: Vec<String>) -> Result<Vec<files::Stats>, String> {
    blocking(move || files::stats(Path::new(&path), &shas)).await
}

#[tauri::command]
async fn range_files(path: String, from: String, to: String) -> Result<Vec<git::FileChange>, String> {
    blocking(move || files::range_files(Path::new(&path), &from, &to)).await
}

#[tauri::command]
async fn tracked_files(path: String) -> Result<Vec<String>, String> {
    blocking(move || files::tracked_files(Path::new(&path))).await
}

#[tauri::command]
async fn read_ssh_public_key(path: String) -> Result<String, String> { blocking(move || accounts::read_public_key(&path)).await? }

#[tauri::command]
async fn rebase_plan(path: String, base: String) -> Result<serde_json::Value, String> {
    blocking(move || {
        let dir = Path::new(&path);
        let lock = operation::lock_for(dir); let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        let expected = rewrite::plan_state(dir, &base)?;
        let commits = rewrite::rebase_commits(dir, &base)?;
        Ok(serde_json::json!({ "expected": expected, "commits": commits }))
    }).await?
}

#[tauri::command]
async fn rebase_commits(path: String, base: String) -> Result<Vec<git::Commit>, String> {
    blocking(move || rewrite::rebase_commits(Path::new(&path), &base)).await?
}

#[tauri::command]
async fn conflict_info(path: String, file: String) -> Result<rewrite::ConflictInfo, String> {
    blocking(move || rewrite::conflict_info(Path::new(&path), &file)).await
}

#[tauri::command]
async fn gitflow_config(path: String) -> Result<manage::GitflowConfig, String> {
    blocking(move || manage::gitflow_config(Path::new(&path))).await
}

#[tauri::command]
async fn cleanup_list(path: String) -> Result<Vec<manage::CleanupBranch>, String> {
    blocking(move || manage::cleanup_list(Path::new(&path))).await
}

#[tauri::command]
async fn remotes_list(path: String) -> Result<Vec<manage::Remote>, String> {
    blocking(move || manage::remotes(Path::new(&path))).await
}

#[tauri::command]
async fn submodules_list(path: String) -> Result<Vec<manage::Submodule>, String> {
    blocking(move || manage::submodules(Path::new(&path))).await
}

#[tauri::command]
async fn lfs_locks(path: String) -> Result<Vec<manage::LfsLock>, String> { blocking(move || manage::lfs_locks(Path::new(&path))).await? }

#[tauri::command]
async fn lfs_info(path: String) -> Result<manage::LfsInfo, String> {
    blocking(move || manage::lfs_info(Path::new(&path))).await
}

#[tauri::command]
async fn sparse_info(path: String) -> Result<manage::SparseInfo, String> {
    blocking(move || manage::sparse_info(Path::new(&path))).await
}

#[tauri::command]
fn gitignore_templates() -> Vec<String> {
    manage::GITIGNORE_TEMPLATES.iter().map(|(n, _)| n.to_string()).collect()
}

#[tauri::command]
async fn account_info(path: Option<String>) -> Result<accounts::AccountInfo, String> {
    blocking(move || accounts::info(Path::new(&path.unwrap_or_else(|| ".".into())))).await
}

#[tauri::command]
async fn set_git_config(path: Option<String>, scope: String, key: String, value: String) -> Result<(), String> {
    blocking(move || accounts::set(Path::new(&path.unwrap_or_else(|| ".".into())), &scope, &key, &value)).await?
}

#[tauri::command]
async fn generate_ssh_key(email: String, name: String) -> Result<String, String> {
    blocking(move || accounts::generate_ssh_key(&email, &name)).await?
}

#[tauri::command]
fn cancel_job(id: String, state: State<'_, AppState>) -> bool {
    state.engine.jobs.cancel(&id)
}

#[tauri::command]
fn term_open(cwd: String, cols: u16, rows: u16, shell: Option<String>, agent_env: Option<HashMap<String, String>>, app: tauri::AppHandle, state: State<'_, AppState>) -> Result<u32, String> {
    let a1 = app.clone();
    let a2 = app.clone();
    state.terminals.open(
        shell.as_deref().unwrap_or(""),
        &cwd,
        cols,
        rows,
        agent_env.as_ref(),
        std::sync::Arc::new(move |id, data| {
            let _ = a1.emit("term-data", serde_json::json!({ "id": id, "data": data }));
        }),
        std::sync::Arc::new(move |id| {
            let _ = a2.emit("term-exit", id);
        }),
    )
}

#[tauri::command]
fn term_write(id: u32, data: String, state: State<'_, AppState>) -> Result<(), String> {
    state.terminals.write(id, &data)
}

#[tauri::command]
fn term_ack(id: u32, bytes: usize, state: State<'_, AppState>) {
    state.terminals.acknowledge(id, bytes);
}

#[tauri::command]
fn term_resize(id: u32, cols: u16, rows: u16, state: State<'_, AppState>) -> Result<(), String> {
    state.terminals.resize(id, cols, rows)
}

#[tauri::command]
fn term_close(id: u32, state: State<'_, AppState>) {
    state.terminals.close(id)
}

/// The hosting service of a repository's origin (or first) remote, if it's GitHub or GitLab.
fn remote_of(path: &str, gitlab_host: &str) -> Result<hosting::RemoteRepo, String> {
    let dir = Path::new(path);
    let mut url = git::run(dir, &["remote", "get-url", "origin"]).stdout.trim().to_string();
    if url.is_empty() {
        if let Some(first) = git::run(dir, &["remote"]).stdout.lines().next() {
            url = git::run(dir, &["remote", "get-url", first]).stdout.trim().to_string();
        }
    }
    hosting::parse_remote(&url, gitlab_host).ok_or_else(|| "This repository's remote isn't on a connected service (Preferences > Integrations).".to_string())
}

#[derive(Serialize)]
struct HostingStatus {
    github: Option<String>,
    gitlab: Option<String>,
    github_error: Option<String>,
    gitlab_error: Option<String>,
    ai: bool,
}

#[tauri::command]
async fn hosting_status(gitlab_host: String) -> Result<HostingStatus, String> {
    blocking(move || {
        let check = |p: hosting::Provider, name: &str| -> (Option<String>, Option<String>) {
            if hosting::secret_get(name).is_none() {
                return (None, None);
            }
            match hosting::whoami(&p, &gitlab_host) {
                Ok(u) => (Some(u), None),
                Err(e) => (None, Some(e)),
            }
        };
        let (github, github_error) = check(hosting::Provider::Github, "github-token");
        let (gitlab, gitlab_error) = check(hosting::Provider::Gitlab, "gitlab-token");
        HostingStatus { github, gitlab, github_error, gitlab_error, ai: ai::configured() }
    })
    .await
}

/// Store (or with an empty value, remove) a token or API key in the OS credential store.
#[tauri::command]
fn set_secret(name: String, value: String) -> Result<(), String> {
    let allowed = hosting::SERVICES.iter().any(|id| hosting::secret_name(id) == name) || ["anthropic-key", "openai-key", "google-key", "ollama-key", "custom-ai-key"].contains(&name.as_str());
    if !allowed {
        return Err("unknown secret".into());
    }
    hosting::secret_set(&name, value.trim())?;
    if name.ends_with("-token") { hosting::sync_git_credentials(""); }
    Ok(())
}

#[tauri::command]
async fn repo_remote(path: String, gitlab_host: String) -> Result<Option<hosting::RemoteRepo>, String> {
    blocking(move || remote_of(&path, &gitlab_host).ok()).await
}

#[tauri::command]
async fn list_prs(path: String, gitlab_host: String) -> Result<Vec<hosting::PullRequest>, String> {
    blocking(move || hosting::pull_requests(&remote_of(&path, &gitlab_host)?)).await?
}

#[tauri::command]
async fn pr_details(path: String, gitlab_host: String, number: u64) -> Result<hosting::PullRequest, String> {
    blocking(move || hosting::pull_request(&remote_of(&path, &gitlab_host)?, number)).await?
}

#[tauri::command]
async fn pr_checkout_target(path: String, gitlab_host: String, number: u64) -> Result<hosting::PrCheckout, String> {
    blocking(move || {
        let remote = remote_of(&path, &gitlab_host)?;
        hosting::pr_checkout(&remote, &hosting::pull_request(&remote, number)?)
    }).await?
}

/// Issues for a repository: from its own issue tracker choice (Preferences > Issue Tracker), else its host.
#[tauri::command]
async fn list_issues(path: String, gitlab_host: String, tracker: Option<hosting::Tracker>) -> Result<Vec<hosting::Issue>, String> {
    blocking(move || match tracker.filter(|t| !t.kind.is_empty()) {
        Some(t) => hosting::tracker_issues(&t, false),
        None => hosting::issues(&remote_of(&path, &gitlab_host)?),
    })
    .await?
}

#[tauri::command]
async fn create_issue(path: Option<String>, gitlab_host: String, tracker: Option<hosting::Tracker>, title: String, body: String) -> Result<hosting::Issue, String> {
    blocking(move || {
        let r = path.as_deref().and_then(|p| remote_of(p, &gitlab_host).ok());
        hosting::create_issue(r.as_ref(), &tracker.unwrap_or_default(), &title, &body)
    })
    .await?
}

/// Preferences > Integrations: hosts, user names and organisations (tokens stay in the credential store).
#[tauri::command]
fn set_integration_config(config: hosting::Config) {
    hosting::set_config(config);
    std::thread::spawn(|| hosting::sync_git_credentials(""));
}

#[tauri::command]
async fn integration_statuses(ids: Vec<String>) -> Result<Vec<hosting::ServiceStatus>, String> {
    blocking(move || {
        let handles: Vec<_> = ids.into_iter().map(|id| std::thread::spawn(move || hosting::status(&id))).collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    })
    .await
}

/// "Connect": browser sign-in through Git Credential Manager, or the GitHub CLI's sign-in ("gh").
#[tauri::command]
async fn integration_sign_in(id: String, method: String) -> Result<String, String> {
    blocking(move || {
        let token = if method == "gh" { hosting::gh_cli_token()? } else { hosting::browser_sign_in(&id)? };
        hosting::secret_set(&hosting::secret_name(&id), &token)?;
        hosting::sync_git_credentials("");
        hosting::whoami_id(&id)
    })
    .await?
}

#[tauri::command]
async fn integration_repos(id: String) -> Result<Vec<hosting::HostedRepo>, String> {
    blocking(move || hosting::repositories(&id)).await?
}

#[tauri::command]
async fn integration_add_ssh_key(id: String, title: String, key: String) -> Result<(), String> {
    blocking(move || hosting::add_ssh_key(&id, &title, &key)).await?
}

#[tauri::command]
fn integration_ssh_page(id: String) -> String {
    hosting::ssh_keys_page(&id)
}

#[tauri::command]
async fn pr_comments(path: String, gitlab_host: String, number: u64) -> Result<Vec<hosting::Comment>, String> {
    blocking(move || hosting::comments(&remote_of(&path, &gitlab_host)?, number)).await?
}

#[tauri::command]
async fn pr_comment(path: String, gitlab_host: String, number: u64, body: String) -> Result<(), String> {
    blocking(move || hosting::add_comment(&remote_of(&path, &gitlab_host)?, number, &body)).await?
}

#[tauri::command]
async fn create_pr(path: String, gitlab_host: String, source: String, target: String, title: String, body: String, draft: bool) -> Result<hosting::PullRequest, String> {
    blocking(move || {
        let (remote, source) = hosting::pull_request_destination(Path::new(&path), &source, &gitlab_host)?;
        hosting::create_pr(&remote, &source, &target, &title, &body, draft)
    }).await?
}

#[tauri::command]
async fn cancel_pr_merge(path: String, gitlab_host: String, number: u64) -> Result<bool, String> {
    blocking(move || Ok(hosting_api::cancel_merge(&remote_of(&path, &gitlab_host)?, number))).await?
}

#[tauri::command]
async fn merge_pr(path: String, gitlab_host: String, number: u64, method: String) -> Result<(), String> {
    blocking(move || hosting::merge_pr(&remote_of(&path, &gitlab_host)?, number, &method)).await?
}

#[tauri::command]
async fn launchpad(gitlab_host: String) -> Result<hosting::Launchpad, String> {
    blocking(move || hosting::launchpad(&gitlab_host)).await
}

#[tauri::command]
fn issue_branch_name(number: u64, title: String) -> String {
    hosting::branch_for_issue(number, &title)
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Only web links can be opened".into());
    }
    // Open a web link with the system: ShellExecuteExW "open".
    shell::open_external(&url)
}

#[tauri::command]
async fn ai_commit_message(path: String, sha: Option<String>, config: ai::AiConfig) -> Result<(String, String), String> {
    blocking(move || ai::with_config(config, || ai::commit_message(Path::new(&path), sha.as_deref()))).await?
}

#[tauri::command]
async fn ai_stash_message(path: String, config: ai::AiConfig) -> Result<String, String> {
    blocking(move || ai::with_config(config, || ai::stash_message(Path::new(&path)))).await?
}

#[tauri::command]
async fn ai_explain_commit(path: String, sha: String, config: ai::AiConfig) -> Result<String, String> {
    blocking(move || ai::with_config(config, || ai::explain_commit(Path::new(&path), &sha))).await?
}

#[tauri::command]
async fn ai_explain_branch(path: String, branch: String, base: Option<String>, config: ai::AiConfig) -> Result<String, String> {
    blocking(move || ai::with_config(config, || {
        let base = base.or_else(|| extras::default_target(Path::new(&path))).ok_or("No branch to compare with")?;
        ai::explain_branch(Path::new(&path), &branch, &base)
    }))
    .await?
}

#[tauri::command]
fn set_ai_config(config: ai::AiConfig) {
    ai::set_config(config);
}

#[tauri::command]
async fn ai_test_connection(config: ai::AiConfig) -> Result<String, String> {
    blocking(move || ai::with_config(config, ai::test_connection)).await?
}

#[tauri::command]
async fn ai_list_models(config: ai::AiConfig) -> Result<Vec<ai_models::Model>, String> {
    blocking(move || ai_models::list(&config)).await?
}

#[tauri::command]
async fn ai_find_program(provider: String) -> Result<ai_cli::DetectedProgram, String> {
    let agent = ai_cli::Agent::from_provider(&provider).ok_or("This provider does not use a local program.")?;
    blocking(move || ai_cli::detect_program(agent)).await?
}

#[tauri::command]
async fn ai_pull_request(path: String, source: String, target: String, config: ai::AiConfig) -> Result<(String, String), String> {
    blocking(move || ai::with_config(config, || ai::pull_request_text(Path::new(&path), &source, &target))).await?
}

#[tauri::command]
async fn ai_resolve_conflict(base: String, ours: String, theirs: String, file: String, config: ai::AiConfig) -> Result<String, String> {
    blocking(move || ai::with_config(config, || ai::resolve_conflict(&base, &ours, &theirs, &file))).await?
}

#[tauri::command]
async fn ai_compose(path: String, config: ai::AiConfig) -> Result<Vec<ai::ComposedCommit>, String> {
    blocking(move || ai::with_config(config, || ai::compose(Path::new(&path)))).await?
}

/// Save a patch (uncommitted changes when `shas` is empty, else those commits) to a file the person picks.
#[tauri::command]
async fn save_patch(path: String, shas: Vec<String>, name: String) -> Result<Option<String>, String> {
    let patch = blocking(move || -> Result<_, String> {
        let text = extras::make_patch(Path::new(&path), &shas)?;
        let file = rfd::FileDialog::new().set_title("Save patch").set_file_name(&name).add_filter("Patch", &["patch", "diff"]).save_file();
        Ok(file.map(|file| (file, text)))
    })
    .await??;
    let Some((file, text)) = patch else {
        return Ok(None);
    };
    tokio::fs::write(&file, text).await.map_err(err)?;
    Ok(Some(file.to_string_lossy().into_owned()))
}

#[tauri::command]
async fn pick_patch_file() -> Result<Option<String>, String> {
    blocking(|| rfd::FileDialog::new().set_title("Apply a patch").add_filter("Patch", &["patch", "diff"]).pick_file().map(|p| p.to_string_lossy().into_owned())).await
}

#[tauri::command]
async fn conflict_check(path: String, target: Option<String>) -> Result<extras::ConflictCheck, String> {
    blocking(move || extras::conflict_check(Path::new(&path), target.as_deref())).await
}

/// Just the file lists (one `git status`), for staging and working-folder edits.
#[tauri::command]
async fn repo_status(path: String) -> Result<git::WorkStatus, String> {
    blocking(move || git::try_status(Path::new(&path))).await?
}

/// Hand the tokens of connected services to Git (HTTPS push / pull / fetch / clone).
#[tauri::command]
async fn sync_git_credentials(gitlab_host: String) -> Result<(), String> {
    blocking(move || hosting::sync_git_credentials(&gitlab_host)).await
}

#[tauri::command]
async fn remote_auth(path: String, gitlab_host: String) -> Result<hosting::RemoteAuth, String> {
    blocking(move || hosting::remote_auth(Path::new(&path), &gitlab_host)).await
}

/// Preferences that change how Git runs: executable, SSH key / agent, extended logging.
#[tauri::command]
fn set_runtime_settings(settings: prefs::RuntimeSettings) {
    prefs::set_runtime(settings);
}

#[tauri::command]
async fn tool_versions(gpg_program: String) -> Result<prefs::Versions, String> {
    blocking(move || prefs::versions(&gpg_program)).await
}

#[tauri::command]
async fn git_config_get(path: Option<String>, scope: String, keys: Vec<String>) -> Result<HashMap<String, String>, String> {
    blocking(move || prefs::config_get(path.as_deref().map(Path::new), &scope, &keys)).await
}

#[tauri::command]
async fn gpg_generate_key(program: String, name: String, email: String, passphrase: String) -> Result<String, String> {
    blocking(move || prefs::gpg_generate(&program, &name, &email, &passphrase)).await?
}

#[tauri::command]
async fn gpg_export_key(program: String, key: String) -> Result<String, String> {
    blocking(move || prefs::gpg_export(&program, &key)).await?
}

#[tauri::command]
async fn forget_credentials(hosts: Vec<String>) -> Result<usize, String> {
    blocking(move || prefs::forget_credentials(&hosts)).await
}

#[tauri::command]
async fn orig_cleanup_candidates(path: String) -> Result<Vec<prefs::OrigCandidate>, String> { blocking(move || prefs::orig_cleanup_candidates(Path::new(&path))).await }

#[tauri::command]
async fn delete_orig_files(path: String, candidates: Vec<prefs::OrigCandidate>) -> Result<Vec<String>, String> {
    blocking(move || {
        let dir = Path::new(&path); let lock = operation::lock_for(dir); let _guard = lock.lock().unwrap_or_else(|error| error.into_inner());
        prefs::delete_orig_selected(dir, &candidates)
    }).await?
}

#[tauri::command]
async fn pick_file(title: String) -> Result<Option<String>, String> {
    blocking(move || rfd::FileDialog::new().set_title(&title).pick_file().map(|p| p.to_string_lossy().into_owned())).await
}

#[tauri::command]
async fn repo_summary(path: String) -> Result<extras::RepoSummary, String> {
    blocking(move || extras::repo_summary(Path::new(&path))).await
}

#[tauri::command]
async fn pr_template(path: String) -> Result<Option<String>, String> {
    blocking(move || extras::pr_template(Path::new(&path))).await
}

#[tauri::command]
async fn agent_sessions(path: String, paths: Option<Vec<String>>, claude_dir: Option<String>, codex_dir: Option<String>) -> Result<Vec<agents::AgentSession>, String> {
    blocking(move || {
        let mut roots = paths.unwrap_or_default();
        if !roots.contains(&path) { roots.push(path); }
        agents::sessions(&roots, claude_dir.as_deref(), codex_dir.as_deref())
    }).await
}

/// Whether automatic updates are set up in this build (a public key is configured), and where they come from.
#[tauri::command]
fn update_config(app: tauri::AppHandle) -> serde_json::Value {
    let u = app.config().plugins.0.get("updater").cloned().unwrap_or_default();
    let configured = u["pubkey"].as_str().map(|k| !k.trim().is_empty()).unwrap_or(false);
    serde_json::json!({ "configured": configured, "endpoint": u["endpoints"][0] })
}

/// File > Open Repo in <editor> (Preferences > External Tools > Editor); with `file`, open that file.
#[tauri::command]
fn open_in_editor(path: String, file: Option<String>, tool: Option<String>, exe: Option<String>, file_args: Option<String>, repo_args: Option<String>) -> Result<(), String> {
    external::open_editor(&path, file.as_deref(), tool.as_deref().unwrap_or("vscode"), exe.as_deref().unwrap_or(""), file_args.as_deref().unwrap_or(""), repo_args.as_deref().unwrap_or(""))
}

#[tauri::command]
async fn run_action(path: String, action: Action, state: State<'_, AppState>) -> Result<ActionResult, String> {
    let engine = state.engine.clone();
    blocking(move || engine.perform(&path, action)).await
}

#[tauri::command]
fn activity_log_snapshot(version: Option<u64>, state: State<'_, AppState>) -> Option<(u64, Vec<git::LogEntry>)> { state.engine.log_snapshot(version) }

#[tauri::command]
fn activity_log(state: State<'_, AppState>) -> Vec<git::LogEntry> {
    state.engine.log_entries()
}

#[tauri::command]
async fn sample_repo() -> Result<String, String> {
    blocking(|| sample::ensure_sample().map(|p| p.to_string_lossy().into_owned())).await?
}

#[tauri::command]
async fn conflict_sample() -> Result<String, String> {
    blocking(|| sample::ensure_conflict_sample().map(|p| p.to_string_lossy().into_owned())).await?
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
async fn init_repo(parent: String, name: String, options: Option<manage::InitOptions>, state: State<'_, AppState>) -> Result<manage::InitOutcome, String> {
    let engine = state.engine.clone();
    let name = name.trim();
    if name.is_empty() || Path::new(name).components().count() != 1 || !matches!(Path::new(name).components().next(), Some(std::path::Component::Normal(_))) {
        return Err("Enter a folder name, without parent directories.".into());
    }
    let dir = PathBuf::from(parent).join(name);
    if dir.exists() && std::fs::read_dir(&dir).map_err(err)?.next().is_some() {
        return Err("Choose an empty folder for a new repository. Existing files were not changed.".into());
    }
    let name = name.to_owned();
    tokio::fs::create_dir_all(&dir).await.map_err(err)?;
    blocking(move || {
        let opts = options.unwrap_or_default();
        let branch = if opts.branch.trim().is_empty() { "main".to_string() } else { opts.branch.trim().to_string() };
        let o = engine.git(&dir, &["init", "-b", &branch]);
        if o.ok {
            let f = manage::init_files(&dir, name.trim(), &opts, &|a: &[&str]| engine.git(&dir, a));
            if !f.ok {
                return Err(f.message());
            }
            Ok(manage::initialize_lfs(&dir, opts.lfs, &|args| engine.git(&dir, args)))
        } else {
            Err(o.message())
        }
    })
    .await?
}

/// What the Clone dialog can ask for besides the address and folder.
#[derive(serde::Deserialize, Default)]
struct CloneOptions {
    /// Only the newest this-many commits (--depth).
    depth: Option<u32>,
    /// Only commits newer than this date, YYYY-MM-DD (--shallow-since).
    since: Option<String>,
    /// Clone just this branch (--single-branch --branch).
    branch: Option<String>,
    /// Extra flags parsed as quoted argument fields without shell expansion.
    custom_flags: Option<String>,
    /// Start with a sparse checkout (--sparse) and check out only `sparse_paths`.
    sparse: Option<bool>,
    sparse_paths: Option<Vec<String>>,
    /// Leave file contents out of the first download (--filter blob:none).
    blobless: Option<bool>,
}

/// The `git clone` arguments in a fixed order: branch, depth, progress, since, custom flags, sparse, filter.
fn clone_args(o: &CloneOptions) -> Result<Vec<String>, String> {
    let mut a: Vec<String> = Vec::new();
    if let Some(b) = o.branch.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
        a.extend(["--single-branch".to_string(), "--branch".to_string(), b.to_string()]);
    }
    if let Some(d) = o.depth.filter(|d| *d > 0) {
        a.extend(["--depth".to_string(), d.to_string()]);
    }
    a.push("--progress".into());
    if let Some(since) = o.since.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        a.extend(["--shallow-since".to_string(), since.to_string()]);
    }
    if let Some(flags) = o.custom_flags.as_deref() {
        a.extend(external::argument_fields(flags)?);
    }
    if o.sparse == Some(true) {
        a.push("--sparse".into());
    }
    if o.blobless == Some(true) {
        a.extend(["--filter".to_string(), "blob:none".to_string()]);
    }
    Ok(a)
}

#[tauri::command]
async fn clone_repo(url: String, parent: String, options: Option<CloneOptions>, state: State<'_, AppState>) -> Result<String, String> {
    let engine = state.engine.clone();
    let parent = PathBuf::from(parent);
    tokio::fs::create_dir_all(&parent).await.map_err(err)?;
    blocking(move || {
        let opts = options.unwrap_or_default();
        let name = url.trim_end_matches('/').rsplit(['/', '\\', ':']).next().unwrap_or("repo").trim_end_matches(".git").to_string();
        let dest = parent.join(&name);
        let dest_s = dest.to_string_lossy().into_owned();
        let mut args: Vec<String> = vec!["clone".into()];
        args.extend(clone_args(&opts)?);
        args.push(url.trim().to_string());
        args.push(dest_s);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let o = engine.job("clone", "clone", &parent, &refs);
        if !o.ok {
            return Err(o.message());
        }
        if opts.sparse == Some(true) {
            // Only the chosen paths are checked out; with none, only the files in the top folder.
            let mut set: Vec<String> = vec!["sparse-checkout".into(), "set".into(), "--no-cone".into()];
            let paths: Vec<String> = opts.sparse_paths.unwrap_or_default().into_iter().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
            if !paths.is_empty() {
                set.push("--".into());
                set.extend(paths);
            } else { set.extend(["/*".into(), "!/*/".into()]); }
            let refs: Vec<&str> = set.iter().map(String::as_str).collect();
            let s = engine.git(&dest, &refs);
            if !s.ok {
                return Err(format!("The repository was cloned, but the sparse checkout could not be set: {}", s.message()));
            }
        }
        Ok(dest.to_string_lossy().into_owned())
    })
    .await?
}

/// File > Open External Terminal (Preferences > External Tools > Terminal).
#[tauri::command]
fn open_terminal(path: String, tool: Option<String>, exe: Option<String>, args: Option<String>) -> Result<(), String> {
    external::open_terminal(&path, tool.as_deref().unwrap_or("default"), exe.as_deref().unwrap_or(""), args.as_deref().unwrap_or(""))
}

#[tauri::command]
fn open_file_manager(path: String) -> Result<(), String> {
    // A folder opens with the "explore" verb.
    shell::open_path(Path::new(&path))
}

/// File menu "Open file in default program": the system opens it with its usual app.
#[tauri::command]
fn open_default(path: String, file: String) -> Result<(), String> {
    let p = files::repo_file(Path::new(&path), &file)?;
    // Default action; "Failed to open path" on failure.
    if !p.exists() {
        return Err(shell::OPEN_PATH_FAILED.into());
    }
    shell::open_path(&p)
}

/// File menu "Show in File Manager": opens the folder with the file selected (its folder when the file is gone).
#[tauri::command]
fn reveal_file(path: String, file: String) -> Result<(), String> {
    let p = files::repo_file(Path::new(&path), &file)?;
    // Opens the folder with the file selected (SHOpenFolderAndSelectItems). For a file
    // that is gone, GitCraft opens the folder it was in instead.
    if !p.exists() {
        return shell::open_path(p.parent().unwrap_or(Path::new(&path)));
    }
    shell::show_item_in_folder(&p)
}

/// File menu "Copy file contents to clipboard": the working copy, or the file as it was in `rev`.
#[tauri::command]
async fn file_text(path: String, file: String, rev: Option<String>) -> Result<String, String> {
    blocking(move || files::file_text(Path::new(&path), &file, rev.as_deref())).await?
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
    // Replace the settings only after the complete JSON is written, so an interrupted save
    // cannot truncate the last usable preferences file.
    let pending = f.with_extension("json.tmp");
    std::fs::write(&pending, serde_json::to_string_pretty(&settings).map_err(err)?).map_err(err)?;
    std::fs::rename(pending, f).map_err(err)
}

/// Watch a repository and tell the window when something changed (rules in src/watch.rs).
#[tauri::command]
fn watch_repo(path: String, app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut w = state.watchers.lock().unwrap();
    if w.contains_key(&path) {
        return Ok(());
    }
    let key = path.clone();
    let watcher = watch::start(Path::new(&path), move |full| {
        let _ = app.emit("repo-changed", serde_json::json!({ "path": key, "full": full }));
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
    let shot_view = args.iter().position(|a| a == "--shot-view").and_then(|i| args.get(i + 1)).cloned();
    let screenshot = args.iter().position(|a| a == "--screenshot").and_then(|i| args.get(i + 1)).map(|p| {
        let p = PathBuf::from(p);
        if p.is_absolute() {
            p
        } else {
            std::env::current_dir().unwrap_or_default().join(p)
        }
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState { engine: Arc::new(Engine::default()), screenshot: screenshot.clone(), shot_view, watchers: Mutex::new(HashMap::new()), terminals: terminal::Terminals::default() })
        .setup(move |app| {
            // Long jobs report progress to the window ("job-progress" events).
            let handle = app.handle().clone();
            let engine = app.state::<AppState>().engine.clone();
            engine.jobs.set_listener(std::sync::Arc::new(move |id: &str, text: &str, pct: Option<u32>| {
                let _ = handle.emit("job-progress", serde_json::json!({ "id": id, "text": text, "percent": pct }));
            }));
            // Test aid: `--shot-size 1920x1040` opens the window at that size (to check wide layouts).
            let args: Vec<String> = std::env::args().collect();
            if let Some((w, h)) = args
                .iter()
                .position(|a| a == "--shot-size")
                .and_then(|i| args.get(i + 1))
                .and_then(|s| s.split_once('x'))
                .and_then(|(w, h)| Some((w.parse::<f64>().ok()?, h.parse::<f64>().ok()?)))
            {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.set_size(tauri::LogicalSize::new(w, h));
                    let _ = win.center();
                }
            }
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
            history_page,
            commit_files,
            commit_details,
            file_diff,
            file_versions,
            save_file,
            file_history,
            blame,
            commit_stats,
            range_files,
            tracked_files,
            open_in_editor,
            rebase_commits,
            rebase_plan,
            read_ssh_public_key,
            gitflow_config,
            update_config,
            ai_commit_message,
            ai_stash_message,
            ai_explain_commit,
            ai_explain_branch,
            ai_compose,
            set_ai_config,
            ai_test_connection,
            ai_list_models,
            ai_find_program,
            ai_pull_request,
            ai_resolve_conflict,
            save_patch,
            pick_patch_file,
            conflict_check,
            agent_sessions,
            pr_template,
            repo_summary,
            set_runtime_settings,
            tool_versions,
            git_config_get,
            gpg_generate_key,
            gpg_export_key,
            forget_credentials,
            delete_orig_files,
            orig_cleanup_candidates,
            pick_file,
            sync_git_credentials,
            remote_auth,
            repo_status,
            hosting_status,
            set_secret,
            repo_remote,
            list_prs,
            pr_details,
            pr_checkout_target,
            list_issues,
            pr_comments,
            pr_comment,
            create_pr,
            merge_pr,
            cancel_pr_merge,
            launchpad,
            create_issue,
            set_integration_config,
            integration_statuses,
            integration_sign_in,
            integration_repos,
            integration_add_ssh_key,
            integration_ssh_page,
            issue_branch_name,
            open_url,
            term_open,
            term_write,
            term_ack,
            term_resize,
            term_close,
            account_info,
            set_git_config,
            generate_ssh_key,
            cancel_job,
            lfs_info,
            lfs_locks,
            sparse_info,
            gitignore_templates,
            cleanup_list,
            remotes_list,
            submodules_list,
            conflict_sample,
            conflict_info,
            run_action,
            activity_log,
            activity_log_snapshot,
            sample_repo,
            last_commit_message,
            pick_folder,
            init_repo,
            clone_repo,
            open_terminal,
            open_file_manager,
            open_default,
            reveal_file,
            file_text,
            load_settings,
            save_settings,
            watch_repo,
            unwatch_repo,
            screenshot_ready
        ])
        .run(tauri::generate_context!())
        .expect("error while running GitCraft");
}

#[cfg(test)]
mod clone_tests {
    use super::*;

    #[test]
    fn clone_arguments_keep_a_fixed_order() {
        let all = CloneOptions {
            depth: Some(5),
            since: Some("2025-01-31".into()),
            branch: Some("dev".into()),
            custom_flags: Some("--no-tags   --quiet".into()),
            sparse: Some(true),
            sparse_paths: None,
            blobless: Some(true),
        };
        assert_eq!(
            clone_args(&all).unwrap(),
            ["--single-branch", "--branch", "dev", "--depth", "5", "--progress", "--shallow-since", "2025-01-31", "--no-tags", "--quiet", "--sparse", "--filter", "blob:none"]
        );
        assert_eq!(clone_args(&CloneOptions::default()).unwrap(), ["--progress"]);
        let blank = CloneOptions { branch: Some("  ".into()), depth: Some(0), ..Default::default() };
        assert_eq!(clone_args(&blank).unwrap(), ["--progress"]);
    }
}
