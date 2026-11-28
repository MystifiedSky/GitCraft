//! What the menus and buttons ask for: prompts for branch and tag names, ref visibility,
//! bisect, history filters, remotes, worktrees, submodules, Gitflow, patches and links.

use super::App;
use super::dialogs::{Body, Dialog, Field, FieldKind, Values};
use super::repo::{HistoryMode, HistoryTarget, Selection};
use super::widgets::Kind;
use gitcraft_core::actions::Action;
use gitcraft_core::git::{HistoryFilter, RefKind};
use std::collections::HashMap;
use std::path::Path;

fn s(v: &Values, key: &str) -> String {
    v.text(key).trim().to_owned()
}

impl App {
    pub(crate) fn confirm_act(&mut self, tab: u64, title: &str, text: &str, confirm: &str, action: Action) {
        self.dialog = Some(Dialog::confirm_with(title, text, confirm, true, move |app, _| app.act(tab, action)));
    }

    pub(crate) fn copy(&mut self, text: &str) {
        self.ctx.copy_text(text.to_owned());
        self.toast(true, "Copied to clipboard");
    }

    /// A failure also goes to the bell's list.
    pub(crate) fn notify(&mut self, text: &str) {
        self.notices.push(super::Notice { text: text.to_owned(), at: chrono::Local::now().timestamp(), read: false });
        if self.notices.len() > 100 {
            self.notices.remove(0);
        }
        if self.settings.flag("desktopNotifications", false) && !self.ctx.input(|i| i.focused) {
            super::platform::desktop_notification("GitCraft", text);
        }
    }

