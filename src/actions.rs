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
    Commit { summary: String, description: String, amend: bool, stage_all: bool },
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
    Push { force: bool },
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
    AddRemote { name: String, url: String },
    /// Stage, unstage or discard one hunk, or some lines of it.
    Patch { file: String, untracked: bool, op: crate::files::PatchOp, hunk: usize, lines: Option<Vec<usize>> },
    CherryPickMany { shas: Vec<String> },
    Undo,
    Redo,
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
            Action::Commit { summary, description, amend, stage_all } => {
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
            Action::Fetch => done(self.git(dir, &["fetch", "--all", "--prune"]), "Fetched all remotes"),
            Action::Pull { mode } => match mode.as_str() {
                "fetch" => done(self.git(dir, &["fetch", "--all", "--prune"]), "Fetched all remotes"),
                "ffonly" => done(self.git(dir, &["pull", "--ff-only"]), "Pulled (fast-forward only)"),
                "rebase" => done(self.git(dir, &["pull", "--rebase"]), "Pulled (rebase)"),
                _ => done(self.git(dir, &["pull", "--no-rebase"]), "Pulled"),
            },
            Action::Push { force } => {
                let branch = git::run(dir, &["symbolic-ref", "--short", "-q", "HEAD"]).stdout.trim().to_string();
                if branch.is_empty() {
                    return ActionResult { ok: false, message: "Push is not available when in a detached HEAD state".into() };
                }
                let upstream = git::run(dir, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]);
                let o = if upstream.ok {
                    if force {
                        self.git(dir, &["push", "--force-with-lease"])
                    } else {
                        self.git(dir, &["push"])
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
                    self.git(dir, &["push", "-u", &remote, &branch])
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
            Action::Rebase { onto } => done(self.git(dir, &["rebase", &onto]), &format!("Rebased onto {onto}")),
            Action::AbortMerge => done(self.git(dir, &["merge", "--abort"]), "Merge aborted"),
            Action::AbortRebase => done(self.git(dir, &["rebase", "--abort"]), "Rebase aborted"),
            Action::ContinueRebase => done(self.git(dir, &["-c", "core.editor=true", "rebase", "--continue"]), "Rebase continued"),
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
        assert!(e.perform(&repo, Action::Commit { summary: "first".into(), description: String::new(), amend: false, stage_all: false }).ok);
        let first = head(&d);

        fs::write(d.join("a.txt"), "one\ntwo\n").unwrap();
        let st = git::status(&d);
        assert_eq!(st.unstaged.len(), 1);
        assert!(e.perform(&repo, Action::Stage { paths: vec!["a.txt".into()] }).ok);
        assert_eq!(git::status(&d).staged.len(), 1);
        assert!(e.perform(&repo, Action::Commit { summary: "second".into(), description: "body".into(), amend: false, stage_all: false }).ok);
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
        assert!(e.perform(&repo, Action::Commit { summary: "base".into(), description: String::new(), amend: false, stage_all: true }).ok);
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
        assert!(e.perform(&repo, Action::Commit { summary: "next".into(), description: String::new(), amend: false, stage_all: true }).ok);
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
        let view = git::RefView { hidden: vec!["refs/heads/side".into()], solo: vec![] };
        assert_eq!(git::load_view(&d, 100, &view).commits.len(), 2);
        let solo = git::RefView { hidden: vec![], solo: vec!["refs/heads/side".into()] };
        assert_eq!(git::load_view(&d, 100, &solo).commits.len(), 1);
        let more = git::load_view(&d, 1, &git::RefView::default());
        assert!(more.has_more);
        let _ = fs::remove_dir_all(&d);
    }

}
