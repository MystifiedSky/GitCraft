//! A repository tab's data and what the tab is showing: reading the repository away from the
//! window, more history pages, status-only refreshes after small changes, the file watcher,
//! and Git actions run one after another per repository.

use super::{App, Msg};
use gitcraft_core::actions::{Action, ActionResult};
use gitcraft_core::git::{self, Commit, CommitFile, DiffSource, FileChange, HistoryFilter, RefView, WorkStatus};
use gitcraft_core::session::{self, HistoryPage, RepoView};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

pub(super) fn other_worktree<'a>(view: &'a RepoView, branch: &str) -> Option<&'a git::Worktree> {
    view.data.worktrees.iter().find(|w| w.branch == branch && !super::same_folder(&w.path, &view.path))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    Wip,
    Commit(String),
}

/// A file opened in the centre: whose diff, and from where.
#[derive(Clone, Debug, PartialEq)]
pub struct DiffTarget {
    pub source: DiffSource,
    pub file: String,
    pub untracked: bool,
    pub parent: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistoryMode {
    History,
    Blame,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistoryTarget {
    pub file: String,
    pub mode: HistoryMode,
    pub sha: Option<String>,
}

/// Something read for the tab, by key.
pub enum Loaded<T> {
    Loading,
    Ready(T),
    Failed(String),
}

/// What the tab shows besides its data.
#[derive(Clone, Default)]
pub struct TabUi {
    pub selection: Option<Selection>,
    pub multi: Vec<String>,
    pub diff: Option<DiffTarget>,
    pub search: String,
    pub search_open: bool,
    pub focus_search: bool,
    pub history_filter: Option<HistoryFilter>,
    pub compare_workdir: Option<String>,
    pub history: Option<HistoryTarget>,
    pub conflict: Option<String>,
    pub rebase: Option<super::rebase::RebasePlan>,
    pub pr: Option<u64>,
    pub issue: Option<String>,
    /// Scroll the graph so the selection shows.
    pub reveal: bool,
}

impl TabUi {
    pub fn selection(&self) -> Selection {
        self.selection.clone().unwrap_or(Selection::Wip)
    }

    /// Show a commit (or the working changes) in the right panel, closing whatever the centre showed.
    pub fn select(&mut self, sel: Selection) {
        self.selection = Some(sel);
        self.diff = None;
        self.multi.clear();
        self.compare_workdir = None;
        self.history = None;
        self.reveal = true;
    }

    /// Whether the centre shows a file, conflict or rebase instead of the graph.
    pub fn viewing(&self) -> bool {
        self.diff.is_some() || self.history.is_some() || self.conflict.is_some() || self.rebase.is_some()
    }
}

/// A commit draft: what the commit box holds, kept per repository.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Draft {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub amend: bool,
}

pub struct RepoTab {
    pub path: String,
    pub view: Option<RepoView>,
    pub index: HashMap<String, usize>,
    pub error: Option<String>,
    pub loading: bool,
    /// Bumped by every change that makes running reads stale.
    pub generation: u64,
    /// What the running full read asks for, and what the history shown was read with: loaded
    /// pages are kept across a refresh only when both are the same.
    requested: Option<RefView>,
    shown_query: Option<RefView>,
    /// Another full read was asked for while one ran.
    again: bool,
    status_loading: bool,
    status_again: bool,
    /// A status read finished while a full read of the repository was running: that read's own status is older.
    status_during_read: bool,
    pub page_loading: bool,
    pub page_error: Option<String>,
    /// The tab is in the background; refresh when it comes to the front (true: a full read).
    pub deferred: Option<bool>,
    pub ui: TabUi,
    watch: Option<gitcraft_core::watch::RepoWatch>,
    watching: bool,
    pub commit_files: HashMap<String, Loaded<Vec<CommitFile>>>,
    pub commit_details: HashMap<String, Loaded<Commit>>,
    pub signatures: HashMap<String, String>,
    pub stats: HashMap<String, gitcraft_core::files::Stats>,
    stats_asked: std::collections::HashSet<String>,
    pub range_files: HashMap<(String, String), Loaded<Vec<FileChange>>>,
    /// The read each `Loading` entry of `range_files` waits for; only that read's answer is installed.
    pub range_requests: HashMap<(String, String), u64>,
    /// Actions waiting to run, oldest first; one runs at a time.
    queue: VecDeque<(Action, bool)>,
    pub running: Option<String>,
    /// A change the watcher saw while an action ran (true: a full read).
    action_change: Option<bool>,
    /// Which toolbar buttons spin: "fetch", "pull", "push", "stash", "pop".
    pub spinning: Option<&'static str>,
    pub work: super::right::WorkUi,
    pub side: super::left::SideData,
    pub diff_ui: super::diff::DiffUi,
    pub history_ui: super::history::HistoryUi,
    pub conflict_ui: super::conflict::ConflictUi,
    /// What the tab showed at the end of the last frame with unsaved editor text, to go back
    /// to when a move away is held for the unsaved-changes prompt.
    pub guard: Option<TabUi>,
    /// After a full read: the selected commit and how many commits were loaded before, paged
    /// back in before the selection is given up.
    restore: Option<(String, usize)>,
    /// The running full read and page read, by their cancellable read ids.
    reading: Option<String>,
    paging: Option<String>,
    pub pr_ui: super::hosting::PrUi,
    pub agents: super::agents::AgentsUi,
}

impl RepoTab {
    pub fn new(path: &str) -> RepoTab {
        RepoTab {
            path: path.to_owned(),
            view: None,
            index: HashMap::new(),
            error: None,
            loading: false,
            generation: 0,
            requested: None,
            shown_query: None,
            again: false,
            status_loading: false,
            status_again: false,
            status_during_read: false,
            page_loading: false,
            page_error: None,
            deferred: None,
            ui: TabUi::default(),
            watch: None,
            watching: false,
            commit_files: HashMap::new(),
            commit_details: HashMap::new(),
            signatures: HashMap::new(),
            stats: HashMap::new(),
            stats_asked: Default::default(),
            range_files: HashMap::new(),
            range_requests: HashMap::new(),
            queue: VecDeque::new(),
            running: None,
            action_change: None,
            spinning: None,
            work: Default::default(),
            side: Default::default(),
            diff_ui: Default::default(),
            history_ui: Default::default(),
            conflict_ui: Default::default(),
            guard: None,
            restore: None,
            reading: None,
            paging: None,
            pr_ui: Default::default(),
            agents: Default::default(),
        }
    }

