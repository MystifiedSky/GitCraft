//! Right-click menus for commits, branches, tags, stashes and files, with GitCraft's
//! items in their order. Menus are built as data while a frame is drawn; the chosen item runs
//! afterwards with the whole app.

use super::App;
use super::dialogs::{Body, Dialog, fit_button_label};
use super::rebase::{base_for, head_line};
use super::theme::p;
use super::widgets::{self, Kind, REMOVED};
use gitcraft_core::actions::Action;
use gitcraft_core::git::{Commit, GitRef, RefKind, Stash};
use gitcraft_core::session::RepoView;

/// Work to do with the app once the frame is drawn.
pub type Later = Box<dyn FnOnce(&mut App)>;

pub enum Entry {
    Item { label: String, enabled: bool, danger: bool, run: Later },
    Sub { label: String, enabled: bool, children: Vec<Entry> },
    Separator,
    Heading(String),
}

pub fn item(label: impl Into<String>, run: impl FnOnce(&mut App) + 'static) -> Entry {
    Entry::Item { label: label.into(), enabled: true, danger: false, run: Box::new(run) }
}

pub fn danger(label: impl Into<String>, run: impl FnOnce(&mut App) + 'static) -> Entry {
    Entry::Item { label: label.into(), enabled: true, danger: true, run: Box::new(run) }
}

pub fn off(entry: Entry, enabled: bool) -> Entry {
    match entry {
        Entry::Item { label, danger, run, .. } => Entry::Item { label, enabled, danger, run },
        Entry::Sub { label, children, .. } => Entry::Sub { label, enabled, children },
        other => other,
    }
}

pub fn sub(label: impl Into<String>, children: Vec<Entry>) -> Entry {
    Entry::Sub { label: label.into(), enabled: true, children }
}

pub fn act(tab: u64, action: Action) -> impl FnOnce(&mut App) {
    move |app| app.act(tab, action)
}

/// Draw `entries` in an open menu; the chosen one goes to `later`.
pub fn show(ui: &mut egui::Ui, entries: Vec<Entry>, later: &mut Vec<Later>) {
    let c = p();
    ui.set_min_width(220.0);
    let mut last_sep = true;
    let n = entries.len();
    for (i, e) in entries.into_iter().enumerate() {
        match e {
            Entry::Separator => {
                if !last_sep && i + 1 < n {
                    ui.separator();
                    last_sep = true;
                }
            }
            Entry::Heading(text) => {
                ui.label(egui::RichText::new(text).size(11.0).color(c.fg3));
                last_sep = false;
            }
            Entry::Item { label, enabled, danger, run } => {
                let text = egui::RichText::new(label).size(13.0);
                let text = if danger { text.color(REMOVED) } else { text };
                if ui.add_enabled(enabled, egui::Button::new(text).frame_when_inactive(false)).clicked() {
                    later.push(run);
                    ui.close();
                }
                last_sep = false;
            }
            Entry::Sub { label, enabled, children } => {
                ui.add_enabled_ui(enabled, |ui| {
                    ui.menu_button(egui::RichText::new(label).size(13.0), |ui| show(ui, children, later));
                });
                last_sep = false;
            }
        }
    }
}

/// Attach a right-click menu, built only when it opens.
pub fn attach(response: &egui::Response, later: &mut Vec<Later>, build: impl FnOnce() -> Vec<Entry>) {
    response.context_menu(|ui| show(ui, build(), later));
}

/// Which remote a remote-tracking name (origin/feature/x) belongs to; remote names can hold "/" too.
pub fn split_remote(repo: &RepoView, name: &str) -> Option<(String, String)> {
    // The longest remote name that ends at a "/" wins, so `team` never shadows `team/forks`.
    let remote = repo.data.remotes.iter().filter(|r| name.strip_prefix(r.as_str()).is_some_and(|rest| rest.starts_with('/'))).max_by_key(|r| r.len())?;
    Some((remote.clone(), name[remote.len() + 1..].to_owned()))
}

/// Whether `ancestor` is reachable from `tip` through the loaded parents, or Git's full graph.
pub fn is_loaded_ancestor(repo: &RepoView, ancestor: &str, tip: &str) -> bool {
    let parents: std::collections::HashMap<&str, &Vec<String>> = repo.data.commits.iter().map(|c| (c.sha.as_str(), &c.parents)).collect();
    let mut pending = vec![tip.to_owned()];
    let mut seen = std::collections::HashSet::new();
    while let Some(sha) = pending.pop() {
        if sha == ancestor {
            return true;
        }
        if !seen.insert(sha.clone()) {
            continue;
        }
        if let Some(ps) = parents.get(sha.as_str()) {
            pending.extend(ps.iter().cloned());
        }
    }
    !repo.path.is_empty() && gitcraft_core::git::run(std::path::Path::new(&repo.path), &["merge-base", "--is-ancestor", ancestor, tip]).ok
}

fn reset_menu(tab: u64, repo: &RepoView, sha: &str) -> Entry {
    let head = repo.data.head_branch.clone().unwrap_or_else(|| "HEAD".into());
    let (s1, s2, s3) = (sha.to_owned(), sha.to_owned(), sha.to_owned());
    let head2 = head.clone();
    sub(
        format!("Reset {head} to this commit"),
        vec![
            item("Soft - keep all changes", act(tab, Action::Reset { sha: s1, mode: "soft".into() })),
            item("Mixed - keep working copy but reset index", act(tab, Action::Reset { sha: s2, mode: "mixed".into() })),
            danger("Hard - discard all changes", move |app| {
                app.confirm_act(
                    tab,
                    &format!("Reset {head2} to this commit?"),
                    "Hard reset throws away all uncommitted changes. Undo can move the branch back, but cannot restore discarded work.",
                    "Reset Hard",
                    Action::Reset { sha: s3, mode: "hard".into() },
                )
            }),
        ],
    )
}

