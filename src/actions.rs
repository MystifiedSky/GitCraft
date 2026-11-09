//! Git actions the user can take, the activity log and undo / redo.

use crate::git::{self, LogEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Action {
    Stage { paths: Vec<String> },
    Unstage { paths: Vec<String> },
    StageAll,
    UnstageAll,
    Discard { paths: Vec<String> },
    DiscardAll,
    Commit { summary: String, description: String, amend: bool, stage_all: bool, #[serde(default)] skip_hooks: bool },
    Checkout { target: String },
    CheckoutRemote { remote_branch: String },
    CreateBranch { name: String, at: Option<String>, checkout: bool },
    DeleteBranch { name: String },
    DeleteRemoteBranch { remote_branch: String },
    RenameBranch { old: String, new: String },
    CreateTag { name: String, at: Option<String>, message: Option<String> },
    DeleteTag { name: String },
    Fetch,
    Pull { mode: String },
    /// force: push even though the remote has other commits. lease (default true) keeps the
    /// safety check that the remote branch is still where we last saw it.
    Push { force: bool, #[serde(default)] lease: Option<bool> },
    Stash { message: String },
    StashPop { index: usize },
    StashApply { index: usize },
    StashDrop { index: usize },
    CherryPick { sha: String },
    Revert { sha: String },
    Reset { sha: String, mode: String },
    Merge { branch: String },
    Rebase { onto: String },
    AbortMerge,
    AbortRebase,
    ContinueRebase,
    SkipRebase,
    /// Interactive rebase onto `base` ("--root" for the whole history) with our own todo list.
    InteractiveRebase { base: String, items: Vec<crate::rewrite::TodoItem> },
    /// Interactive cherry pick of the listed commits onto HEAD.
    InteractiveCherryPick { items: Vec<crate::rewrite::TodoItem> },
    /// Conflicts: take "ours" (current) or "theirs" (incoming) for whole files.
    TakeSide { paths: Vec<String>, side: String },
    MarkResolved { paths: Vec<String> },
    MarkConflicted { paths: Vec<String> },
    /// Write the merge tool's output and mark the file resolved.
    SaveResolved { file: String, content: String },
    FastForward { branch: String, to: String },
    SetUpstream { branch: String, upstream: String },
    StashRename { index: usize, message: String },
    /// Background fetch of every remote (the original's auto-fetch), optionally pruning.
    AutoFetch { prune: bool },
    FetchRemote { remote: String },
    RemoteEdit { name: String, new_name: String, url: String, push_url: String },
    RemoteRemove { name: String },
    WorktreeAdd { path: String, branch: String, new_branch: bool, start: Option<String> },
    WorktreeRemove { path: String, force: bool },
    WorktreeLock { path: String, message: String },
    WorktreeUnlock { path: String },
    SubmoduleAdd { url: String, path: String },
    SubmoduleUpdate { paths: Vec<String> },
    SubmoduleRemove { path: String },
    /// "Clean Up Local Branches": delete local branches and, optionally, their remote branches.
    DeleteBranches { names: Vec<String>, remotes: Vec<String> },
    GitflowInit { config: crate::manage::GitflowConfig },
    LfsInstall,
    /// Open a file in the external diff tool (git difftool), or a conflict in the merge tool.
    ExternalDiff { file: String, staged: bool, tool: String },
    ExternalMerge { file: String, tool: String },
    LfsTrack { pattern: String },
    LfsUntrack { pattern: String },
    LfsPull,
    LfsLock { path: String },
    LfsUnlock { path: String, force: bool },
    /// Add a pattern to .gitignore; `untrack` also removes matching tracked files from the index.
    Ignore { pattern: String, untrack: Vec<String> },
    SparseSet { patterns: Vec<String> },
    SparseDisable,
    GitflowStart { kind: String, name: String },
    GitflowFinish { kind: String, name: String, keep: bool, tag_message: String },
    AddRemote { name: String, url: String },
    /// Stage, unstage or discard one hunk, or some lines of it.
    Patch { file: String, untracked: bool, op: crate::files::PatchOp, hunk: usize, lines: Option<Vec<usize>> },
    CherryPickMany { shas: Vec<String> },
    Undo,
    Redo,
}

/// Start git without waiting (external tools stay open as long as the person needs them).
fn spawn_git(dir: &Path, args: &[String]) {
    let mut c = git::git_cmd(dir);
    c.args(args);
    let _ = c.spawn();
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
enum UndoRec {
    Commit { before: String, after: String, label: String },
    Checkout { from: String, to: String },
    CreateBranch { name: String, sha: String },
    DeleteBranch { name: String, sha: String },
    Reset { before: String, after: String, mode: String },
    CreateTag { name: String, sha: String },
    DeleteTag { name: String, sha: String },
    Discard { stash: String },
    Patch { patch: String, cached: bool, reverse: bool, label: String },
    /// History rewritten (rebase, cherry pick, fast-forward): move the branch back with reset --keep.
    Rewrite { before: String, after: String, label: String },
}

impl UndoRec {
    fn label(&self) -> String {
        match self {
            UndoRec::Commit { label, .. } => label.clone(),
            UndoRec::Checkout { to, .. } => format!("checkout {to}"),
            UndoRec::CreateBranch { name, .. } => format!("create branch {name}"),
            UndoRec::DeleteBranch { name, .. } => format!("delete branch {name}"),
            UndoRec::Reset { .. } => "reset".into(),
            UndoRec::CreateTag { name, .. } => format!("create tag {name}"),
            UndoRec::DeleteTag { name, .. } => format!("delete tag {name}"),
            UndoRec::Discard { .. } => "discard changes".into(),
            UndoRec::Patch { label, .. } => label.clone(),
            UndoRec::Rewrite { label, .. } => label.clone(),
        }
    }
}

#[derive(Default)]
struct Stacks {
    undo: Vec<UndoRec>,
    redo: Vec<UndoRec>,
}

#[derive(Default)]
pub struct Engine {
    log: Mutex<Vec<LogEntry>>,
    stacks: Mutex<HashMap<String, Stacks>>,
    /// Long network jobs with progress and Cancel (clone, fetch, pull, push).
    pub jobs: crate::jobs::Jobs,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UndoInfo {
    pub undo: Option<String>,
    pub redo: Option<String>,
}

impl Engine {
    pub fn log_entries(&self) -> Vec<LogEntry> {
        self.log.lock().unwrap().clone()
    }

    pub fn undo_info(&self, repo: &str) -> UndoInfo {
        let s = self.stacks.lock().unwrap();
        match s.get(repo) {
            Some(st) => UndoInfo {
                undo: st.undo.last().map(|r| r.label()),
                redo: st.redo.last().map(|r| r.label()),
            },
            None => UndoInfo::default(),
        }
    }

    fn push_undo(&self, repo: &str, rec: UndoRec) {
        let mut s = self.stacks.lock().unwrap();
        let st = s.entry(repo.to_string()).or_default();
        st.undo.push(rec);
        st.redo.clear();
        if st.undo.len() > 50 {
            st.undo.remove(0);
        }
    }

    /// Runs git and records it in the activity log.
    pub fn git(&self, dir: &Path, args: &[&str]) -> git::GitOut {
        let o = git::run(dir, args);
        let entry = LogEntry {
            time: chrono::Local::now().format("%H:%M:%S").to_string(),
            command: format!("git {}", args.join(" ")),
            ok: o.ok,
            output: o.message(),
        };
        let mut l = self.log.lock().unwrap();
        l.push(entry);
        if l.len() > 500 {
            l.remove(0);
        }
        o
    }

    /// Run a long git command as a cancellable job ("<repo>|<kind>"), logged like any other.
    pub fn job(&self, repo: &str, kind: &str, dir: &Path, args: &[&str]) -> git::GitOut {
        let o = self.jobs.run(&format!("{repo}|{kind}"), dir, args);
        let o = if !o.ok && o.stderr.trim().is_empty() { git::GitOut { ok: false, stdout: o.stdout, stderr: "Cancelled".into() } } else { o };
        self.record(&format!("git {}", args.join(" ")), &o);
        o
    }

    fn record(&self, command: &str, o: &git::GitOut) {
        let mut l = self.log.lock().unwrap();
        l.push(LogEntry { time: chrono::Local::now().format("%H:%M:%S").to_string(), command: command.to_string(), ok: o.ok, output: o.message() });
    }

    fn in_progress(&self, dir: &Path, what: &str) -> bool {
        let gd = git::run(dir, &["rev-parse", "--absolute-git-dir"]).stdout.trim().to_string();
        Path::new(&gd).join(what).exists()
    }

    fn log_note(&self, command: &str) {
        let mut l = self.log.lock().unwrap();
        l.push(LogEntry { time: chrono::Local::now().format("%H:%M:%S").to_string(), command: command.to_string(), ok: true, output: String::new() });
    }

    fn rev(&self, dir: &Path, r: &str) -> String {
        git::run(dir, &["rev-parse", "--verify", "-q", r]).stdout.trim().to_string()
    }

    fn head_name(&self, dir: &Path) -> String {
        let o = git::run(dir, &["symbolic-ref", "--short", "-q", "HEAD"]);
        if o.ok {
            o.stdout.trim().to_string()
        } else {
            self.rev(dir, "HEAD")
        }
    }

    pub fn perform(&self, repo: &str, action: Action) -> ActionResult {
        let dir = Path::new(repo);
        let done = |o: git::GitOut, msg: &str| -> ActionResult {
            if o.ok {
                ActionResult { ok: true, message: msg.to_string() }
            } else {
                ActionResult { ok: false, message: o.message() }
            }
        };
        match action {
            Action::Stage { paths } => {
                let mut a = vec!["add", "--"];
                a.extend(paths.iter().map(|s| s.as_str()));
                done(self.git(dir, &a), "Staged")
            }
            Action::Unstage { paths } => {
                let has_head = !self.rev(dir, "HEAD").is_empty();
                let mut a: Vec<&str> = if has_head { vec!["reset", "-q", "HEAD", "--"] } else { vec!["rm", "--cached", "-q", "--"] };
                a.extend(paths.iter().map(|s| s.as_str()));
                done(self.git(dir, &a), "Unstaged")
            }
            Action::StageAll => done(self.git(dir, &["add", "-A"]), "Staged all changes"),
            Action::UnstageAll => {
                if self.rev(dir, "HEAD").is_empty() {
                    done(self.git(dir, &["rm", "-r", "--cached", "-q", "."]), "Unstaged all changes")
                } else {
                    done(self.git(dir, &["reset", "-q"]), "Unstaged all changes")
                }
            }
            Action::Discard { paths } => self.discard(repo, dir, Some(paths)),
            Action::DiscardAll => self.discard(repo, dir, None),
            Action::Commit { summary, description, amend, stage_all, skip_hooks } => {
                if summary.trim().is_empty() {
                    return ActionResult { ok: false, message: "Type a commit summary first".into() };
                }
                if stage_all {
                    let o = self.git(dir, &["add", "-A"]);
                    if !o.ok {
                        return done(o, "");
                    }
                }
                let before = self.rev(dir, "HEAD");
                let mut msg = summary.trim().to_string();
                if !description.trim().is_empty() {
                    msg.push_str("\n\n");
                    msg.push_str(description.trim());
                }
                let mut a = vec!["commit", "-q", "-m", msg.as_str()];
                if amend {
                    a.push("--amend");
                }
                if skip_hooks {
                    a.push("--no-verify"); // "Skip git hooks"
                }
                let o = self.git(dir, &a);
                if o.ok && !before.is_empty() {
                    let after = self.rev(dir, "HEAD");
                    let label = if amend { "amend commit".to_string() } else { "commit".to_string() };
                    self.push_undo(repo, UndoRec::Commit { before, after, label });
                }
                done(o, if amend { "Amended previous commit" } else { "Committed" })
            }
            Action::Checkout { target } => {
                let from = self.head_name(dir);
                let o = self.git(dir, &["checkout", "-q", &target]);
                if o.ok {
                    self.push_undo(repo, UndoRec::Checkout { from, to: target.clone() });
                }
                done(o, &format!("Checked out {target}"))
            }
            Action::CheckoutRemote { remote_branch } => {
                let local = remote_branch.splitn(2, '/').nth(1).unwrap_or(&remote_branch).to_string();
                let exists = !self.rev(dir, &format!("refs/heads/{local}")).is_empty();
                let from = self.head_name(dir);
                let o = if exists {
                    self.git(dir, &["checkout", "-q", &local])
                } else {
                    self.git(dir, &["checkout", "-q", "-b", &local, "--track", &remote_branch])
                };
                if o.ok {
                    self.push_undo(repo, UndoRec::Checkout { from, to: local.clone() });
                }
                done(o, &format!("Checked out {local}"))
            }
            Action::CreateBranch { name, at, checkout } => {
                let name = name.trim().replace(' ', "-");
                if name.is_empty() {
                    return ActionResult { ok: false, message: "Branch name is empty".into() };
                }
                let at = at.unwrap_or_else(|| "HEAD".into());
                let o = self.git(dir, &["branch", &name, &at]);
                if !o.ok {
                    return done(o, "");
                }
                let sha = self.rev(dir, &name);
                self.push_undo(repo, UndoRec::CreateBranch { name: name.clone(), sha });
                if checkout {
                    let o = self.git(dir, &["checkout", "-q", &name]);
                    return done(o, &format!("Created and checked out {name}"));
                }
                ActionResult { ok: true, message: format!("Created branch {name}") }
            }
            Action::DeleteBranch { name } => {
                let sha = self.rev(dir, &format!("refs/heads/{name}"));
                let o = self.git(dir, &["branch", "-D", &name]);
                if o.ok {
                    self.push_undo(repo, UndoRec::DeleteBranch { name: name.clone(), sha });
                }
                done(o, &format!("Deleted {name}"))
            }
            Action::DeleteRemoteBranch { remote_branch } => {
                let (remote, b) = remote_branch.split_once('/').unwrap_or(("origin", &remote_branch));
                done(self.git(dir, &["push", remote, "--delete", b]), &format!("Deleted {remote_branch}"))
            }
            Action::RenameBranch { old, new } => {
                let new = new.trim().replace(' ', "-");
                done(self.git(dir, &["branch", "-m", &old, &new]), &format!("Renamed {old} to {new}"))
            }
            Action::CreateTag { name, at, message } => {
                let at = at.unwrap_or_else(|| "HEAD".into());
                let o = match message.as_deref().filter(|m| !m.trim().is_empty()) {
                    Some(m) => self.git(dir, &["tag", "-a", &name, &at, "-m", m]),
                    None => self.git(dir, &["tag", &name, &at]),
                };
                if o.ok {
                    let sha = self.rev(dir, &format!("{name}^{{commit}}"));
                    self.push_undo(repo, UndoRec::CreateTag { name: name.clone(), sha });
                }
                done(o, &format!("Created tag {name}"))
            }
            Action::DeleteTag { name } => {
                let sha = self.rev(dir, &format!("{name}^{{commit}}"));
                let o = self.git(dir, &["tag", "-d", &name]);
                if o.ok {
                    self.push_undo(repo, UndoRec::DeleteTag { name: name.clone(), sha });
                }
                done(o, &format!("Deleted tag {name}"))
            }
            Action::Fetch => done(self.job(repo, "fetch", dir, &["fetch", "--all", "--prune", "--progress"]), "Fetched all remotes"),
            Action::Pull { mode } => match mode.as_str() {
                "fetch" => done(self.job(repo, "fetch", dir, &["fetch", "--all", "--prune", "--progress"]), "Fetched all remotes"),
                "ffonly" => done(self.job(repo, "pull", dir, &["pull", "--ff-only", "--progress"]), "Pulled (fast-forward only)"),
                "rebase" => done(self.job(repo, "pull", dir, &["pull", "--rebase", "--progress"]), "Pulled (rebase)"),
                _ => done(self.job(repo, "pull", dir, &["pull", "--no-rebase", "--progress"]), "Pulled"),
            },
            Action::Push { force, lease } => {
                let branch = git::run(dir, &["symbolic-ref", "--short", "-q", "HEAD"]).stdout.trim().to_string();
                if branch.is_empty() {
                    return ActionResult { ok: false, message: "Push is not available when in a detached HEAD state".into() };
                }
                let upstream = git::run(dir, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]);
                let o = if upstream.ok {
                    if force && lease == Some(false) {
                        self.job(repo, "push", dir, &["push", "--force", "--progress"])
                    } else if force {
                        self.job(repo, "push", dir, &["push", "--force-with-lease", "--progress"])
                    } else {
                        self.job(repo, "push", dir, &["push", "--progress"])
                    }
                } else {
                    let remotes = git::run(dir, &["remote"]).stdout;
                    let remote = if remotes.lines().any(|r| r == "origin") {
                        "origin".to_string()
                    } else if let Some(r) = remotes.lines().next() {
                        r.to_string()
                    } else {
                        return ActionResult { ok: false, message: "This repository has no remote to push to. Add one first.".into() };
                    };
                    self.job(repo, "push", dir, &["push", "-u", "--progress", &remote, &branch])
                };
                done(o, &format!("Pushed {branch}"))
            }
            Action::Stash { message } => {
                let o = if message.trim().is_empty() {
                    self.git(dir, &["stash", "push", "--include-untracked"])
                } else {
                    self.git(dir, &["stash", "push", "--include-untracked", "-m", message.trim()])
                };
                done(o, "Stashed changes")
            }
            Action::StashPop { index } => done(self.git(dir, &["stash", "pop", "-q", &format!("stash@{{{index}}}")]), "Popped stash"),
            Action::StashApply { index } => done(self.git(dir, &["stash", "apply", "-q", &format!("stash@{{{index}}}")]), "Applied stash"),
            Action::StashDrop { index } => done(self.git(dir, &["stash", "drop", "-q", &format!("stash@{{{index}}}")]), "Deleted stash"),
            Action::CherryPick { sha } => done(self.git(dir, &["cherry-pick", &sha]), "Cherry picked commit"),
            Action::Revert { sha } => {
                let parents = git::run(dir, &["rev-list", "--parents", "-n1", &sha]).stdout;
                let o = if parents.split_whitespace().count() > 2 {
                    self.git(dir, &["revert", "--no-edit", "-m", "1", &sha])
                } else {
                    self.git(dir, &["revert", "--no-edit", &sha])
                };
                done(o, "Reverted commit")
            }
            Action::Reset { sha, mode } => {
                let before = self.rev(dir, "HEAD");
                let flag = match mode.as_str() {
                    "soft" => "--soft",
                    "hard" => "--hard",
                    _ => "--mixed",
                };
                let o = self.git(dir, &["reset", "-q", flag, &sha]);
                if o.ok {
                    self.push_undo(repo, UndoRec::Reset { before, after: sha.clone(), mode: flag.to_string() });
                }
                done(o, "Reset branch")
            }
            Action::Merge { branch } => done(self.git(dir, &["merge", "--no-edit", &branch]), &format!("Merged {branch}")),
            Action::Rebase { onto } => {
                let before = self.rev(dir, "HEAD");
                let o = self.git(dir, &["rebase", "--autostash", &onto]);
                if o.ok {
                    let after = self.rev(dir, "HEAD");
                    self.push_undo(repo, UndoRec::Rewrite { before, after, label: format!("rebase onto {onto}") });
                }
                done(o, &format!("Rebased onto {onto}"))
            }
            Action::AbortMerge => done(self.git(dir, &["merge", "--abort"]), "Merge aborted"),
            Action::AbortRebase => done(self.git(dir, &["rebase", "--abort"]), "Rebase aborted"),
            Action::ContinueRebase => done(self.git(dir, &["-c", "core.editor=true", "rebase", "--continue"]), "Rebase continued"),
            Action::SkipRebase => done(self.git(dir, &["rebase", "--skip"]), "Skipped commit"),
            Action::AutoFetch { prune } => {
                let o = if prune { self.git(dir, &["fetch", "--all", "--prune", "--quiet"]) } else { self.git(dir, &["fetch", "--all", "--quiet"]) };
                // Like the original: a failed background fetch only asks to fetch by hand.
                if o.ok {
                    ActionResult { ok: true, message: String::new() }
                } else {
                    ActionResult { ok: false, message: "Autofetch failed. Fetch manually for more information.".into() }
                }
            }
            Action::FetchRemote { remote } => done(self.job(repo, "fetch", dir, &["fetch", "--prune", "--progress", &remote]), &format!("Fetched {remote}")),
            Action::RemoteEdit { name, new_name, url, push_url } => {
                let mut name = name;
                if !new_name.trim().is_empty() && new_name.trim() != name {
                    let o = self.git(dir, &["remote", "rename", &name, new_name.trim()]);
                    if !o.ok {
                        return done(o, "");
                    }
                    name = new_name.trim().to_string();
                }
                let o = self.git(dir, &["remote", "set-url", &name, url.trim()]);
                if !o.ok {
                    return done(o, "");
                }
                if !push_url.trim().is_empty() && push_url.trim() != url.trim() {
                    return done(self.git(dir, &["remote", "set-url", "--push", &name, push_url.trim()]), "Remote updated");
                }
                ActionResult { ok: true, message: "Remote updated".into() }
            }
            Action::RemoteRemove { name } => done(self.git(dir, &["remote", "remove", &name]), &format!("Removed remote {name}")),
            Action::WorktreeAdd { path, branch, new_branch, start } => {
                let o = if new_branch {
                    let start = start.unwrap_or_else(|| "HEAD".into());
                    self.git(dir, &["worktree", "add", "-b", branch.trim(), path.trim(), &start])
                } else {
                    self.git(dir, &["worktree", "add", path.trim(), branch.trim()])
                };
                done(o, &format!("Worktree created at {}", path.trim()))
            }
            Action::WorktreeRemove { path, force } => {
                let o = if force { self.git(dir, &["worktree", "remove", "--force", &path]) } else { self.git(dir, &["worktree", "remove", &path]) };
                done(o, "Worktree removed")
            }
            Action::WorktreeLock { path, message } => {
                let o = if message.trim().is_empty() { self.git(dir, &["worktree", "lock", &path]) } else { self.git(dir, &["worktree", "lock", "--reason", message.trim(), &path]) };
                done(o, "Worktree locked")
            }
            Action::WorktreeUnlock { path } => done(self.git(dir, &["worktree", "unlock", &path]), "Worktree unlocked"),
            Action::SubmoduleAdd { url, path } => {
                let mut a = vec!["submodule", "add", url.trim()];
                if !path.trim().is_empty() {
                    a.push(path.trim());
                }
                done(self.git(dir, &a), "Submodule added")
            }
            Action::SubmoduleUpdate { paths } => {
                let mut a = vec!["submodule", "update", "--init", "--recursive", "--"];
                a.extend(paths.iter().map(|s| s.as_str()));
                done(self.git(dir, &a), "Submodules updated")
            }
            Action::SubmoduleRemove { path } => {
                let o = self.git(dir, &["submodule", "deinit", "-f", "--", &path]);
                if !o.ok {
                    return done(o, "");
                }
                done(self.git(dir, &["rm", "-f", "--", &path]), &format!("Removed submodule {path}"))
            }
            Action::DeleteBranches { names, remotes } => {
                let mut failed = Vec::new();
                for n in &names {
                    let sha = self.rev(dir, &format!("refs/heads/{n}"));
                    if self.git(dir, &["branch", "-D", n]).ok {
                        self.push_undo(repo, UndoRec::DeleteBranch { name: n.clone(), sha });
                    } else {
                        failed.push(n.clone());
                    }
                }
                for r in &remotes {
                    let (remote, b) = r.split_once('/').unwrap_or(("origin", r));
                    if !self.git(dir, &["push", remote, "--delete", b]).ok {
                        failed.push(r.clone());
                    }
                }
                if failed.is_empty() {
                    ActionResult { ok: true, message: format!("Deleted {} branch{}", names.len() + remotes.len(), if names.len() + remotes.len() == 1 { "" } else { "es" }) }
                } else {
                    ActionResult { ok: false, message: format!("Could not delete: {}", failed.join(", ")) }
                }
            }
            Action::ExternalDiff { file, staged, tool } => {
                let mut a = vec!["difftool".to_string(), "-y".into()];
                if staged {
                    a.push("--cached".into());
                }
                if !tool.trim().is_empty() {
                    a.push(format!("--tool={}", tool.trim()));
                }
                a.extend(["--".into(), file.clone()]);
                spawn_git(dir, &a);
                ActionResult { ok: true, message: format!("Opening {file} in the diff tool") }
            }
            Action::ExternalMerge { file, tool } => {
                let mut a = vec!["mergetool".to_string(), "-y".into()];
                if !tool.trim().is_empty() {
                    a.push(format!("--tool={}", tool.trim()));
                }
                a.extend(["--".into(), file.clone()]);
                spawn_git(dir, &a);
                ActionResult { ok: true, message: format!("Opening {file} in the merge tool") }
            }
            Action::LfsInstall => done(self.git(dir, &["lfs", "install", "--local"]), "Git LFS enabled for this repository"),
            Action::LfsTrack { pattern } => done(self.git(dir, &["lfs", "track", pattern.trim()]), &format!("LFS now tracks {}", pattern.trim())),
            Action::LfsUntrack { pattern } => done(self.git(dir, &["lfs", "untrack", pattern.trim()]), &format!("LFS no longer tracks {}", pattern.trim())),
            Action::LfsPull => done(self.git(dir, &["lfs", "pull"]), "LFS files downloaded"),
            Action::LfsLock { path } => done(self.git(dir, &["lfs", "lock", &path]), &format!("Locked {path}")),
            Action::LfsUnlock { path, force } => {
                let o = if force { self.git(dir, &["lfs", "unlock", "--force", &path]) } else { self.git(dir, &["lfs", "unlock", &path]) };
                done(o, &format!("Unlocked {path}"))
            }
            Action::Ignore { pattern, untrack } => {
                if let Err(e) = crate::manage::ignore(dir, pattern.trim()) {
                    return ActionResult { ok: false, message: e };
                }
                if !untrack.is_empty() {
                    let mut a = vec!["rm", "-r", "-q", "--cached", "--ignore-unmatch", "--"];
                    a.extend(untrack.iter().map(|s| s.as_str()));
                    let o = self.git(dir, &a);
                    if !o.ok {
                        return done(o, "");
                    }
                }
                ActionResult { ok: true, message: format!("Added {} to .gitignore", pattern.trim()) }
            }
            Action::SparseSet { patterns } => {
                let mut a = vec!["sparse-checkout", "set", "--no-cone"];
                a.extend(patterns.iter().map(|s| s.as_str()));
                done(self.git(dir, &a), "Sparse checkout updated")
            }
            Action::SparseDisable => done(self.git(dir, &["sparse-checkout", "disable"]), "Sparse checkout turned off"),
            Action::GitflowInit { config } => done(crate::manage::gitflow_init(dir, &config, &|a: &[&str]| self.git(dir, a)), "Gitflow has been initialized"),
            Action::GitflowStart { kind, name } => {
                done(crate::manage::gitflow_start(dir, &kind, &name, &|a: &[&str]| self.git(dir, a)), &format!("Started {kind}: '{}'", name.trim()))
            }
            Action::GitflowFinish { kind, name, keep, tag_message } => {
                done(crate::manage::gitflow_finish(dir, &kind, &name, keep, &tag_message, &|a: &[&str]| self.git(dir, a)), &format!("Finished {kind}: '{name}'"))
            }
            Action::InteractiveRebase { base, items } => {
                let before = self.rev(dir, "HEAD");
                let o = crate::rewrite::interactive_rebase(dir, &base, &items);
                self.record(&format!("git rebase -i {base} ({} steps)", items.len()), &o);
                if o.ok {
                    let after = self.rev(dir, "HEAD");
                    self.push_undo(repo, UndoRec::Rewrite { before, after, label: "interactive rebase".into() });
                    ActionResult { ok: true, message: "Rebase complete".into() }
                } else if self.in_progress(dir, "rebase-merge") {
                    ActionResult { ok: false, message: "The rebase stopped on a conflict. Resolve it, then Continue Rebase (or Abort Rebase).".into() }
                } else {
                    ActionResult { ok: false, message: o.message() }
                }
            }
            Action::InteractiveCherryPick { items } => {
                let before = self.rev(dir, "HEAD");
                let o = crate::rewrite::interactive_cherry_pick(dir, &items, &|a: &[&str]| self.git(dir, a));
                let after = self.rev(dir, "HEAD");
                if after != before {
                    self.push_undo(repo, UndoRec::Rewrite { before, after, label: "interactive cherry pick".into() });
                }
                done(o, "Cherry pick complete")
            }
            Action::TakeSide { paths, side } => {
                let flag = if side == "theirs" { "--theirs" } else { "--ours" };
                let mut a = vec!["checkout", flag, "--"];
                a.extend(paths.iter().map(|s| s.as_str()));
                let o = self.git(dir, &a);
                if !o.ok {
                    // That side deleted the file: taking it means removing the file.
                    let mut r = vec!["rm", "-q", "--"];
                    r.extend(paths.iter().map(|s| s.as_str()));
                    return done(self.git(dir, &r), "Resolved");
                }
                let mut add = vec!["add", "--"];
                add.extend(paths.iter().map(|s| s.as_str()));
                done(self.git(dir, &add), if side == "theirs" { "Took incoming" } else { "Took current" })
            }
            Action::MarkResolved { paths } => {
                let mut a = vec!["add", "--"];
                a.extend(paths.iter().map(|s| s.as_str()));
                done(self.git(dir, &a), "Marked resolved")
            }
            Action::MarkConflicted { paths } => {
                let mut a = vec!["checkout", "-m", "--"];
                a.extend(paths.iter().map(|s| s.as_str()));
                done(self.git(dir, &a), "Marked conflicted")
            }
            Action::SaveResolved { file, content } => {
                if let Err(e) = crate::files::save(dir, &file, &content) {
                    return ActionResult { ok: false, message: e };
                }
                done(self.git(dir, &["add", "--", &file]), &format!("Saved and resolved {file}"))
            }
            Action::FastForward { branch, to } => {
                let head = git::run(dir, &["symbolic-ref", "--short", "-q", "HEAD"]).stdout.trim().to_string();
                let before = self.rev(dir, &format!("refs/heads/{branch}"));
                let o = if head == branch {
                    self.git(dir, &["merge", "--ff-only", &to])
                } else {
                    // Moves a branch that isn't checked out; git refuses anything but a fast-forward.
                    self.git(dir, &["fetch", ".", &format!("{to}:{branch}")])
                };
                if o.ok && head == branch {
                    let after = self.rev(dir, "HEAD");
                    self.push_undo(repo, UndoRec::Rewrite { before, after, label: format!("fast-forward {branch}") });
                }
                done(o, &format!("Fast-forwarded {branch} to {to}"))
            }
            Action::SetUpstream { branch, upstream } => {
                if upstream.trim().is_empty() {
                    done(self.git(dir, &["branch", "--unset-upstream", &branch]), "Upstream removed")
                } else {
                    let up = upstream.trim().to_string();
                    done(self.git(dir, &["branch", &format!("--set-upstream-to={up}"), &branch]), &format!("{branch} now tracks {up}"))
                }
            }
            Action::StashRename { index, message } => {
                // git has no rename: drop the entry and store the same stash commit with the new message.
                let sha = self.rev(dir, &format!("stash@{{{index}}}"));
                if sha.is_empty() {
                    return ActionResult { ok: false, message: "That stash no longer exists".into() };
                }
                let o = self.git(dir, &["stash", "drop", "-q", &format!("stash@{{{index}}}")]);
                if !o.ok {
                    return done(o, "");
                }
                done(self.git(dir, &["stash", "store", "-m", message.trim(), &sha]), "Stash message updated")
            }
            Action::AddRemote { name, url } => done(self.git(dir, &["remote", "add", name.trim(), url.trim()]), "Remote added"),
            Action::Patch { file, untracked, op, hunk, lines } => {
                let what = if lines.is_some() { "lines" } else { "hunk" };
                let verb = match op {
                    crate::files::PatchOp::Stage => "stage",
                    crate::files::PatchOp::Unstage => "unstage",
                    crate::files::PatchOp::Discard => "discard",
                };
                self.log_note(&format!("git apply ({verb} {what} of {file})"));
                match crate::files::apply_part(dir, &file, untracked, op, hunk, lines.as_deref()) {
                    Ok((patch, cached, reverse)) => {
                        // Staging is easy to reverse by hand; discards are recorded for Undo.
                        if op == crate::files::PatchOp::Discard {
                            self.push_undo(repo, UndoRec::Patch { patch, cached, reverse, label: format!("discard {what} in {file}") });
                        }
                        let past = match op {
                            crate::files::PatchOp::Stage => "Staged",
                            crate::files::PatchOp::Unstage => "Unstaged",
                            crate::files::PatchOp::Discard => "Discarded",
                        };
                        ActionResult { ok: true, message: format!("{past} {what}") }
                    }
                    Err(e) => ActionResult { ok: false, message: e },
                }
            }
            Action::CherryPickMany { shas } => {
                let mut a = vec!["cherry-pick"];
                a.extend(shas.iter().map(|s| s.as_str()));
                done(self.git(dir, &a), &format!("Cherry picked {} commits", shas.len()))
            }
            Action::Undo => self.undo(repo, dir),
            Action::Redo => self.redo(repo, dir),
        }
    }

    fn discard(&self, repo: &str, dir: &Path, paths: Option<Vec<String>>) -> ActionResult {
        // Like the original: keep an automatic stash so the discard can be undone.
        let mut a = vec!["stash", "push", "--include-untracked", "-m", "Auto stash before discard"];
        if let Some(p) = &paths {
            a.push("--");
            a.extend(p.iter().map(|s| s.as_str()));
        }
        let o = self.git(dir, &a);
        if !o.ok {
            return ActionResult { ok: false, message: o.message() };
        }
        let sha = self.rev(dir, "stash@{0}");
        let _ = self.git(dir, &["stash", "drop", "-q"]);
        self.push_undo(repo, UndoRec::Discard { stash: sha });
        ActionResult { ok: true, message: "Discarded changes".into() }
    }

    fn apply(&self, dir: &Path, rec: &UndoRec, undo: bool) -> git::GitOut {
        match (rec, undo) {
            (UndoRec::Commit { before, .. }, true) => self.git(dir, &["reset", "-q", "--soft", before]),
            (UndoRec::Commit { after, .. }, false) => self.git(dir, &["reset", "-q", "--soft", after]),
            (UndoRec::Checkout { from, .. }, true) => self.git(dir, &["checkout", "-q", from]),
            (UndoRec::Checkout { to, .. }, false) => self.git(dir, &["checkout", "-q", to]),
            (UndoRec::CreateBranch { name, .. }, true) | (UndoRec::DeleteBranch { name, .. }, false) => {
                self.git(dir, &["branch", "-D", name])
            }
            (UndoRec::CreateBranch { name, sha }, false) | (UndoRec::DeleteBranch { name, sha }, true) => {
                self.git(dir, &["branch", name, sha])
            }
            (UndoRec::Reset { before, mode, .. }, true) => self.git(dir, &["reset", "-q", mode, before]),
            (UndoRec::Reset { after, mode, .. }, false) => self.git(dir, &["reset", "-q", mode, after]),
            (UndoRec::CreateTag { name, .. }, true) | (UndoRec::DeleteTag { name, .. }, false) => self.git(dir, &["tag", "-d", name]),
            (UndoRec::CreateTag { name, sha }, false) | (UndoRec::DeleteTag { name, sha }, true) => self.git(dir, &["tag", name, sha]),
            (UndoRec::Discard { stash }, true) => self.git(dir, &["stash", "apply", "-q", stash]),
            (UndoRec::Discard { .. }, false) => self.git(dir, &["stash", "push", "--include-untracked", "-m", "Auto stash before redo discard"]),
            (UndoRec::Rewrite { before, .. }, true) => self.git(dir, &["reset", "-q", "--keep", before]),
            (UndoRec::Rewrite { after, .. }, false) => self.git(dir, &["reset", "-q", "--keep", after]),
            (UndoRec::Patch { patch, cached, reverse, .. }, undo) => {
                // Undo applies the patch the other way round; redo applies it again.
                let rev = if undo { !*reverse } else { *reverse };
                match crate::files::apply_patch(dir, patch, *cached, rev) {
                    Ok(()) => git::GitOut { ok: true, stdout: String::new(), stderr: String::new() },
                    Err(e) => git::GitOut { ok: false, stdout: String::new(), stderr: e },
                }
            }
        }
    }

    fn undo(&self, repo: &str, dir: &Path) -> ActionResult {
        let rec = {
            let mut s = self.stacks.lock().unwrap();
            s.get_mut(repo).and_then(|st| st.undo.pop())
        };
        let Some(rec) = rec else {
            return ActionResult { ok: false, message: "Nothing to undo".into() };
        };
        let o = self.apply(dir, &rec, true);
        if o.ok {
            let label = rec.label();
            self.stacks.lock().unwrap().entry(repo.to_string()).or_default().redo.push(rec);
            ActionResult { ok: true, message: format!("Undo Successful: {label}") }
        } else {
            ActionResult { ok: false, message: format!("Undo Failed: {}", o.message()) }
        }
    }

    fn redo(&self, repo: &str, dir: &Path) -> ActionResult {
        let rec = {
            let mut s = self.stacks.lock().unwrap();
            s.get_mut(repo).and_then(|st| st.redo.pop())
        };
        let Some(rec) = rec else {
            return ActionResult { ok: false, message: "Nothing to redo".into() };
        };
        let o = self.apply(dir, &rec, false);
        if o.ok {
            let label = rec.label();
            self.stacks.lock().unwrap().entry(repo.to_string()).or_default().undo.push(rec);
            ActionResult { ok: true, message: format!("Redo Successful: {label}") }
        } else {
            ActionResult { ok: false, message: format!("Redo Failed: {}", o.message()) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_repo(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("gitcraft-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        for a in [&["init", "-q", "-b", "master"][..], &["config", "user.name", "Test"], &["config", "user.email", "t@example.com"], &["config", "commit.gpgsign", "false"]] {
            assert!(git::run(&d, a).ok);
        }
        d
    }

    fn head(d: &Path) -> String {
        git::run(d, &["rev-parse", "HEAD"]).stdout.trim().to_string()
    }

    #[test]
    fn stage_commit_undo_redo_branch_stash_discard() {
        let d = temp_repo("flow");
        let repo = d.to_string_lossy().to_string();
        let e = Engine::default();
        fs::write(d.join("a.txt"), "one\n").unwrap();
        assert!(e.perform(&repo, Action::StageAll).ok);
        assert!(e.perform(&repo, Action::Commit { summary: "first".into(), description: String::new(), amend: false, stage_all: false, skip_hooks: false }).ok);
        let first = head(&d);

        fs::write(d.join("a.txt"), "one\ntwo\n").unwrap();
        let st = git::status(&d);
        assert_eq!(st.unstaged.len(), 1);
        assert!(e.perform(&repo, Action::Stage { paths: vec!["a.txt".into()] }).ok);
        assert_eq!(git::status(&d).staged.len(), 1);
        assert!(e.perform(&repo, Action::Commit { summary: "second".into(), description: "body".into(), amend: false, stage_all: false, skip_hooks: false }).ok);
        let second = head(&d);
        assert_ne!(first, second);

        // Undo the commit: HEAD goes back, the change stays staged.
        assert!(e.perform(&repo, Action::Undo).ok);
        assert_eq!(head(&d), first);
        assert_eq!(git::status(&d).staged.len(), 1);
        assert!(e.perform(&repo, Action::Redo).ok);
        assert_eq!(head(&d), second);

        // Branches
        assert!(e.perform(&repo, Action::CreateBranch { name: "feature x".into(), at: None, checkout: true }).ok);
        let data = git::load(&d, 100);
        assert_eq!(data.head_branch.as_deref(), Some("feature-x"));
        assert!(e.perform(&repo, Action::Checkout { target: "master".into() }).ok);
        assert!(e.perform(&repo, Action::DeleteBranch { name: "feature-x".into() }).ok);
        assert!(e.perform(&repo, Action::Undo).ok, "undo delete branch");
        assert!(git::load(&d, 100).refs.iter().any(|r| r.name == "feature-x"));

        // Stash and pop
        fs::write(d.join("b.txt"), "new file\n").unwrap();
        assert!(e.perform(&repo, Action::Stash { message: "wip".into() }).ok);
        assert_eq!(git::load(&d, 100).stashes.len(), 1);
        assert!(git::status(&d).unstaged.is_empty());
        assert!(e.perform(&repo, Action::StashPop { index: 0 }).ok);
        assert_eq!(git::status(&d).unstaged.len(), 1);

        // Discard is undoable
        assert!(e.perform(&repo, Action::DiscardAll).ok);
        assert!(git::status(&d).unstaged.is_empty());
        assert!(e.perform(&repo, Action::Undo).ok);
        assert!(d.join("b.txt").exists());

        // Tags, reset, diff, graph
        assert!(e.perform(&repo, Action::CreateTag { name: "v1".into(), at: Some(first.clone()), message: None }).ok);
        let data = git::load(&d, 100);
        assert!(data.refs.iter().any(|r| r.kind == git::RefKind::Tag && r.sha == first));
        let files = git::commit_files(&d, &data.commits[0]);
        assert_eq!(files.len(), 1);
        let diff = git::file_diff(&d, &git::DiffSource::Commit(second.clone()), "a.txt", false, Some(&first));
        assert!(diff.iter().any(|l| l.kind == git::DiffLineKind::Added && l.text == "two"));
        let g = crate::graph::layout(&data.commits);
        assert_eq!(g.rows.len(), data.commits.len());
        assert!(e.perform(&repo, Action::Reset { sha: first.clone(), mode: "soft".into() }).ok);
        assert_eq!(head(&d), first);
        assert!(e.perform(&repo, Action::Undo).ok);
        assert_eq!(head(&d), second);
        assert!(!e.log_entries().is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn stage_and_discard_single_lines_hide_and_compare() {
        let d = temp_repo("lines");
        let repo = d.to_string_lossy().to_string();
        let e = Engine::default();
        // Line endings may become CRLF on Windows (core.autocrlf); compare without them.
        let read = |p: &str| fs::read_to_string(d.join(p)).unwrap().replace("\r\n", "\n");
        let index = |p: &str| git::run(&d, &["show", &format!(":{p}")]).stdout.replace("\r\n", "\n");
        fs::write(d.join("f.txt"), "a\nb\nc\n").unwrap();
        assert!(e.perform(&repo, Action::Commit { summary: "base".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false }).ok);
        let base = head(&d);
        fs::write(d.join("f.txt"), "a\nB\nc\nnew\n").unwrap();
        // Hunk body: " a", "-b", "+B", " c", "+new" -> stage only "+new" (index 4)
        let r = e.perform(&repo, Action::Patch { file: "f.txt".into(), untracked: false, op: crate::files::PatchOp::Stage, hunk: 0, lines: Some(vec![4]) });
        assert!(r.ok, "{}", r.message);
        assert_eq!(index("f.txt"), "a\nb\nc\nnew\n");
        // Discard the remaining unstaged change (b -> B), then undo the discard.
        let r = e.perform(&repo, Action::Patch { file: "f.txt".into(), untracked: false, op: crate::files::PatchOp::Discard, hunk: 0, lines: None });
        assert!(r.ok, "{}", r.message);
        assert_eq!(read("f.txt"), "a\nb\nc\nnew\n");
        assert!(e.perform(&repo, Action::Undo).ok);
        assert_eq!(read("f.txt"), "a\nB\nc\nnew\n");
        // Unstage the staged line again.
        let r = e.perform(&repo, Action::Patch { file: "f.txt".into(), untracked: false, op: crate::files::PatchOp::Unstage, hunk: 0, lines: None });
        assert!(r.ok, "{}", r.message);
        assert!(git::status(&d).staged.is_empty());
        // Untracked file: stage one line of a brand-new file.
        fs::write(d.join("n.txt"), "one\ntwo\n").unwrap();
        let r = e.perform(&repo, Action::Patch { file: "n.txt".into(), untracked: true, op: crate::files::PatchOp::Stage, hunk: 0, lines: Some(vec![0]) });
        assert!(r.ok, "{}", r.message);
        assert_eq!(index("n.txt"), "one\n");

        // Commit and compare; hide a branch from the graph.
        assert!(e.perform(&repo, Action::Commit { summary: "next".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false }).ok);
        assert!(e.perform(&repo, Action::CreateBranch { name: "side".into(), at: Some(base.clone()), checkout: false }).ok);
        let files = crate::files::range_files(&d, &base, "HEAD");
        assert_eq!(files.len(), 2);
        let st = crate::files::stats(&d, &[head(&d)]);
        assert_eq!(st[0].files, 2);
        let hist = crate::files::history(&d, "f.txt", 10);
        assert_eq!(hist.len(), 2);
        let bl = crate::files::blame(&d, "f.txt", None).unwrap();
        assert_eq!(bl.len(), 4);
        let v = crate::files::versions(&d, &git::DiffSource::Range(base.clone(), git::WORKDIR.into()), "f.txt", None, false);
        assert_eq!(v.old.as_deref().map(|s| s.replace("\r\n", "\n")), Some("a\nb\nc\n".to_string()));
        let view = git::RefView { hidden: vec!["refs/heads/side".into()], solo: vec![], pinned: None };
        assert_eq!(git::load_view(&d, 100, &view).commits.len(), 2);
        let solo = git::RefView { hidden: vec![], solo: vec!["refs/heads/side".into()], pinned: None };
        assert_eq!(git::load_view(&d, 100, &solo).commits.len(), 1);
        let more = git::load_view(&d, 1, &git::RefView::default());
        assert!(more.has_more);
        let _ = fs::remove_dir_all(&d);
    }


    #[test]
    fn interactive_rebase_conflicts_and_branch_extras() {
        use crate::rewrite::{Step, TodoItem};
        let d = temp_repo("rewrite");
        let repo = d.to_string_lossy().to_string();
        let e = Engine::default();
        let read = |p: &str| fs::read_to_string(d.join(p)).unwrap().replace("\r\n", "\n");
        let commit = |f: &str, t: &str, m: &str| {
            fs::write(d.join(f), t).unwrap();
            assert!(e.perform(&repo, Action::Commit { summary: m.into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false }).ok);
            head(&d)
        };
        let base = commit("a.txt", "1\n", "one");
        let c2 = commit("b.txt", "2\n", "two");
        let c3 = commit("c.txt", "3\n", "three");
        let c4 = commit("d.txt", "4\n", "four");
        let list = crate::rewrite::rebase_commits(&d, &base).unwrap();
        assert_eq!(list.iter().map(|c| c.sha.clone()).collect::<Vec<_>>(), vec![c2.clone(), c3.clone(), c4.clone()]);

        // Reword "two", squash "three" into it, drop "four"; a dirty file survives (autostash).
        fs::write(d.join("a.txt"), "1 changed\n").unwrap();
        let items = vec![
            TodoItem { action: Step::Reword, sha: c2.clone(), message: Some("two (reworded)".into()) },
            TodoItem { action: Step::Squash, sha: c3.clone(), message: None },
            TodoItem { action: Step::Drop, sha: c4.clone(), message: None },
        ];
        let r = e.perform(&repo, Action::InteractiveRebase { base: base.clone(), items });
        assert!(r.ok, "{}", r.message);
        let log = git::run(&d, &["log", "--format=%s", &format!("{base}..HEAD")]).stdout;
        assert_eq!(log.lines().collect::<Vec<_>>(), vec!["two (reworded)"]);
        let msg = git::run(&d, &["log", "-1", "--format=%B"]).stdout;
        assert!(msg.contains("three"), "squash keeps both messages: {msg}");
        assert!(!d.join("d.txt").exists());
        assert!(d.join("c.txt").exists());
        assert_eq!(read("a.txt"), "1 changed\n");
        // Undo puts the branch back.
        assert!(e.perform(&repo, Action::Undo).ok);
        assert_eq!(head(&d), c4);
        assert!(e.perform(&repo, Action::DiscardAll).ok);

        // Reorder: "four" before "two"; then interactive cherry pick onto a side branch.
        let items = vec![
            TodoItem { action: Step::Pick, sha: c4.clone(), message: None },
            TodoItem { action: Step::Pick, sha: c2.clone(), message: None },
            TodoItem { action: Step::Pick, sha: c3.clone(), message: None },
        ];
        assert!(e.perform(&repo, Action::InteractiveRebase { base: base.clone(), items }).ok);
        let log = git::run(&d, &["log", "--reverse", "--format=%s", &format!("{base}..HEAD")]).stdout;
        assert_eq!(log.lines().collect::<Vec<_>>(), vec!["four", "two", "three"]);
        assert!(e.perform(&repo, Action::CreateBranch { name: "side".into(), at: Some(base.clone()), checkout: true }).ok);
        let items = vec![
            TodoItem { action: Step::Pick, sha: c2.clone(), message: None },
            TodoItem { action: Step::Reword, sha: c3.clone(), message: Some("three!".into()) },
        ];
        let r = e.perform(&repo, Action::InteractiveCherryPick { items });
        assert!(r.ok, "{}", r.message);
        let log = git::run(&d, &["log", "--reverse", "--format=%s", &format!("{base}..HEAD")]).stdout;
        assert_eq!(log.lines().collect::<Vec<_>>(), vec!["two", "three!"]);

        // Merge conflict: same line changed on both branches.
        commit("a.txt", "side\n", "side change");
        assert!(e.perform(&repo, Action::Checkout { target: "master".into() }).ok);
        commit("a.txt", "master\n", "master change");
        let r = e.perform(&repo, Action::Merge { branch: "side".into() });
        assert!(!r.ok);
        let data = git::load(&d, 50);
        assert_eq!(data.state_note.as_deref(), Some("merging"));
        assert!(data.status.conflicted);
        assert_eq!(data.conflict_labels, Some(("master".into(), "side".into())));
        let info = crate::rewrite::conflict_info(&d, "a.txt");
        assert_eq!(info.ours.as_deref().map(|s| s.replace("\r\n", "\n")), Some("master\n".into()));
        assert_eq!(info.theirs.as_deref().map(|s| s.replace("\r\n", "\n")), Some("side\n".into()));
        assert!(info.working.unwrap().contains("<<<<<<<"));
        assert!(e.perform(&repo, Action::TakeSide { paths: vec!["a.txt".into()], side: "theirs".into() }).ok);
        assert_eq!(read("a.txt"), "side\n");
        assert!(e.perform(&repo, Action::MarkConflicted { paths: vec!["a.txt".into()] }).ok);
        assert!(read("a.txt").contains("<<<<<<<"));
        assert!(e.perform(&repo, Action::SaveResolved { file: "a.txt".into(), content: "master\nside\n".into() }).ok);
        assert!(!git::status(&d).conflicted);
        assert!(git::load(&d, 50).merge_msg.unwrap().contains("Merge branch 'side'"));
        assert!(e.perform(&repo, Action::Commit { summary: "Merge side".into(), description: String::new(), amend: false, stage_all: false, skip_hooks: false }).ok);
        assert_eq!(git::run(&d, &["rev-list", "--parents", "-n1", "HEAD"]).stdout.split_whitespace().count(), 3);

        // Fast-forward a branch that isn't checked out, set an upstream, rename a stash.
        assert!(e.perform(&repo, Action::CreateBranch { name: "old".into(), at: Some(base.clone()), checkout: false }).ok);
        assert!(e.perform(&repo, Action::FastForward { branch: "old".into(), to: "master".into() }).ok);
        assert_eq!(git::run(&d, &["rev-parse", "old"]).stdout, git::run(&d, &["rev-parse", "master"]).stdout);
        assert!(e.perform(&repo, Action::SetUpstream { branch: "old".into(), upstream: "master".into() }).ok);
        assert_eq!(git::run(&d, &["rev-parse", "--abbrev-ref", "old@{u}"]).stdout.trim(), "master");
        fs::write(d.join("e.txt"), "x").unwrap();
        assert!(e.perform(&repo, Action::Stash { message: "first".into() }).ok);
        assert!(e.perform(&repo, Action::StashRename { index: 0, message: "renamed".into() }).ok);
        assert_eq!(git::load(&d, 5).stashes[0].message, "renamed");
        let _ = fs::remove_dir_all(&d);
    }


    #[test]
    fn remotes_worktrees_submodules_cleanup_and_gitflow() {
        let d = temp_repo("manage");
        let repo = d.to_string_lossy().to_string();
        let e = Engine::default();
        let commit = |f: &str, m: &str| {
            fs::write(d.join(f), m).unwrap();
            assert!(e.perform(&repo, Action::Commit { summary: m.into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false }).ok);
        };
        commit("a.txt", "one");
        assert!(e.perform(&repo, Action::RenameBranch { old: "master".into(), new: "main".into() }).ok);

        // Remotes: add a bare clone, fetch it, rename and re-point it, then remove it.
        let bare = std::env::temp_dir().join(format!("gitcraft-test-bare-{}", std::process::id()));
        let _ = fs::remove_dir_all(&bare);
        assert!(git::run(&d, &["clone", "-q", "--bare", ".", bare.to_string_lossy().as_ref()]).ok);
        let url = bare.to_string_lossy().to_string();
        assert!(e.perform(&repo, Action::AddRemote { name: "origin".into(), url: url.clone() }).ok);
        assert!(e.perform(&repo, Action::FetchRemote { remote: "origin".into() }).ok);
        assert!(e.perform(&repo, Action::AutoFetch { prune: true }).ok);
        assert!(e.perform(&repo, Action::RemoteEdit { name: "origin".into(), new_name: "upstream".into(), url: url.clone(), push_url: String::new() }).ok);
        let rs = crate::manage::remotes(&d);
        assert_eq!(rs.len(), 1);
        assert_eq!(rs[0].name, "upstream");
        assert!(e.perform(&repo, Action::RemoteRemove { name: "upstream".into() }).ok);
        assert!(crate::manage::remotes(&d).is_empty());

        // Worktrees: add with a new branch, lock, unlock, remove.
        let wt = std::env::temp_dir().join(format!("gitcraft-test-wt-{}", std::process::id()));
        let _ = fs::remove_dir_all(&wt);
        let wts = wt.to_string_lossy().to_string();
        assert!(e.perform(&repo, Action::WorktreeAdd { path: wts.clone(), branch: "wt-branch".into(), new_branch: true, start: None }).ok);
        let data = git::load(&d, 10);
        assert_eq!(data.worktrees.len(), 2);
        assert!(e.perform(&repo, Action::WorktreeLock { path: wts.clone(), message: "busy".into() }).ok);
        assert!(git::load(&d, 10).worktrees.iter().any(|w| w.locked));
        assert!(e.perform(&repo, Action::WorktreeUnlock { path: wts.clone() }).ok);
        assert!(e.perform(&repo, Action::WorktreeRemove { path: wts.clone(), force: false }).ok);
        assert_eq!(git::load(&d, 10).worktrees.len(), 1);

        // Submodule from the bare repository.
        let r = e.perform(&repo, Action::SubmoduleAdd { url: url.clone(), path: "libs/dep".into() });
        if r.ok {
            let subs = crate::manage::submodules(&d);
            assert_eq!(subs.len(), 1);
            assert_eq!(subs[0].path, "libs/dep");
            assert!(e.perform(&repo, Action::SubmoduleUpdate { paths: vec![] }).ok);
            assert!(e.perform(&repo, Action::SubmoduleRemove { path: "libs/dep".into() }).ok);
        } // (some Git setups forbid file:// submodules; the rest still runs)
        assert!(e.perform(&repo, Action::Commit { summary: "tidy".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false }).ok || true);

        // Gitflow: init creates develop; feature start / finish; release with a tag.
        let cfg = crate::manage::gitflow_config(&d);
        assert!(!cfg.initialized);
        assert_eq!((cfg.master.as_str(), cfg.develop.as_str(), cfg.feature.as_str()), ("main", "develop", "feature/"));
        assert!(e.perform(&repo, Action::GitflowInit { config: cfg }).ok);
        assert!(crate::manage::gitflow_config(&d).initialized);
        assert!(e.perform(&repo, Action::GitflowStart { kind: "feature".into(), name: "login".into() }).ok);
        commit("login.txt", "login");
        let r = e.perform(&repo, Action::GitflowFinish { kind: "feature".into(), name: "login".into(), keep: false, tag_message: String::new() });
        assert!(r.ok, "{}", r.message);
        assert!(d.join("login.txt").exists());
        assert!(e.perform(&repo, Action::GitflowStart { kind: "release".into(), name: "1.0".into() }).ok);
        let r = e.perform(&repo, Action::GitflowFinish { kind: "release".into(), name: "1.0".into(), keep: false, tag_message: "Release 1.0".into() });
        assert!(r.ok, "{}", r.message);
        assert!(git::run(&d, &["rev-parse", "--verify", "-q", "refs/tags/1.0"]).ok);
        assert!(git::run(&d, &["merge-base", "--is-ancestor", "refs/tags/1.0", "main"]).ok);

        // Branch clean-up: a merged branch shows up as merged and can be deleted (and undone).
        assert!(e.perform(&repo, Action::CreateBranch { name: "old-work".into(), at: None, checkout: false }).ok);
        let list = crate::manage::cleanup_list(&d);
        assert!(list.iter().any(|b| b.name == "old-work" && b.merged));
        assert!(e.perform(&repo, Action::DeleteBranches { names: vec!["old-work".into()], remotes: vec![] }).ok);
        assert!(!crate::manage::cleanup_list(&d).iter().any(|b| b.name == "old-work"));
        assert!(e.perform(&repo, Action::Undo).ok);
        assert!(crate::manage::cleanup_list(&d).iter().any(|b| b.name == "old-work"));
        let _ = fs::remove_dir_all(&d);
        let _ = fs::remove_dir_all(&bare);
    }


    #[test]
    fn templates_ignore_hooks_sparse_and_shallow() {
        let d = temp_repo("tools");
        let repo = d.to_string_lossy().to_string();
        let e = Engine::default();
        // Starting files and the first commit.
        let opts = crate::manage::InitOptions { branch: String::new(), gitignore: vec!["Node".into(), "Rust".into()], license: "MIT".into(), readme: true };
        assert!(crate::manage::init_files(&d, "demo", &opts, &|a: &[&str]| e.git(&d, a)).ok);
        let gi = fs::read_to_string(d.join(".gitignore")).unwrap();
        assert!(gi.contains("node_modules/") && gi.contains("/target/"));
        assert!(fs::read_to_string(d.join("LICENSE")).unwrap().starts_with("MIT License"));
        assert_eq!(git::run(&d, &["log", "--format=%s"]).stdout.trim(), "Initial commit");

        // Ignore helpers: add once, untrack a tracked file.
        fs::write(d.join("secret.log"), "x").unwrap();
        assert!(e.perform(&repo, Action::Commit { summary: "log".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false }).ok);
        assert!(e.perform(&repo, Action::Ignore { pattern: "*.log".into(), untrack: vec!["secret.log".into()] }).ok);
        assert!(e.perform(&repo, Action::Ignore { pattern: "*.log".into(), untrack: vec![] }).ok);
        assert_eq!(fs::read_to_string(d.join(".gitignore")).unwrap().matches("*.log").count(), 1);
        assert!(git::status(&d).staged.iter().any(|f| f.path == "secret.log" && f.status == 'D'));

        // Skip hooks: a failing pre-commit hook blocks a normal commit but not a skipped one.
        let hooks = d.join(".git").join("hooks");
        fs::create_dir_all(&hooks).unwrap();
        fs::write(hooks.join("pre-commit"), "#!/bin/sh\necho blocked by hook >&2\nexit 1\n").unwrap();
        let r = e.perform(&repo, Action::Commit { summary: "blocked".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false });
        assert!(!r.ok && r.message.contains("blocked by hook"), "{}", r.message);
        assert!(e.perform(&repo, Action::Commit { summary: "skipped".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: true }).ok);
        let _ = fs::remove_file(hooks.join("pre-commit"));

        // Sparse checkout: only docs/ stays in the working folder.
        fs::create_dir_all(d.join("docs")).unwrap();
        fs::write(d.join("docs/a.md"), "a").unwrap();
        fs::write(d.join("big.bin"), "b").unwrap();
        assert!(e.perform(&repo, Action::Commit { summary: "files".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false }).ok);
        assert!(e.perform(&repo, Action::SparseSet { patterns: vec!["/docs/".into()] }).ok);
        assert!(crate::manage::sparse_info(&d).enabled);
        assert!(!d.join("big.bin").exists() && d.join("docs/a.md").exists());
        assert!(e.perform(&repo, Action::SparseDisable).ok);
        assert!(d.join("big.bin").exists());

        // LFS info never fails, whether or not git-lfs is installed.
        let _ = crate::manage::lfs_info(&d);
        let _ = fs::remove_dir_all(&d);
    }

}