    fn queue_action(&mut self, action: Action, quiet: bool, busy: bool) -> bool {
        if busy && matches!(&action, Action::Fetch | Action::FetchRemote { .. } | Action::FetchHistory { .. } | Action::Pull { .. }) {
            return false;
        }
        self.queue.push_back((action, quiet));
        true
    }

    /// Whether more history is to be paged in: the initial amount (or all), and back to the depth
    /// a commit selected before a full read was found at.
    fn wants_more(&self, show_all: bool, initial: usize) -> bool {
        let Some(v) = &self.view else { return false };
        let len = v.data.commits.len();
        v.history_cursor.is_some() && (show_all || len < initial || self.restore.as_ref().is_some_and(|(_, depth)| len < *depth))
    }

    /// A selection being paged back to: kept once its commit is loaded, given up when the
    /// history is loaded as far as before (or has no more, or paging failed) without it.
    fn settle_restore(&mut self, failed: bool) {
        let Some((sha, depth)) = self.restore.clone() else { return };
        if self.ui.selection != Some(Selection::Commit(sha.clone())) {
            self.restore = None;
            return;
        }
        let (len, more) = self.view.as_ref().map_or((0, false), |v| (v.data.commits.len(), v.history_cursor.is_some()));
        let found = self.index.contains_key(&sha);
        if failed || len >= depth || !more {
            self.restore = None;
            if !found {
                self.ui.selection = None;
            }
        }
    }

    /// Stop the tab's running reads (the tab is closing).
    pub fn cancel_reads(&mut self) {
        for id in [self.reading.take(), self.paging.take()].into_iter().flatten() {
            gitcraft_core::read_jobs::cancel(&id);
        }
    }

    fn finish_page(&mut self, request: &str) -> bool {
        if self.paging.as_deref() != Some(request) {
            return false;
        }
        self.paging = None;
        self.page_loading = false;
        true
    }

    pub fn commit(&self, sha: &str) -> Option<&Commit> {
        let v = self.view.as_ref()?;
        self.index.get(sha).and_then(|i| v.data.commits.get(*i))
    }

    fn reindex(&mut self) {
        self.index = self.view.as_ref().map(|v| v.data.commits.iter().enumerate().map(|(i, c)| (c.sha.clone(), i)).collect()).unwrap_or_default();
    }
}

pub(super) fn pull_busy(running: bool, background: &[String], path: &str) -> bool {
    running || background.iter().any(|p| super::same_folder(p, path))
}

/// History equal when HEAD and every ref are where they were.
fn same_history(a: &RepoView, b: &RepoView) -> bool {
    let sig = |v: &RepoView| {
        let mut refs: Vec<String> = v.data.refs.iter().map(|r| format!("{}:{}", r.full, r.sha)).collect();
        refs.sort();
        (v.data.head_sha.clone(), refs)
    };
    sig(a) == sig(b)
}

/// Actions that only move changes in or out of the index.
fn index_only(a: &Action) -> bool {
    match a {
        Action::Stage { .. } | Action::Unstage { .. } | Action::StageAll | Action::UnstageAll => true,
        Action::Patch { op, .. } => !matches!(op, gitcraft_core::files::PatchOp::Discard),
        _ => false,
    }
}

/// What the file lists will look like once Git has staged or unstaged, shown straight away.
pub fn predict_status(s: &WorkStatus, a: &Action) -> Option<WorkStatus> {
    let shift = |from: &[FileChange], to: &[FileChange], paths: Option<&[String]>, to_staged: bool| -> Option<(Vec<FileChange>, Vec<FileChange>)> {
        let moving: Vec<&FileChange> = from.iter().filter(|f| f.status != 'U' && paths.is_none_or(|p| p.contains(&f.path))).collect();
        if moving.is_empty() {
            return None;
        }
        let left: Vec<FileChange> = from.iter().filter(|f| !moving.iter().any(|m| m.path == f.path)).cloned().collect();
        let mut merged: Vec<FileChange> = to.iter().filter(|f| !moving.iter().any(|m| m.path == f.path)).cloned().collect();
        merged.extend(moving.iter().map(|f| FileChange {
            path: f.path.clone(),
            status: if to_staged { if f.status == '?' { 'A' } else { f.status } } else if f.status == 'A' { '?' } else { f.status },
        }));
        merged.sort_by(|x, y| x.path.cmp(&y.path));
        Some((left, merged))
    };
    match a {
        Action::Stage { paths } => shift(&s.unstaged, &s.staged, Some(paths), true).map(|(l, m)| WorkStatus { unstaged: l, staged: m, conflicted: s.conflicted }),
        Action::StageAll => shift(&s.unstaged, &s.staged, None, true).map(|(l, m)| WorkStatus { unstaged: l, staged: m, conflicted: s.conflicted }),
        Action::Unstage { paths } => shift(&s.staged, &s.unstaged, Some(paths), false).map(|(l, m)| WorkStatus { staged: l, unstaged: m, conflicted: s.conflicted }),
        Action::UnstageAll => shift(&s.staged, &s.unstaged, None, false).map(|(l, m)| WorkStatus { staged: l, unstaged: m, conflicted: s.conflicted }),
        _ => None,
    }
}

fn busy_label(a: &Action) -> &'static str {
    match a {
        Action::Stage { .. } => "Staging…",
        Action::StageAll => "Staging all changes…",
        Action::Unstage { .. } => "Unstaging…",
        Action::UnstageAll => "Unstaging all changes…",
        Action::Patch { .. } => "Updating…",
        Action::Fetch => "Fetching…",
        Action::FetchHistory { .. } => "Fetching history…",
        Action::Pull { .. } => "Pulling…",
        Action::Push { .. } | Action::PushBranch { .. } => "Pushing…",
        Action::PushTag { .. } => "Pushing tag…",
        Action::DeleteRemoteTag { .. } => "Deleting remote tag…",
        Action::Commit { .. } => "Committing…",
        Action::Checkout { .. } => "Checking out…",
        Action::CheckoutRemote { .. } => "Checking out remote branch…",
        Action::Merge { .. } => "Merging…",
        Action::Rebase { .. } => "Rebasing…",
        Action::Stash { .. } => "Stashing…",
        _ => "Working…",
    }
}

