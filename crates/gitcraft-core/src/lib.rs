//! GitCraft back end: Git, hosting, AI, terminal and watcher services that do not
//! depend on a user interface toolkit. Each desktop shell calls into these modules.

pub mod accounts;
pub mod actions;
pub mod agent_status;
pub mod agents;
pub mod avatars;
pub mod ai_cli;
pub mod ai_context;
pub mod ai_models;
pub mod ai;
pub mod diagnostics;
pub mod discard_backups;
pub mod external;
pub mod extras;
pub mod files;
pub mod git;
pub mod graph;
pub mod hosting_api;
pub mod hosting;
pub mod jobs;
pub mod manage;
pub mod metrics;
pub mod operation;
pub mod pagination;
pub mod prefs;
pub mod process_tree;
pub mod publishing;
pub mod read_jobs;
pub mod rewrite;
pub mod sample;
pub mod session;
pub mod settings_file;
pub mod shell;
pub mod terminal;
pub mod updates;
pub mod watch_plan;
pub mod watch;
mod worktrees;
#[cfg(test)]
mod regressions;

/// The shared async runtime for network work done from blocking calls.
/// Desktop shells hand this to their own async layer so everything runs on one runtime.
pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("gitcraft-async")
            .build().expect("could not start the async runtime")
    })
}