/// Actions offered when `source` is dropped on `target`; `head` is the checked-out branch.
fn dropped_actions(v: &RepoView, source: &GitRef, target: &GitRef, head: Option<&str>) -> Vec<(String, Action)> {
    let is_head = |r: &GitRef| r.kind == RefKind::Local && head == Some(r.name.as_str());
    let (s, t) = (&source.name, &target.name);
    let mut choices: Vec<(String, Action)> = Vec::new();
    // The branch behind moves to the one ahead, whichever side of the drop it is on.
    if target.kind == RefKind::Local && target.sha != source.sha && is_loaded_ancestor(v, &target.sha, &source.sha) {
        choices.push((format!("Fast-forward {t} to {s}"), Action::FastForward { branch: t.clone(), to: source.full.clone() }));
    }
    if source.kind == RefKind::Local && target.sha != source.sha && is_loaded_ancestor(v, &source.sha, &target.sha) {
        choices.push((format!("Fast-forward {s} to {t}"), Action::FastForward { branch: s.clone(), to: target.full.clone() }));
    }
    // Merge and rebase check out the branch they act on; a remote branch can only be merged from or rebased onto.
    if target.kind == RefKind::Local {
        choices.push((format!("Merge {s} into {t}"), Action::Merge { branch: source.full.clone(), into: Some(t.clone()), changes: gitcraft_core::actions::CheckoutChanges::Ask }));
    }
    if source.kind == RefKind::Local {
        choices.push((format!("Rebase {s} onto {t}"), Action::Rebase { onto: target.full.clone(), branch: Some(s.clone()), changes: gitcraft_core::actions::CheckoutChanges::Ask }));
    }
    if source.kind == RefKind::Local && !is_head(target) {
        choices.push((format!("Merge {t} into {s}"), Action::Merge { branch: target.full.clone(), into: Some(s.clone()), changes: gitcraft_core::actions::CheckoutChanges::Ask }));
    }
    if source.kind == RefKind::Local && target.kind == RefKind::Remote {
        choices.push((format!("Push {s}"), Action::PushBranch { branch: s.clone() }));
    }
    choices
}

/// One line under a dropped branch's action saying what it does.
fn drop_detail(action: &Action) -> String {
    match action {
        Action::FastForward { branch, to } => format!("Move {} to {} without a merge commit.", short_ref(branch), short_ref(to)),
        Action::Merge { branch, into: Some(into), .. } => format!("Merge {} into {}; fast-forwards when possible.", short_ref(branch), short_ref(into)),
        Action::Merge { branch, .. } => format!("Merge {}; fast-forwards when possible.", short_ref(branch)),
        Action::Rebase { onto, branch: Some(branch), .. } => format!("Replay {}'s commits on top of {}.", short_ref(branch), short_ref(onto)),
        Action::Rebase { onto, .. } => format!("Replay the commits on top of {}.", short_ref(onto)),
        Action::PushBranch { branch } => format!("Send {} to its remote.", short_ref(branch)),
        _ => String::new(),
    }
}

fn branch_drop_dialog(title: &str, mut actions: Vec<(String, String, Later)>) -> Dialog {
    let draw = move |app: &mut App, ui: &mut egui::Ui| -> bool {
        let selected = actions.iter().enumerate().position(|(index, (label, detail, _))| {
            if index > 0 {
                ui.add_space(10.0);
            }
            let display = fit_button_label(ui, label, ui.available_width());
            let pressed = widgets::button_labeled(ui, Kind::Default, None, &display, label, 28.0, true)
                .on_hover_text(format!("{label}\n{detail}"))
                .clicked();
            ui.add_space(4.0);
            ui.add(egui::Label::new(egui::RichText::new(detail).size(11.5).color(p().fg3)).truncate());
            pressed
        });
        if let Some(index) = selected {
            (actions.remove(index).2)(app);
        }
        selected.is_some()
    };
    Dialog::new(title, Body::Custom(Box::new(draw))).width(520.0)
}

/// A ref name without its `refs/…` prefix, for messages.
pub(crate) fn short_ref(name: &str) -> &str {
    name.strip_prefix("refs/heads/").or_else(|| name.strip_prefix("refs/remotes/")).or_else(|| name.strip_prefix("refs/tags/")).unwrap_or(name)
}

impl App {
    fn per_repo(&self, key: &str, path: &str) -> Vec<String> {
        self.settings.get::<std::collections::HashMap<String, Vec<String>>>(key).and_then(|m| m.get(path).cloned()).unwrap_or_default()
    }

    pub(crate) fn hidden_refs(&self, path: &str) -> Vec<String> {
        self.per_repo("hidden", path)
    }

    pub(crate) fn solo_refs(&self, path: &str) -> Vec<String> {
        self.per_repo("solo", path)
    }