fn spin_for(a: &Action) -> Option<&'static str> {
    match a {
        Action::Fetch | Action::FetchRemote { .. } | Action::FetchHistory { .. } => Some("fetch"),
        Action::Pull { .. } => Some("pull"),
        Action::Push { .. } => Some("push"),
        Action::Stash { .. } | Action::StashKeepIndex { .. } | Action::StashUntracked { .. } => Some("stash"),
        Action::StashPop { .. } => Some("pop"),
        _ => None,
    }
}

#[cfg(test)]
#[test]
fn headless_fetch_and_pull_toolbar_spinners_are_distinct() {
    assert_eq!(spin_for(&Action::Fetch), Some("fetch"));
    assert_eq!(spin_for(&Action::FetchRemote { remote: "origin".into() }), Some("fetch"));
    assert_eq!(spin_for(&Action::FetchHistory { remote: "origin".into(), mode: "deepen".into() }), Some("fetch"));
    assert_eq!(spin_for(&Action::Pull { mode: "ff".into(), autostash: None }), Some("pull"));
    assert_eq!(spin_for(&Action::Undo), None);
}

/// SSH refused the server's key: Some(true) when it changed, Some(false) when it is unknown.
pub fn ssh_host_key_problem(message: &str) -> Option<bool> {
    let m = message.to_lowercase();
    if m.contains("remote host identification has changed") || m.contains("possible dns spoofing") || (m.contains("offending ") && m.contains(" key in ")) {
        return Some(true);
    }
    if m.contains("host key verification failed") || (m.contains("no ") && m.contains("host key is known for")) || (m.contains("the authenticity of host") && m.contains("can't be established")) {
        return Some(false);
    }
    None
}

fn widgets_kind_primary() -> super::widgets::Kind {
    super::widgets::Kind::Primary
}

fn network(a: &Action) -> bool {
    matches!(a, Action::Fetch | Action::Pull { .. } | Action::Push { .. } | Action::PushBranch { .. } | Action::PushTag { .. } | Action::DeleteRemoteTag { .. } | Action::FetchRemote { .. } | Action::FetchHistory { .. })
}

impl App {
    pub(crate) fn repo(&self, tab: u64) -> Option<&RepoTab> {
        self.repos.get(&tab)
    }

    pub(crate) fn repo_mut(&mut self, tab: u64) -> Option<&mut RepoTab> {
        self.repos.get_mut(&tab)
    }

    /// The tab showing `path`.
    pub(crate) fn tab_of(&self, path: &str) -> Option<u64> {
        self.repos.iter().find(|(_, r)| super::same_folder(&r.path, path)).map(|(id, _)| *id)
    }

    /// What the graph is asked to show: hidden, soloed and pinned refs and the history filters.
    fn ref_view(&self, tab: u64) -> RefView {
        let Some(r) = self.repo(tab) else { return RefView::default() };
        RefView { filter: r.ui.history_filter.clone().unwrap_or_default(), ..saved_view(&self.settings, &r.path) }
    }

    fn initial_commits(&self) -> usize {
        initial_commits(&self.settings)
    }

    /// Run `job` away from the window; what it returns comes back as a message.
    pub(crate) fn spawn(&self, job: impl FnOnce() -> Msg + Send + 'static) {
        let inbox = self.inbox.clone();
        let ctx = self.ctx.clone();
        gitcraft_core::runtime().spawn_blocking(move || {
            let msg = job();
            inbox.post(msg);
            ctx.request_repaint();
        });
    }

    /// Read the repository again (commits, refs, status).
    pub(crate) fn load_repo(&mut self, tab: u64) {
        let active = tab == self.active;
        let view = self.ref_view(tab);
        let max = self.initial_commits();
        let engine = self.engine.clone();
        let Some(r) = self.repo_mut(tab) else { return };
        if !active {
            r.generation += 1;
            r.deferred = Some(true);
            return;
        }
        if r.loading {
            // Another filter or set of branches: the running read is of no use any more. A
            // refresh of the same view lets it finish (its result stays valid, so steady file changes
            // can't starve it) and is run once it has published.
            if r.requested.as_ref() != Some(&view) {
                r.generation += 1;
                if let Some(id) = &r.reading {
                    gitcraft_core::read_jobs::cancel(id);
                }
            }
            r.again = true;
            return;
        }
        r.generation += 1;
        r.status_during_read = false;
        r.loading = true;
        r.again = false;
        r.deferred = None;
        let (path, generation) = (r.path.clone(), r.generation);
        r.requested = Some(view.clone());
        let id = format!("repo:{tab}:{generation}");
        r.reading = Some(id.clone());
        // A page read for the history being replaced is of no use either.
        if let Some(page) = r.paging.take() {
            gitcraft_core::read_jobs::cancel(&page);
        }
        r.page_loading = false;
        super::bench::mark("repo-read");
        if let Some(early) = take_prefetch(&path, &view, max) {
            return self.spawn(move || Msg::Repo { tab, generation, result: Box::new(early.recv().unwrap_or_else(|_| Err("The repository read stopped unexpectedly.".into()))) });
        }
        self.spawn(move || Msg::Repo { tab, generation, result: Box::new(gitcraft_core::read_jobs::scope(Some(id), || session::open_repo(&engine, &path, Some(max), Some(view)))) });
    }

