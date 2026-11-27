//! The window: title bar, menu bar, tabs, the tab's screen and the status bar, laid out from
//! top to bottom.

pub mod frame;
mod agents;
mod ai;
mod avatars;
pub mod bench;
mod chrome;
mod commands;
mod conflict;
mod ctxmenu;
mod dialogs;
mod diff;
mod external_tools;
mod fonts;
mod graph;
mod guard;
mod highlight;
mod history;
mod keys;
mod hosting;
mod icons;
mod left;
mod locale;
mod manage;
mod markdown;
mod menu;
mod patches;
pub mod placement;
mod platform;
mod prefs;
mod prefs_ui;
mod rebase;
pub mod repo;
mod right;
mod screen;
mod tabs;
pub(crate) mod updater;
mod terminal;
mod textdiff;
pub mod theme;
mod toolbar;
mod weblinks;
mod widgets;

use crate::settings::{Store, ZOOM_FACTORS};
use gitcraft_core::actions::{Action, DesktopActionResult, Engine};
use gitcraft_core::session::{HistoryPage, RepoView};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
pub enum TabKind {
    /// The repositories list (a new tab).
    Start,
    Repo(String),
    /// Pull requests and issues across services, opened from the rocket in the tabs bar.
    Launchpad,
}

#[derive(Clone, Debug)]
pub struct Tab {
    pub id: u64,
    pub kind: TabKind,
    pub title: String,
}

impl Tab {
    pub fn path(&self) -> Option<&str> {
        match &self.kind {
            TabKind::Repo(path) => Some(path),
            _ => None,
        }
    }
}

pub struct Toast {
    pub ok: bool,
    pub text: String,
    pub until: Instant,
}

/// One entry of the bell's list.
pub struct Notice {
    pub text: String,
    pub at: i64,
    pub read: bool,
}

/// A long job's progress (clone, fetch, pull, push).
#[derive(Clone)]
pub struct Job {
    pub id: String,
    pub text: String,
    pub percent: Option<u32>,
}

/// What background work hands back to the window.
pub enum Msg {
    Nothing,
    /// Work done away from the window, applied to the app when it arrives.
    Apply(Box<dyn FnOnce(&mut App) + Send>),
    Toast(bool, String),
    Copy(String),
    OpenRepo(String),
    /// A missing repository's tab was pointed at this repository: replace the tab with it.
    Located { tab: u64, path: String },
    Repo { tab: u64, generation: u64, result: Box<Result<RepoView, String>> },
    Page { tab: u64, generation: u64, request: String, result: Box<Result<HistoryPage, String>> },
    Status { tab: u64, generation: u64, result: Result<gitcraft_core::git::WorkStatus, String> },
    Changed { path: String, full: bool },
    Watching { tab: u64, result: Result<Arc<gitcraft_core::watch::RepoWatch>, String> },
    Acted { tab: u64, path: String, action: Box<Action>, quiet: bool, result: DesktopActionResult },
    CommitFiles { tab: u64, sha: String, files: Vec<gitcraft_core::git::CommitFile>, signature: String },
    CommitDetails { tab: u64, sha: String, result: Result<gitcraft_core::git::Commit, String> },
    RangeFiles { tab: u64, request: u64, key: (String, String), result: Result<Vec<gitcraft_core::git::FileChange>, String> },
    Stats { tab: u64, stats: Vec<gitcraft_core::files::Stats> },
    Side { tab: u64, side: Box<left::SideData> },
    Locks { tab: u64, result: Result<Vec<gitcraft_core::manage::LfsLock>, String> },
    Plan { tab: u64, result: Result<rebase::RebasePlan, String> },
    QuickEdit { tab: u64, result: Result<Option<Action>, String> },
    Cloned { token: u64, dir: String, result: Result<String, String> },
    Initialized(u64, Result<gitcraft_core::manage::InitOutcome, String>),
    Mono { title: String, result: Result<String, String> },
    Job(Job),
    Tracked { request: u64, result: Result<Vec<String>, String> },
    Diff { tab: u64, key: String, generation: u64, versions: Box<gitcraft_core::files::FileVersions>, lines: Option<Vec<gitcraft_core::git::DiffLine>> },
    Saved { tab: u64, file: String, editor: u64, edits: u64, token: Option<String>, result: Result<(), String> },
    FileHistory { tab: u64, file: String, request: u64, result: Result<gitcraft_core::files::FileHistoryPage, String> },
    ConflictInfo { tab: u64, file: String, request: u64, info: Box<gitcraft_core::rewrite::ConflictInfo> },
    ConflictSaved { tab: u64, result: Result<String, String> },
    Ai { id: String, result: Box<Result<ai::AiOutput, String>> },
    AiPreview { tab: u64, kind: u8, result: Result<(Option<u32>, Vec<String>), String> },
    Blame { tab: u64, key: (String, Option<String>, bool), request: u64, result: Result<Vec<gitcraft_core::files::BlameLine>, String> },
}

/// Where background threads leave [`Msg`]s; the window takes them each frame.
#[derive(Clone, Default)]
pub struct Inbox(std::sync::Arc<std::sync::Mutex<Vec<Msg>>>);

impl Inbox {
    pub fn send(&self, ok: bool, text: impl Into<String>) {
        self.post(Msg::Toast(ok, text.into()));
    }

    pub fn post(&self, msg: Msg) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push(msg);
    }

    fn take(&self) -> Vec<Msg> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

/// What the program was started with.
#[derive(Default)]
pub struct StartOptions {
    /// Draw a few frames, save the window as a PNG and quit.
    pub screenshot: Option<PathBuf>,
    /// The theme for this run only (picture-taking runs).
    pub theme: Option<String>,
    /// Repositories to open in tabs and clone links to offer.
    pub launch: Vec<gitcraft_core::session::LaunchIntent>,
    /// A repository window of its own (Open Repo in New Window): no saved tabs, no hand-off.
    pub new_window: bool,
    /// The running window's hand-off server, kept until GitCraft quits.
    pub primary: Option<Box<crate::instance::Primary>>,
    /// Picture-taking runs: which view to show (diff, inline, split, history, blame, palette).
    pub shot_view: Option<String>,
    /// The engine, when `main` started reading the first repository with it.
    pub engine: Option<Arc<Engine>>,
}