    /// A branch label was dropped on another in the graph: offer what can be done with the two.
    pub(crate) fn ref_dropped(&mut self, tab: u64, source: GitRef, target: GitRef) {
        let Some(v) = self.repo(tab).and_then(|r| r.view.as_ref()) else { return };
        let (s, t) = (source.name.clone(), target.name.clone());
        let choices = dropped_actions(v, &source, &target, v.data.head_branch.as_deref());
        let pull_request = source.kind == RefKind::Local && target.kind != RefKind::Tag;
        if choices.is_empty() && !pull_request {
            return self.toast(false, format!("Check out {t} or {s} first to merge or rebase them."));
        }
        let mut actions: Vec<(String, String, Later)> = choices.into_iter().map(|(label, action)| {
            let detail = drop_detail(&action);
            (label, detail, Box::new(move |app: &mut App| app.act(tab, action)) as Later)
        }).collect();
        if pull_request {
            // Into the target's own branch name (a remote branch's name without its remote).
            let into = if target.kind == RefKind::Remote { split_remote(v, &t).map_or(t.clone(), |(_, b)| b) } else { t.clone() };
            let source = s.clone();
            let label = format!("Start a pull request from {source} into {into}");
            let detail = "Open the pull request form with these branches selected.".into();
            actions.push((label, detail, Box::new(move |app| app.start_pull_request_into(tab, &source, Some(into)))));
        }
        self.dialog = Some(branch_drop_dialog(&format!("{s} → {t}"), actions));
    }

    pub(crate) fn pinned_ref(&self, path: &str) -> Option<String> {
        self.settings.get::<std::collections::HashMap<String, String>>("pinned").and_then(|m| m.get(path).cloned())
    }

    pub(crate) fn branch_menu(&self, tab: u64, repo: &RepoView, r: &GitRef) -> Vec<Entry> {
        if r.kind == RefKind::Tag {
            return self.tag_menu(tab, repo, r);
        }
        let head = repo.data.head_branch.clone();
        let is_head = r.kind == RefKind::Local && Some(&r.name) == head.as_ref();
        let name = r.name.clone();
        let in_worktree = if r.kind == RefKind::Local && !is_head { super::repo::other_worktree(repo, &name) } else { None };
        let mut items = Vec::new();
        match r.kind {
            RefKind::Local if !is_head => {
                let label = if in_worktree.is_some() { format!("Open worktree for {name}") } else { format!("Checkout {name}") };
                items.push(item(label, act(tab, Action::Checkout { target: name.clone(), changes: gitcraft_core::actions::CheckoutChanges::Ask, branch: true })));
            }
            RefKind::Remote => items.push(item(format!("Checkout {name}"), act(tab, Action::CheckoutRemote { remote_branch: name.clone(), stash_if_needed: false, changes: gitcraft_core::actions::CheckoutChanges::Ask }))),
            _ => {}
        }
        if let Some(head) = head.clone().filter(|_| !is_head) {
            items.push(item(format!("Merge {name} into {head}"), act(tab, Action::Merge { branch: name.clone(), into: None, changes: Default::default() })));
            let n2 = name.clone();
            items.push(item(format!("Squash merge {name} into {head}"), move |app| app.act(tab, Action::MergeSquash { branch: n2, into: None, changes: Default::default() })));
            items.push(item(format!("Rebase {head} onto {name}"), act(tab, Action::Rebase { onto: name.clone(), branch: None, changes: Default::default() })));
            let n3 = name.clone();
            items.push(item(format!("Interactive Rebase {head} onto {name}"), move |app| app.open_rebase(tab, &n3, &n3, false)));
            if r.sha != repo.data.head_sha && is_loaded_ancestor(repo, &repo.data.head_sha, &r.sha) {
                items.push(item(format!("Fast-forward {head} to {name}"), act(tab, Action::FastForward { branch: head.clone(), to: name.clone() })));
            }
        }
        items.push(Entry::Separator);
        if is_head {
            items.push(item(format!("Push {name}"), act(tab, Action::Push { force: false, lease: None, review: None })));
        }
        if r.kind == RefKind::Local {
            let n = name.clone();
            items.push(item(format!("Start a pull request from {name}"), move |app| app.start_pull_request(tab, &n)));
        }
        let n = name.clone();
        items.push(item("Explain Branch Changes (Preview)", move |app| app.explain_branch(tab, &n)));
        if r.kind == RefKind::Local && !is_head && !r.upstream.is_empty() && r.behind > 0 && r.ahead == 0 {
            items.push(item(format!("Fast-forward {name} to {}", r.upstream), act(tab, Action::FastForward { branch: name.clone(), to: r.upstream.clone() })));
        }
        if r.kind == RefKind::Local {
            let label = if r.upstream.is_empty() { "Set Upstream".to_owned() } else { format!("Set Upstream (now {})", r.upstream) };
            let n = name.clone();
            items.push(item(label, move |app| app.prompt_upstream_for(tab, &n)));
        }
        let sha = r.sha.clone();
        items.push(item("Create branch here", move |app| app.prompt_branch(tab, Some(sha), true)));
        items.push(reset_menu(tab, repo, &r.sha));
        let sha = r.sha.clone();
        items.push(item(format!("Create worktree from {name}"), move |app| app.add_worktree(tab, Some(sha))));
        if repo.data.head_branch.is_some() && head_line(repo).contains(&r.sha) {
            let sha = r.sha.clone();
            items.push(item("Rename commit", move |app| app.reword_commit(tab, &sha)));
        }
        let sha = r.sha.clone();
        items.push(item("Create tag here", move |app| app.prompt_tag(tab, Some(sha), false, String::new())));
        items.push(Entry::Separator);
        // A branch checked out in another worktree can't be deleted: removing that worktree is offered instead.
        if let Some(wt) = in_worktree {
            let wt = wt.path.clone();
            let n = name.clone();
            items.push(item(format!("Rename {name}"), move |app| app.prompt_rename_branch(tab, &n)));
            let n = name.clone();
            items.push(danger(format!("Remove worktree and delete {name}…"), move |app| app.remove_worktree_and_branch(tab, &wt, &n)));
        } else if r.kind == RefKind::Local {
            let n = name.clone();
            items.push(item(format!("Rename {name}"), move |app| app.prompt_rename_branch(tab, &n)));
            let n = name.clone();
            items.push(off(
                danger(format!("Delete {name}"), move |app| {
                    app.confirm_act(tab, &format!("Delete {n}?"), "The branch label is removed. You can undo this with Undo.", "Delete", Action::DeleteBranch { name: n.clone() })
                }),
                !is_head,
            ));
        } else {
            let n = name.clone();
            items.push(danger(format!("Delete {name}"), move |app| {
                app.confirm_act(tab, &format!("Delete {n} from the remote?"), "This removes the branch on the remote server for everyone.", "Delete", Action::DeleteRemoteBranch { remote_branch: n.clone() })
            }));
        }
        items.push(Entry::Separator);
        let path = repo.path.clone();
        let hidden = self.hidden_refs(&path).contains(&r.full);
        let soloed = self.solo_refs(&path).contains(&r.full);
        let full = r.full.clone();
        items.push(off(item(if hidden { "Show in the graph" } else { "Hide in the graph" }, move |app| app.toggle_ref_list(tab, "hidden", &full)), !(is_head && !hidden)));
        let full = r.full.clone();
        items.push(item(if soloed { "Stop soloing this branch" } else { "Solo" }, move |app| app.toggle_ref_list(tab, "solo", &full)));
        let full = r.full.clone();
        let pinned = self.pinned_ref(&path).as_deref() == Some(r.full.as_str());
        items.push(item(if pinned { "Unpin from Left" } else { "Pin to Left" }, move |app| app.toggle_pinned(tab, &full)));
        if !self.solo_refs(&path).is_empty() {
            items.push(item("Stop soloing all branches", move |app| app.clear_solo(tab)));
        }
        items.push(Entry::Separator);
        let n = name.clone();
        items.push(item("Copy branch name", move |app| app.copy(&n)));
        let sha = r.sha.clone();
        items.push(item("Copy commit sha", move |app| app.copy(&sha)));
        let on_remote = split_remote(repo, if r.kind == RefKind::Remote { &r.name } else { &r.upstream });
        if let Some((remote, branch)) = on_remote {
            items.extend(self.link_items(tab, repo, "Copy link to branch", super::weblinks::Target::Branch(branch), Some(remote)));
        }
        items
    }