    pub(crate) fn repo_loaded(&mut self, tab: u64, generation: u64, result: Result<RepoView, String>) {
        super::bench::mark("repo-loaded");
        let show_all = self.settings.flag("showAllCommits", false);
        let initial = self.initial_commits();
        let mut stale_status = false;
        let start_watch = {
            let Some(r) = self.repo_mut(tab) else { return };
            r.loading = false;
            r.reading = None;
            if r.generation == generation {
                let query = r.requested.take();
                match result {
                    Ok(mut next) => {
                        let loaded_before = r.view.as_ref().map_or(0, |v| v.data.commits.len()).max(r.restore.as_ref().map_or(0, |x| x.1));
                        r.restore = None;
                        if let Some(prev) = r.view.take() {
                            // Keep the pages already loaded when the history itself didn't change:
                            // the same branches, read the same way, and the new first page is
                            // the start of what was loaded.
                            let same_query = query.is_some() && r.shown_query == query;
                            let starts_the_same = !next.data.commits.is_empty() && next.data.commits.len() <= prev.data.commits.len() && next.data.commits.iter().zip(prev.data.commits.iter()).all(|(a, b)| a.sha == b.sha);
                            if same_query && same_history(&prev, &next) && starts_the_same {
                                next.data.commits = prev.data.commits;
                                next.graph = prev.graph;
                                next.data.has_more = prev.data.has_more;
                                next.history_cursor = prev.history_cursor;
                            }
                        }
                        r.view = Some(next);
                        // A newer working-tree status arrived while this read ran: its own is older, so read again.
                        if std::mem::take(&mut r.status_during_read) {
                            r.status_again = true;
                            stale_status = true;
                        }
                        r.shown_query = query;
                        r.error = None;
                        r.page_error = None;
                        // The working folder may have changed: comparisons against it are read again.
                        r.range_files.retain(|(_, to), _| to != git::WORKDIR);
                        r.reindex();
                        if let Some(Selection::Commit(sha)) = r.ui.selection.clone()
                            && !r.index.contains_key(&sha)
                            && !r.view.as_ref().is_some_and(|v| v.data.stashes.iter().any(|s| s.sha == sha))
                        {
                            // An older commit: page back to where the history was loaded to
                            // before giving the selection up.
                            r.restore = Some((sha, loaded_before));
                            r.settle_restore(false);
                        }
                    }
                    Err(e) => r.error = Some(e),
                }
            }
            let again = r.again;
            let watch = r.view.is_some() && !r.watching;
            if again {
                r.again = false;
            }
            (again, watch)
        };
        if start_watch.0 {
            self.load_repo(tab);
        } else if stale_status {
            self.refresh_status(tab);
        }
        if start_watch.1 {
            self.watch(tab);
        }
        // Load the rest of the initial history, or all of it, a page at a time.
        if self.repo(tab).is_some_and(|r| r.wants_more(show_all, initial)) {
            self.load_more(tab);
        }
    }

    /// The next page of history.
    pub(crate) fn load_more(&mut self, tab: u64) {
        let max = self.initial_commits();
        let Some(r) = self.repo_mut(tab) else { return };
        if r.page_loading || r.loading {
            return;
        }
        let Some(cursor) = r.view.as_ref().and_then(|v| v.history_cursor.clone()) else { return };
        r.page_loading = true;
        r.page_error = None;
        let (path, generation) = (r.path.clone(), r.generation);
        let id = format!("page:{tab}:{generation}:{}", super::unique_id());
        r.paging = Some(id.clone());
        self.spawn(move || {
            let result = gitcraft_core::read_jobs::scope(Some(id.clone()), || session::history_page(&path, cursor, Some(max)));
            Msg::Page { tab, generation, request: id, result: Box::new(result) }
        });
    }

    pub(crate) fn page_loaded(&mut self, tab: u64, generation: u64, request: &str, result: Result<HistoryPage, String>) {
        let show_all = self.settings.flag("showAllCommits", false);
        let initial = self.initial_commits();
        let Some(r) = self.repo_mut(tab) else { return };
        if !r.finish_page(request) {
            return;
        }
        if r.generation != generation {
            // An outdated page is dropped; paging goes on from the history shown, unless a full
            // read is running (it starts paging itself when it's done).
            let more = !r.loading && r.wants_more(show_all, initial);
            if more {
                self.load_more(tab);
            }
            return;
        }
        match result {
            Ok(page) => {
                let Some(v) = r.view.as_mut() else { return };
                v.data.commits.extend(page.commits);
                v.graph.rows.extend(page.graph.rows);
                v.graph.width = page.graph.width;
                v.data.has_more = page.has_more;
                v.history_cursor = page.history_cursor;
                r.reindex();
                r.settle_restore(false);
                if r.wants_more(show_all, initial) {
                    self.load_more(tab);
                }
            }
            // Git's plain order put a parent before its child: read it all again in date order,
            // keeping the selection and how far it was loaded.
            Err(e) if e == gitcraft_core::git::HISTORY_REORDERED => self.load_repo(tab),
            Err(e) => {
                r.page_error = Some(e);
                r.settle_restore(true);
            }
        }
    }

    /// Re-read only the file lists (after edits in the working folder or the index).
    pub(crate) fn refresh_status(&mut self, tab: u64) {
        let active = tab == self.active;
        let Some(r) = self.repo_mut(tab) else { return };
        if !active {
            if r.deferred.is_none() {
                r.deferred = Some(false);
            }
            return;
        }
        if r.status_loading {
            r.status_again = true;
            return;
        }
        r.status_loading = true;
        r.status_again = false;
        let (path, generation) = (r.path.clone(), r.generation);
        self.spawn(move || Msg::Status { tab, generation, result: git::try_status(std::path::Path::new(&path)) });
    }

    pub(crate) fn status_loaded(&mut self, tab: u64, generation: u64, result: Result<WorkStatus, String>) {
        let Some(r) = self.repo_mut(tab) else { return };
        r.status_loading = false;
        let again = std::mem::take(&mut r.status_again);
        match result {
            Ok(status) if r.generation == generation => {
                r.status_during_read |= r.loading;
                if let Some(v) = r.view.as_mut() {
                    v.data.status = status;
                }
                r.range_files.retain(|(_, to), _| to != git::WORKDIR);
                self.diff_changed(tab);
            }
            Ok(_) => {}
            Err(_) => return self.load_repo(tab),
        }
        if again {
            self.refresh_status(tab);
        }
    }

    /// The tab came to the front: catch up with what changed while it was behind.
    pub(crate) fn tab_shown(&mut self, tab: u64) {
        let Some(r) = self.repo(tab) else { return };
        match r.deferred {
            Some(true) => self.load_repo(tab),
            Some(false) => {
                if let Some(r) = self.repo_mut(tab) {
                    r.deferred = None;
                }
                self.refresh_status(tab);
            }
            None if r.view.is_none() && !r.loading && r.error.is_none() => self.load_repo(tab),
            None => {}
        }
    }