pub struct App {
    pub(crate) settings: Store,
    pub(crate) tabs: Vec<Tab>,
    pub(crate) active: u64,
    /// Recently closed tabs, newest last, for Reopen Closed Tab.
    pub(crate) closed: Vec<Tab>,
    next_id: u64,
    pub(crate) toasts: Vec<Toast>,
    pub(crate) tab_drag: Option<tabs::Drag>,
    pub(crate) inbox: Inbox,
    screenshot: Option<PathBuf>,
    shot_view: Option<String>,
    shot_at: u64,
    shot_since: Instant,
    shot_wait: u64,
    screenshot_sent: bool,
    frames: u64,
    movable: Option<bool>,
    /// Where the tabs bar ends, so Preferences start below it.
    below_tabs: f32,
    pub(crate) ctx: egui::Context,
    pub(crate) engine: Arc<Engine>,
    pub(crate) repos: HashMap<u64, repo::RepoTab>,
    pub(crate) dialog: Option<dialogs::Dialog>,
    pub(crate) pickers: craft_dialogs::Dialogs<dialogs::Pick>,
    /// What the status bar says is running.
    pub(crate) busy: Option<String>,
    sample_loading: bool,
    pub(crate) job: Option<Job>,
    pub(crate) drafts: HashMap<String, repo::Draft>,
    /// Drafts to set once the action that asked for them is queued.
    pub(crate) pending_drafts: Vec<(String, repo::Draft)>,
    /// What waits for the saves chosen in the unsaved-changes prompt.
    pub(crate) leaving: Option<guard::Leaving>,
    /// The window was asked to close while another dialog was open.
    pub(crate) close_pending: bool,
    /// Discard was chosen for a close or restart: unsaved text no longer holds it up.
    pub(crate) discarding: bool,
    /// The benchmark probe (`GITCRAFT_BENCH`).
    bench: Option<bench::Bench>,
    pub(crate) notices: Vec<Notice>,
    /// The left panel shown over an open file until the file closes.
    pub(crate) left_peek: bool,
    pub(crate) focus_left_filter: std::cell::Cell<bool>,
    /// Cached diffs, by tab and file.
    pub(crate) diffs: HashMap<(u64, String), diff::DiffCache>,
    shown_tab: u64,
    pub(crate) palette: Option<keys::Palette>,
    pub(crate) palette_files: Option<Vec<String>>,
    /// Counts palette openings, so a file list for an earlier one is dropped.
    pub(crate) palette_request: u64,
    /// The bulk pull or fetch running in Repository Management (its number and the count
    /// shown), and how many have been started.
    pub(crate) bulk: Option<(u64, usize)>,
    pub(crate) bulk_batch: u64,
    /// The bulk run's status-bar text, kept apart from the tabs' own so ending one never clears the other.
    pub(crate) bulk_message: Option<String>,
    /// Repository paths reserved by background fetch and bulk operations, including queued work.
    pub(crate) busy_repos: Vec<String>,
    /// Counts diff reads, so a read only fills the cache entry it was started for.
    pub(crate) diff_reads: u64,
    /// Counts file-history reads, so a page is added only by the latest read.
    pub(crate) history_reads: u64,
    pub(crate) shortcuts: Option<keys::ShortcutsUi>,
    /// The command whose new keys are being recorded.
    pub(crate) recording: Option<String>,
    /// Focus the commit summary of this tab next frame.
    pub(crate) focus_summary: Option<u64>,
    pub(crate) ai_requests: u64,
    /// AI requests in flight (their read ids, for Stop AI).
    pub(crate) ai_reads: Vec<String>,
    pub(crate) ai_label: Option<String>,
    /// The last few AI explanations (what was explained, title, text), newest first.
    pub(crate) explanations: std::collections::VecDeque<(String, String, String)>,
    /// Recompose with AI is waiting for its soft reset in this tab, to compose the changes after it.
    pub(crate) recompose_pending: Option<(u64, gitcraft_core::actions::RecomposeSource)>,
    /// The pull request being started stacks on another: its tab, branch and the note the form shows.
    pub(crate) pr_stack: Option<(u64, String, String)>,
    /// The preferences revision the backend last heard about.
    pub(crate) synced: u64,
    pub(crate) prefs: Option<prefs_ui::PrefsUi>,
    pub(crate) prefs_data: prefs_ui::PrefsData,
    pub(crate) terminals: gitcraft_core::terminal::Terminals,
    pub(crate) term_panels: HashMap<u64, terminal::Panel>,
    pub(crate) term_pending: terminal::Pending,
    pub(crate) term_exits: Arc<std::sync::Mutex<Vec<u32>>>,
    pub(crate) hosted: HashMap<u64, hosting::Hosted>,
    pub(crate) launchpad: hosting::Launchpad,
    /// Counts the Launchpad resets, so a page requested before one is dropped.
    pub(crate) launchpad_generation: u64,
    /// A Start a Pull Request dialog waiting for AI text: tab, branch, target.
    /// The pull request form waiting for its AI text: the tab, branch, target, draft choice and the request's id.
    pub(crate) pr_waiting: Option<(u64, String, String, bool, u64)>,
    fetched_at: Option<std::time::Instant>,
    primary: Option<Box<crate::instance::Primary>>,
    /// Clone links waiting for the dialog in front to close.
    pending_clones: Vec<String>,
    pub(crate) update: crate::app::updater::UpdateState,
    fetching: bool,
    fetch_warned: std::collections::HashSet<String>,
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, opts: StartOptions, settings: Store) -> App {
        bench::mark("app");
        egui_extras::install_image_loaders(&cc.egui_ctx);
        fonts::install(&cc.egui_ctx);
        let engine = opts.engine.clone().unwrap_or_default();
        let ctx = cc.egui_ctx.clone();
        let mut app = App {
            settings,
            tabs: Vec::new(),
            active: 0,
            closed: Vec::new(),
            next_id: 1,
            toasts: Vec::new(),
            tab_drag: None,
            inbox: Inbox::default(),
            screenshot: opts.screenshot,
            shot_view: opts.shot_view,
            shot_at: 0,
            shot_since: Instant::now(),
            shot_wait: 40,
            screenshot_sent: false,
            frames: 0,
            movable: None,
            below_tabs: 0.0,
            ctx: ctx.clone(),
            engine: engine.clone(),
            repos: HashMap::new(),
            dialog: None,
            pickers: craft_dialogs::Dialogs::new(Arc::new({
                let ctx = ctx.clone();
                move || ctx.request_repaint()
            })),
            busy: None,
            sample_loading: false,
            job: None,
            drafts: HashMap::new(),
            pending_drafts: Vec::new(),
            leaving: None,
            close_pending: false,
            discarding: false,
            bench: bench::Bench::from_env(),
            notices: Vec::new(),
            left_peek: false,
            focus_left_filter: std::cell::Cell::new(false),
            diffs: HashMap::new(),
            shown_tab: 0,
            palette: None,
            palette_files: None,
            palette_request: 0,
            bulk: None,
            bulk_batch: 0,
            bulk_message: None,
            busy_repos: Vec::new(),
            diff_reads: 0,
            history_reads: 0,
            shortcuts: None,
            recording: None,
            focus_summary: None,
            ai_requests: 0,
            ai_reads: Vec::new(),
            ai_label: None,
            explanations: Default::default(),
            recompose_pending: None,
            pr_stack: None,
            synced: 0,
            prefs: None,
            prefs_data: Default::default(),
            terminals: Default::default(),
            term_panels: HashMap::new(),
            term_pending: Default::default(),
            term_exits: Default::default(),
            hosted: HashMap::new(),
            launchpad: Default::default(),
            launchpad_generation: 0,
            pr_waiting: None,
            fetched_at: None,
            primary: None,
            pending_clones: Vec::new(),
            update: Default::default(),
            fetching: false,
            fetch_warned: Default::default(),
        };
        app.drafts = app.settings.get_root("drafts").unwrap_or_default();
        // Long jobs report progress to the status bar.
        let inbox = app.inbox.clone();
        let wake = ctx.clone();
        engine.jobs.set_listener(Arc::new(move |id: &str, text: &str, percent: Option<u32>| {
            inbox.post(Msg::Job(Job { id: id.to_owned(), text: text.to_owned(), percent }));
            wake.request_repaint();
        }));
        let choice = opts.theme.unwrap_or_else(|| app.settings.theme());
        theme::choose(theme::Theme::from_id(&choice), &cc.egui_ctx);
        cc.egui_ctx.set_zoom_factor(app.settings.zoom());
        // The zoom keys are commands of their own, so they step through the zoom levels and are saved.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        if let Some(warning) = app.settings.warning.take() {
            app.toast(false, warning);
        }

        let (saved, active) = app.settings.tabs();
        let saved = if app.settings.flag("rememberTabs", true) && !opts.new_window { saved } else { Vec::new() };
        if opts.new_window {
            app.settings.shares_tabs = false;
        }
        for path in saved {
            let tab = app.repo_tab(path);
            app.tabs.push(tab);
        }
        if let Some(tab) = app.tabs.get(active.min(app.tabs.len().saturating_sub(1))) {
            app.active = tab.id;
        }
        app.primary = opts.primary;
        crate::instance::attach(&cc.egui_ctx);
        #[cfg(target_os = "macos")]
        if app.screenshot.is_none() {
            crate::macos::register_services();
        }
        app.launch(opts.launch);
        if app.tabs.is_empty() {
            app.new_tab();
        }
        if app.screenshot.is_none() {
            let (git, gitlab) = (app.settings.string("gitPath", ""), app.settings.string("gitlabHost", ""));
            app.spawn(move || {
                let r = gitcraft_core::git::check_version(&git);
                gitcraft_core::hosting::sync_git_credentials(&gitlab);
                Msg::Apply(Box::new(move |app: &mut App| {
                    if let Err(e) = r {
                        app.toast(false, e);
                    }
                }))
            });
        }
        app
    }

    /// Open what a launch asked for: repositories as tabs, clone links in the Clone dialog.
    pub(crate) fn launch(&mut self, intents: Vec<gitcraft_core::session::LaunchIntent>) {
        use gitcraft_core::session::LaunchIntent;
        for intent in intents {
            match intent {
                LaunchIntent::Repository { path } => self.open_repo(&path),
                LaunchIntent::Clone { url } => self.pending_clones.push(url),
            }
        }
    }

    /// Auto-fetch every open repository every few minutes (0 turns it off), pruning when set.
    /// A failure is reported once per repository until it fetches again.
    fn auto_fetch(&mut self, ctx: &egui::Context) {
        let minutes = self.settings.num("autoFetchMin", 5.0).clamp(0.0, 60.0) as u64;
        if minutes == 0 || self.screenshot.is_some() {
            return;
        }
        let every = std::time::Duration::from_secs(minutes * 60);
        let due = self.fetched_at.get_or_insert_with(std::time::Instant::now).elapsed() >= every;
        ctx.request_repaint_after(every.saturating_sub(self.fetched_at.map_or(every, |t| t.elapsed())).max(std::time::Duration::from_secs(1)));
        if !due || self.fetching || self.busy.is_some() || self.job.is_some() {
            return;
        }
        self.fetched_at = Some(std::time::Instant::now());
        let mut paths: Vec<(u64, String)> = Vec::new();
        for t in &self.tabs {
            if let (TabKind::Repo(p), Some(r)) = (&t.kind, self.repos.get(&t.id))
                && r.view.as_ref().is_some_and(|v| !v.data.remotes.is_empty())
                && !self.repo_pull_busy(p)
                && !paths.iter().any(|(_, q)| same_folder(q, p))
            {
                paths.push((t.id, p.clone()));
            }
        }
        if paths.is_empty() {
            return;
        }
        self.fetching = true;
        self.busy_repos.extend(paths.iter().map(|(_, p)| p.clone()));
        let prune = self.settings.flag("autoPrune", false);
        let engine = self.engine.clone();
        let inbox = self.inbox.clone();
        let repaint = ctx.clone();
        self.spawn(move || {
            for (tab, path) in paths {
                let r = engine.perform(&path, Action::AutoFetch { prune });
                inbox.post(Msg::Apply(Box::new(move |app: &mut App| {
                    app.finish_busy_repo(&path);
                    if r.ok {
                        app.fetch_warned.remove(&path);
                        if !r.message.is_empty() && app.repos.contains_key(&tab) {
                            app.load_repo(tab);
                        }
                    } else if app.fetch_warned.insert(path) {
                        app.toast(false, r.message);
                    }
                })));
                repaint.request_repaint();
            }
            Msg::Apply(Box::new(|app: &mut App| app.fetching = false))
        });
    }

    fn repo_pull_busy(&self, path: &str) -> bool {
        repo::pull_busy(false, &self.busy_repos, path)
    }

    fn finish_busy_repo(&mut self, path: &str) {
        if let Some(i) = self.busy_repos.iter().position(|p| same_folder(p, path)) {
            self.busy_repos.remove(i);
        }
    }

    fn take_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn repo_tab(&mut self, path: String) -> Tab {
        let title = self.settings.alias(&path).unwrap_or_else(|| folder_name(&path));
        let id = self.take_id();
        self.ensure_repo(id, &path);
        Tab { id, kind: TabKind::Repo(path), title }
    }

    /// Bring a tab to the front.
    pub(crate) fn activate(&mut self, id: u64) {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active = id;
            self.remember_tabs();
        }
    }

    pub(crate) fn active_tab(&self) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == self.active)
    }

    pub(crate) fn new_tab(&mut self) {
        let tab = Tab { id: self.take_id(), kind: TabKind::Start, title: "New Tab".into() };
        self.active = tab.id;
        self.tabs.push(tab);
        self.remember_tabs();
    }

    pub(crate) fn open_launchpad(&mut self) {
        if let Some(tab) = self.tabs.iter().find(|t| t.kind == TabKind::Launchpad) {
            self.active = tab.id;
            return;
        }
        let tab = Tab { id: self.take_id(), kind: TabKind::Launchpad, title: "Launchpad".into() };
        self.active = tab.id;
        self.tabs.push(tab);
    }

    /// Show the repository at `path`: its tab when one is open, otherwise in place of the start
    /// screen in front, otherwise in a new tab.
    pub(crate) fn open_repo(&mut self, path: &str) {
        // A folder already known under another spelling (a `\\?\` path on Windows, a trailing
        // separator) keeps the name its settings, drafts and aliases are stored under.
        let known = self.tabs.iter().filter_map(|t| t.path().map(str::to_owned)).chain(self.settings.recents()).find(|p| same_folder(p, path));
        let path = known.as_deref().unwrap_or(path);
        let mut recents = self.settings.recents();
        recents.retain(|r| !same_folder(r, path));
        recents.insert(0, path.to_owned());
        recents.truncate(20);
        self.settings.set("recents", recents);
        if let Some(tab) = self.tabs.iter().find(|t| t.path().is_some_and(|p| same_folder(p, path))) {
            self.active = tab.id;
            return self.remember_tabs();
        }
        let tab = self.repo_tab(path.to_owned());
        let id = tab.id;
        let active = self.active;
        match self.tabs.iter_mut().find(|t| t.id == active && t.kind == TabKind::Start) {
            Some(start) => *start = tab,
            None => self.tabs.push(tab),
        }
        self.active = id;
        self.remember_tabs();
    }

    /// The tab for `path`, opened in the background if it isn't open yet.
    pub(crate) fn repo_tab_id(&mut self, path: &str) -> u64 {
        if let Some(tab) = self.tabs.iter().find(|t| t.path().is_some_and(|p| same_folder(p, path))) {
            return tab.id;
        }
        let tab = self.repo_tab(path.to_owned());
        let id = tab.id;
        self.tabs.push(tab);
        self.remember_tabs();
        id
    }

    /// A missing repository was found at `path`: its tab shows that repository in the same place, without being
    /// counted as a closed tab. When the repository is already open in another tab, that tab is used instead.
    fn locate_tab(&mut self, tab: u64, path: &str) {
        let Some(at) = self.tabs.iter().position(|t| t.id == tab) else { return };
        let known = self.tabs.iter().filter_map(|t| t.path().map(str::to_owned)).chain(self.settings.recents()).find(|p| same_folder(p, path));
        let path = known.as_deref().unwrap_or(path).to_owned();
        let mut recents = self.settings.recents();
        recents.retain(|r| !same_folder(r, &path));
        recents.insert(0, path.clone());
        recents.truncate(20);
        self.settings.set("recents", recents);
        let existing = self.tabs.iter().find(|t| t.id != tab && t.path().is_some_and(|p| same_folder(p, &path))).map(|t| t.id);
        let replacement = existing.is_none().then(|| self.repo_tab(path.clone()));
        let old = match replacement {
            Some(new) => {
                if self.active == tab {
                    self.active = new.id;
                }
                std::mem::replace(&mut self.tabs[at], new)
            }
            None => {
                let old = self.tabs.remove(at);
                if self.active == tab {
                    self.active = existing.unwrap_or(0);
                }
                old
            }
        };
        if let Some(mut r) = self.repos.remove(&old.id) {
            r.cancel_reads();
        }
        if let Some(panel) = self.term_panels.remove(&old.id) {
            for s in panel.sessions {
                if let Some(t) = s.id {
                    self.terminals.close("main", t);
                }
            }
        }
        self.diffs.retain(|k, _| k.0 != old.id);
        self.remember_tabs();
    }

    pub(crate) fn close_tab(&mut self, id: u64) {
        self.close_tabs(vec![id]);
    }

    /// Close tabs, asking first when one has unsaved editor text.
    pub(crate) fn close_tabs(&mut self, ids: Vec<u64>) {
        if self.ask_unsaved(ids.clone(), true, Box::new({
            let ids = ids.clone();
            move |app: &mut App| app.close_tabs(ids)
        })) {
            return;
        }
        for id in ids {
            self.close_tab_now(id);
        }
    }

    fn close_tab_now(&mut self, id: u64) {
        let Some(at) = self.tabs.iter().position(|t| t.id == id) else { return };
        let tab = self.tabs.remove(at);
        if let Some(mut r) = self.repos.remove(&id) {
            r.cancel_reads();
        }
        if let Some(panel) = self.term_panels.remove(&id) {
            for s in panel.sessions {
                if let Some(t) = s.id {
                    self.terminals.close("main", t);
                }
            }
        }
        self.diffs.retain(|k, _| k.0 != id);
        if tab.kind != TabKind::Launchpad {
            self.closed.push(tab);
            if self.closed.len() > 20 {
                self.closed.remove(0);
            }
        }
        if self.active == id {
            let next = self.tabs.get(at).or_else(|| self.tabs.get(at.wrapping_sub(1))).or(self.tabs.last());
            self.active = next.map_or(0, |t| t.id);
        }
        if self.tabs.iter().all(|t| t.kind == TabKind::Launchpad) {
            self.new_tab();
        }
        self.remember_tabs();
    }

    pub(crate) fn reopen_tab(&mut self) {
        if let Some(id) = self.closed.last().map(|tab| tab.id) {
            self.reopen_closed_tab(id);
        }
    }

    fn reopen_closed_tab(&mut self, id: u64) {
        let Some(index) = self.closed.iter().position(|tab| tab.id == id) else { return };
        let mut tab = self.closed.remove(index);
        // A repository opened again since its tab was closed: go to that tab, so one folder
        // never has two tabs (file changes refresh only one of them).
        if let Some(path) = tab.path()
            && let Some(open) = self.tabs.iter().find(|t| t.path().is_some_and(|p| same_folder(p, path)))
        {
            self.activate(open.id);
            return;
        }
        if let Some(path) = tab.path().map(str::to_owned) {
            tab = self.repo_tab(path);
        } else {
            tab.id = self.take_id();
        }
        self.active = tab.id;
        self.tabs.push(tab);
        self.remember_tabs();
    }

    /// Move a tab `by` places among the tabs shown in the strip.
    pub(crate) fn shift_tab(&mut self, id: u64, by: isize) {
        // The Launchpad tab isn't in the strip, so it is no neighbour to swap with.
        let shown: Vec<usize> = self.tabs.iter().enumerate().filter(|(_, t)| t.kind != TabKind::Launchpad).map(|(i, _)| i).collect();
        let Some(at) = shown.iter().position(|&i| self.tabs[i].id == id) else { return };
        let to = at as isize + by;
        if to >= 0 && (to as usize) < shown.len() {
            self.tabs.swap(shown[at], shown[to as usize]);
            self.remember_tabs();
        }
    }

    /// Note the repository tabs and the one in front, for the next start.
    pub(crate) fn remember_tabs(&mut self) {
        let repos: Vec<&Tab> = self.tabs.iter().filter(|t| t.path().is_some()).collect();
        let active = repos.iter().position(|t| t.id == self.active).unwrap_or(0);
        let paths = repos.iter().filter_map(|t| t.path().map(str::to_owned)).collect();
        self.settings.set_tabs(paths, active);
    }

    pub(crate) fn toast(&mut self, ok: bool, text: impl Into<String>) {
        let text = text.into();
        if !ok {
            self.notify(&text);
        }
        self.toasts.push(Toast { ok, text, until: Instant::now() + Duration::from_secs(if ok { 4 } else { 8 }) });
        if self.toasts.len() > 5 {
            self.toasts.remove(0);
        }
    }

    pub(crate) fn set_zoom(&mut self, ctx: &egui::Context, zoom: f32) {
        ctx.set_zoom_factor(zoom);
        self.settings.set("zoom", zoom);
    }

    /// One step larger (`up`) or smaller through the zoom steps.
    pub(crate) fn step_zoom(&mut self, ctx: &egui::Context, up: bool) {
        let now = self.settings.zoom();
        let next = if up {
            ZOOM_FACTORS.iter().copied().filter(|z| *z > now + 0.001).fold(None, |m: Option<f32>, z| Some(m.map_or(z, |m| m.min(z))))
        } else {
            ZOOM_FACTORS.iter().copied().filter(|z| *z < now - 0.001).fold(None, |m: Option<f32>, z| Some(m.map_or(z, |m| m.max(z))))
        };
        if let Some(zoom) = next {
            self.set_zoom(ctx, zoom);
        }
    }

    fn drive_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        if self.screenshot_sent {
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(16));
        // Reserve a little time for rendering and bound the wait for each load in wall time.
        let timeout = Duration::from_secs(8);
        let reading = self.repos.values().any(|r| r.loading || r.page_loading || (r.view.is_none() && r.error.is_none()) || r.running.is_some());
        if self.frames < 30 || (reading && self.shot_since.elapsed() < timeout) {
            return;
        }
        if let Some(view) = self.shot_view.take() {
            self.show_for_picture(&view);
            self.shot_at = self.frames;
            self.shot_since = Instant::now();
            return;
        }
        let busy = self.diffs.values().any(|d| matches!(d.versions, Some(repo::Loaded::Loading)) || d.comparing.is_some())
            || self.repos.values().any(|r| r.history_ui.loading || matches!(r.conflict_ui.info, Some(repo::Loaded::Loading)));
        if self.shot_at > 0 && (self.frames < self.shot_at + self.shot_wait || (busy && self.shot_since.elapsed() < timeout)) {
            return;
        }
        self.screenshot_sent = true;
        let target = ctx.clone();
        ctx.request_screenshot(move |image| {
            let [w, h] = image.size;
            let mut rgba = Vec::with_capacity(w * h * 4);
            for c in &image.pixels {
                rgba.extend_from_slice(&c.to_srgba_unmultiplied());
            }
            match save_png(&path, w as u32, h as u32, &rgba) {
                Ok(()) => println!("Saved window picture to {}", path.display()),
                Err(e) => eprintln!("Could not save the window picture: {e}"),
            }
            target.send_viewport_cmd(egui::ViewportCommand::Close);
            target.request_repaint();
        });
    }
}