    fn tag_menu(&self, tab: u64, repo: &RepoView, r: &GitRef) -> Vec<Entry> {
        let name = r.name.clone();
        // The tag's commit itself: a branch of the same name must not be checked out instead.
        let mut items = vec![item(format!("Checkout the commit at {name}"), act(tab, Action::Checkout { target: r.sha.clone(), changes: gitcraft_core::actions::CheckoutChanges::Ask, branch: false }))];
        let sha = r.sha.clone();
        items.push(item("Create branch here", move |app| app.prompt_branch(tab, Some(sha), true)));
        items.push(Entry::Separator);
        if !repo.data.remotes.is_empty() {
            items.push(sub(format!("Push {name} to remote"), repo.data.remotes.iter().map(|remote| item(remote.clone(), act(tab, Action::PushTag { name: name.clone(), remote: remote.clone() }))).collect()));
            items.push(sub(
                format!("Delete {name} from remote"),
                repo.data
                    .remotes
                    .iter()
                    .map(|remote| {
                        let (n, rem) = (name.clone(), remote.clone());
                        danger(remote.clone(), move |app| {
                            app.confirm_act(tab, &format!("Delete {n} from {rem}?"), "This deletes the tag on the remote for everyone. Your local tag is kept.", "Delete Remote Tag", Action::DeleteRemoteTag { name: n.clone(), remote: rem.clone() })
                        })
                    })
                    .collect(),
            ));
            items.push(Entry::Separator);
        }
        items.push(danger(format!("Delete {name} locally"), act(tab, Action::DeleteTag { name: name.clone() })));
        let hidden = self.hidden_refs(&repo.path).contains(&r.full);
        let full = r.full.clone();
        items.push(item(if hidden { "Show in the graph" } else { "Hide in the graph" }, move |app| app.toggle_ref_list(tab, "hidden", &full)));
        items.push(Entry::Separator);
        let n = name.clone();
        items.push(item("Copy tag name", move |app| app.copy(&n)));
        items.extend(self.link_items(tab, repo, "Copy link to this tag", super::weblinks::Target::Tag(name), None));
        items
    }

    fn commit_action(&self, tab: u64, repo: &RepoView, c: &Commit, revert: bool, enabled: bool) -> Entry {
        let label = if revert { "Revert commit" } else { "Cherry pick commit" };
        let sha = c.sha.clone();
        if c.parents.len() < 2 {
            let action = if revert { Action::Revert { sha, mainline: None } } else { Action::CherryPick { sha, mainline: None } };
            return off(item(label, act(tab, action)), enabled);
        }
        let children = c
            .parents
            .iter()
            .enumerate()
            .map(|(i, parent)| {
                let summary = repo.data.commits.iter().find(|p| &p.sha == parent).map(|p| format!(" — {}", p.summary)).unwrap_or_default();
                let short: String = parent.chars().take(6).collect();
                let (sha, parent) = (sha.clone(), parent.clone());
                item(format!("Relative to parent {}: {short}{summary}", i + 1), move |app| {
                    let action = if revert { Action::Revert { sha, mainline: Some(i + 1) } } else { Action::CherryPick { sha, mainline: Some(i + 1) } };
                    let verb = if revert { "Undo" } else { "Apply" };
                    app.confirm_act(
                        tab,
                        &format!("{label} relative to parent {}?", i + 1),
                        &format!("{verb} the merge's changes compared with parent {} ({parent}).", i + 1),
                        if revert { "Revert" } else { "Cherry Pick" },
                        action,
                    );
                })
            })
            .collect();
        off(sub(label, children), enabled)
    }