    fn watch(&mut self, tab: u64) {
        let Some(r) = self.repo_mut(tab) else { return };
        r.watching = true;
        let path = r.path.clone();
        let inbox = self.inbox.clone();
        let ctx = self.ctx.clone();
        let (error_inbox, error_ctx) = (self.inbox.clone(), self.ctx.clone());
        self.spawn(move || {
            let notify_path = path.clone();
            // Said once per watch: the system stopped telling GitCraft about changes (or reported an error).
            let told = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let result = gitcraft_core::watch::start(std::path::Path::new(&path), move |full| {
                inbox.post(Msg::Changed { path: notify_path.clone(), full });
                ctx.request_repaint();
            }, move |error| {
                if !told.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    error_inbox.post(Msg::Toast(false, format!("GitCraft stopped receiving file change events from the system, so changes made outside it may not show until you refresh: {error}")));
                    error_ctx.request_repaint();
                }
            });
            Msg::Watching { tab, result: result.map(Arc::new) }
        });
    }

    pub(crate) fn watching(&mut self, tab: u64, result: Result<Arc<gitcraft_core::watch::RepoWatch>, String>) {
        let Some(r) = self.repo_mut(tab) else { return };
        match result.map(Arc::try_unwrap) {
            Ok(Ok(watch)) => r.watch = Some(watch),
            Ok(Err(_)) => {}
            Err(e) => {
                r.watching = false;
                self.toast(false, format!("Changes made outside GitCraft won't show until you refresh: {e}"));
            }
        }
    }

    /// The watcher saw a change: the file lists, or (full) commits, branches and HEAD.
    pub(crate) fn changed(&mut self, path: &str, full: bool) {
        let Some(tab) = self.tab_of(path) else { return };
        let Some(r) = self.repo_mut(tab) else { return };
        // A working-copy blame may be out of date now (the event doesn't say which file), also
        // one still being read: it is read again. Blames of a revision stay.
        r.history_ui.blame.retain(|(_, rev, _), _| rev.is_some());
        r.history_ui.blame_reads.retain(|(_, rev, _), _| rev.is_some());
        if r.running.is_some() {
            let seen = r.action_change.unwrap_or(false);
            r.action_change = Some(seen || full);
            return;
        }
        if full { self.load_repo(tab) } else { self.refresh_status(tab) }
    }

    /// Something the open diff, conflict or comparison shows may have changed.
    pub(crate) fn diff_changed(&mut self, tab: u64) {
        self.diffs.retain(|k, _| k.0 != tab);
        // Picked lines point into the diff just dropped; the new one may differ.
        if let Some(r) = self.repo_mut(tab) {
            r.diff_ui.picked = None;
        }
    }

    /// Run a Git action in the background, after the ones already waiting for this repository.
    pub(crate) fn act(&mut self, tab: u64, action: Action) {
        self.act_quiet(tab, action, false);
    }

    pub(crate) fn act_quiet(&mut self, tab: u64, action: Action, quiet: bool) {
        let mut action = action;
        let path = match self.repo(tab) {
            Some(r) => r.path.clone(),
            None => return,
        };
        if let Action::Pull { mode, autostash: None } = &action
            && mode == "rebase"
            && let Some(a) = self.repo_pref(&path, "pullAutostash").and_then(|v| v.as_bool())
        {
            action = Action::Pull { mode: mode.clone(), autostash: Some(a) };
        }
        if let Action::Merge { branch, into: None, .. } = &action
            && self.settings.flag("squashMerge", false)
        {
            // Its commit message draft is filled in once the squash succeeds (see `squash_done`).
            action = Action::MergeSquash { branch: branch.clone() };
        }
        if let Action::Commit { strip_comments, .. } = &mut action {
            *strip_comments = self.settings.flag("stripComments", false);
        }
        let busy = self.repo(tab).is_some_and(|r| self.repo_pull_busy(&r.path));
        let Some(r) = self.repo_mut(tab) else { return };
        if !r.queue_action(action, quiet, busy) {
            return;
        }
        if r.running.is_none() {
            self.next_action(tab);
        }
    }

    /// A checkout that stopped for uncommitted changes (it asked first, or they block it): bring
    /// them along, stash them, commit first, or discard them after confirming. Returns whether
    /// it asked.
    fn checkout_changes_dialog(&mut self, tab: u64, action: &Action, msg: &str) -> bool {
        use super::dialogs::{Body, Dialog};
        use super::widgets::Kind;
        use gitcraft_core::actions::CheckoutChanges;
        let (target, changes, remote_stash) = match action {
            Action::Checkout { target, changes } => (target.clone(), *changes, false),
            Action::CheckoutRemote { remote_branch, stash_if_needed, changes } => (remote_branch.clone(), *changes, *stash_if_needed),
            Action::Merge { into: Some(target), changes, .. } => (target.clone(), *changes, false),
            Action::Rebase { branch: Some(target), changes, .. } => (target.clone(), *changes, false),
            _ => return false,
        };
        let choose = changes == CheckoutChanges::Ask && msg == "Choose how to handle your uncommitted changes before checkout.";
        let blocked = !matches!(changes, CheckoutChanges::Stash | CheckoutChanges::Discard) && !remote_stash && msg.starts_with("Local changes block this checkout.");
        if !choose && !blocked {
            return false;
        }
        let with = |changes: CheckoutChanges| match action.clone() {
            Action::Checkout { target, .. } => Action::Checkout { target, changes },
            Action::CheckoutRemote { remote_branch, stash_if_needed, .. } => Action::CheckoutRemote { remote_branch, stash_if_needed, changes },
            Action::Merge { branch, into, .. } => Action::Merge { branch, into, changes },
            Action::Rebase { onto, branch, .. } => Action::Rebase { onto, branch, changes },
            other => other,
        };
        let stash = match action {
            // A remote branch blocked while updating: stash only if needed, then retry.
            Action::CheckoutRemote { remote_branch, .. } if blocked => Action::CheckoutRemote { remote_branch: remote_branch.clone(), stash_if_needed: true, changes: CheckoutChanges::Carry },
            _ => with(CheckoutChanges::Stash),
        };
        let (carry, discard) = (with(CheckoutChanges::Carry), with(CheckoutChanges::Discard));
        let mut text = if choose { format!("Choose what to do with your uncommitted changes before checking out {target}.") } else { msg.to_owned() };
        if choose {
            text.push_str("\n\nBring changes: carry compatible edits over and keep staged files staged.");
        }
        text.push_str("\n\nStash: save staged, unstaged, and untracked work in a named stash.\n\nCommit first: stay on the current branch and open the commit panel.\n\nDiscard: clear uncommitted changes after confirming.");
        let mut d = Dialog::new(if choose { "Switch with uncommitted changes?" } else { "Local changes block checkout" }, Body::Text(text)).width(520.0);
        if choose {
            d = d.choice("Bring changes", Kind::Primary, move |app, _| app.act(tab, carry));
        }
        d = d
            .choice(if choose { "Stash and switch" } else { "Stash and continue" }, Kind::Default, move |app, _| app.act(tab, stash))
            .choice("Commit first", Kind::Default, move |app, _| app.focus_commit_summary(tab))
            .choice("Discard and switch", Kind::Danger, move |app, _| {
                app.dialog = Some(Dialog::confirm_with(
                    "Discard changes and switch?",
                    &format!("Staged, unstaged, and untracked changes will be discarded before checking out {target}. GitCraft keeps a recovery backup."),
                    "Discard and switch",
                    true,
                    move |app, _| app.act(tab, discard),
                ));
            });
        self.dialog = Some(d);
        true
    }

    fn next_action(&mut self, tab: u64) {
        let engine = self.engine.clone();
        let Some(r) = self.repo_mut(tab) else { return };
        let Some((action, quiet)) = r.queue.pop_front() else {
            r.running = None;
            r.spinning = None;
            return self.update_busy();
        };
        r.running = Some(busy_label(&action).to_owned());
        r.spinning = spin_for(&action);
        r.action_change = Some(false);
        r.generation += 1;
        if index_only(&action)
            && let Some(v) = r.view.as_mut()
            && let Some(guess) = predict_status(&v.data.status, &action)
        {
            v.data.status = guess;
        }
        let path = r.path.clone();
        // The operation can outlive its tab. Keep its reservation until completion.
        self.busy_repos.push(path.clone());
        self.update_busy();
        let sent = action.clone();
        self.spawn(move || {
            let result = engine.perform_for_desktop(&path, sent);
            Msg::Acted { tab, path, action: Box::new(action), quiet, result }
        });
    }

    pub(crate) fn checkout_opened_worktree(&mut self, tab: u64, path: &str) {
        let Some(repo) = self.repo_mut(tab) else { return };
        repo.running = None;
        repo.action_change = None;
        self.load_repo(tab);
        self.diff_changed(tab);
        self.next_action(tab);
        if self.active == tab {
            self.open_repo(path);
        }
    }

    pub(crate) fn acted(&mut self, tab: u64, action: Action, quiet: bool, result: ActionResult) {
        let notify = self.settings.flag("notifySuccess", true);
        let handled = self.after_action(tab, &action, &result, quiet);
        // A checkout that stashed or discarded local changes always says so (and where they went).
        let stashed = matches!(action, Action::Checkout { .. } | Action::CheckoutRemote { .. } | Action::Merge { .. } | Action::Rebase { .. }) && (result.message.contains("Your local changes are saved in the stash") || result.message.contains("Discarded changes."));
        if !handled && (!result.ok || (!quiet && (notify || stashed))) {
            self.toast(result.ok, result.message.clone());
        }
        let light = index_only(&action) && result.ok;
        let Some(r) = self.repo_mut(tab) else { return };
        let full = r.action_change.take().unwrap_or(false);
        r.running = None;
        if light && !full { self.refresh_status(tab) } else { self.load_repo(tab) }
        self.diff_changed(tab);
        self.next_action(tab);
    }

    /// Follow-ups for some failures: offer to add a remote, force push, set an upstream.
    /// Returns whether it showed something instead of the plain message.
    fn after_action(&mut self, tab: u64, action: &Action, result: &ActionResult, quiet: bool) -> bool {
        use super::dialogs::Dialog;
        // What the commit was made from: only that is cleared, so edits made while it ran stay.
        let submitted = if let Action::Commit { .. } = action { self.repo_mut(tab).and_then(|r| r.work.submitted.take()) } else { None };
        if !result.ok && matches!(action, Action::SaveResolved { .. }) {
            if let Some(r) = self.repo_mut(tab) {
                r.conflict_ui.saving = false;
            }
            self.left_saved(tab, super::guard::Editor::Conflict, false);
        }
        if result.ok {
            if let Action::SaveResolved { file, .. } = action {
                self.conflict_resolved(tab, file);
            }
            if let Action::Commit { .. } = action
                && let Some(r) = self.repo(tab)
            {
                let path = r.path.clone();
                let sent = submitted.unwrap_or_default();
                if self.draft(&path) == sent.draft {
                    self.set_draft(&path, Draft::default());
                }
                if let Some(r) = self.repo_mut(tab) {
                    r.ui.diff = None;
                    r.work.template_applied = false;
                    if r.work.coauthors == sent.coauthors {
                        r.work.coauthors.clear();
                    }
                }
                // Next in line, before actions queued while the commit ran, so it pushes the
                // branch just committed to.
                if sent.push
                    && let Some(r) = self.repo_mut(tab)
                {
                    r.queue.push_front((Action::Push { force: false, lease: None, review: None }, false));
                }
            }
            return false;
        }
        let msg = result.message.as_str();
        match action {
            Action::Checkout { .. } | Action::CheckoutRemote { .. } if !quiet && self.checkout_changes_dialog(tab, action, msg) => true,
            Action::Push { force: false, .. } if msg == "This repository has no remote to push to. Add one first." => {
                self.dialog = Some(Dialog::confirm(
                    "No remote to push to",
                    "This repository has no remote to push to. Would you like to add one? Connect an existing URL or create a repository on a connected hosting account.",
                    "Add Remote",
                    false,
                    super::dialogs::Then::AddRemote(tab),
                ));
                true
            }
            Action::Push { force: false, .. } if msg.lines().any(|l| l.trim_start().starts_with("! [rejected]") && (l.contains("(non-fast-forward)") || l.contains("(fetch first)"))) => {
                self.dialog = Some(Dialog::confirm(
                    "Push rejected",
                    "The remote branch has commits you don't have. Pull first, or force push to replace them (with lease, so other people's new work is not lost).",
                    "Force Push",
                    true,
                    super::dialogs::Then::Act(tab, Action::Push { force: true, lease: Some(true), review: None }),
                ));
                true
            }
            Action::Pull { .. } if !quiet && msg == "Set an upstream branch before pulling." => {
                self.prompt_upstream(tab);
                true
            }
            _ if network(action) && !quiet && ssh_host_key_problem(msg).is_some() => {
                let changed = ssh_host_key_problem(msg) == Some(true);
                let instructions = if changed {
                    "SSH found a different key than the one saved for this host. Ask the server administrator to confirm the new fingerprint. Once verified, update the host's entry in your SSH known_hosts file and retry the Git command."
                } else {
                    "SSH could not verify the server's identity. Check the expected fingerprint with your host or server administrator. Retry the Git command in a terminal and accept the key only if its displayed fingerprint matches."
                };
                let fingerprint = msg.split_whitespace().find(|w| w.starts_with("SHA256:")).map(|f| f.trim_end_matches(['.', ',']));
                let text = match fingerprint {
                    Some(f) => format!("{instructions}\n\nFingerprint reported by SSH: {f}"),
                    None => instructions.to_owned(),
                };
                let title = if changed { "SSH host key changed" } else { "SSH host key needs verification" };
                self.dialog = Some(Dialog::choice_list(title, &text, vec![("Open Terminal".into(), widgets_kind_primary(), super::dialogs::Then::OpenTerminal(tab))]));
                true
            }
            _ => false,
        }
    }

    pub(crate) fn update_busy(&mut self) {
        let running: Vec<&String> = self.repos.values().filter_map(|r| r.running.as_ref()).collect();
        let tabs = match running.len() {
            0 => None,
            1 => Some(running[0].clone()),
            n => Some(format!("Working in {n} repositories…")),
        };
        // A bulk fetch or pull reports beside whatever the tabs are doing.
        self.busy = match (tabs, self.bulk_message.clone()) {
            (Some(tabs), Some(bulk)) => Some(format!("{tabs} · {bulk}")),
            (tabs, bulk) => tabs.or(bulk),
        };
        if self.busy.is_none() {
            self.job = None;
        }
    }

    /// A repository's own preference (`repoPrefs[path][key]`).
    pub(crate) fn repo_pref(&self, path: &str, key: &str) -> Option<serde_json::Value> {
        self.settings.get::<serde_json::Value>("repoPrefs").and_then(|v| v.get(path).and_then(|p| p.get(key)).cloned())
    }

    pub(crate) fn pull_mode(&self, path: &str) -> String {
        self.repo_pref(path, "pullMode").and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_else(|| self.settings.string("pullMode", "ff"))
    }

    pub(crate) fn draft(&self, path: &str) -> Draft {
        self.drafts.get(path).cloned().unwrap_or_default()
    }

    /// Update the submodules after a pull or checkout when Preferences ask for it (the repository's "Update submodules
    /// after pull or checkout", else General's "Keep submodules up to date"). Only repositories with submodules.
    pub(crate) fn update_submodules_after(&mut self, tab: u64) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        let choice = self.repo_pref(&path, "pullCheckoutSubmodules").and_then(|v| match v {
            serde_json::Value::Bool(b) => Some(b),
            serde_json::Value::String(s) if s == "true" => Some(true),
            serde_json::Value::String(s) if s == "false" => Some(false),
            _ => None,
        });
        let on = choice.unwrap_or_else(|| self.settings.flag("autoUpdateSubmodules", true));
        if on && std::path::Path::new(&path).join(".gitmodules").is_file() {
            self.act_quiet(tab, Action::SubmoduleUpdate { paths: Vec::new() }, true);
        }
    }

    /// Delete the `.orig` backups that appeared since the last merge-like operation began, once it is finished.
    pub(crate) fn clean_orig_files(&mut self, tab: u64) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        self.spawn(move || {
            let dir = std::path::Path::new(&path);
            let candidates = gitcraft_core::prefs::orig_cleanup_candidates(dir);
            if candidates.is_empty() {
                return Msg::Nothing;
            }
            match gitcraft_core::prefs::delete_orig_selected(dir, &candidates) {
                Ok(removed) if !removed.is_empty() => Msg::Toast(true, format!("Removed {} .orig backup file{}", removed.len(), if removed.len() == 1 { "" } else { "s" })),
                Ok(_) => Msg::Nothing,
                Err(e) => Msg::Toast(false, e),
            }
        });
    }

    /// A squash merge succeeded: offer its message as the commit draft, unless the user already has a draft.
    pub(crate) fn squash_done(&mut self, tab: u64, branch: &str) {
        let Some(r) = self.repo(tab) else { return };
        let path = r.path.clone();
        if self.draft(&path) != Draft::default() {
            return;
        }
        let head = r.view.as_ref().and_then(|v| v.data.head_branch.clone()).unwrap_or_else(|| "HEAD".into());
        self.pending_drafts.push((path, Draft { summary: format!("Squash merge {branch} into {head}"), description: String::new(), amend: false }));
    }

    pub(crate) fn set_draft(&mut self, path: &str, draft: Draft) {
        if draft == Draft::default() {
            self.drafts.remove(path);
        } else {
            self.drafts.insert(path.to_owned(), draft);
        }
        self.settings.set_root("drafts", &self.drafts);
    }

    /// Read a commit's files once, for the right panel.
    pub(crate) fn want_commit_files(&mut self, tab: u64, sha: &str) {
        let Some(r) = self.repo_mut(tab) else { return };
        if r.commit_files.contains_key(sha) {
            return;
        }
        r.commit_files.insert(sha.to_owned(), Loaded::Loading);
        let (path, sha) = (r.path.clone(), sha.to_owned());
        self.spawn(move || {
            let files = session::commit_files(&path, &sha);
            let signature = session::commit_signature(&path, &sha).unwrap_or_default();
            Msg::CommitFiles { tab, sha, files, signature }
        });
    }

    /// A commit not among the loaded ones (a stash, an older commit).
    pub(crate) fn want_commit_details(&mut self, tab: u64, sha: &str) {
        let Some(r) = self.repo_mut(tab) else { return };
        if r.commit_details.contains_key(sha) {
            return;
        }
        r.commit_details.insert(sha.to_owned(), Loaded::Loading);
        let (path, sha) = (r.path.clone(), sha.to_owned());
        self.spawn(move || Msg::CommitDetails { tab, result: session::commit_details(&path, &sha), sha });
    }

    pub(crate) fn want_range_files(&mut self, tab: u64, from: &str, to: &str) {
        let Some(r) = self.repo_mut(tab) else { return };
        let key = (from.to_owned(), to.to_owned());
        if r.range_files.contains_key(&key) {
            return;
        }
        r.range_files.insert(key.clone(), Loaded::Loading);
        let request = super::unique_id();
        r.range_requests.insert(key.clone(), request);
        let path = r.path.clone();
        self.spawn(move || Msg::RangeFiles { tab, request, result: gitcraft_core::files::range_files(std::path::Path::new(&path), &key.0, &key.1), key });
    }

    /// Line counts for the CHANGES column, for commits on screen.
    pub(crate) fn want_stats(&mut self, tab: u64, shas: Vec<String>) {
        let Some(r) = self.repo_mut(tab) else { return };
        let need: Vec<String> = shas.into_iter().filter(|s| r.stats_asked.insert(s.clone())).collect();
        if need.is_empty() {
            return;
        }
        let path = r.path.clone();
        self.spawn(move || Msg::Stats { tab, stats: gitcraft_core::files::stats(std::path::Path::new(&path), &need).unwrap_or_default() });
    }

    /// Open a repository tab's data store when the tab is created.
    pub(crate) fn ensure_repo(&mut self, tab: u64, path: &str) {
        self.repos.entry(tab).or_insert_with(|| RepoTab::new(path));
    }
}