    /// A name for a branch or tag, checked by Git as it is typed.
    fn ref_prompt(&mut self, tab: u64, title: &str, label: &str, value: &str, confirm: &str, kind: &'static str, extra: Option<&str>, then: impl FnOnce(&mut App, &Values) + 'static) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        let mut fields = vec![Field::new("value", label, FieldKind::Text)];
        if let Some(extra) = extra {
            fields.push(Field::new("extra", extra, FieldKind::Text));
        }
        let cache: std::cell::RefCell<HashMap<String, Option<String>>> = Default::default();
        let d = Dialog::new(title, Body::Form { intro: String::new(), fields, grid: false })
            .text_value("value", value)
            .checked(move |v| {
                let name = v.text("value");
                if name.trim().is_empty() {
                    return Some(String::new());
                }
                let mut c = cache.borrow_mut();
                let result = c.entry(name.clone()).or_insert_with(|| gitcraft_core::actions::validate_ref_name(Path::new(&path), kind, &name).err());
                result.clone()
            })
            .choice(confirm, Kind::Primary, then);
        self.dialog = Some(d);
    }

    pub(crate) fn prompt_branch(&mut self, tab: u64, at: Option<String>, checkout: bool) {
        let title = if at.is_some() { "Create branch here" } else { "Create branch" };
        self.ref_prompt(tab, title, "Branch name", "", "Create Branch", "branch", None, move |app, v| app.act(tab, Action::CreateBranch { name: s(v, "value"), at, checkout }));
    }

    pub(crate) fn prompt_tag(&mut self, tab: u64, at: Option<String>, annotated: bool, name: String) {
        let title = if annotated { "Create annotated tag here" } else { "Create tag here" };
        let extra = if annotated { "Annotation message" } else { "Annotation message (optional, makes an annotated tag)" };
        self.ref_prompt(tab, title, "Tag name", &name, "Create Tag", "tag", Some(extra), move |app, v| {
            let msg = v.text("extra");
            if annotated && msg.trim().is_empty() {
                app.toast(false, "Enter an annotation message to create an annotated tag.");
                return app.prompt_tag(tab, at, true, s(v, "value"));
            }
            app.act(tab, Action::CreateTag { name: s(v, "value"), at, message: Some(msg).filter(|m| !m.is_empty()) });
        });
    }

    pub(crate) fn prompt_rename_branch(&mut self, tab: u64, old: &str) {
        let old = old.to_owned();
        self.ref_prompt(tab, &format!("Rename {old}"), "New branch name", &old.clone(), "Rename", "branch", None, move |app, v| app.act(tab, Action::RenameBranch { old, new: s(v, "value") }));
    }

    pub(crate) fn prompt_stash(&mut self, tab: u64, mode: &'static str) {
        let title = match mode {
            "keep" => "Stash changes and keep staged",
            "untracked" => "Stash untracked files only",
            _ => "Stash changes",
        };
        let d = Dialog::prompt(title, "Stash title (optional)", "", "Stash", move |app, v| {
            let message = v.text("value").trim().to_owned();
            let action = match mode {
                "keep" => Action::StashKeepIndex { message },
                "untracked" => Action::StashUntracked { message },
                _ => Action::Stash { message },
            };
            app.act(tab, action);
        });
        let mut d = d;
        d.check = None;
        self.dialog = Some(d);
    }

    /// "Set Upstream" for the checked-out branch, when a pull found none.
    pub(crate) fn prompt_upstream(&mut self, tab: u64) {
        let Some(v) = self.repo(tab).and_then(|r| r.view.as_ref()) else { return };
        let Some(head) = v.data.head_branch.clone() else { return };
        if !v.data.refs.iter().any(|r| r.kind == RefKind::Remote) {
            return self.toast(false, "Set an upstream branch before pulling.");
        }
        self.dialog = Some(Dialog::confirm_with("Set an upstream branch?", "Choose the remote branch this local branch should pull from.", "Choose upstream", false, move |app, _| app.prompt_upstream_for(tab, &head)));
    }

    pub(crate) fn prompt_upstream_for(&mut self, tab: u64, branch: &str) {
        let Some(v) = self.repo(tab).and_then(|r| r.view.as_ref()) else { return };
        let Some(r) = v.data.refs.iter().find(|r| r.kind == RefKind::Local && r.name == branch) else { return };
        let guess = if !r.upstream.is_empty() {
            r.upstream.clone()
        } else {
            v.data.refs.iter().filter(|x| x.kind == RefKind::Remote).map(|x| x.name.clone()).find(|n| n.ends_with(&format!("/{branch}"))).unwrap_or_else(|| format!("{}/{branch}", v.data.remotes.first().map_or("origin", String::as_str)))
        };
        let branch = branch.to_owned();
        let mut d = Dialog::prompt("Set Upstream", &format!("What remote/branch should \"{branch}\" push to and pull from? (empty = stop tracking)"), &guess, "Set Upstream", move |app, v| {
            app.act(tab, Action::SetUpstream { branch, upstream: s(v, "value") })
        });
        d.check = None;
        self.dialog = Some(d);
    }

    /// Hide or solo a ref (`key` "hidden" or "solo"), or show it again.
    pub(crate) fn toggle_ref_list(&mut self, tab: u64, key: &str, full: &str) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        let mut all: HashMap<String, Vec<String>> = self.settings.get(key).unwrap_or_default();
        let list = all.entry(path).or_default();
        if let Some(i) = list.iter().position(|x| x == full) {
            list.remove(i);
        } else {
            list.push(full.to_owned());
        }
        self.settings.set(key, all);
        self.load_repo(tab);
    }

    pub(crate) fn set_hidden_many(&mut self, tab: u64, fulls: Vec<String>, hide: bool) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        let mut all: HashMap<String, Vec<String>> = self.settings.get("hidden").unwrap_or_default();
        let list = all.entry(path).or_default();
        if hide {
            for f in fulls {
                if !list.contains(&f) {
                    list.push(f);
                }
            }
        } else {
            list.retain(|x| !fulls.contains(x));
        }
        self.settings.set("hidden", all);
        self.load_repo(tab);
    }

    pub(crate) fn clear_solo(&mut self, tab: u64) {
        self.clear_list(tab, "solo");
    }

    pub(crate) fn clear_list(&mut self, tab: u64, key: &str) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        let mut all: HashMap<String, Vec<String>> = self.settings.get(key).unwrap_or_default();
        all.insert(path, Vec::new());
        self.settings.set(key, all);
        self.load_repo(tab);
    }

    /// Copy a repository's hidden, soloed and pinned refs to another folder (a new worktree of it).
    pub(crate) fn inherit_view(&mut self, source: &str, target: &str) {
        for key in ["hidden", "solo"] {
            let mut all: HashMap<String, Vec<String>> = self.settings.get(key).unwrap_or_default();
            if let Some(list) = all.get(source).cloned().filter(|l| !l.is_empty()) {
                all.insert(target.to_owned(), list);
                self.settings.set(key, all);
            }
        }
        let mut pinned: HashMap<String, String> = self.settings.get("pinned").unwrap_or_default();
        if let Some(p) = pinned.get(source).cloned() {
            pinned.insert(target.to_owned(), p);
            self.settings.set("pinned", pinned);
        }
    }

    pub(crate) fn toggle_pinned(&mut self, tab: u64, full: &str) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        let mut all: HashMap<String, String> = self.settings.get("pinned").unwrap_or_default();
        if all.get(&path).map(String::as_str) == Some(full) {
            all.remove(&path);
        } else {
            all.insert(path, full.to_owned());
        }
        self.settings.set("pinned", all);
        self.load_repo(tab);
    }

    pub(crate) fn run_bisect(&mut self, tab: u64, mode: &str, commit: Option<String>, good: Option<String>) {
        self.act(tab, Action::Bisect { mode: mode.into(), commit, good });
        if let Some(r) = self.repo_mut(tab) {
            r.ui.select(Selection::Wip);
        }
    }

    pub(crate) fn edit_history_filter(&mut self, tab: u64) {
        let current = self.repo(tab).and_then(|r| r.ui.history_filter.clone()).unwrap_or_default();
        let fields = vec![
            Field::new("author", "Author name or email", FieldKind::Text),
            Field::new("path", "File or folder path", FieldKind::Text).placeholder("src/components"),
            Field::new("text", "Changes containing", FieldKind::Text),
            Field::new("mode", "Content search", FieldKind::Select(vec![("text".into(), "Added or removed text".into()), ("regex".into(), "Added or removed lines matching a regular expression".into())])),
            Field::new("help", "", FieldKind::Note),
        ];
        let mut fields = fields;
        fields.pop();
        let d = Dialog::new("Filter commit history", Body::Form { intro: "Filters apply to the selected branches across their full history.".into(), fields, grid: false })
            .text_value("author", current.author.as_deref().unwrap_or_default())
            .text_value("path", current.path.as_deref().unwrap_or_default())
            .text_value("text", current.text.as_deref().unwrap_or_default())
            .text_value("mode", current.mode.as_deref().unwrap_or("text"))
            .choice("Apply filters", Kind::Primary, move |app, v| {
                let (author, path, text) = (s(v, "author"), s(v, "path"), v.text("text"));
                let mode = if v.text("mode") == "regex" { "regex" } else { "text" };
                let filter = (!author.is_empty() || !path.is_empty() || !text.is_empty()).then(|| HistoryFilter { author: Some(author), path: Some(path), text: Some(text), mode: Some(mode.into()) });
                app.set_history_filter(tab, filter);
            });
        self.dialog = Some(d);
    }

    pub(crate) fn set_history_filter(&mut self, tab: u64, filter: Option<HistoryFilter>) {
        if let Some(r) = self.repo_mut(tab) {
            r.ui.history_filter = filter;
            r.ui.select(Selection::Wip);
        }
        self.load_repo(tab);
    }

    pub(crate) fn open_history(&mut self, tab: u64, file: &str, mode: HistoryMode) {
        if let Some(r) = self.repo_mut(tab) {
            r.ui.history = Some(HistoryTarget { file: file.to_owned(), mode, sha: None });
            r.ui.diff = None;
        }
    }

    pub(crate) fn add_remote(&mut self, tab: u64) {
        let fields = vec![Field::new("name", "Remote name", FieldKind::Text), Field::new("url", "Remote URL", FieldKind::Text).placeholder("https://github.com/owner/project.git")];
        let d = Dialog::form("Add a remote", fields, "Add Remote", move |app, v| app.act(tab, Action::AddRemote { name: s(v, "name"), url: s(v, "url") }))
            .text_value("name", "origin")
            .checked(|v| (v.text("name").trim().is_empty() || v.text("url").trim().is_empty()).then(String::new));
        self.dialog = Some(d);
    }

    pub(crate) fn edit_remote(&mut self, tab: u64, name: &str) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        let remote = gitcraft_core::manage::remotes(Path::new(&path)).into_iter().find(|r| r.name == name);
        let fields = vec![Field::new("name", "Name", FieldKind::Text), Field::new("url", "Pull URL", FieldKind::Text), Field::new("push", "Push URL", FieldKind::Text)];
        let old = name.to_owned();
        let d = Dialog::form("Edit Remote", fields, "Save", move |app, v| app.act(tab, Action::RemoteEdit { name: old, new_name: s(v, "name"), url: s(v, "url"), push_url: s(v, "push") }))
            .text_value("name", name)
            .text_value("url", remote.as_ref().map_or("", |r| r.fetch_url.as_str()))
            .text_value("push", remote.as_ref().map_or("", |r| r.push_url.as_str()));
        self.dialog = Some(d);
    }

    pub(crate) fn remove_remote(&mut self, tab: u64, name: &str) {
        self.confirm_act(tab, &format!("Remove {name}?"), "The remote and its remote-tracking branches are removed from this repository. Nothing on the server changes.", "Remove", Action::RemoteRemove { name: name.to_owned() });
    }

    pub(crate) fn add_worktree(&mut self, tab: u64, start: Option<String>) {
        let Some(v) = self.repo(tab).and_then(|r| r.view.as_ref()) else { return };
        let sep = if v.path.contains('\\') { "\\" } else { "/" };
        let parent = v.path.rsplit_once(['/', '\\']).map_or(v.path.as_str(), |(p, _)| p).to_owned();
        let folder = format!("{parent}{sep}{}-worktree", v.name);
        let new_label = match &start {
            Some(s) => format!("Create a new branch from {}", s.chars().take(8).collect::<String>()),
            None => "Create this as a new branch from the checked-out commit".into(),
        };
        let fields = vec![
            Field::new("path", "Folder for the worktree", FieldKind::Folder),
            Field::new("branch", "Branch", FieldKind::Text).placeholder("existing branch, or a new branch name"),
            Field::new("new", &new_label, FieldKind::Check),
        ];
        let d = Dialog::new("Add a new worktree", Body::Form { intro: "A worktree is a second working folder for the same repository, with its own branch checked out.".into(), fields, grid: false })
            .text_value("path", &folder)
            .flag_value("new", true)
            .choice("Create Worktree", Kind::Primary, move |app, values| {
                let folder = s(values, "path");
                app.act(tab, Action::WorktreeAdd { path: folder, branch: s(values, "branch"), new_branch: values.flag("new"), start });
            });
        self.dialog = Some(d);
    }

    pub(crate) fn lock_worktree(&mut self, tab: u64, wt: &str) {
        let wt = wt.to_owned();
        let mut d = Dialog::prompt("Lock worktree", "Message (optional):", "", "Lock", move |app, v| app.act(tab, Action::WorktreeLock { path: wt, message: s(v, "value") }));
        d.check = None;
        self.dialog = Some(d);
    }

    pub(crate) fn remove_worktree(&mut self, tab: u64, wt: &str) {
        let (prompt, stale_only) = self.worktree_removal_prompt(tab, wt);
        let wt = wt.to_owned();
        // Its tab closes once the worktree is removed (see `worktree_done`), so a refused removal
        // leaves it open.
        self.dialog = Some(Dialog::confirm_with("Remove worktree?", &format!("{prompt} Its branch stays."), "Remove", true, move |app, _| {
            app.act_worktree_removal(tab, Action::WorktreeRemove { path: wt, force: false, stale_only });
        }));
    }

    /// Remove a worktree and, if chosen, the branch it had checked out: locally, or locally and on its remote.
    pub(crate) fn remove_worktree_and_branch(&mut self, tab: u64, wt: &str, branch: &str) {
        let upstream = self.repo(tab).and_then(|r| r.view.as_ref()).and_then(|v| v.data.refs.iter().find(|r| r.kind == gitcraft_core::git::RefKind::Local && r.name == branch).map(|r| r.upstream.clone())).filter(|u| !u.is_empty());
        let mut fields = vec![Field::new("local", &format!("Also delete the branch {branch}"), FieldKind::Check)];
        if let Some(up) = &upstream {
            fields.push(Field::new("remote", &format!("Also delete {up} from the remote"), FieldKind::Check).when("local"));
        }
        let (intro, stale_only) = self.worktree_removal_prompt(tab, wt);
        let (wt, branch) = (wt.to_owned(), branch.to_owned());
        let d = Dialog::new("Remove worktree?", Body::Form { intro, fields, grid: false })
            .flag_value("local", true)
            .choice("Remove", Kind::Danger, move |app, values| {
                let action = if values.flag("local") {
                    Action::WorktreeRemoveAndBranch {
                        path: wt, branch, stale_only, force: false,
                        remote_branch: upstream.filter(|_| values.flag("remote")),
                    }
                } else {
                    Action::WorktreeRemove { path: wt, force: false, stale_only }
                };
                app.act_worktree_removal(tab, action);
            });
        self.dialog = Some(d);
    }

    fn worktree_removal_prompt(&self, tab: u64, path: &str) -> (String, bool) {
        let prunable = self.repo(tab).and_then(|repo| repo.view.as_ref()).is_some_and(|view| {
            view.data.worktrees.iter().any(|worktree| worktree.prunable && super::same_folder(&worktree.path, path))
        });
        if prunable {
            (format!("Remove the stale worktree registration for {path}? Files in its folder are kept."), true)
        } else {
            (format!("Remove the worktree at {path}? Its folder and files, including ignored files, will be deleted."), false)
        }
    }

    fn act_worktree_removal(&mut self, tab: u64, action: Action) {
        let target = match &action {
            Action::WorktreeRemove { path, .. } | Action::WorktreeRemoveAndBranch { path, .. } => self.tab_of(path),
            _ => None,
        };
        if target.is_some_and(|target| self.has_unsaved(Some(&[target]))) {
            self.toast(false, "Save or discard open editor changes in the worktree before removing it.");
            return;
        }
        self.act(tab, action);
    }

    pub(super) fn offer_force_worktree_removal(
        &mut self, tab: u64, action: &Action, result: &gitcraft_core::actions::ActionResult,
    ) -> bool {
        if self.dialog.is_some() || self.repo(tab).is_none() {
            return false;
        }
        let Some(action) = action.force_worktree_retry(result) else { return false };
        let (path, branches) = match &action {
            Action::WorktreeRemove { path, .. } => (path, String::new()),
            Action::WorktreeRemoveAndBranch { path, branch, remote_branch, .. } => {
                let mut branches = format!("\n\nLocal branch {branch} will also be deleted.");
                if let Some(remote_branch) = remote_branch {
                    branches.push_str(&format!(" Remote branch {remote_branch} will also be deleted."));
                }
                (path, branches)
            }
            _ => return false,
        };
        let warning = format!(
            "This worktree has local changes. Force removal permanently deletes the folder and all files at:\n\n{path}\n\nStaged changes, unstaged changes, untracked files, and ignored files in this folder will be lost. Commit or back up anything you want to keep before continuing.{branches}"
        );
        self.dialog = Some(Dialog::confirm_with("Force remove worktree?", &warning, "Force Remove", true, move |app, _| {
            app.act_worktree_removal(tab, action);
        }));
        true
    }

    pub(crate) fn add_submodule(&mut self, tab: u64) {
        let fields = vec![Field::new("url", "Remote URL", FieldKind::Text).placeholder("https://github.com/owner/library.git"), Field::new("path", "Path", FieldKind::Text).placeholder("libs/library")];
        self.dialog = Some(Dialog::form("Add Submodule", fields, "Add Submodule", move |app, v| app.act(tab, Action::SubmoduleAdd { url: s(v, "url"), path: s(v, "path") })));
    }

    pub(crate) fn init_gitflow(&mut self, tab: u64) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        let c = gitcraft_core::manage::gitflow_config(Path::new(&path));
        let title = if c.initialized { "Update Gitflow Config" } else { "Initialize Gitflow" };
        let fields = vec![
            Field::new("master", "Main/Master", FieldKind::Text),
            Field::new("develop", "Develop", FieldKind::Text),
            Field::new("feature", "Feature prefix", FieldKind::Text),
            Field::new("release", "Release prefix", FieldKind::Text),
            Field::new("hotfix", "Hotfix prefix", FieldKind::Text),
            Field::new("versiontag", "Version Tag prefix", FieldKind::Text),
        ];
        let intro = if c.initialized { "Changing these prefixes will cause any existing branches with different prefixes to be ignored by Gitflow." } else { "" };
        let d = Dialog::new(title, Body::Form { intro: intro.into(), fields, grid: false })
            .text_value("master", &c.master)
            .text_value("develop", &c.develop)
            .text_value("feature", &c.feature)
            .text_value("release", &c.release)
            .text_value("hotfix", &c.hotfix)
            .text_value("versiontag", &c.versiontag)
            .choice(title, Kind::Primary, move |app, v| {
                let config = gitcraft_core::manage::GitflowConfig { master: s(v, "master"), develop: s(v, "develop"), feature: s(v, "feature"), release: s(v, "release"), hotfix: s(v, "hotfix"), versiontag: s(v, "versiontag"), initialized: true };
                app.act(tab, Action::GitflowInit { config });
            });
        self.dialog = Some(d);
    }

    pub(crate) fn start_gitflow(&mut self, tab: u64, kind: &'static str) {
        let label = match kind {
            "feature" => "Feature",
            "release" => "Release",
            _ => "Hotfix",
        };
        let field = if kind == "feature" { "Feature name".to_owned() } else { format!("{label} version") };
        self.dialog = Some(Dialog::prompt(&format!("Start {label}"), &field, "", &format!("Start {label}"), move |app, v| app.act(tab, Action::GitflowStart { kind: kind.into(), name: s(v, "value") })));
    }

    pub(crate) fn finish_gitflow(&mut self, tab: u64, kind: &'static str, name: &str) {
        let label = match kind {
            "feature" => "Feature",
            "release" => "Release",
            _ => "Hotfix",
        };
        let mut fields = vec![Field::new("delete", "Delete branch", FieldKind::Check)];
        if kind != "feature" {
            fields.push(Field::new("tag", "Tag message", FieldKind::Text).placeholder("tag message"));
        }
        let name = name.to_owned();
        let d = Dialog::form(&format!("Finish {label}: {name}"), fields, &format!("Finish {label}"), move |app, v| {
            app.act(tab, Action::GitflowFinish { kind: kind.into(), name, keep: !v.flag("delete"), tag_message: s(v, "tag") })
        })
        .flag_value("delete", true);
        self.dialog = Some(d);
    }

    pub(crate) fn repository_maintenance(&mut self, tab: u64) {
        let d = Dialog::new("Repository maintenance", Body::Text("Optimize local Git data, rebuild the commit graph, or clean up stale remote-tracking branches.".into()))
            .choice("Run garbage collection", Kind::Default, move |app, _| {
                app.dialog = Some(Dialog::confirm_with(
                    "Run Git garbage collection?",
                    "Git may remove unreachable objects according to its configured expiry. Named branches, tags, and stash entries stay available.",
                    "Run maintenance",
                    false,
                    move |app, _| app.act(tab, Action::MaintainRepository { task: "gc".into() }),
                ));
            })
            .choice("Write commit graph", Kind::Default, move |app, _| app.act(tab, Action::MaintainRepository { task: "commitGraph".into() }))
            .choice("Fetch and prune remotes", Kind::Default, move |app, _| app.act(tab, Action::Fetch));
        self.dialog = Some(d);
    }

    pub(crate) fn fetch_more_history(&mut self, tab: u64) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        let info = match gitcraft_core::extras::history_fetch_info(Path::new(&path)) {
            Ok(i) => i,
            Err(e) => return self.toast(false, e),
        };
        if info.remotes.is_empty() {
            return self.toast(false, "Add a remote before fetching history.");
        }
        let mut options: Vec<(&str, &str)> = Vec::new();
        if info.shallow {
            options.push(("Fetch 500 more commits", "deepen"));
            options.push(("Fetch all history", "unshallow"));
        }
        if !info.promisor_remotes.is_empty() {
            options.push(("Download filtered objects", "objects"));
        }
        if options.is_empty() {
            return self.toast(true, "This repository already has complete history.");
        }
        let mut d = Dialog::new("Fetch more repository history", Body::Text("Choose how much to download. Full history or filtered objects can be large.".into()));
        for (label, mode) in options {
            let remotes = if mode == "objects" { info.promisor_remotes.clone() } else { info.remotes.clone() };
            d = d.choice(label, Kind::Default, move |app, _| {
                let first = remotes.first().cloned().unwrap_or_default();
                let field = Field::new("remote", "Remote", FieldKind::Select(remotes.iter().map(|r| (r.clone(), r.clone())).collect()));
                let d = Dialog::form(label, vec![field], "Fetch", move |app, v| app.act(tab, Action::FetchHistory { remote: s(v, "remote"), mode: mode.into() })).text_value("remote", &first);
                app.dialog = Some(d);
            });
        }
        self.dialog = Some(d);
    }

    pub(crate) fn recover_discarded(&mut self, tab: u64) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        let backups = match gitcraft_core::discard_backups::list(Path::new(&path)) {
            Ok(b) => b,
            Err(e) => return self.toast(false, e),
        };
        if backups.is_empty() {
            return self.toast(true, "No discarded changes to recover.");
        }
        let options = backups.iter().enumerate().map(|(i, b)| (b.reference.clone(), format!("{} · {}. {}", super::widgets::format_date(b.time as i64, "MM/DD/YYYY h:mm:ss A"), i + 1, b.subject))).collect();
        let first = backups[0].reference.clone();
        let d = Dialog::new("Recover discarded changes", Body::Form { intro: "Restores the files and staging from this backup. If your current changes overlap, Git may refuse the restore or report conflicts.".into(), fields: vec![Field::new("reference", "Backup", FieldKind::Select(options))], grid: false })
            .text_value("reference", &first)
            .choice("Restore", Kind::Primary, move |app, v| app.act(tab, Action::RestoreDiscard { reference: s(v, "reference") }));
        self.dialog = Some(d);
    }

    pub(crate) fn compare_ranges(&mut self, tab: u64) {
        let fields = vec![Field::new("before", "Before", FieldKind::Text).placeholder("base..old-tip"), Field::new("after", "After", FieldKind::Text).placeholder("base..new-tip")];
        let d = Dialog::new("Compare commit ranges", Body::Form { intro: "Compare two versions of a series of commits, such as before and after a rebase.".into(), fields, grid: false }).choice("Compare", Kind::Primary, move |app, v| {
            let Some(path) = app.repo(tab).map(|r| r.path.clone()) else { return };
            let (before, after) = (s(v, "before"), s(v, "after"));
            app.spawn(move || {
                let result = gitcraft_core::extras::range_diff(Path::new(&path), &before, &after).map(|text| if text.trim().is_empty() { "No commits to compare.".into() } else { text });
                super::Msg::Mono { title: format!("Compare commit ranges: {before} → {after}"), result }
            });
        });
        self.dialog = Some(d);
    }

    pub(crate) fn reflog_dialog(&mut self, tab: u64) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        match gitcraft_core::extras::reflog(Path::new(&path)) {
            Ok(entries) => {
                let rows = entries.iter().map(|e| (e.message.clone(), format!("{} · {}", e.sha.chars().take(10).collect::<String>(), e.selector))).collect();
                let shas: Vec<String> = entries.iter().map(|e| e.sha.clone()).collect();
                let on_pick = std::rc::Rc::new(move |app: &mut App, i: usize| app.prompt_branch(tab, shas.get(i).cloned(), false));
                let body = Body::Pick { hint: "Search the last 200 entries".into(), filter: String::new(), rows, button: "Create branch here".into(), on_pick };
                self.dialog = Some(Dialog::new("Recover commits from reflog", body).width(740.0).cancel_label("Close"));
            }
            Err(e) => self.toast(false, e),
        }
    }

    pub(crate) fn lfs_track_prompt(&mut self, tab: u64) {
        self.dialog = Some(Dialog::prompt("Track with Git LFS", "Pattern (for example *.psd or assets/**)", "*.psd", "Track", move |app, v| app.act(tab, Action::LfsTrack { pattern: s(v, "value"), lockable: false })));
    }

    /// After a worktree action succeeded: a new worktree opens in a tab of its own, and a removed
    /// one's tab closes.
    pub(crate) fn worktree_done(&mut self, tab: u64, action: &Action) {
        match action {
            Action::WorktreeAdd { path, .. } => {
                // Git resolves a relative folder against the repository, not against GitCraft's own folder.
                let folder = match self.repo(tab) {
                    Some(r) => Path::new(&r.path).join(path),
                    None => Path::new(path).to_path_buf(),
                };
                if folder.is_dir() {
                    let folder = folder.to_string_lossy().into_owned();
                    // The new worktree starts with the source repository's view: hidden, soloed and pinned refs.
                    if let Some(source) = self.repo(tab).map(|r| r.path.clone()) {
                        self.inherit_view(&source, &folder);
                    }
                    self.open_repo(&folder);
                }
            }
            Action::WorktreeRemove { path, .. } | Action::WorktreeRemoveAndBranch { path, .. } => {
                if let Some(t) = self.tab_of(path) {
                    self.close_tab(t);
                }
            }
            _ => {}
        }
    }
}