    pub(crate) fn commit_menu(&self, tab: u64, repo: &RepoView, c: &Commit) -> Vec<Entry> {
        let head = repo.data.head_branch.clone().unwrap_or_else(|| "HEAD".into());
        let is_head = c.sha == repo.data.head_sha;
        let line = head_line(repo);
        let on_line = repo.data.head_branch.is_some() && line.contains(&c.sha);
        let refs: Vec<&GitRef> = repo.data.refs.iter().filter(|r| r.sha == c.sha && r.kind != RefKind::Tag).collect();
        let sha = c.sha.clone();
        let mut items = Vec::new();
        match &repo.data.bisect {
            Some(b) if b.first_bad.is_none() => {
                let (a, b2, s) = (sha.clone(), sha.clone(), sha.clone());
                items.push(sub(
                    "Bisect",
                    vec![
                        item("Mark this commit good", move |app| app.run_bisect(tab, "good", Some(a), None)),
                        item("Mark this commit bad", move |app| app.run_bisect(tab, "bad", Some(b2), None)),
                        item("Skip this commit", move |app| app.run_bisect(tab, "skip", Some(s), None)),
                    ],
                ));
            }
            None => {
                let s = sha.clone();
                items.push(item("Start bisect here (bad)", move |app| {
                    let s = s.clone();
                    app.show_dialog(super::dialogs::Dialog::prompt("Start bisect", "Known-good commit, branch, or tag", "", "Start bisect", move |app, v| app.run_bisect(tab, "start", Some(s), Some(v.text("value").trim().to_owned()))));
                }));
            }
            _ => {}
        }
        if repo.data.head_branch.is_some() && !is_head {
            items.push(item(format!("Rebase {head} onto this commit"), act(tab, Action::Rebase { onto: sha.clone(), branch: None, changes: Default::default() })));
        }
        let checkout: Vec<Entry> = refs
            .iter()
            .map(|r| {
                let action = if r.kind == RefKind::Remote { Action::CheckoutRemote { remote_branch: r.name.clone(), stash_if_needed: false, changes: gitcraft_core::actions::CheckoutChanges::Ask } } else { Action::Checkout { target: r.name.clone(), changes: gitcraft_core::actions::CheckoutChanges::Ask, branch: true } };
                let label = if r.kind == RefKind::Local && super::repo::other_worktree(repo, &r.name).is_some() { format!("Open worktree for {}", r.name) } else { r.name.clone() };
                off(item(label, act(tab, action)), !(r.kind == RefKind::Local && Some(&r.name) == repo.data.head_branch.as_ref()))
            })
            .collect();
        if !checkout.is_empty() {
            items.push(sub("Checkout branch", checkout));
        }
        items.push(item("Checkout this commit (detached HEAD)", act(tab, Action::Checkout { target: sha.clone(), changes: gitcraft_core::actions::CheckoutChanges::Ask, branch: false })));
        let s = sha.clone();
        let mut worktree = vec![item("this commit", move |app| app.add_worktree(tab, Some(s)))];
        for r in &refs {
            let s = r.sha.clone();
            worktree.push(item(r.name.clone(), move |app| app.add_worktree(tab, Some(s))));
        }
        items.push(sub("Create worktree from", worktree));
        let s = sha.clone();
        items.push(item("Create branch here", move |app| app.prompt_branch(tab, Some(s), true)));
        items.push(Entry::Separator);
        items.push(self.commit_action(tab, repo, c, false, !is_head));
        items.push(reset_menu(tab, repo, &c.sha));
        let s = sha.clone();
        items.push(off(item("Rename commit", move |app| app.reword_commit(tab, &s)), on_line));
        let (s, summary) = (sha.clone(), c.summary.clone());
        items.push(off(
            item("Create fixup commit for this commit", move |app| {
                app.confirm_act(tab, "Create fixup commit", &format!("Commit the currently staged changes as a fixup for “{summary}”?"), "Create Fixup", Action::CreateFixup { target: s.clone() })
            }),
            on_line && !repo.data.status.staged.is_empty(),
        ));
        items.push(self.commit_action(tab, repo, c, true, true));
        let (s, name) = (sha.clone(), super::patches::patch_name(&c.summary));
        items.push(item("Create patch from commit", move |app| app.save_patch(tab, vec![s], &name)));
        if repo.data.head_branch.is_some() && !repo.data.remotes.is_empty() {
            let h = head.clone();
            items.push(item(format!("Push {head} and start a pull request"), move |app| app.start_pull_request(tab, &h)));
        }
        if let Some(r) = refs.first() {
            let n = r.name.clone();
            items.push(item("Explain Branch Changes (Preview)", move |app| app.explain_branch(tab, &n)));
        }
        items.push(Entry::Separator);
        let s = sha.clone();
        items.push(item("Compare commit against working directory", move |app| {
            if let Some(r) = app.repo_mut(tab) {
                r.ui.compare_workdir = Some(s.clone());
                r.ui.multi.clear();
                r.ui.selection = Some(super::repo::Selection::Commit(s));
                r.ui.diff = None;
                r.ui.history = None;
            }
        }));
        items.push(Entry::Separator);
        items.push(sub("Edit commit history", self.history_items(tab, repo, c)));
        items.push(Entry::Separator);
        let pinned = self.pinned_ref(&repo.path);
        for r in &refs {
            let n = r.name.clone();
            items.push(item(format!("Copy branch name: {}", r.name), move |app| app.copy(&n)));
            let full = r.full.clone();
            let label = if pinned.as_deref() == Some(r.full.as_str()) { format!("Unpin {} from Left", r.name) } else { format!("Pin to Left: {}", r.name) };
            items.push(item(label, move |app| app.toggle_pinned(tab, &full)));
            let full = r.full.clone();
            items.push(item(format!("Solo: {}", r.name), move |app| app.toggle_ref_list(tab, "solo", &full)));
        }
        let s = sha.clone();
        items.push(item("Copy commit sha", move |app| app.copy(&s)));
        let message = if c.body.is_empty() { c.summary.clone() } else { format!("{}\n\n{}", c.summary, c.body) };
        items.push(item("Copy commit message", move |app| app.copy(&message)));
        items.extend(self.link_items(tab, repo, "Copy link to this commit", super::weblinks::Target::Commit(sha.clone()), None));
        items.push(Entry::Separator);
        let s = sha.clone();
        items.push(item("Create tag here", move |app| app.prompt_tag(tab, Some(s), false, String::new())));
        let s = sha.clone();
        items.push(item("Create annotated tag here", move |app| app.prompt_tag(tab, Some(s), true, String::new())));
        items
    }