fn initial_commits(settings: &crate::settings::Store) -> usize {
    (settings.num("initialCommits", 2000.0) as usize).clamp(500, 5000)
}

/// The refs a repository's graph shows by its saved preferences: hidden, soloed and pinned.
fn saved_view(settings: &crate::settings::Store, path: &str) -> RefView {
    let per = |key: &str| -> Vec<String> { settings.get::<HashMap<String, Vec<String>>>(key).and_then(|m| m.get(path).cloned()).unwrap_or_default() };
    RefView {
        hidden: per("hidden"),
        solo: per("solo"),
        pinned: settings.get::<HashMap<String, String>>("pinned").and_then(|m| m.get(path).cloned()),
        smart: settings.flag("smartBranches", false),
        filter: Default::default(),
    }
}

/// A read of the first repository shown, started before the window opens.
struct Prefetch {
    path: String,
    view: RefView,
    max: usize,
    result: std::sync::mpsc::Receiver<Result<RepoView, String>>,
}

static PREFETCH: std::sync::Mutex<Option<Prefetch>> = std::sync::Mutex::new(None);

/// Start reading the repository GitCraft will show first (the last one asked for at launch,
/// otherwise the saved tab in front), so the read runs while the window and the graphics are
/// set up. The first read of that tab takes it if it asks for the same history.
pub fn prefetch(engine: &Arc<gitcraft_core::actions::Engine>, settings: &crate::settings::Store, launch: &[session::LaunchIntent], saved_tabs: bool) {
    let launched = launch.iter().rev().find_map(|l| match l {
        session::LaunchIntent::Repository { path } => Some(path.clone()),
        _ => None,
    });
    let path = launched.or_else(|| {
        if !saved_tabs || !settings.flag("rememberTabs", true) {
            return None;
        }
        let (tabs, active) = settings.tabs();
        tabs.get(active.min(tabs.len().saturating_sub(1))).cloned()
    });
    let Some(path) = path else { return };
    let (view, max, engine) = (saved_view(settings, &path), initial_commits(settings), engine.clone());
    let (tx, rx) = std::sync::mpsc::channel();
    let (p, v) = (path.clone(), view.clone());
    gitcraft_core::runtime().spawn_blocking(move || {
        let _ = tx.send(session::open_repo(&engine, &p, Some(max), Some(v)));
    });
    *PREFETCH.lock().unwrap_or_else(|e| e.into_inner()) = Some(Prefetch { path, view, max, result: rx });
}