impl eframe::App for App {
    /// Pictures are taken without a mouse pointer, so no hover shows in them.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if let Some(b) = self.bench.as_mut() {
            b.input(_ctx, raw_input);
        }
        if self.screenshot.is_some() {
            raw_input.events.retain(|e| !matches!(e, egui::Event::PointerMoved(_) | egui::Event::PointerButton { .. }));
            raw_input.events.push(egui::Event::PointerGone);
        }
    }

    fn on_exit(&mut self) {
        if let Some(p) = self.primary.as_mut() {
            p.shutdown();
        }
        self.terminals.close_all();
        gitcraft_core::process_tree::shutdown();
        if let Err(e) = self.settings.flush() {
            eprintln!("Could not save preferences: {e}");
        }
        // Picture-taking runs leave the saved window geometry alone.
        if self.screenshot.is_none() {
            placement::save();
        }
    }

    /// Closing the window quits the way the system expects.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(windows)]
        if self.screenshot.is_none() {
            placement::position_window(ctx, _frame);
        }
        self.guard_close(ctx);
        craft_quit::on_close_request(ctx);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if frame::rounded() { [0.0; 4] } else { theme::p().app.to_normalized_gamma_f32() }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let built = Instant::now();
        if self.frames == 0 {
            bench::mark("first-frame");
        }
        self.frames += 1;
        theme::follow_system(&ctx);
        // UI Customization > Display author initials … instead of avatars.
        avatars::enable(!self.settings.flag("authorInitials", false));
        for msg in self.inbox.take() {
            self.handle(msg);
        }
        for (path, draft) in std::mem::take(&mut self.pending_drafts) {
            self.set_draft(&path, draft);
        }
        if self.active != self.shown_tab {
            self.shown_tab = self.active;
            self.left_peek = false;
        }
        self.take_picks();
        let arrived = crate::instance::take();
        if !arrived.is_empty() {
            self.launch(arrived);
        }
        if self.dialog.is_none()
            && let Some(url) = self.pending_clones.first().cloned()
        {
            self.pending_clones.remove(0);
            self.clone_dialog(&url);
        }
        self.sync_backend();
        self.pump_terminals();
        self.auto_fetch(&ctx);
        self.schedule_updates(&ctx);
        ctx.data_mut(|d| d.insert_temp(egui::Id::NULL.with("dialog_open"), self.dialog.is_some()));
        if self.shortcuts.is_none() {
            self.dispatch_keys(&ctx);
        }

        frame::title_bar(ui, "GitCraft");
        self.menu_bar(ui);
        self.tabs_bar(ui);
        // Preferences cover everything below the tabs: what they hide isn't drawn, so it can't
        // take focus or be reached by screen readers.
        if self.prefs.is_none() {
            self.status_bar(ui);
            self.screen(ui);
        }
        self.draw_toasts(&ctx);
        self.draw_prefs(&ctx);
        self.draw_palette(&ctx);
        self.draw_shortcuts(&ctx);
        // After everything that can open another file or close one, before the prompt it asks.
        self.guard_unsaved_moves();
        self.draw_dialog(&ctx);

        frame::window_edges(&ctx);
        frame::after_ui(&ctx, &mut self.movable);
        if self.screenshot.is_none() {
            placement::track(&ctx);
        }
        if let Err(e) = self.settings.save_when_due(&ctx) {
            self.toast(false, format!("Could not save preferences: {e}"));
        }
        // Another window changed the preferences: follow its theme and zoom here too.
        if self.settings.take_reloaded() {
            theme::choose(theme::Theme::from_id(&self.settings.theme()), &ctx);
            ctx.set_zoom_factor(self.settings.zoom());
        }
        self.drive_screenshot(&ctx);
        if let Some(b) = self.bench.as_mut() {
            let has_graph = self.repos.get(&self.active).and_then(|r| r.view.as_ref()).is_some_and(|v| !v.data.commits.is_empty());
            b.frame(&ctx, has_graph, built, _frame.info().cpu_usage);
        }
        // Last, so a file dialog asked for anywhere this frame shows now, over this window.
        self.pickers.show_pending(Some(&*_frame));
    }
}