    fn history_items(&self, tab: u64, repo: &RepoView, c: &Commit) -> Vec<Entry> {
        let line = head_line(repo);
        let Some(pos) = line.iter().position(|s| *s == c.sha).filter(|_| repo.data.head_branch.is_some()) else {
            return vec![off(item("Select a commit on the checked-out branch", |_| {}), false)];
        };
        let head = repo.data.head_branch.clone().unwrap_or_default();
        let short: String = c.sha.chars().take(6).collect();
        let mut items = Vec::new();
        if pos > 0 {
            let s = c.sha.clone();
            let label = short.clone();
            items.push(item(format!("Interactive Rebase {pos} children of {short}"), move |app| app.open_rebase(tab, &s, &label, false)));
        }
        let base = base_for(repo, &c.sha);
        let h = head.clone();
        items.push(item(format!("Interactive Rebase {head} from here"), move |app| app.open_rebase(tab, &base, &h, false)));
        // This commit and every newer one on the branch, composed again into new commits with AI.
        if let Some(parent) = c.parents.first().cloned() {
            let count = pos + 1;
            items.push(item(format!("Recompose {count} commit{} from here with AI", if count == 1 { "" } else { "s" }), move |app| app.recompose_commits(tab, parent.clone(), count)));
        }
        if pos + 1 < line.len() {
            let s = c.sha.clone();
            items.push(item("Squash into the commit below", move |app| app.squash_commits(tab, vec![s])));
        }
        let s = c.sha.clone();
        items.push(danger("Drop commit", move |app| app.drop_commits(tab, vec![s])));
        if pos > 0 {
            let s = c.sha.clone();
            items.push(item("Move commit up", move |app| app.move_commit(tab, &s, true)));
        }
        if pos + 1 < line.len() {
            let s = c.sha.clone();
            items.push(item("Move commit down", move |app| app.move_commit(tab, &s, false)));
        }
        items
    }

    pub(crate) fn stash_menu(&self, tab: u64, st: &Stash) -> Vec<Entry> {
        let (index, sha, message) = (st.index, st.sha.clone(), st.message.clone());
        let (s1, s2, s3, s4, s5) = (sha.clone(), sha.clone(), sha.clone(), sha.clone(), sha.clone());
        let m = message.clone();
        vec![
            item("Apply Stash", act(tab, Action::StashApply { index, expected: Some(s1), without_index: false })),
            item("Pop Stash", act(tab, Action::StashPop { index, expected: Some(s2), without_index: false })),
            item("Edit stash message", move |app| {
                app.show_dialog(super::dialogs::Dialog::prompt("Edit stash message", "Stash message", &m, "Save", move |app, v| {
                    app.act(tab, Action::StashRename { index, expected: Some(s3), message: v.text("value") })
                }))
            }),
            danger("Delete Stash", move |app| app.confirm_act(tab, "Delete this stash?", &message, "Delete", Action::StashDrop { index, expected: Some(s4) })),
            Entry::Separator,
            item("Copy stash sha", move |app| app.copy(&s5)),
        ]
    }

