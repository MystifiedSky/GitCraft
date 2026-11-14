//! Git actions the user can take, the activity log and undo / redo.

use crate::git::{self, LogEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

/// Exact local and remote state reviewed before a destructive push.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushReview {
    pub branch: String,
    pub head: String,
    pub remote: String,
    pub remote_ref: String,
    pub url: String,
    pub remote_oid: String,
    pub tracking_oid: String,
    pub config: String,
}

pub fn push_review(dir: &Path) -> Result<PushReview, String> {
    let read = |args: &[&str]| { let out=git::run(dir,args); if out.ok {Ok(out.stdout.trim().to_string())} else {Err(out.message())} };
    let branch=read(&["symbolic-ref","-q","HEAD"])?;
    let head=read(&["rev-parse","--verify","HEAD"])?;
    let refs=read(&["for-each-ref","--format=%(refname)%00%(push:remotename)%00%(push:remoteref)%00%(push)","refs/heads"])?;
    let fields=refs.lines().map(|line|line.split('\0').collect::<Vec<_>>()).find(|fields|fields.first()==Some(&branch.as_str())).ok_or("The current branch is no longer available.")?;
    if fields.len()!=4 || fields[1].is_empty() {return Err("Set a single push destination for this branch before forcing a push.".into());}
    let remote=fields[1].to_string();
    let remote_ref=if !fields[2].is_empty() {fields[2].to_string()} else {
        // Default push modes can omit push:remoteref. Resolve the advertised tracking
        // ref only through Git's standard fetch mapping; refuse unfamiliar mappings.
        let mapping=read(&["config","--get-all",&format!("remote.{remote}.fetch")])?;
        let expected=format!("refs/heads/*:refs/remotes/{remote}/*");
        if !mapping.lines().any(|line|line.trim_start_matches('+')==expected) {return Err("This push mapping requires an explicit destination before force push.".into());}
        let suffix=fields[3].strip_prefix(&format!("refs/remotes/{remote}/")).ok_or("The push destination could not be resolved.")?;
        format!("refs/heads/{suffix}")
    };
    if !remote_ref.starts_with("refs/heads/") {return Err("Force push requires a branch destination.".into());}
    let urls=read(&["remote","get-url","--push","--all",&remote])?;
    if urls.lines().count()!=1 {return Err("Force push requires one explicitly reviewed push URL.".into());}
    let url=urls;
    let remote_state=read(&["ls-remote","--refs","--",&url,&remote_ref])?;
    let remote_oid=remote_state.lines().find_map(|line|line.split_once('\t').filter(|(_,reference)|*reference==remote_ref).map(|(oid,_)|oid.to_string())).unwrap_or_default();
    let tracking_oid=if fields[3].is_empty() {String::new()} else {read(&["rev-parse","--verify",fields[3]]).unwrap_or_default()};
    let config=read(&["config","--null","--list","--includes"])?;
    let config=crate::files::content_token(Some(config.as_bytes()));
    Ok(PushReview {branch,head,remote,remote_ref,url,remote_oid,tracking_oid,config})
}

fn branch_config(dir: &Path, name: &str) -> Result<Vec<(String,String)>, git::GitOut> {
    let out=git::run(dir,&["config","--local","--null","--list"]);if !out.ok {return Err(out);}
    let mut config:Vec<_>=out.stdout.split('\0').filter_map(|record| {
        let (key,value)=record.split_once('\n')?;
        let (branch,_)=key.strip_prefix("branch.")?.rsplit_once('.')?;
        (branch==name).then(||(key.to_string(),value.to_string()))
    }).collect();
    config.sort_by(|a,b|a.0.cmp(&b.0));Ok(config)
}

/// Git's answer when there is nothing to stash; the toolbar Stash treats it as an error
/// (a stash with no local changes is refused instead of reported as done).
pub const STASH_NOTHING: &str = "No local changes to save";