impl App {
    /// Picture-taking runs: open a view of the first repository tab.
    fn show_for_picture(&mut self, view: &str) {
        // The palette needs no repository: it shows over the Start tab too.
        if view == "palette" {
            self.open_palette(keys::Picker::Root);
            return;
        }
        let tab = self.active;
        let Some(r) = self.repo_mut(tab) else { return };
        let Some(v) = r.view.as_ref() else { return };
        use gitcraft_core::git::DiffSource;
        let file = v.data.status.unstaged.first().map(|f| (f.path.clone(), f.status == '?', DiffSource::Unstaged)).or_else(|| v.data.status.staged.first().map(|f| (f.path.clone(), false, DiffSource::Staged)));
        let commit = v.data.commits.iter().find(|c| c.parents.len() == 1).map(|c| c.sha.clone());
        match view {
            "agents" => {
                self.settings.set("leftView", "agents");
                self.shot_wait = 60;
            }
            "launchpad" => {
                self.open_launchpad();
                self.shot_wait = 60;
            }
            "diff" | "inline" | "split" => {
                if let Some((path, untracked, source)) = file {
                    r.ui.diff = Some(repo::DiffTarget { source, file: path, untracked, parent: None });
                }
                let mode = if view == "diff" { "hunk" } else { view };
                self.settings.set("diffMode", mode);
            }
            "history" | "blame" => {
                let tracked = gitcraft_core::files::tracked_files(std::path::Path::new(&r.path)).ok().and_then(|f| f.into_iter().next());
                if let Some(f) = tracked {
                    r.ui.history = Some(repo::HistoryTarget { file: f, mode: if view == "blame" { repo::HistoryMode::Blame } else { repo::HistoryMode::History }, sha: None });
                }
            }
            "commit" => {
                if let Some(sha) = commit {
                    r.ui.select(repo::Selection::Commit(sha));
                }
            }
            "prefs" => self.open_prefs(None),
            "terminal" => {
                self.settings.set("showTerminal", true);
                self.shot_wait = 150;
            }
            "integrations" => self.open_prefs(Some("integrations")),
            "ai" => self.open_prefs(Some("ai")),
            "conflict" => {
                if let Some(f) = v.data.status.unstaged.iter().find(|f| f.status == 'U') {
                    r.ui.conflict = Some(f.path.clone());
                }
            }
            "rebase" => {
                let base = v.data.commits.get(3).map(|c| c.sha.clone()).unwrap_or_else(|| "--root".into());
                let onto: String = base.chars().take(6).collect();
                self.open_rebase(tab, &base, &onto, false);
            }
            _ => {}
        }
    }

    fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Nothing => {}
            Msg::Apply(f) => f(self),
            Msg::Toast(ok, text) => self.toast(ok, text),
            Msg::Copy(text) => self.copy(&text),
            Msg::OpenRepo(path) => self.open_repo(&path),
            Msg::Located { tab, path } => self.locate_tab(tab, &path),
            Msg::Repo { tab, generation, result } => self.repo_loaded(tab, generation, *result),
            Msg::Page { tab, generation, request, result } => self.page_loaded(tab, generation, &request, *result),
            Msg::Status { tab, generation, result } => self.status_loaded(tab, generation, result),
            Msg::Changed { path, full } => self.changed(&path, full),
            Msg::Watching { tab, result } => self.watching(tab, result),
            Msg::Acted { tab, path, action, quiet, result } => {
                self.finish_busy_repo(&path);
                let result = match result {
                    DesktopActionResult::Performed(result) => result,
                    DesktopActionResult::OpenWorktree { path, action } => {
                        self.checkout_opened_worktree(tab, &path);
                        if let Some(action) = action {
                            let dest = self.repo_tab_id(&path);
                            self.act_quiet(dest, action, quiet);
                        }
                        return;
                    }
                };
                // The submodule list in Preferences is read again only after the change is in place, so a frame drawn
                // meanwhile can't fill the cache with the old list.
                if matches!(*action, Action::SubmoduleAdd { .. } | Action::SubmoduleUpdate { .. } | Action::SubmoduleRemove { .. })
                    && let Some(p) = self.prefs.as_mut()
                {
                    p.submodules = None;
                }
                // General > Delete ".orig" files after merging: the backups a merge tool left during this operation.
                if result.ok
                    && matches!(*action, Action::Merge { .. } | Action::MergeSquash { .. } | Action::Rebase { .. } | Action::CherryPick { .. } | Action::CherryPickMany { .. } | Action::InteractiveRebase { .. } | Action::Commit { .. } | Action::ContinueRebase | Action::SkipRebase | Action::SaveResolved { .. })
                    && self.settings.flag("deleteOrigAfterMerge", true)
                {
                    self.clean_orig_files(tab);
                }
                if matches!(*action, Action::Reset { .. }) {
                    self.recompose_reset_done(tab, result.ok);
                }
                // A checkout, merge, rebase or squash merge shows the working changes they leave behind.
                if result.ok
                    && matches!(*action, Action::Checkout { .. } | Action::CheckoutRemote { .. } | Action::Merge { .. } | Action::MergeSquash { .. } | Action::Rebase { .. })
                    && let Some(r) = self.repo_mut(tab)
                {
                    r.ui.select(repo::Selection::Wip);
                }
                // Keep submodules up to date: after a pull, checkout or squash merge, by the repository's choice or the global one.
                if result.ok && matches!(*action, Action::Pull { .. } | Action::Checkout { .. } | Action::CheckoutRemote { .. } | Action::Merge { .. } | Action::MergeSquash { .. } | Action::Rebase { .. }) {
                    self.update_submodules_after(tab);
                }
                if let Action::Stash { message } = &*action
                    && result.ok
                    && let Some(r) = self.repo_mut(tab)
                    && r.work.stash_message == *message
                {
                    // Newer text typed while the stash ran is kept.
                    r.work.stash_message.clear();
                }
                if let Action::MergeSquash { branch, into, .. } = &*action
                    && result.ok
                {
                    self.squash_done(tab, branch, into.as_deref());
                }
                if let Action::TakeSide { paths, .. } = &*action
                    && result.ok
                {
                    self.take_side_done(tab, paths);
                }
                let worktree = (matches!(*action, Action::WorktreeAdd { .. } | Action::WorktreeRemove { .. }) && result.ok).then(|| (*action).clone());
                self.acted(tab, *action, quiet, result);
                if let Some(action) = worktree {
                    self.worktree_done(tab, &action);
                }
            }
            Msg::CommitFiles { tab, sha, files, signature } => {
                if let Some(r) = self.repo_mut(tab) {
                    r.signatures.insert(sha.clone(), signature);
                    r.commit_files.insert(sha, repo::Loaded::Ready(files));
                }
            }
            Msg::CommitDetails { tab, sha, result } => {
                if let Some(r) = self.repo_mut(tab) {
                    r.commit_details.insert(sha, match result {
                        Ok(c) => repo::Loaded::Ready(c),
                        Err(e) => repo::Loaded::Failed(e),
                    });
                }
            }
            Msg::RangeFiles { tab, request, key, result } => {
                if let Some(r) = self.repo_mut(tab)
                    // Only the read the entry still waits for: one evicted or asked again meanwhile is dropped.
                    && matches!(r.range_files.get(&key), Some(repo::Loaded::Loading))
                    && r.range_requests.get(&key) == Some(&request)
                {
                    r.range_requests.remove(&key);
                    r.range_files.insert(key, match result {
                        Ok(f) => repo::Loaded::Ready(f),
                        Err(e) => repo::Loaded::Failed(e),
                    });
                }
            }
            Msg::Stats { tab, stats } => {
                if let Some(r) = self.repo_mut(tab) {
                    for s in stats {
                        r.stats.insert(s.sha.clone(), s);
                    }
                }
            }
            Msg::Side { tab, side } => self.side_loaded(tab, *side),
            Msg::Locks { tab, result } => {
                if let Some(r) = self.repo_mut(tab) {
                    r.side.locks = Some(result);
                    r.side.locks_loading = false;
                }
            }
            Msg::Plan { tab, result } => self.plan_ready(tab, result),
            Msg::QuickEdit { tab, result } => self.quick_ready(tab, result),
            Msg::Cloned { token, dir, result } => self.cloned(token, dir, result),
            Msg::Initialized(token, result) => self.initialized(token, result),
            Msg::Mono { title, result } => match result {
                Ok(text) => self.dialog = Some(dialogs::Dialog::new(&title, dialogs::Body::Mono(text)).width(820.0).cancel_label("Close")),
                Err(e) => self.toast(false, e),
            },
            Msg::Job(job) => {
                self.engine.jobs.acknowledge(&job.id);
                self.job = Some(job);
            }
            Msg::Tracked { request, result } => self.tracked_loaded(request, result),
            Msg::Diff { tab, key, generation, versions, lines } => self.diff_loaded(tab, key, generation, *versions, lines),
            Msg::Saved { tab, file, editor, edits, token, result } => self.saved(tab, file, editor, edits, token, result),
            Msg::FileHistory { tab, file, request, result } => self.history_loaded(tab, file, request, result),
            Msg::ConflictInfo { tab, file, request, info } => self.conflict_loaded(tab, file, request, *info),
            Msg::ConflictSaved { tab, result } => self.conflict_saved(tab, result),
            Msg::Ai { id, result } => self.ai_done(id, *result),
            Msg::AiPreview { tab, kind, result } => self.ai_preview(tab, kind, result),
            Msg::Blame { tab, key, request, result } => {
                if let Some(r) = self.repo_mut(tab)
                    && r.history_ui.blame_reads.get(&key) == Some(&request)
                {
                    r.history_ui.blame_reads.remove(&key);
                    r.history_ui.blame.insert(key, match result {
                        Ok(l) => repo::Loaded::Ready(l),
                        Err(e) => repo::Loaded::Failed(e),
                    });
                }
            }
        }
    }
}