    /// The menu of a file in the right panel. `commit`: the commit (or stash) the file is shown from.
    pub(crate) fn file_menu(&self, tab: u64, repo: &RepoView, file: &str, staged: bool, in_commit: bool, commit: Option<&str>, storage: Option<&str>) -> Vec<Entry> {
        let f = file.to_owned();
        let mut items = Vec::new();
        if let Some(stash) = commit.and_then(|sha| repo.data.stashes.iter().find(|s| s.sha == sha)) {
            items.push(item("Apply stashed file", act(tab, Action::StashTakeFile { index: stash.index, expected: Some(stash.sha.clone()), path: f.clone() })));
            items.push(Entry::Separator);
        }
        // A file of a commit: put the working copy back to that version.
        if in_commit
            && let Some(sha) = commit.filter(|sha| !repo.data.stashes.iter().any(|s| s.sha == *sha) && *sha != gitcraft_core::git::WORKDIR)
        {
            let (f2, sha) = (f.clone(), sha.to_owned());
            let short: String = sha.chars().take(7).collect();
            items.push(item("Restore file from this commit", move |app| {
                app.confirm_act(tab, "Restore this file?", &format!("Replace {f2} in the working folder with its version from {short}? Its current changes are kept as a backup you can restore from Actions."), "Restore", Action::RestoreFiles { sha: sha.clone(), paths: vec![f2.clone()] })
            }));
            items.push(Entry::Separator);
        }
        if !in_commit {
            if staged {
                let f2 = f.clone();
                items.push(item("Unstage", move |app| app.act_quiet(tab, Action::Unstage { paths: vec![f2] }, true)));
            } else {
                let f2 = f.clone();
                items.push(item("Stage", move |app| app.act_quiet(tab, Action::Stage { paths: vec![f2] }, true)));
                items.push(item("Stash file", act(tab, Action::StashFiles { paths: vec![f.clone()], message: String::new() })));
                let f2 = f.clone();
                items.push(danger("Discard changes", move |app| {
                    app.confirm_act(tab, "Discard changes?", &format!("Discard all changes to {f2}? You can undo this."), "Discard", Action::Discard { paths: vec![f2.clone()] })
                }));
            }
            items.push(Entry::Separator);
        }
        let dir = file.rfind('/').map(|i| file[..=i].to_owned()).unwrap_or_default();
        let name = file.rsplit('/').next().unwrap_or(file);
        let ext = name.rfind('.').filter(|i| *i > 0).map(|i| name[i..].to_owned());
        let tracked = staged || in_commit || repo.data.status.unstaged.iter().any(|x| x.path == file && x.status != '?');
        let ignore = |pattern: String, untrack: Vec<String>| act(tab, Action::Ignore { pattern, untrack: if tracked { untrack } else { Vec::new() } });
        let mut ign = vec![item("Ignore this file", ignore(literal_ignore_path(file), vec![f.clone()]))];
        if !dir.is_empty() {
            ign.push(item(format!("Ignore all files in \"{dir}\""), ignore(literal_ignore_path(&dir), vec![dir.clone()])));
        }
        if let Some(ext) = &ext {
            ign.push(item(format!("Ignore all files with extension {ext}"), ignore(format!("*{ext}"), vec![f.clone()])));
        }
        items.push(sub(if tracked { "Ignore and stop tracking" } else { "Ignore" }, ign));
        if let Some(ext) = &ext {
            items.push(item(format!("Track {ext} files with Git LFS"), act(tab, Action::LfsTrack { pattern: format!("*{ext}"), lockable: false })));
        }
        if !in_commit {
            let mut lfs = vec![
                item("Lock file", act(tab, Action::LfsLock { path: f.clone() })),
                item("Unlock file", act(tab, Action::LfsUnlock { path: f.clone(), force: false })),
            ];
            if let Some(ext) = &ext {
                lfs.push(item("Track this extension and require a lock", act(tab, Action::LfsTrack { pattern: format!("*{ext}"), lockable: true })));
            }
            items.push(sub("Git LFS", lfs));
            let tool = self.settings.string("diffTool", "");
            items.push(item("Open in External Diff Tool", act(tab, Action::ExternalDiff { file: f.clone(), staged, tool })));
        }
        if repo.data.status.unstaged.iter().any(|x| x.path == file && x.status == 'U') {
            let tool = self.settings.string("mergeTool", "");
            items.push(item("Open in External Merge Tool", act(tab, Action::ExternalMerge { file: f.clone(), tool })));
        }
        items.push(Entry::Separator);
        let f2 = f.clone();
        items.push(item("Copy file path", move |app| app.copy(&f2)));
        let rev = if in_commit { storage.or(commit).map(str::to_owned) } else if staged { Some(":".to_owned()) } else { None };
        let f2 = f.clone();
        items.push(item("Copy file contents to clipboard", move |app| app.copy_file_text(tab, &f2, rev)));
        let editor = super::external_tools::editor_name(&self.settings.string("externalEditor", "vscode"));
        let f2 = f.clone();
        items.push(item(format!("Open in {editor}"), move |app| app.open_in_editor(tab, Some(&f2))));
        let f2 = f.clone();
        items.push(item("Open file in default program", move |app| app.open_default(tab, &f2)));
        let f2 = f.clone();
        items.push(item("Open in File Manager", move |app| app.reveal_file(tab, &f2)));
        items.push(Entry::Separator);
        let f2 = f.clone();
        items.push(item("File History", move |app| app.open_history(tab, &f2, super::repo::HistoryMode::History)));
        if let Some(sep) = file.rfind('/').filter(|i| *i > 0) {
            let folder = file[..sep].to_owned();
            items.push(item("Containing folder history", move |app| {
                app.set_history_filter(tab, Some(gitcraft_core::git::HistoryFilter { path: Some(folder), ..Default::default() }));
            }));
        }
        let f2 = f.clone();
        items.push(item("Blame", move |app| app.open_history(tab, &f2, super::repo::HistoryMode::Blame)));
        items
    }
}