/// The early read, when it is of this repository, view and history size. Any other read drops it.
fn take_prefetch(path: &str, view: &RefView, max: usize) -> Option<std::sync::mpsc::Receiver<Result<RepoView, String>>> {
    let p = PREFETCH.lock().unwrap_or_else(|e| e.into_inner()).take()?;
    (super::same_folder(&p.path, path) && p.view == *view && p.max == max).then_some(p.result)
}

#[cfg(test)]
#[test]
fn headless_superseded_page_cannot_clear_the_active_request() {
    let mut tab = RepoTab::new("fixture");
    tab.page_loading = true;
    tab.paging = Some("new request".into());
    assert!(!tab.finish_page("previous request"));
    assert!(tab.page_loading);
    assert_eq!(tab.paging.as_deref(), Some("new request"));
    assert!(tab.finish_page("new request"));
    assert!(!tab.page_loading);
}

#[cfg(test)]
#[test]
fn headless_busy_pull_fetch_dispatch_is_rejected() {
    for running in ["pull", "fetch"] {
        for quiet in [false, true] {
            for action in [Action::Fetch, Action::Pull { mode: "ff".into(), autostash: None }] {
                let mut repo = RepoTab::new("/repo");
                repo.running = Some(running.into());
                let busy = pull_busy(repo.running.is_some(), &[], &repo.path);
                assert!(!repo.queue_action(action, quiet, busy));
                assert!(repo.queue.is_empty());
            }
        }
    }

    for quiet in [false, true] {
        for action in [
            Action::Fetch,
            Action::FetchRemote { remote: "origin".into() },
            Action::FetchHistory { remote: "origin".into(), mode: "deepen".into() },
            Action::Pull { mode: "ff".into(), autostash: None },
        ] {
            let mut repo = RepoTab::new("/repo");
            let background = vec!["/repo/".into()];
            let busy = pull_busy(repo.running.is_some(), &background, &repo.path);
            assert!(!repo.queue_action(action, quiet, busy));
            assert!(repo.queue.is_empty());
        }
    }

    let mut repo = RepoTab::new("/repo");
    let unrelated = vec!["/other".into()];
    let busy = pull_busy(repo.running.is_some(), &unrelated, &repo.path);
    assert!(repo.queue_action(Action::Pull { mode: "ff".into(), autostash: None }, false, busy));
    assert_eq!(repo.queue.len(), 1);

    let mut repo = RepoTab::new("/repo");
    repo.running = Some("pull".into());
    assert!(repo.queue_action(Action::StageAll, false, true));
    assert_eq!(repo.queue.len(), 1);
}