/// Whether a dialog covers the window this frame.
pub fn dialog_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::NULL.with("dialog_open"))).unwrap_or(false)
}

/// A number never handed out before in this run: tells apart editors and reads whose own state
/// is reset when a view opens again.
pub fn unique_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// The last part of a path, as a tab title.
pub fn folder_name(path: &str) -> String {
    // A backslash separates names only on Windows; elsewhere it can be part of a folder's name.
    let name = if cfg!(windows) { path.split(['/', '\\']).rfind(|p| !p.is_empty()) } else { path.split('/').rfind(|p| !p.is_empty()) };
    name.unwrap_or(path).to_owned()
}

/// Whether two paths name the same folder, ignoring a trailing separator (and case on Windows).
pub fn same_folder(a: &str, b: &str) -> bool {
    // A backslash is a separator only on Windows; elsewhere it is part of a name. On Windows the
    // extended-length form (`\\?\C:\x`, `\\?\UNC\server\x`) names the same folder as the usual one.
    let trim = |p: &str| {
        if !cfg!(windows) {
            return p.trim_end_matches('/').to_owned();
        }
        let p = p.replace('/', "\\");
        let p = match p.strip_prefix("\\\\?\\UNC\\") {
            Some(rest) => format!("\\\\{rest}"),
            None => p.strip_prefix("\\\\?\\").map(str::to_owned).unwrap_or(p),
        };
        p.trim_end_matches('\\').to_owned()
    };
    if cfg!(windows) { trim(a).eq_ignore_ascii_case(&trim(b)) } else { Path::new(&trim(a)) == Path::new(&trim(b)) }
}

fn save_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_titles_are_the_folder_name() {
        assert_eq!(folder_name("/home/sam/code/gitcraft/"), "gitcraft");
        #[cfg(windows)]
        assert_eq!(folder_name(r"C:\src\app"), "app");
        #[cfg(not(windows))]
        assert_eq!(folder_name(r"/home/a\b"), r"a\b");
        assert!(same_folder("/a/b/", "/a/b"));
        assert!(!same_folder("/a/b", "/a/c"));
    }

    #[test]
    fn carried_actions_find_a_background_tab_for_the_worktree() {
        let ctx = egui::Context::default();
        let cc = eframe::CreationContext::_new_kittest(ctx);
        let mut app = App::new(&cc, StartOptions::default(), crate::settings::Store::in_memory());
        let before = app.tabs.len();
        let path = std::env::temp_dir().join(format!("gitcraft-carried-tab-{}", std::process::id()));
        let path = path.to_string_lossy().to_string();
        let id = app.repo_tab_id(&path);
        assert_eq!(app.tabs.len(), before + 1);
        assert_eq!(app.repo_tab_id(&path), id);
        assert_eq!(app.tabs.len(), before + 1);
    }
}