/// A .gitignore pattern that matches exactly this path (special characters escaped).
pub fn literal_ignore_path(path: &str) -> String {
    let mut out = String::from("/");
    for ch in path.chars() {
        if matches!(ch, '\\' | '*' | '?' | '[' | ']' | '#' | '!' | ' ') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignore_patterns_are_literal() {
        assert_eq!(literal_ignore_path("src/a*.rs"), "/src/a\\*.rs");
        assert_eq!(literal_ignore_path("#my notes"), "/\\#my\\ notes");
    }

    #[test]
    fn ancestry_falls_back_to_git_when_history_is_not_loaded() {
        let d = std::env::temp_dir().join(format!("gitcraft-test-ancestry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let g = |args: &[&str]| {
            let o = gitcraft_core::git::run(&d, args);
            assert!(o.ok, "{args:?}: {}", o.message());
        };
        g(&["init", "-q", "-b", "master"]);
        g(&["config", "user.name", "Test"]);
        g(&["config", "user.email", "t@example.com"]);
        std::fs::write(d.join("a.txt"), "a\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "a"]);
        let parent = gitcraft_core::git::run(&d, &["rev-parse", "HEAD"]).stdout.trim().to_string();
        std::fs::write(d.join("b.txt"), "b\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "b"]);
        let tip = gitcraft_core::git::run(&d, &["rev-parse", "HEAD"]).stdout.trim().to_string();
        let v = RepoView { path: d.to_string_lossy().to_string(), name: String::new(), data: gitcraft_core::git::RepoData::default(), graph: Default::default(), history_cursor: None, undo: Default::default() };
        assert!(is_loaded_ancestor(&v, &parent, &tip));
        assert!(!is_loaded_ancestor(&v, &tip, &parent));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn headless_branch_drop_buttons_fit_and_scroll_in_a_compact_dialog() {
        let ctx = egui::Context::default();
        let cc = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = App::new(&cc, crate::app::StartOptions::default(), crate::settings::Store::in_memory());
        let long_ref = format!("feature/{}", "long-".repeat(30));
        let labels = [
            "Fast-forward main to feature".to_owned(),
            "Merge feature into main".to_owned(),
            "Merge main into feature".to_owned(),
            "Rebase feature onto main".to_owned(),
            format!("Start a pull request from {long_ref} into main"),
        ];
        let actions = labels.iter().map(|label| {
            let run: Later = Box::new(|app| app.dialog = Some(Dialog::info("Action selected", "Fixture")));
            (label.clone(), "Description of the action.".into(), run)
        }).collect();
        app.dialog = Some(branch_drop_dialog(&format!("{long_ref} → main"), actions));
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(330.0, 265.0));
        let render = |app: &mut App, events| {
            let mut output = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() }, |_| app.draw_dialog(&ctx));
            output.textures_delta.clear();
            output.shapes
        };
        let visible = |shapes: &[egui::epaint::ClippedShape], prefix: &str| shapes.iter().find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.text().starts_with(prefix)
                && shape.clip_rect.contains_rect(text.visual_bounding_rect())
                && screen.contains_rect(text.visual_bounding_rect())
            {
                return Some(text.visual_bounding_rect().center());
            }
            None
        });

        render(&mut app, vec![]);
        let first = render(&mut app, vec![]);
        let action = visible(&first, "Fast-forward main").expect("first action fits");
        let cancel = visible(&first, "Cancel").expect("Cancel stays visible");
        assert!(visible(&first, "Start a pull request").is_none());

        let wheel = egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, -800.0),
            phase: egui::TouchPhase::Move, source: egui::MouseWheelSource::Unknown,
            modifiers: egui::Modifiers::NONE,
        };
        render(&mut app, vec![egui::Event::PointerMoved(action), wheel]);
        for _ in 0..12 {
            render(&mut app, vec![]);
        }
        let last = render(&mut app, vec![]);
        let action = visible(&last, "Start a pull request").expect("last action fits after scrolling");
        assert_eq!(visible(&last, "Cancel"), Some(cancel));

        let click = |pressed| egui::Event::PointerButton {
            pos: action, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE,
        };
        render(&mut app, vec![egui::Event::PointerMoved(action), click(true)]);
        render(&mut app, vec![click(false)]);
        assert_eq!(app.dialog.as_ref().unwrap().title, "Action selected");
    }

    #[test]
    fn merge_descriptions_do_not_promise_a_merge_commit() {
        let action = Action::Merge { branch: "refs/heads/ff".into(), into: Some("dev".into()), changes: gitcraft_core::actions::CheckoutChanges::Ask };
        assert_eq!(drop_detail(&action), "Merge ff into dev; fast-forwards when possible.");
    }

    #[test]
    fn a_branch_behind_the_one_it_is_dropped_on_can_fast_forward_to_it() {
        let commit = |sha: &str, parents: &[&str]| Commit {
            sha: sha.into(),
            parents: parents.iter().map(|parent| (*parent).into()).collect(),
            author: String::new(),
            email: String::new(),
            time: 0,
            summary: String::new(),
            body: String::new(),
        };
        let branch = |name: &str, sha: &str| GitRef {
            name: name.into(),
            full: format!("refs/heads/{name}"),
            sha: sha.into(),
            kind: RefKind::Local,
            upstream: String::new(),
            ahead: 0,
            behind: 0,
        };
        let refs = vec![branch("ff", "a"), branch("dev", "c")];
        let v = RepoView {
            path: String::new(),
            name: String::new(),
            data: gitcraft_core::git::RepoData { commits: vec![commit("a", &[]), commit("b", &["a"]), commit("c", &["b"])], refs: refs.clone(), head_branch: Some("ff".into()), ..Default::default() },
            graph: Default::default(),
            history_cursor: None,
            undo: Default::default(),
        };
        let choices = dropped_actions(&v, &refs[0], &refs[1], Some("ff"));
        let labels: Vec<&str> = choices.iter().map(|(label, _)| label.as_str()).collect();
        assert!(labels.contains(&"Fast-forward ff to dev"), "{labels:?}");
        assert!(!labels.contains(&"Fast-forward dev to ff"), "{labels:?}");
        let other_head = dropped_actions(&v, &refs[0], &refs[1], Some("scratch"));
        assert!(other_head.iter().any(|(label, _)| label == "Merge dev into ff"));
        assert!(choices.iter().any(|(_, action)| matches!(action, Action::FastForward { branch, to } if branch == "ff" && to == "refs/heads/dev")));
        assert!(choices.iter().any(|(_, action)| matches!(action, Action::Merge { branch, into: Some(into), .. } if branch == "refs/heads/ff" && into == "dev")));
        assert!(choices.iter().any(|(_, action)| matches!(action, Action::Rebase { onto, branch: Some(branch), .. } if branch == "ff" && onto == "refs/heads/dev")));
    }
}