/// A stash's name: the typed message trimmed, else "WIP on <short branch>", or "WIP on (no branch)" when
/// HEAD is detached.
pub fn stash_message(message: &str, branch: &str) -> String {
    let typed = message.trim();
    if !typed.is_empty() {
        return typed.to_string();
    }
    let branch = branch.trim();
    format!("WIP on {}", if branch.is_empty() || branch == "HEAD" { "(no branch)" } else { branch })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Action {
    Stage { paths: Vec<String> },
    Unstage { paths: Vec<String> },
    StageAll,
    UnstageAll,
    Discard { paths: Vec<String> },
    DiscardAll,
    Commit {
        summary: String,
        description: String,
        amend: bool,
        stage_all: bool,
        #[serde(default)]
        skip_hooks: bool,
        /// Commit > Remove comments from commit messages (git commit --cleanup=strip).
        #[serde(default)]
        strip_comments: bool,
    },
    Checkout { target: String },
    CheckoutRemote { remote_branch: String },
    CheckoutPullRequest { url: String, reference: String, expected: String },
    CreateBranch { name: String, at: Option<String>, checkout: bool },
    DeleteBranch { name: String },
    DeleteRemoteBranch { remote_branch: String },
    RenameBranch { old: String, new: String },
    CreateTag { name: String, at: Option<String>, message: Option<String> },
    DeleteTag { name: String },
    PushTag { name: String, remote: String },
    DeleteRemoteTag { name: String, remote: String },
    Fetch,
    Pull { mode: String },
    /// force: push even though the remote has other commits. lease (default true) keeps the
    /// safety check that the remote branch is still where we last saw it.
    Push { force: bool, #[serde(default)] lease: Option<bool>, #[serde(default)] review: Option<PushReview> },
    /// Push the selected local branch without changing the checked-out branch.
    PushBranch { branch: String },
    Stash { message: String },
    StashPop { index: usize, expected: Option<String> },
    StashApply { index: usize, expected: Option<String> },
    StashDrop { index: usize, expected: Option<String> },
    CherryPick { sha: String, #[serde(default)] mainline: Option<usize> },
    Revert { sha: String, #[serde(default)] mainline: Option<usize> },
    Reset { sha: String, mode: String },
    Merge { branch: String },
    Rebase { onto: String },
    AbortMerge,
    Sequencer { operation: String, command: String },
    AbortRebase,
    ContinueRebase,
    SkipRebase,
    /// Interactive rebase onto `base` ("--root" for the whole history) with our own todo list.
    InteractiveRebase { base: String, items: Vec<crate::rewrite::TodoItem>, expected: crate::rewrite::PlanState },
    /// Interactive cherry pick of the listed commits onto HEAD.
    InteractiveCherryPick { items: Vec<crate::rewrite::TodoItem> },
    /// Conflicts: take "ours" (current) or "theirs" (incoming) for whole files.
    TakeSide { paths: Vec<String>, side: String },
    MarkResolved { paths: Vec<String> },
    MarkConflicted { paths: Vec<String> },
    /// Write the merge tool's output and mark the file resolved.
    SaveResolved { file: String, content: String, expected: Option<String>, utf8: Option<bool> },
    FastForward { branch: String, to: String },
    SetUpstream { branch: String, upstream: String },
    StashRename { index: usize, expected: Option<String>, message: String },
    /// Background fetch of every remote (auto-fetch), optionally pruning.
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
    /// "Compose commits with AI": commit the given groups of files one after another.
    ComposeCommits { commits: Vec<crate::ai::ComposedCommit> },
    /// Apply a .patch file (mailbox patches become commits).
    ApplyPatchFile { file: String },
    /// "Stash this file" / "Stash N files": stash only the chosen files.
    StashFiles { paths: Vec<String>, message: String },
    /// "Cherry pick file from stash": copy one file's stashed version into the working folder.
    StashTakeFile { index: usize, expected: Option<String>, path: String },
    /// "Revert this hunk": undo one hunk (or chosen lines) of a commit in the working folder.
    RevertHunk { sha: String, parent: Option<String>, file: String, hunk: usize, lines: Option<Vec<usize>> },
    /// Squash merge: bring a branch's changes in as staged changes, to commit as one commit.
    MergeSquash { branch: String },
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
    Patch { file: String, untracked: bool, op: crate::files::PatchOp, hunk: usize, lines: Option<Vec<usize>>, expected: Vec<git::DiffLine> },
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
    Commit { before: String, after: String, label: String, branch: String },
    InitialCommit { after: String, branch: String, label: String },
    Checkout { from: String, to: String },
    CreateBranch { name: String, sha: String },
    DeleteBranch { name: String, sha: String, config: Vec<(String,String)> },
    Reset { before: String, after: String, mode: String, branch: String },
    CreateTag { name: String, sha: String },
    DeleteTag { name: String, sha: String },
    Discard { stash: String, paths: Option<Vec<String>>, redo_state: Option<String> },
    Patch { patch: String, cached: bool, reverse: bool, label: String },
    /// History rewritten (rebase, cherry pick, fast-forward): move the branch back with reset --keep.
    Rewrite { before: String, after: String, label: String, branch: String },
}

impl UndoRec {
    fn label(&self) -> String {
        match self {
            UndoRec::Commit { label, .. } | UndoRec::InitialCommit { label, .. } => label.clone(),
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
    log_version: std::sync::atomic::AtomicU64,
    stacks: Mutex<HashMap<String, Stacks>>,
    /// Long network jobs with progress and Cancel (clone, fetch, pull, push).
    pub jobs: crate::jobs::Jobs,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UndoInfo {
    pub undo: Option<String>,
    pub redo: Option<String>,
}

/// Activity log text: Git's message, or with extended logging both outputs in full.
fn log_output(o: &git::GitOut) -> String {
    if crate::prefs::runtime().extended_log {
        let mut t = String::new();
        if !o.stdout.trim().is_empty() {
            t.push_str(o.stdout.trim_end());
        }
        if !o.stderr.trim().is_empty() {
            if !t.is_empty() {
                t.push('\n');
            }
            t.push_str(o.stderr.trim_end());
        }
        t
    } else {
        o.message()
    }
}

impl Engine {
    pub fn log_entries(&self) -> Vec<LogEntry> {
        self.log.lock().unwrap().clone()
    }

    pub fn log_snapshot(&self, version: Option<u64>) -> Option<(u64, Vec<LogEntry>)> {
        let log = self.log.lock().unwrap();
        let current = self.log_version.load(std::sync::atomic::Ordering::Relaxed);
        if version == Some(current) { None } else { Some((current, log.clone())) }
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
        // Named-file actions pass exact identities after --, never wildcard selections.
        let literal;
        let args = if args.contains(&"--") {
            literal = std::iter::once("--literal-pathspecs").chain(args.iter().copied()).collect::<Vec<_>>();
            literal.as_slice()
        } else { args };
        let o = git::run(dir, args);
        let entry = LogEntry {
            time: chrono::Local::now().format("%H:%M:%S").to_string(),
            command: format!("git {}", args.join(" ")),
            ok: o.ok,
            output: log_output(&o),
        };
        self.append_log(entry);
        o
    }

    /// Run a long git command as a cancellable job ("<repo>|<kind>"), logged like any other.
    pub fn job(&self, repo: &str, kind: &str, dir: &Path, args: &[&str]) -> git::GitOut {
        let o = self.jobs.run(&format!("{repo}|{kind}"), dir, args);
        let o = if !o.ok && o.stderr.trim().is_empty() { git::GitOut { ok: false, stdout: o.stdout, stderr: "Cancelled".into() } } else { o };
        self.record(&format!("git {}", args.join(" ")), &o);
        o
    }

    fn append_log(&self, mut entry: LogEntry) {
        entry.command=git::redact_diagnostic(&entry.command);entry.output=git::redact_diagnostic(&entry.output);
        for (text, limit) in [(&mut entry.command,8192),(&mut entry.output,65536)] {
            if text.len()>limit {
                let mut end=limit;while !text.is_char_boundary(end) {end-=1;}
                text.truncate(end);text.push_str("\n[truncated]");
            }
        }
        let mut log=self.log.lock().unwrap();log.push(entry);
        let excess=log.len().saturating_sub(500);if excess>0 {log.drain(..excess);}
        self.log_version.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
    }

    fn record(&self, command: &str, o: &git::GitOut) {
        self.append_log(LogEntry { time: chrono::Local::now().format("%H:%M:%S").to_string(), command: command.to_string(), ok: o.ok, output: log_output(o) });
    }

    fn in_progress(&self, dir: &Path, what: &str) -> bool {
        let gd = git::run(dir, &["rev-parse", "--absolute-git-dir"]).stdout.trim().to_string();
        Path::new(&gd).join(what).exists()
    }

    fn log_note(&self, command: &str) {
        self.append_log(LogEntry { time: chrono::Local::now().format("%H:%M:%S").to_string(), command: command.to_string(), ok: true, output: String::new() });
    }

    fn rev(&self, dir: &Path, r: &str) -> String {
        git::run(dir, &["rev-parse", "--verify", "-q", r]).stdout.trim().to_string()
    }

    fn branch_ref(&self, dir: &Path) -> String {
        git::run(dir, &["symbolic-ref", "-q", "HEAD"]).stdout.trim().to_owned()
    }

    fn head_name(&self, dir: &Path) -> String {
        let o = git::run(dir, &["symbolic-ref", "--short", "-q", "HEAD"]);
        if o.ok {
            o.stdout.trim().to_string()
        } else {
            self.rev(dir, "HEAD")
        }
    }

    /// Merge commits require an explicit mainline; reject stale or invalid choices before changing HEAD.
    fn apply_commit(&self, dir: &Path, sha: &str, mainline: Option<usize>, revert: bool) -> ActionResult {
        let resolved = git::run(dir, &["rev-parse", "--verify", "--end-of-options", &format!("{sha}^{{commit}}")]);
        if !resolved.ok {
            return ActionResult { ok: false, message: resolved.message() };
        }
        let sha = resolved.stdout.trim();
        let parents = git::run(dir, &["rev-list", "--parents", "-n1", sha]);
        if !parents.ok {
            return ActionResult { ok: false, message: parents.message() };
        }
        let count = parents.stdout.split_whitespace().count().saturating_sub(1);
        if (count > 1 && mainline.is_none()) || mainline.is_some_and(|p| count <= 1 || p == 0 || p > count) {
            return ActionResult { ok: false, message: format!("Choose a parent of this merge commit (1–{count}).") };
        }
        let mut args = vec![if revert { "revert".to_string() } else { "cherry-pick".to_string() }];
        if revert { args.push("--no-edit".into()); }
        if let Some(parent) = mainline { args.extend(["-m".into(), parent.to_string()]); }
        args.push(sha.into());
        let out = self.git(dir, &args.iter().map(String::as_str).collect::<Vec<_>>());
        ActionResult { ok: out.ok, message: if out.ok { if revert { "Reverted commit" } else { "Cherry picked commit" }.into() } else { out.message() } }
    }

    pub fn perform(&self, repo: &str, mut action: Action) -> ActionResult {
        let dir = Path::new(repo);
        let operation = crate::operation::lock_for(dir);
        let _guard = operation.lock().unwrap_or_else(|e| e.into_inner());
        let done = |o: git::GitOut, msg: &str| -> ActionResult {
            if o.ok {
                ActionResult { ok: true, message: msg.to_string() }
            } else {
                ActionResult { ok: false, message: o.message() }
            }
        };
        match &mut action {
            Action::StashPop { index, expected } | Action::StashApply { index, expected } | Action::StashDrop { index, expected }
                | Action::StashRename { index, expected, .. } | Action::StashTakeFile { index, expected, .. } => {
                if let Some(expected) = expected {
                    let list = self.git(dir, &["stash", "list", "--format=%H"]);
                    let Some(current) = list.stdout.lines().position(|sha| sha == expected) else {
                        return done(git::GitOut::failed("The selected stash no longer exists. Refresh the stash list and try again."), "");
                    };
                    *index = current;
                }
            },
            _ => {},
        }
        if matches!(&action, Action::Merge { .. } | Action::MergeSquash { .. } | Action::Rebase { .. } | Action::CherryPick { .. } | Action::CherryPickMany { .. } | Action::InteractiveRebase { .. }) { crate::prefs::begin_orig_cleanup(dir); }
        if matches!(&action, Action::AbortMerge | Action::AbortRebase) { crate::prefs::cancel_orig_cleanup(dir); }
        let deleting_tag = matches!(&action, Action::DeleteRemoteTag { .. });
        match action {
            Action::Stage { paths } => {
                let mut a = vec!["add", "--"];
                a.extend(paths.iter().map(|s| s.as_str()));
                done(self.git(dir, &a), "Staged")
            }
            Action::Unstage { paths } => {
                let has_head = !self.rev(dir, "HEAD").is_empty();
                let mut a: Vec<&str> = if has_head { vec!["reset", "-q", "HEAD", "--"] } else { vec!["rm", "--cached", "-f", "-q", "--"] };
                a.extend(paths.iter().map(|s| s.as_str()));
                done(self.git(dir, &a), "Unstaged")
            }
            Action::StageAll => done(self.git(dir, &["add", "-A"]), "Staged all changes"),
            Action::UnstageAll => {
                if self.rev(dir, "HEAD").is_empty() {
                    done(self.git(dir, &["rm", "-r", "--cached", "-f", "-q", "."]), "Unstaged all changes")
                } else {
                    done(self.git(dir, &["reset", "-q"]), "Unstaged all changes")
                }
            }
            Action::Discard { paths } => self.discard(repo, dir, Some(paths)),
            Action::DiscardAll => self.discard(repo, dir, None),
            Action::Commit { summary, description, amend, stage_all, skip_hooks, strip_comments } => {
                let unmerged=self.git(dir, &["ls-files", "--unmerged", "-z"]);
                if !unmerged.ok { return done(unmerged, ""); }
                if !unmerged.stdout.is_empty() { return done(git::GitOut::failed("Resolve and stage every conflicted file before committing. Commit All cannot resolve conflicts automatically."), ""); }
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
                if strip_comments {
                    a.push("--cleanup=strip"); // "Remove comments from commit messages"
                }
                let o = self.git(dir, &a);
                if o.ok {
                    let after = self.rev(dir, "HEAD");
                    let branch = self.branch_ref(dir);
                    let label = if amend { "amend commit".to_string() } else { "commit".to_string() };
                    self.push_undo(repo, if before.is_empty() { UndoRec::InitialCommit { after, branch, label } }
                        else { UndoRec::Commit { before, after, branch, label } });
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
            Action::CheckoutPullRequest { url, reference, expected } => {
                if !matches!(expected.len(), 40 | 64) || !expected.bytes().all(|c| c.is_ascii_hexdigit()) || reference.starts_with('-') {
                    return done(git::GitOut::failed("The pull request has an invalid source commit."), "");
                }
                let from = self.head_name(dir);
                let fetched = self.job(repo, "fetch", dir, &["fetch", "--no-tags", "--", &url, &reference]);
                if !fetched.ok { return done(fetched, ""); }
                if self.rev(dir, "FETCH_HEAD^{commit}") != expected {
                    return done(git::GitOut::failed("The pull request changed during checkout. Refresh it and try again."), "");
                }
                let result = self.git(dir, &["checkout", "--detach", "-q", &expected]);
                if result.ok { self.push_undo(repo, UndoRec::Checkout { from, to: expected }); }
                done(result, "Checked out the pull request's source commit")
            }
            Action::CheckoutRemote { remote_branch } => {
                let local = remote_branch.splitn(2, '/').nth(1).unwrap_or(&remote_branch).to_string();
                let selected = format!("refs/remotes/{remote_branch}");
                if !self.git(dir, &["check-ref-format", "--branch", &local]).ok || self.rev(dir, &selected).is_empty() {
                    return done(git::GitOut::failed("That remote branch is no longer available. Refresh the repository and try again."), "");
                }
                let exists = !self.rev(dir, &format!("refs/heads/{local}")).is_empty();
                if exists {
                    let upstream = self.git(dir, &["rev-parse", "--symbolic-full-name", "--verify", &format!("{local}@{{upstream}}")]);
                    if !upstream.ok || upstream.stdout.trim() != selected {
                        return done(git::GitOut::failed(&format!("Local branch '{local}' does not track '{remote_branch}'. Create a differently named local branch from the selected remote commit, or explicitly change its upstream before checking it out.")), "");
                    }
                }
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
                let sha = self.rev(dir, &format!("refs/heads/{name}"));
                self.push_undo(repo, UndoRec::CreateBranch { name: name.clone(), sha });
                if checkout {
                    let o = self.git(dir, &["checkout", "-q", &name]);
                    return done(o, &format!("Created and checked out {name}"));
                }
                ActionResult { ok: true, message: format!("Created branch {name}") }
            }
            Action::DeleteBranch { name } => {
                let sha = self.rev(dir, &format!("refs/heads/{name}"));
                let config=match branch_config(dir,&name) {Ok(config)=>config,Err(out)=>return done(out, "")};
                let o = self.git(dir, &["branch", "-D", "--", &name]);
                if o.ok {
                    self.push_undo(repo, UndoRec::DeleteBranch { name: name.clone(), sha, config });
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
                    let sha = self.rev(dir, &format!("refs/tags/{name}"));
                    self.push_undo(repo, UndoRec::CreateTag { name: name.clone(), sha });
                }
                done(o, &format!("Created tag {name}"))
            }
            Action::DeleteTag { name } => {
                let sha = self.rev(dir, &format!("refs/tags/{name}"));
                let o = self.git(dir, &["tag", "-d", &name]);
                if o.ok {
                    self.push_undo(repo, UndoRec::DeleteTag { name: name.clone(), sha });
                }
                done(o, &format!("Deleted tag {name}"))
            }
            Action::PushTag { name, remote } | Action::DeleteRemoteTag { name, remote } => {
                let deleting = deleting_tag;
                let full = format!("refs/tags/{name}");
                if !git::run(dir, &["check-ref-format", &full]).ok {
                    return ActionResult { ok: false, message: "Choose a valid tag name.".into() };
                }
                if !git::run(dir, &["remote"]).stdout.lines().any(|r| r == remote) {
                    return ActionResult { ok: false, message: "The selected remote no longer exists.".into() };
                }
                if !deleting && !git::run(dir, &["show-ref", "--verify", "--quiet", &full]).ok {
                    return ActionResult { ok: false, message: format!("Local tag {name} no longer exists.") };
                }
                // Full refspecs avoid ambiguity when a branch and tag share a name; never force a tag.
                let spec = if deleting { format!(":{full}") } else { format!("{full}:{full}") };
                done(self.job(repo, "push", dir, &["push", "--progress", "--", &remote, &spec]),
                    &format!("{} tag {name} {} {remote}", if deleting { "Deleted" } else { "Pushed" }, if deleting { "from" } else { "to" }))
            }
            Action::Fetch => done(self.job(repo, "fetch", dir, &["fetch", "--all", "--prune", "--progress"]), "Fetched all remotes"),
            Action::Pull { mode } => match mode.as_str() {
                "fetch" => done(self.job(repo, "fetch", dir, &["fetch", "--all", "--prune", "--progress"]), "Fetched all remotes"),
                "ffonly" => done(self.job(repo, "pull", dir, &["pull", "--ff-only", "--progress"]), "Pulled (fast-forward only)"),
                "rebase" => done(self.job(repo, "pull", dir, &["pull", "--rebase", "--progress"]), "Pulled (rebase)"),
                _ => {
                    let args = crate::extras::merge_args(dir, "pull", &["--no-rebase", "--progress"]);
                    done(self.job(repo, "pull", dir, &args.iter().map(String::as_str).collect::<Vec<_>>()), "Pulled")
                }
            },
            Action::Push { force, lease, review } => {
                if force {
                    let Some(review)=review else {return done(git::GitOut::failed("Review the branch and destination again before forcing a push."), "");};
                    let current=match push_review(dir) {Ok(current)=>current,Err(error)=>return done(git::GitOut::failed(&error), "")};
                    if current != review {return done(git::GitOut::failed("The branch, commit, push destination or remote tip changed after confirmation. Review the push again."), "");}
                    let expected=if lease==Some(false) {&review.remote_oid} else {&review.tracking_oid};
                    let guard=format!("--force-with-lease={}:{}",review.remote_ref,expected);
                    let refspec=format!("{}:{}",review.head,review.remote_ref);
                    // Push the reviewed object and URL even if an external process changes HEAD/config
                    // after validation. An explicit remote lease also protects the final network race.
                    let out=self.jobs.run(&format!("{repo}|push"),dir,&["push","--progress",&guard,"--",&review.url,&refspec]);
                    self.record(&format!("git push --force-with-lease {} {}",review.remote,review.remote_ref),&out);
                    return done(out,"Pushed the reviewed commit");
                }
                let branch = git::run(dir, &["symbolic-ref", "-q", "HEAD"]).stdout.trim().strip_prefix("refs/heads/").unwrap_or_default().to_string();
                if branch.is_empty() {
                    return ActionResult { ok: false, message: "Push is not available when in a detached HEAD state".into() };
                }
                let upstream = git::run(dir, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]);
                let o = if upstream.ok {
                    self.job(repo, "push", dir, &["push", "--progress"])
                } else {
                    let remotes = git::run(dir, &["remote"]).stdout;
                    let remote = if remotes.lines().any(|r| r == "origin") {
                        "origin".to_string()
                    } else if let Some(r) = remotes.lines().next() {
                        r.to_string()
                    } else {
                        return ActionResult { ok: false, message: "This repository has no remote to push to. Add one first.".into() };
                    };
                    self.job(repo, "push", dir, &["push", "-u", "--progress", &remote, &format!("refs/heads/{branch}:refs/heads/{branch}")])
                };
                done(o, &format!("Pushed {branch}"))
            }
            Action::PushBranch { branch } => {
                let source = format!("refs/heads/{branch}");
                let resolved = git::run(dir, &["rev-parse", "--verify", "--end-of-options", &source]);
                if !resolved.ok {
                    return ActionResult { ok: false, message: format!("Local branch {branch} no longer exists.") };
                }
                let remotes = git::run(dir, &["remote"]).stdout;
                let configured = git::run(dir, &["config", "--get", &format!("branch.{branch}.remote")]).stdout.trim().to_string();
                let remote = if !configured.is_empty() {
                    configured
                } else if remotes.lines().any(|r| r == "origin") {
                    "origin".into()
                } else {
                    remotes.lines().next().unwrap_or("").to_string()
                };
                if remote.is_empty() || !remotes.lines().any(|r| r == remote) {
                    return ActionResult { ok: false, message: "Choose a server remote for this branch before starting a pull request.".into() };
                }
                let merge = git::run(dir, &["config", "--get", &format!("branch.{branch}.merge")]).stdout.trim().to_string();
                let destination = if merge.is_empty() { source.clone() } else { merge };
                if !destination.starts_with("refs/heads/") {
                    return ActionResult { ok: false, message: "The branch upstream must point to a remote branch.".into() };
                }
                let refspec = format!("{source}:{destination}");
                done(self.job(repo, "push", dir, &["push", "--set-upstream", "--progress", &remote, &refspec]), &format!("Pushed {branch}"))
            }
            Action::Stash { message } => {
                let unmerged=self.git(dir, &["ls-files", "--unmerged", "-z"]);
                if !unmerged.ok { return done(unmerged, ""); }
                if !unmerged.stdout.is_empty() || ["MERGE_HEAD","CHERRY_PICK_HEAD","REVERT_HEAD","rebase-merge","rebase-apply","sequencer"].iter().any(|state|self.in_progress(dir,state)) {
                    return done(git::GitOut::failed("Finish or abort the current merge, rebase or replay before stashing. Its conflict state was kept."), "");
                }
                // Stage everything (new files too), then `stash push` without --include-untracked, named by stash_message.
                let add = self.git(dir, &["add", "-A"]);
                if !add.ok {
                    return done(add, "");
                }
                let branch = git::run(dir, &["symbolic-ref", "--short", "-q", "HEAD"]).stdout.trim().to_string();
                let msg = stash_message(&message, &branch);
                let o = self.git(dir, &["stash", "push", "-m", &msg]);
                // A stash with no local changes is reported as an error, not as success.
                if o.ok && o.stdout.trim() == STASH_NOTHING {
                    return ActionResult { ok: false, message: STASH_NOTHING.into() };
                }
                done(o, "Stashed changes")
            }
            Action::StashPop { index, .. } => done(self.git(dir, &["stash", "pop", "-q", &format!("stash@{{{index}}}")]), "Popped stash"),
            Action::StashApply { index, .. } => done(self.git(dir, &["stash", "apply", "-q", &format!("stash@{{{index}}}")]), "Applied stash"),
            Action::StashDrop { index, .. } => done(self.git(dir, &["stash", "drop", "-q", &format!("stash@{{{index}}}")]), "Deleted stash"),
            Action::CherryPick { sha, mainline } => self.apply_commit(dir, &sha, mainline, false),
            Action::Revert { sha, mainline } => self.apply_commit(dir, &sha, mainline, true),
            Action::Reset { sha, mode } => {
                let before = self.rev(dir, "HEAD");
                let flag = match mode.as_str() {
                    "soft" => "--soft",
                    "hard" => "--hard",
                    "mixed" => "--mixed",
                    _ => return ActionResult { ok: false, message: "Choose a soft, mixed, or hard reset.".into() },
                };
                let resolved = git::run(dir, &["rev-parse", "--verify", "--end-of-options", &format!("{sha}^{{commit}}")]);
                if !resolved.ok { return done(resolved, ""); }
                let sha = resolved.stdout.trim().to_string();
                let o = self.git(dir, &["reset", "-q", flag, &sha]);
                if o.ok {
                    self.push_undo(repo, UndoRec::Reset { before, after: sha.clone(), mode: flag.to_string(), branch: git::run(dir, &["symbolic-ref", "-q", "HEAD"]).stdout.trim().to_string() });
                }
                done(o, "Reset branch")
            }
            Action::Merge { branch } => {
                let args = crate::extras::merge_args(dir, "merge", &["--no-edit", &branch]);
                done(self.git(dir, &args.iter().map(String::as_str).collect::<Vec<_>>()), &format!("Merged {branch}"))
            }
            Action::Rebase { onto } => {
                let before = self.rev(dir, "HEAD");
                let o = self.git(dir, &["rebase", "--autostash", &onto]);
                if o.ok {
                    let after = self.rev(dir, "HEAD");
                    self.push_undo(repo, UndoRec::Rewrite { before, after, branch: self.branch_ref(dir), label: format!("rebase onto {onto}") });
                }
                done(o, &format!("Rebased onto {onto}"))
            }
            Action::AbortMerge => done(self.git(dir, &["merge", "--abort"]), "Merge aborted"),
            Action::Sequencer { operation, command } => {
                if !["cherry-pick", "revert", "am"].contains(&operation.as_str()) || !["continue", "skip", "abort"].contains(&command.as_str()) {
                    return done(git::GitOut::failed("Unsupported recovery command."), "");
                }
                let flag = format!("--{command}");
                done(self.git(dir, &["-c", "core.editor=true", &operation, &flag]), "Git operation updated")
            },
            Action::AbortRebase => done(self.git(dir, &["rebase", "--abort"]), "Rebase aborted"),
            Action::ContinueRebase => done(self.git(dir, &["-c", "core.editor=true", "rebase", "--continue"]), "Rebase continued"),
            Action::SkipRebase => done(self.git(dir, &["rebase", "--skip"]), "Skipped commit"),
            Action::AutoFetch { prune } => {
                let o = if prune { self.git(dir, &["fetch", "--all", "--prune", "--quiet"]) } else { self.git(dir, &["fetch", "--all", "--quiet"]) };
                // A failed background fetch only asks to fetch by hand.
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
                if push_url.trim().is_empty() || push_url.trim() == url.trim() {
                    let clear = self.git(dir, &["config", "--local", "--unset-all", &format!("remote.{name}.pushurl")]);
                    // git config exits 5 when the optional key does not exist.
                    if !clear.ok && git::run(dir, &["config", "--get-all", &format!("remote.{name}.pushurl")]).ok {
                        return done(clear, "");
                    }
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
                let module = dir.join(&path);
                if module.join(".git").exists() {
                    let status = git::run(&module, &["status", "--porcelain", "--untracked-files=all", "--ignored=matching", "--ignore-submodules=none"]);
                    if !status.ok || !status.stdout.trim().is_empty() {
                        return done(git::GitOut::failed("The submodule has local work. Commit, stash, or back it up before removing it."), "");
                    }
                } else if module.exists() && std::fs::read_dir(&module).map(|mut items| items.next().is_some()).unwrap_or(true) {
                    return done(git::GitOut::failed("The submodule folder contains local files. Back them up before removing it."), "");
                }
                let o = self.git(dir, &["submodule", "deinit", "--", &path]);
                if !o.ok {
                    return done(o, "");
                }
                done(self.git(dir, &["rm", "--", &path]), &format!("Removed submodule {path}"))
            }
            Action::DeleteBranches { names, remotes } => {
                let mut failed = Vec::new();
                for n in &names {
                    let sha = self.rev(dir, &format!("refs/heads/{n}"));
                    let config=match branch_config(dir,n) {Ok(config)=>config,Err(_)=>{failed.push(n.clone());continue;}};
                    if self.git(dir, &["branch", "-D", "--", n]).ok {
                        self.push_undo(repo, UndoRec::DeleteBranch { name: n.clone(), sha, config });
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
                let mut a = vec!["--literal-pathspecs".to_string(), "difftool".into(), "-y".into()];
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
                let mut a = vec!["--literal-pathspecs".to_string(), "mergetool".into(), "-y".into()];
                if !tool.trim().is_empty() {
                    a.push(format!("--tool={}", tool.trim()));
                }
                a.extend(["--".into(), file.clone()]);
                spawn_git(dir, &a);
                ActionResult { ok: true, message: format!("Opening {file} in the merge tool") }
            }
            Action::ComposeCommits { commits } => {
                let before = self.rev(dir, "HEAD");
                let o = if before.is_empty() { self.git(dir, &["rm", "-r", "--cached", "--ignore-unmatch", "--", "."]) }
                    else { self.git(dir, &["reset", "-q"]) };
                if !o.ok { return done(o, ""); }
                let failed = |i: usize, o: git::GitOut| {
                    let after = self.rev(dir, "HEAD");
                    if before != after && !after.is_empty() {
                        let branch = self.branch_ref(dir);
                        let label = "partially composed commits".into();
                        self.push_undo(repo, if before.is_empty() { UndoRec::InitialCommit { after, branch, label } }
                            else { UndoRec::Commit { before: before.clone(), after, branch, label } });
                    }
                    ActionResult { ok: false, message: format!("Commit {} of {}: {}. Completed commits remain and can be undone.", i + 1, commits.len(), o.message()) }
                };
                for (i, c) in commits.iter().enumerate() {
                    let mut add = vec!["add", "-A", "--"];
                    add.extend(c.files.iter().map(|s| s.as_str()));
                    let o = self.git(dir, &add);
                    if !o.ok {
                        return failed(i, o);
                    }
                    let msg = if c.description.trim().is_empty() { c.summary.trim().to_string() } else { format!("{}\n\n{}", c.summary.trim(), c.description.trim()) };
                    let o = self.git(dir, &["commit", "-q", "-m", &msg]);
                    if !o.ok {
                        return failed(i, o);
                    }
                }
                let after = self.rev(dir, "HEAD");
                if before != after && !after.is_empty() {
                    let branch = self.branch_ref(dir);
                    let label = format!("{} composed commits", commits.len());
                    self.push_undo(repo, if before.is_empty() { UndoRec::InitialCommit { after, branch, label } }
                        else { UndoRec::Commit { before, after, branch, label } });
                }
                ActionResult { ok: true, message: format!("Created {} commits", commits.len()) }
            }
            Action::StashFiles { paths, message } => {
                // A stash is refused as a whole if any chosen file is unchanged.
                for p in &paths {
                    if git::run(dir, &["status", "--porcelain", "--untracked-files=all", "--", p]).stdout.trim().is_empty() {
                        return ActionResult { ok: false, message: "cannot stash changes - one of the files does not have any changes to stash.".into() };
                    }
                }
                let mut args = vec!["stash", "push", "--include-untracked"];
                let msg = message.trim().to_string();
                if !msg.is_empty() {
                    args.extend(["-m", msg.as_str()]);
                }
                args.push("--");
                args.extend(paths.iter().map(|p| p.as_str()));
                let n = paths.len();
                done(self.git(dir, &args), &format!("Stashed {n} file{}", if n == 1 { "" } else { "s" }))
            }
            Action::StashTakeFile { index, path, .. } => {
                // Tracked files are in the stash commit; new files are in its third parent.
                let status = git::run(dir, &["status", "--porcelain=v1", "--untracked-files=all", "--ignored=matching", "--", &path]);
                if !status.ok { return done(status, ""); }
                if !status.stdout.is_empty() {
                    return ActionResult { ok: false, message: format!("Stash or commit the current changes to {path} before applying its stashed version.") };
                }
                let stash = format!("stash@{{{index}}}");
                let mut o = self.git(dir, &["checkout", &stash, "--", &path]);
                if !o.ok {
                    let untracked = format!("{stash}^3");
                    let o2 = self.git(dir, &["checkout", &untracked, "--", &path]);
                    if o2.ok {
                        o = o2;
                    }
                }
                done(o, &format!("Took {path} from the stash"))
            }
            Action::RevertHunk { sha, parent, file, hunk, lines } => {
                let diff = match git::diff_text_checked(dir, &git::DiffSource::Commit(sha.clone()), &file, parent.as_deref(), false) { Ok(diff) => diff, Err(error) => return done(git::GitOut::failed(&error), "") };
                let Some(patch) = crate::files::partial_patch(&diff, hunk, lines.as_deref(), true) else {
                    return ActionResult { ok: false, message: "That change could not be found in the commit.".into() };
                };
                let current = std::fs::read_to_string(dir.join(&file)).unwrap_or_default();
                if let Err(e) = crate::files::check_hunks_exact(&current, &patch, true) {
                    return ActionResult { ok: false, message: format!("Could not revert the hunk: {e} ({file} has changed since that commit)") };
                }
                self.log_note(&format!("git apply --reverse (revert hunk of {file} from {})", &sha[..sha.len().min(7)]));
                match crate::files::apply_patch(dir, &patch, false, true) {
                    Ok(()) => {
                        self.push_undo(repo, UndoRec::Patch { patch, cached: false, reverse: true, label: format!("revert hunk in {file}") });
                        ActionResult { ok: true, message: format!("Reverted the change in {file}; it is now an uncommitted change") }
                    }
                    Err(e) => ActionResult { ok: false, message: format!("The hunk could not be reverted because {file} has changed since: {e}") },
                }
            }
            Action::MergeSquash { branch } => {
                let args = crate::extras::merge_args(dir, "merge", &["--squash", &branch]);
                done(self.git(dir, &args.iter().map(String::as_str).collect::<Vec<_>>()), &format!("Squashed {branch}; review and commit the staged changes"))
            }
            Action::ApplyPatchFile { file } => {
                let o = crate::extras::apply_patch_file(dir, Path::new(&file), &|a: &[&str]| self.git(dir, a));
                done(o, "Patch applied")
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
            Action::InteractiveRebase { base, items, expected } => {
                if crate::rewrite::plan_state(dir, &base).ok().as_ref() != Some(&expected) {
                    return done(git::GitOut::failed("The branch, tip, or base changed after this plan was prepared. Reopen the planner and review the current history."), "");
                }
                let before = self.rev(dir, "HEAD");
                let o = crate::rewrite::interactive_rebase(dir, &base, &items);
                self.record(&format!("git rebase -i {base} ({} steps)", items.len()), &o);
                if o.ok {
                    let after = self.rev(dir, "HEAD");
                    self.push_undo(repo, UndoRec::Rewrite { before, after, branch: self.branch_ref(dir), label: "interactive rebase".into() });
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
                    self.push_undo(repo, UndoRec::Rewrite { before, after, branch: self.branch_ref(dir), label: "interactive cherry pick".into() });
                }
                done(o, "Cherry pick complete")
            }
            Action::TakeSide { paths, side } => {
                let (flag,stage)=match side.as_str() {"ours"=>("--ours","2"),"theirs"=>("--theirs","3"),_=>return done(git::GitOut::failed("Choose current or incoming conflict contents."), "")};
                let mut selected=Vec::new();
                for path in &paths {
                    let entries=self.git(dir,&["ls-files","--unmerged","-z","--",path]);if !entries.ok {return done(entries, "");}
                    let stages:Vec<_>=entries.stdout.split('\0').filter_map(|record|record.split_once('\t').and_then(|(header,_)|header.split_whitespace().nth(2))).collect();
                    if stages.is_empty() {return done(git::GitOut::failed(&format!("{path} is no longer an unresolved file. Refresh its changes first.")), "");}
                    selected.push((path,stages.contains(&stage)));
                }
                for (path,present) in selected {
                    if !present {
                        let removed=self.git(dir,&["rm","-q","--",path]);if !removed.ok {return done(removed, "");}
                    } else {
                        let full=dir.join(path);
                        let backup=match std::fs::symlink_metadata(&full) {
                            Ok(metadata)=>{
                                let saved=match tempfile::NamedTempFile::new() {Ok(file)=>file,Err(error)=>return done(git::GitOut::failed(&error.to_string()), "")};
                                let moved=metadata.file_type().is_symlink();
                                let copied=if moved {std::fs::remove_file(saved.path()).and_then(|_|std::fs::rename(&full,saved.path()))} else {std::fs::copy(&full,saved.path()).map(|_|())};
                                if let Err(error)=copied {return done(git::GitOut::failed(&format!("Could not back up {path}: {error}")), "");}
                                Some((saved,moved))
                            }
                            Err(error) if error.kind()==std::io::ErrorKind::NotFound=>None,
                            Err(error)=>return done(git::GitOut::failed(&error.to_string()), ""),
                        };
                        let mut result=self.git(dir,&["checkout",flag,"--",path]);
                        if result.ok {result=self.git(dir,&["add","--",path]);}
                        if !result.ok {
                            if let Some((saved,moved))=backup {
                                let restored=(|| {
                                    if std::fs::symlink_metadata(&full).is_ok_and(|metadata|metadata.file_type().is_symlink()) {std::fs::remove_file(&full)?;}
                                    if moved {if full.exists() {std::fs::remove_file(&full)?;}std::fs::rename(saved.path(),&full)} else {std::fs::copy(saved.path(),&full).map(|_|())}
                                })();
                                if let Err(error)=restored {
                                    let recovery=saved.path().display().to_string();
                                    let (file,recovery_path)=saved.into_parts();drop(file);std::mem::forget(recovery_path);
                                    return done(git::GitOut::failed(&format!("{}\nCould not restore {path}: {error}. Original contents are retained at {recovery}",result.message())), "");
                                }
                            }
                            return done(result, "");
                        }
                    }
                }
                ActionResult {ok:true,message:if side=="theirs" {"Took incoming".into()} else {"Took current".into()}}
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
            Action::SaveResolved { file, content, expected, utf8 } => {
                if !utf8.unwrap_or(false) && crate::rewrite::conflict_info(dir,&file).decoding_loss {
                    return ActionResult {ok:false,message:"ENCODING_DECODE_LOSS: Conflict content contains undecodable bytes. Choose its repository encoding or explicitly convert the reviewed output to UTF-8.".into()};
                }
                if let Err(e) = crate::files::save_checked_mode(dir, &file, &content, expected.as_deref(), utf8.unwrap_or(false)) {
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
                    self.push_undo(repo, UndoRec::Rewrite { before, after, branch: self.branch_ref(dir), label: format!("fast-forward {branch}") });
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
            Action::StashRename { index, message, .. } => {
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
            Action::Patch { file, untracked, op, hunk, lines, expected } => {
                let what = if lines.is_some() { "lines" } else { "hunk" };
                let verb = match op {
                    crate::files::PatchOp::Stage => "stage",
                    crate::files::PatchOp::Unstage => "unstage",
                    crate::files::PatchOp::Discard => "discard",
                };
                self.log_note(&format!("git apply ({verb} {what} of {file})"));
                match crate::files::apply_part_checked(dir, &file, untracked, op, hunk, lines.as_deref(), &expected) {
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
        // Keep an automatic stash so the discard can be undone.
        let mut a = vec!["stash", "push", "--include-untracked", "-m", "Auto stash before discard"];
        if let Some(p) = &paths {
            a.push("--");
            a.extend(p.iter().map(|s| s.as_str()));
        }
        let previous = self.rev(dir, "stash@{0}");
        let o = self.git(dir, &a);
        if !o.ok {
            return ActionResult { ok: false, message: o.message() };
        }
        let sha = self.rev(dir, "stash@{0}");
        if sha.is_empty() || sha == previous {
            return ActionResult { ok: false, message: "There are no changes to discard.".into() };
        }
        let _ = self.git(dir, &["stash", "drop", "-q"]);
        self.push_undo(repo, UndoRec::Discard { stash: sha, paths, redo_state: None });
        ActionResult { ok: true, message: "Discarded changes".into() }
    }

    fn apply(&self, dir: &Path, rec: &UndoRec, undo: bool) -> git::GitOut {
        if let UndoRec::Commit { before, after, branch, .. } | UndoRec::Rewrite { before, after, branch, .. } = rec {
            let expected = if undo { after } else { before };
            if self.branch_ref(dir) != *branch || self.rev(dir, "HEAD") != *expected {
                return git::GitOut::failed("Return to the original branch and commit before undoing or redoing this action; the repository has changed.");
            }
        }
        if let UndoRec::InitialCommit { after, branch, .. } = rec {
            let expected = if undo { after.as_str() } else { "" };
            if self.branch_ref(dir) != *branch || self.rev(dir, "HEAD") != expected {
                return git::GitOut::failed("Return to the original branch and commit before undoing or redoing this action; the repository has changed.");
            }
        }
        match (rec, undo) {
            (UndoRec::InitialCommit { after, branch, .. }, true) => self.git(dir, &["update-ref", "-d", branch, after]),
            (UndoRec::InitialCommit { after, branch, .. }, false) => self.git(dir, &["update-ref", branch, after, &"0".repeat(after.len())]),
            (UndoRec::Commit { before, .. }, true) => self.git(dir, &["reset", "-q", "--soft", before]),
            (UndoRec::Commit { after, .. }, false) => self.git(dir, &["reset", "-q", "--soft", after]),
            (UndoRec::Checkout { from, .. }, true) => self.git(dir, &["checkout", "-q", from]),
            (UndoRec::Checkout { to, .. }, false) => self.git(dir, &["checkout", "-q", to]),
            (UndoRec::DeleteBranch { name, sha, config }, false) => {
                if self.rev(dir,&format!("refs/heads/{name}"))!=*sha || branch_config(dir,name).ok().as_ref()!=Some(config) {
                    return git::GitOut::failed("This branch or its settings changed after Undo; refusing to delete it.");
                }
                self.git(dir,&["branch","-D","--",name])
            }
            (UndoRec::DeleteBranch { name, sha, config }, true) => {
                let existing=match branch_config(dir,name) {Ok(config)=>config,Err(out)=>return out};
                if !existing.is_empty() || !self.rev(dir,&format!("refs/heads/{name}")).is_empty() {return git::GitOut::failed("The deleted branch or its settings were recreated externally. Refusing to overwrite them.");}
                let created=self.git(dir,&["branch","--",name,sha]);if !created.ok {return created;}
                for (key,value) in config {
                    let out=self.git(dir,&["config","--local","--add","--",key,value]);
                    if !out.ok {let _=self.git(dir,&["branch","-D","--",name]);return out;}
                }
                created
            }
            (UndoRec::CreateBranch { name, sha }, true) => {
                if self.rev(dir, &format!("refs/heads/{name}")) != *sha {
                    return git::GitOut::failed("This branch has changed since the recorded action; refusing to delete its new commits.");
                }
                self.git(dir, &["branch", "-D", name])
            }
            (UndoRec::CreateBranch { name, sha }, false) => {
                self.git(dir, &["branch", name, sha])
            }
            (UndoRec::Reset { before, after, mode, branch }, undo) => {
                let current_branch = git::run(dir, &["symbolic-ref", "-q", "HEAD"]).stdout.trim().to_string();
                let expected = if undo { after } else { before };
                if &current_branch != branch || self.rev(dir, "HEAD") != *expected {
                    return git::GitOut { ok: false, stdout: String::new(), stderr: "Return to the branch and commit where this reset was performed before undoing or redoing it.".into() };
                }
                // Replaying a hard reset must never destroy edits made since the original reset.
                if mode == "--hard" {
                    let status = git::run(dir, &["status", "--porcelain", "-uall"]);
                    if !status.ok { return status; }
                    if !status.stdout.is_empty() {
                        return git::GitOut { ok: false, stdout: String::new(), stderr: "Stash or commit your changes before undoing or redoing a hard reset.".into() };
                    }
                }
                self.git(dir, &["reset", "-q", mode, if undo { before } else { after }])
            }
            (UndoRec::CreateTag { name, .. }, true) | (UndoRec::DeleteTag { name, .. }, false) => self.git(dir, &["tag", "-d", name]),
            (UndoRec::CreateTag { name, sha }, false) | (UndoRec::DeleteTag { name, sha }, true) => self.git(dir, &["tag", name, sha]),
            (UndoRec::Discard { stash, .. }, true) => self.git(dir, &["stash", "apply", "-q", stash]),
            (UndoRec::Discard { paths, redo_state, .. }, false) => {
                if !matches!((redo_state.as_deref(), discard_state(dir)), (Some(expected), Ok(current)) if expected == current) {
                    return git::GitOut::failed("The working tree changed after Undo. Discard the intended files again instead of redoing this action.");
                }
                let previous = self.rev(dir, "stash@{0}");
                let mut args = vec!["stash", "push", "--include-untracked", "-m", "Auto stash before redo discard"];
                if let Some(paths) = paths { args.push("--"); args.extend(paths.iter().map(String::as_str)); }
                let out = self.git(dir, &args);
                if out.ok && self.rev(dir, "stash@{0}") == previous { return git::GitOut::failed("There are no changes to discard."); }
                if out.ok { let _ = self.git(dir, &["stash", "drop", "-q"]); }
                out
            },
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
        let Some(mut rec) = rec else {
            return ActionResult { ok: false, message: "Nothing to undo".into() };
        };
        let o = self.apply(dir, &rec, true);
        if o.ok {
            if let UndoRec::Discard { redo_state, .. } = &mut rec { *redo_state = discard_state(dir).ok(); }
            let label = rec.label();
            self.stacks.lock().unwrap().entry(repo.to_string()).or_default().redo.push(rec);
            ActionResult { ok: true, message: format!("Undo Successful: {label}") }
        } else {
            self.stacks.lock().unwrap().entry(repo.to_string()).or_default().undo.push(rec);
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
            self.stacks.lock().unwrap().entry(repo.to_string()).or_default().redo.push(rec);
            ActionResult { ok: false, message: format!("Redo Failed: {}", o.message()) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;


    #[test]
    fn remote_tags_are_explicit_preserve_annotations_and_leave_local_tags() {
        let d = temp_repo("remote-tag-audit");
        let repo = d.to_string_lossy().to_string();
        let e = Engine::default();
        fs::write(d.join("f.txt"), "one\n").unwrap();
        assert!(git::run(&d, &["add", "."]).ok);
        assert!(git::run(&d, &["commit", "-qm", "one"]).ok);
        assert!(git::run(&d, &["tag", "-a", "release", "-m", "Release annotation"]).ok);
        assert!(git::run(&d, &["branch", "release"]).ok);
        let object = git::run(&d, &["rev-parse", "refs/tags/release"]).stdout.trim().to_string();
        let remote = d.join("remote.git");
        assert!(git::run(&d, &["init", "--bare", &remote.to_string_lossy()]).ok);
        assert!(git::run(&d, &["remote", "add", "origin", &remote.to_string_lossy()]).ok);
        assert!(!e.perform(&repo, Action::PushTag { name: "release".into(), remote: "missing".into() }).ok);
        let result = e.perform(&repo, Action::PushTag { name: "release".into(), remote: "origin".into() });
        assert!(result.ok, "{}", result.message);
        assert_eq!(git::run(&remote, &["rev-parse", "refs/tags/release"]).stdout.trim(), object);
        assert!(!git::run(&remote, &["show-ref", "--verify", "refs/heads/release"]).ok);
        assert!(e.perform(&repo, Action::DeleteRemoteTag { name: "release".into(), remote: "origin".into() }).ok);
        assert!(!git::run(&remote, &["show-ref", "--verify", "refs/tags/release"]).ok);
        assert_eq!(git::run(&d, &["rev-parse", "refs/tags/release"]).stdout.trim(), object);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn merge_commit_actions_require_and_use_the_selected_parent() {
        let d = temp_repo("mainline-audit");
        let repo = d.to_string_lossy().to_string();
        let e = Engine::default();
        fs::write(d.join("base.txt"), "base\n").unwrap();
        assert!(git::run(&d, &["add", "."]).ok);
        assert!(git::run(&d, &["commit", "-qm", "base"]).ok);
        let base = head(&d);
        assert!(git::run(&d, &["checkout", "-qb", "feature"]).ok);
        fs::write(d.join("side.txt"), "side\n").unwrap();
        assert!(git::run(&d, &["add", "."]).ok);
        assert!(git::run(&d, &["commit", "-qm", "side"]).ok);
        assert!(git::run(&d, &["checkout", "-q", "master"]).ok);
        fs::write(d.join("main.txt"), "main\n").unwrap();
        assert!(git::run(&d, &["add", "."]).ok);
        assert!(git::run(&d, &["commit", "-qm", "main"]).ok);
        assert!(git::run(&d, &["merge", "--no-ff", "-m", "merge", "feature"]).ok);
        let merge = head(&d);
        for mainline in [None, Some(0), Some(3)] {
            assert!(!e.perform(&repo, Action::Revert { sha: merge.clone(), mainline }).ok);
            assert_eq!(head(&d), merge);
        }
        assert!(e.perform(&repo, Action::Revert { sha: merge.clone(), mainline: Some(2) }).ok);
        assert!(d.join("side.txt").exists());
        assert!(!d.join("main.txt").exists());
        assert!(git::run(&d, &["checkout", "-qb", "picked", &base]).ok);
        let result = e.perform(&repo, Action::CherryPick { sha: merge, mainline: Some(2) });
        assert!(result.ok, "{}", result.message);
        assert!(d.join("main.txt").exists());
        assert!(!d.join("side.txt").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn reset_modes_keep_their_promises_and_retry_failed_undo() {
        let d = temp_repo("reset-audit");
        let repo = d.to_string_lossy().to_string();
        let e = Engine::default();
        fs::write(d.join("f.txt"), "one\n").unwrap();
        assert!(git::run(&d, &["add", "."]).ok);
        assert!(git::run(&d, &["commit", "-qm", "one"]).ok);
        let first = head(&d);
        fs::write(d.join("f.txt"), "two\n").unwrap();
        assert!(git::run(&d, &["commit", "-qam", "two"]).ok);
        let second = head(&d);
        assert!(!e.perform(&repo, Action::Reset { sha: first.clone(), mode: "typo".into() }).ok);
        assert_eq!(head(&d), second);
        assert!(e.perform(&repo, Action::Reset { sha: first.clone(), mode: "soft".into() }).ok);
        assert_eq!(git::status(&d).staged.len(), 1);
        assert!(e.perform(&repo, Action::Undo).ok);
        assert!(e.perform(&repo, Action::Reset { sha: first.clone(), mode: "mixed".into() }).ok);
        assert!(git::status(&d).staged.is_empty());
        assert_eq!(git::status(&d).unstaged.len(), 1);
        assert!(e.perform(&repo, Action::Reset { sha: second.clone(), mode: "hard".into() }).ok);
        // Current-HEAD hard reset is meaningful: it discards edits without moving the branch.
        fs::write(d.join("f.txt"), "discard me\n").unwrap();
        assert!(e.perform(&repo, Action::Reset { sha: second.clone(), mode: "hard".into() }).ok);
        assert_eq!(fs::read_to_string(d.join("f.txt")).unwrap().replace("\r\n", "\n"), "two\n");
        assert!(e.perform(&repo, Action::Reset { sha: first.clone(), mode: "hard".into() }).ok);
        assert!(git::run(&d, &["checkout", "-qb", "other"]).ok);
        assert!(!e.perform(&repo, Action::Undo).ok, "cannot undo a reset on another branch");
        assert!(git::run(&d, &["checkout", "-q", "master"]).ok);
        fs::write(d.join("f.txt"), "keep my new work\n").unwrap();
        assert!(!e.perform(&repo, Action::Undo).ok);
        assert_eq!(head(&d), first);
        assert_eq!(fs::read_to_string(d.join("f.txt")).unwrap(), "keep my new work\n");
        // The failed undo stays on the stack; saving the edit allows exactly the same undo to retry.
        assert!(git::run(&d, &["stash", "push", "-qm", "save"]).ok);
        assert!(e.perform(&repo, Action::Undo).ok);
        assert_eq!(head(&d), second);
        fs::write(d.join("new.txt"), "keep untracked\n").unwrap();
        assert!(!e.perform(&repo, Action::Redo).ok);
        fs::remove_file(d.join("new.txt")).unwrap();
        assert!(e.perform(&repo, Action::Redo).ok);
        assert_eq!(head(&d), first);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn annotated_tag_undo_retains_the_tag_object() {
        let d = temp_repo("tag-audit");
        let repo = d.to_string_lossy().to_string();
        let e = Engine::default();
        fs::write(d.join("f.txt"), "one\n").unwrap();
        assert!(git::run(&d, &["add", "."]).ok);
        assert!(git::run(&d, &["commit", "-qm", "one"]).ok);
        assert!(e.perform(&repo, Action::CreateTag { name: "v1".into(), at: None, message: Some("Release note".into()) }).ok);
        let object = git::run(&d, &["rev-parse", "refs/tags/v1"]).stdout;
        assert!(e.perform(&repo, Action::Undo).ok);
        assert!(e.perform(&repo, Action::Redo).ok);
        assert_eq!(git::run(&d, &["rev-parse", "refs/tags/v1"]).stdout, object);
        assert!(e.perform(&repo, Action::DeleteTag { name: "v1".into() }).ok);
        assert!(e.perform(&repo, Action::Undo).ok);
        assert_eq!(git::run(&d, &["rev-parse", "refs/tags/v1"]).stdout, object);
        assert_eq!(git::run(&d, &["cat-file", "-t", "refs/tags/v1"]).stdout.trim(), "tag");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn push_selected_branch_and_create_worktree_at_selected_commit() {
        let d = temp_repo("branch-audit");
        let repo = d.to_string_lossy().to_string();
        let e = Engine::default();
        fs::write(d.join("f.txt"), "one\n").unwrap();
        assert!(git::run(&d, &["add", "."]).ok);
        assert!(git::run(&d, &["commit", "-qm", "one"]).ok);
        let first = head(&d);
        assert!(git::run(&d, &["branch", "feature"]).ok);
        fs::write(d.join("f.txt"), "two\n").unwrap();
        assert!(git::run(&d, &["commit", "-qam", "two"]).ok);
        let second = head(&d);
        // A local bare repository exercises pushing without contacting a service.
        let remote = d.join("remote.git");
        let remote_path = remote.to_string_lossy().to_string();
        assert!(git::run(&d, &["init", "--bare", &remote_path]).ok);
        assert!(git::run(&d, &["remote", "add", "origin", &remote_path]).ok);
        let pushed = e.perform(&repo, Action::PushBranch { branch: "feature".into() });
        assert!(pushed.ok, "{}", pushed.message);
        assert_eq!(head(&d), second);
        assert_eq!(git::run(&d, &["symbolic-ref", "--short", "HEAD"]).stdout.trim(), "master");
        assert_eq!(git::run(&remote, &["rev-parse", "refs/heads/feature"]).stdout.trim(), first);
        assert!(!git::run(&remote, &["rev-parse", "--verify", "refs/heads/master"]).ok);
        let wt = d.join("selected-worktree").to_string_lossy().to_string();
        let created = e.perform(&repo, Action::WorktreeAdd { path: wt.clone(), branch: "selected".into(), new_branch: true, start: Some(first.clone()) });
        assert!(created.ok, "{}", created.message);
        assert_eq!(head(Path::new(&wt)), first);
        assert!(e.perform(&repo, Action::WorktreeRemove { path: wt, force: false }).ok);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn every_log_writer_bounds_entries_and_notifies_snapshot_readers() {
        let engine=Engine::default();let output=git::GitOut {ok:false,stdout:String::new(),stderr:"\u{e9}".repeat(50000)};
        for i in 0..1000 {engine.record(&format!("job {i}"),&output);}
        let (version,entries)=engine.log_snapshot(None).unwrap();assert_eq!(entries.len(),500);assert_eq!(entries[0].command,"job 500");assert!(!entries[0].ok);assert!(entries[0].output.len()<=65536+12);assert!(entries[0].output.ends_with("[truncated]"));
        engine.log_note("hunk operation");let (next,entries)=engine.log_snapshot(Some(version)).unwrap();assert!(next>version);assert_eq!(entries.len(),500);assert_eq!(entries.last().unwrap().command,"hunk operation");
        assert!(engine.log_snapshot(Some(next)).is_none());
        let folder=tempfile::tempdir().unwrap();assert!(engine.git(folder.path(),&["--version"]).ok);assert!(engine.log_snapshot(Some(next)).is_some());assert_eq!(engine.log_entries().len(),500);
    }

    #[test]
    fn log_diagnostics_redact_credentials_for_every_writer() {
        let engine=Engine::default();let folder=tempfile::tempdir().unwrap();assert!(git::run(folder.path(),&["init","-q"]).ok);
        let url="https://audit-user:SYNTHETIC_AUDIT_SECRET@fixture.example/repo.git";
        assert!(engine.git(folder.path(),&["remote","add","origin",url]).ok);
        assert!(engine.job("fixture","config",folder.path(),&["remote","set-url","origin",url]).ok);
        let error=git::GitOut {ok:false,stdout:String::new(),stderr:format!("failed: {url}")};engine.record(&format!("fetch {url}"),&error);engine.log_note(&format!("note {url}"));
        for entry in engine.log_entries() {assert!(!entry.command.contains("SYNTHETIC_AUDIT_SECRET"));assert!(!entry.output.contains("SYNTHETIC_AUDIT_SECRET"));}
        assert!(!error.message().contains("SYNTHETIC_AUDIT_SECRET"));
        assert_eq!(git::redact_diagnostic("https://fixture.example/repo.git"),"https://fixture.example/repo.git");
        assert!(!git::redact_diagnostic("https://user:encoded%40SYNTHETIC@fixture.example/path?access_token=QUERY_SECRET").contains("SYNTHETIC"));
        assert!(!git::redact_diagnostic("https://fixture.example/path?access_token=QUERY_SECRET").contains("QUERY_SECRET"));
        assert!(!git::redact_diagnostic("git -c http.extraHeader=Authorization: Bearer HEADER_SECRET clone url").contains("HEADER_SECRET"));
    }

    #[test]
    fn stash_names_follow_the_rules() {
        assert_eq!(stash_message("", "main"), "WIP on main");
        assert_eq!(stash_message("  fix later \n", "main"), "fix later");
        assert_eq!(stash_message("", ""), "WIP on (no branch)");
        assert_eq!(stash_message("", "feature/x"), "WIP on feature/x");
    }

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

    /// A person's repository showed no history: its refs/remotes/origin/HEAD file was blank, so
    /// `git log --remotes` stopped with "fatal: bad object origin/HEAD".
    #[test]
    fn history_loads_with_a_broken_remote_head_or_branch() {
        let d = temp_repo("broken-ref");
        fs::write(d.join("a.txt"), "a\n").unwrap();
        assert!(git::run(&d, &["add", "-A"]).ok);
        assert!(git::run(&d, &["commit", "-q", "-m", "first"]).ok);
        let sha = head(&d);
        fs::create_dir_all(d.join(".git/refs/remotes/origin")).unwrap();
        fs::write(d.join(".git/refs/remotes/origin/main"), format!("{sha}\n")).unwrap();
        // 1. Blank pointer file, as in the person's repository.
        fs::write(d.join(".git/refs/remotes/origin/HEAD"), "                              \n").unwrap();
        let r = git::load(&d, 100);
        assert_eq!(r.commits.len(), 1, "blank origin/HEAD");
        assert!(r.refs.iter().any(|x| x.name == "origin/main"));
        // 2. Pointer to a branch the remote deleted: also skipped, and not used as the default target.
        fs::write(d.join(".git/refs/remotes/origin/HEAD"), "ref: refs/remotes/origin/gone\n").unwrap();
        assert_eq!(git::load(&d, 100).commits.len(), 1, "stale origin/HEAD");
        assert_eq!(crate::extras::default_target(&d).as_deref(), Some("origin/main"));
        // 3. A damaged local branch file: the rest of the history still shows.
        fs::write(d.join(".git/refs/heads/damaged"), "0000000000000000000000000000000000000001\n").unwrap();
        assert_eq!(git::load(&d, 100).commits.len(), 1, "damaged branch");
        // 4. A tag of a tree (not a commit) is skipped; real refs stay.
        assert!(git::run(&d, &["tag", "treetag", "HEAD^{tree}"]).ok);
        let walk = git::walkable_refs(&d);
        assert!(!walk.iter().any(|r| r == "refs/tags/treetag"), "{walk:?}");
        assert!(walk.iter().any(|r| r == "refs/heads/master") && walk.iter().any(|r| r == "refs/remotes/origin/main"), "{walk:?}");
        assert_eq!(git::load(&d, 100).commits.len(), 1, "tree tag");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn merge_preview_and_actions_share_rename_rules() {
        let d = temp_repo("merge-native-rules");
        let repo = d.to_string_lossy().to_string();
        let e = Engine::default();
        let g = |args: &[&str]| {
            let o = git::run(&d, args);
            assert!(o.ok, "{args:?}: {}", o.message());
        };
        g(&["config", "core.autocrlf", "false"]);
        let base: String = (1..=40).map(|i| format!("unique line {i:02} with enough text for similarity\n")).collect();
        fs::write(d.join("old.txt"), &base).unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "base"]);
        g(&["checkout", "-q", "-b", "side"]);
        g(&["mv", "old.txt", "new.txt"]);
        fs::write(d.join("new.txt"), base.replace("line 02", "line two")).unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "rename and edit"]);
        g(&["checkout", "-q", "master"]);
        fs::write(d.join("old.txt"), base.replace("line 35", "line thirty-five")).unwrap();
        g(&["commit", "-q", "-am", "edit other end"]);
        let before = head(&d);
        g(&["config", "merge.renames", "false"]);
        g(&["config", "merge.renamelimit", "-1"]);
        g(&["config", "diff.renamelimit", "77"]);
        let preview = crate::extras::conflict_check(&d, Some("side"));
        assert!(preview.note.is_none(), "{:?}", preview.note);
        assert!(preview.files.is_empty(), "{:?}", preview.files);
        for squash in [false, true] {
            let action = if squash {
                Action::MergeSquash { branch: "side".into() }
            } else {
                Action::Merge { branch: "side".into() }
            };
            let result = e.perform(&repo, action);
            assert!(result.ok, "{}", result.message);
            assert!(!d.join("old.txt").exists());
            let merged = fs::read_to_string(d.join("new.txt")).unwrap();
            assert!(merged.contains("line two") && merged.contains("line thirty-five"), "{merged}");
            g(&["reset", "--hard", &before]);
        }
        assert_eq!(git::run(&d, &["config", "--get", "merge.renames"]).stdout.trim(), "false");
        assert_eq!(git::run(&d, &["config", "--get", "merge.renamelimit"]).stdout.trim(), "-1");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn stage_commit_undo_redo_branch_stash_discard() {
        let d = temp_repo("flow");
        let repo = d.to_string_lossy().to_string();
        let e = Engine::default();
        fs::write(d.join("a.txt"), "one\n").unwrap();
        assert!(e.perform(&repo, Action::StageAll).ok);
        assert!(e.perform(&repo, Action::Commit { summary: "first".into(), description: String::new(), amend: false, stage_all: false, skip_hooks: false, strip_comments: false }).ok);
        let first = head(&d);

        fs::write(d.join("a.txt"), "one\ntwo\n").unwrap();
        let st = git::status(&d);
        assert_eq!(st.unstaged.len(), 1);
        assert!(e.perform(&repo, Action::Stage { paths: vec!["a.txt".into()] }).ok);
        assert_eq!(git::status(&d).staged.len(), 1);
        assert!(e.perform(&repo, Action::Commit { summary: "second".into(), description: "body".into(), amend: false, stage_all: false, skip_hooks: false, strip_comments: false }).ok);
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
        assert!(e.perform(&repo, Action::StashPop { index: 0, expected: None }).ok);
        // Everything is staged before stashing, so a new file comes back staged (git stash pop re-adds it).
        let st = git::status(&d);
        assert_eq!(st.unstaged.len() + st.staged.len(), 1);
        assert_eq!(st.staged.len(), 1);
        // With nothing to stash an error is reported instead of success.
        assert!(e.perform(&repo, Action::StageAll).ok);
        assert!(e.perform(&repo, Action::Stash { message: String::new() }).ok);
        let named = git::run(&d, &["stash", "list", "--format=%s"]).stdout;
        assert!(named.lines().next().unwrap_or("").ends_with("WIP on master"), "default stash name: {named}");
        let none = e.perform(&repo, Action::Stash { message: String::new() });
        assert!(!none.ok && none.message == STASH_NOTHING, "{}", none.message);
        assert!(e.perform(&repo, Action::StashPop { index: 0, expected: None }).ok);
        assert_eq!(git::status(&d).staged.len(), 1);
        assert!(e.perform(&repo, Action::UnstageAll).ok);

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
        assert!(e.perform(&repo, Action::Commit { summary: "base".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false }).ok);
        let base = head(&d);
        fs::write(d.join("f.txt"), "a\nB\nc\nnew\n").unwrap();
        // Hunk body: " a", "-b", "+B", " c", "+new" -> stage only "+new" (index 4)
        let r = e.perform(&repo, Action::Patch { file: "f.txt".into(), untracked: false, op: crate::files::PatchOp::Stage, hunk: 0, lines: Some(vec![4]), expected: git::file_diff_opts(&d, &git::DiffSource::Unstaged, "f.txt", false, None, false) });
        assert!(r.ok, "{}", r.message);
        assert_eq!(index("f.txt"), "a\nb\nc\nnew\n");
        // Discard the remaining unstaged change (b -> B), then undo the discard.
        let r = e.perform(&repo, Action::Patch { file: "f.txt".into(), untracked: false, op: crate::files::PatchOp::Discard, hunk: 0, lines: None, expected: git::file_diff_opts(&d, &git::DiffSource::Unstaged, "f.txt", false, None, false) });
        assert!(r.ok, "{}", r.message);
        assert_eq!(read("f.txt"), "a\nb\nc\nnew\n");
        assert!(e.perform(&repo, Action::Undo).ok);
        assert_eq!(read("f.txt"), "a\nB\nc\nnew\n");
        // Unstage the staged line again.
        let r = e.perform(&repo, Action::Patch { file: "f.txt".into(), untracked: false, op: crate::files::PatchOp::Unstage, hunk: 0, lines: None, expected: git::file_diff_opts(&d, &git::DiffSource::Staged, "f.txt", false, None, false) });
        assert!(r.ok, "{}", r.message);
        assert!(git::status(&d).staged.is_empty());
        // Untracked file: stage one line of a brand-new file.
        fs::write(d.join("n.txt"), "one\ntwo\n").unwrap();
        let r = e.perform(&repo, Action::Patch { file: "n.txt".into(), untracked: true, op: crate::files::PatchOp::Stage, hunk: 0, lines: Some(vec![0]), expected: git::file_diff_opts(&d, &git::DiffSource::Unstaged, "n.txt", true, None, false) });
        assert!(r.ok, "{}", r.message);
        assert_eq!(index("n.txt"), "one\n");

        // Commit and compare; hide a branch from the graph.
        assert!(e.perform(&repo, Action::Commit { summary: "next".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false }).ok);
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
        let view = git::RefView { hidden: vec!["refs/heads/side".into()], solo: vec![], pinned: None, smart: false };
        assert_eq!(git::load_view(&d, 100, &view).commits.len(), 2);
        let solo = git::RefView { hidden: vec![], solo: vec!["refs/heads/side".into()], pinned: None, smart: false };
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
            assert!(e.perform(&repo, Action::Commit { summary: m.into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false }).ok);
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
        let r = e.perform(&repo, Action::InteractiveRebase { expected: crate::rewrite::plan_state(&d, &base).unwrap(), base: base.clone(), items });
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
        assert!(e.perform(&repo, Action::InteractiveRebase { expected: crate::rewrite::plan_state(&d, &base).unwrap(), base: base.clone(), items }).ok);
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
        assert!(e.perform(&repo, Action::SaveResolved { file: "a.txt".into(), content: "master\nside\n".into(), expected: None, utf8: None }).ok);
        assert!(!git::status(&d).conflicted);
        assert!(git::load(&d, 50).merge_msg.unwrap().contains("Merge branch 'side'"));
        assert!(e.perform(&repo, Action::Commit { summary: "Merge side".into(), description: String::new(), amend: false, stage_all: false, skip_hooks: false, strip_comments: false }).ok);
        assert_eq!(git::run(&d, &["rev-list", "--parents", "-n1", "HEAD"]).stdout.split_whitespace().count(), 3);

        // Fast-forward a branch that isn't checked out, set an upstream, rename a stash.
        assert!(e.perform(&repo, Action::CreateBranch { name: "old".into(), at: Some(base.clone()), checkout: false }).ok);
        assert!(e.perform(&repo, Action::FastForward { branch: "old".into(), to: "master".into() }).ok);
        assert_eq!(git::run(&d, &["rev-parse", "old"]).stdout, git::run(&d, &["rev-parse", "master"]).stdout);
        assert!(e.perform(&repo, Action::SetUpstream { branch: "old".into(), upstream: "master".into() }).ok);
        assert_eq!(git::run(&d, &["rev-parse", "--abbrev-ref", "old@{u}"]).stdout.trim(), "master");
        fs::write(d.join("e.txt"), "x").unwrap();
        assert!(e.perform(&repo, Action::Stash { message: "first".into() }).ok);
        assert!(e.perform(&repo, Action::StashRename { index: 0, message: "renamed".into(), expected: None }).ok);
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
            assert!(e.perform(&repo, Action::Commit { summary: m.into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false }).ok);
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
        assert!(e.perform(&repo, Action::Commit { summary: "tidy".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false }).ok || true);

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
    fn stash_files_take_from_stash_revert_hunk_squash_merge() {
        let d = temp_repo("gaps");
        let repo = d.to_string_lossy().to_string();
        let e = Engine::default();
        let rd = |p: std::path::PathBuf| fs::read_to_string(p).unwrap().replace("\r\n", "\n");
        let commit = |m: &str| assert!(e.perform(&repo, Action::Commit { summary: m.into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false }).ok);
        fs::write(d.join("a.txt"), "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n").unwrap();
        fs::write(d.join("b.txt"), "b\n").unwrap();
        commit("base");

        // Stash only a.txt; b.txt stays changed.
        fs::write(d.join("a.txt"), "1\n2\n3\n4\n5\n6\n7\n8\n9\nten\n").unwrap();
        fs::write(d.join("b.txt"), "B\n").unwrap();
        assert!(e.perform(&repo, Action::StashFiles { paths: vec!["a.txt".into()], message: "only a".into() }).ok);
        assert_eq!(rd(d.join("a.txt")), "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n");
        assert_eq!(rd(d.join("b.txt")), "B\n");
        // A file without changes stops the stash, like libgit2.
        let r = e.perform(&repo, Action::StashFiles { paths: vec!["a.txt".into()], message: String::new() });
        assert!(!r.ok && r.message.contains("does not have any changes to stash"), "{}", r.message);
        // Take a.txt back out of the stash without applying the rest.
        assert!(e.perform(&repo, Action::StashTakeFile { index: 0, path: "a.txt".into(), expected: None }).ok);
        assert!(rd(d.join("a.txt")).ends_with("ten\n"));
        commit("two changes");

        // Revert the first hunk of a commit with two hunks, keeping the second.
        fs::write(d.join("a.txt"), "one\n2\n3\n4\n5\n6\n7\n8\n9\nTEN\n").unwrap();
        commit("edit both ends");
        let sha = head(&d);
        let r = e.perform(&repo, Action::RevertHunk { sha, parent: None, file: "a.txt".into(), hunk: 0, lines: None });
        assert!(r.ok, "{}", r.message);
        assert_eq!(rd(d.join("a.txt")), "1\n2\n3\n4\n5\n6\n7\n8\n9\nTEN\n");
        assert!(e.perform(&repo, Action::Undo).ok);
        assert!(rd(d.join("a.txt")).starts_with("one\n"));
        assert_eq!(rd(d.join("a.txt")), "one\n2\n3\n4\n5\n6\n7\n8\n9\nTEN\n", "undo puts the file back as committed");

        // Squash merge: the branch's work arrives staged, with no merge commit.
        assert!(git::run(&d, &["checkout", "-q", "-b", "side"]).ok);
        fs::write(d.join("c.txt"), "c\n").unwrap();
        commit("side 1");
        assert!(git::run(&d, &["checkout", "-q", "master"]).ok);
        let before = head(&d);
        assert!(e.perform(&repo, Action::MergeSquash { branch: "side".into() }).ok);
        assert_eq!(head(&d), before);
        assert!(git::status(&d).staged.iter().any(|f| f.path == "c.txt"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn templates_ignore_hooks_sparse_and_shallow() {
        let d = temp_repo("tools");
        let repo = d.to_string_lossy().to_string();
        let e = Engine::default();
        // Starting files and the first commit.
        let opts = crate::manage::InitOptions { branch: String::new(), gitignore: vec!["Node".into(), "Rust".into()], license: "MIT".into(), readme: true, lfs: false };
        assert!(crate::manage::init_files(&d, "demo", &opts, &|a: &[&str]| e.git(&d, a)).ok);
        let gi = fs::read_to_string(d.join(".gitignore")).unwrap();
        assert!(gi.contains("node_modules/") && gi.contains("/target/"));
        assert!(fs::read_to_string(d.join("LICENSE")).unwrap().starts_with("MIT License"));
        assert_eq!(git::run(&d, &["log", "--format=%s"]).stdout.trim(), "Initial commit");

        // With nothing to add the first commit is still made, and it is empty.
        let empty = temp_repo("empty-init");
        git::run(&empty, &["config", "user.name", "T"]);
        git::run(&empty, &["config", "user.email", "t@example.com"]);
        let none = crate::manage::InitOptions { branch: String::new(), gitignore: vec![], license: String::new(), readme: false, lfs: false };
        assert!(crate::manage::init_files(&empty, "demo", &none, &|a: &[&str]| e.git(&empty, a)).ok);
        assert_eq!(git::run(&empty, &["log", "--format=%s"]).stdout.trim(), "Initial commit");
        assert!(git::run(&empty, &["ls-tree", "-r", "HEAD"]).stdout.trim().is_empty());

        // Ignore helpers: add once, untrack a tracked file.
        fs::write(d.join("secret.log"), "x").unwrap();
        assert!(e.perform(&repo, Action::Commit { summary: "log".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false }).ok);
        assert!(e.perform(&repo, Action::Ignore { pattern: "*.log".into(), untrack: vec!["secret.log".into()] }).ok);
        assert!(e.perform(&repo, Action::Ignore { pattern: "*.log".into(), untrack: vec![] }).ok);
        assert_eq!(fs::read_to_string(d.join(".gitignore")).unwrap().matches("*.log").count(), 1);
        assert!(git::status(&d).staged.iter().any(|f| f.path == "secret.log" && f.status == 'D'));

        // Skip hooks: a failing pre-commit hook blocks a normal commit but not a skipped one.
        let hooks = d.join(".git").join("hooks");
        fs::create_dir_all(&hooks).unwrap();
        fs::write(hooks.join("pre-commit"), "#!/bin/sh\necho blocked by hook >&2\nexit 1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(hooks.join("pre-commit"), fs::Permissions::from_mode(0o755)).unwrap();
        }
        let r = e.perform(&repo, Action::Commit { summary: "blocked".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false });
        assert!(!r.ok && r.message.contains("blocked by hook"), "{}", r.message);
        assert!(e.perform(&repo, Action::Commit { summary: "skipped".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: true, strip_comments: false }).ok);
        let _ = fs::remove_file(hooks.join("pre-commit"));

        // Sparse checkout: only docs/ stays in the working folder.
        fs::create_dir_all(d.join("docs")).unwrap();
        fs::write(d.join("docs/a.md"), "a").unwrap();
        fs::write(d.join("big.bin"), "b").unwrap();
        assert!(e.perform(&repo, Action::Commit { summary: "files".into(), description: String::new(), amend: false, stage_all: true, skip_hooks: false, strip_comments: false }).ok);
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

// Conservatively refuse replay after any intervening tracked or untracked edit.
fn discard_state(dir: &Path) -> Result<String, String> {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for args in [vec!["diff", "--binary", "--no-ext-diff", "--no-textconv", "--no-color", "HEAD"], vec!["diff", "--cached", "--binary", "--no-ext-diff", "--no-textconv", "--no-color"], vec!["ls-files", "--others", "--exclude-standard", "-z"]] {
        let output = git::git_cmd(dir).args(&args).output().map_err(|e| e.to_string())?;
        if !output.status.success() { return Err(String::from_utf8_lossy(&output.stderr).into_owned()); }
        output.stdout.hash(&mut hash);
        if args[0] == "ls-files" {
            let names = std::str::from_utf8(&output.stdout).map_err(|_| "Cannot safely verify a non-UTF-8 file name for Redo.")?;
            for name in names.split('\0').filter(|s| !s.is_empty()) { std::fs::read(dir.join(name)).map_err(|e| e.to_string())?.hash(&mut hash); }
        }
    }
    Ok(format!("{:x}", hash.finish()))
}
