//! The 50 px toolbar under the tabs: repository and branch pickers on the left, the main Git
//! buttons centred in the window, Actions and Search at the right edge.

use super::widgets::Tip;
use super::App;
use super::ctxmenu::{self, Entry, Later, item};
use super::icons::{self, Icon};
use super::repo::RepoTab;
#[cfg(test)]
use super::repo;
use super::theme::{ACCENT, GOOD, p};
use egui::{Align2, CornerRadius, FontId, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2};
use gitcraft_core::actions::Action;
use gitcraft_core::git::RefKind;

const PULL_LABELS: [(&str, &str); 4] = [("fetch", "Fetch All"), ("ff", "Pull (fast-forward if possible)"), ("ffonly", "Pull (fast-forward only)"), ("rebase", "Pull (rebase)")];

#[cfg(test)]
#[test]
fn headless_pull_busy_tracks_background_repositories() {
    let paths = vec!["/repo/".to_owned(), "/other".to_owned()];
    assert!(repo::pull_busy(false, &paths, "/repo"));
    assert!(!repo::pull_busy(false, &paths, "/unrelated"));
    assert!(repo::pull_busy(true, &[], "/repo"));
    assert!(!repo::pull_busy(false, &[], "/repo"));
}

fn set_repo_pull_mode(prefs: &mut serde_json::Value, path: &str, mode: String) {
    if !prefs.is_object() {
        *prefs = serde_json::json!({});
    }
    let entry = prefs.as_object_mut().expect("object").entry(path).or_insert_with(|| serde_json::json!({}));
    if !entry.is_object() {
        *entry = serde_json::json!({});
    }
    entry["pullMode"] = serde_json::Value::String(mode);
}

fn save_pull_default(settings: &mut crate::settings::Store, repo_override: Option<&str>, mode: String) {
    if let Some(path) = repo_override {
        let mut prefs: serde_json::Value = settings.get("repoPrefs").unwrap_or_else(|| serde_json::json!({}));
        set_repo_pull_mode(&mut prefs, path, mode);
        settings.set("repoPrefs", prefs);
    } else {
        settings.set("pullMode", mode);
    }
}

#[cfg(test)]
#[test]
fn headless_pull_defaults_survive_reopening_settings() {
    let folder = std::env::temp_dir().join(format!("gitcraft-pull-settings-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("settings.json");
    let mut settings = crate::settings::Store::open_at(path.clone());
    save_pull_default(&mut settings, None, "ffonly".into());
    save_pull_default(&mut settings, Some("/repo"), "rebase".into());
    settings.flush().unwrap();
    drop(settings);
    let reopened = crate::settings::Store::open_at(path);
    assert_eq!(reopened.string("pullMode", ""), "ffonly");
    assert_eq!(reopened.get::<serde_json::Value>("repoPrefs").unwrap()["/repo"]["pullMode"], "rebase");
    std::fs::remove_dir_all(folder).unwrap();
}

#[cfg(test)]
#[test]
fn headless_repo_pull_mode_handles_malformed_preferences() {
    use serde_json::json;

    for malformed in [json!(null), json!(false), json!(42), json!("bad"), json!([])] {
        let mut prefs = malformed.clone();
        set_repo_pull_mode(&mut prefs, "/repo", "fetch".into());
        assert_eq!(prefs, json!({"/repo": {"pullMode": "fetch"}}));

        let mut prefs = json!({"/repo": malformed, "/other": {"pullMode": "ff"}});
        set_repo_pull_mode(&mut prefs, "/repo", "rebase".into());
        assert_eq!(prefs, json!({"/repo": {"pullMode": "rebase"}, "/other": {"pullMode": "ff"}}));
    }
    let mut prefs = json!({"/repo": {"pullMode": "ff", "pullAutostash": true}, "/other": {"pullMode": "ffonly"}});
    set_repo_pull_mode(&mut prefs, "/repo", "fetch".into());
    assert_eq!(prefs, json!({"/repo": {"pullMode": "fetch", "pullAutostash": true}, "/other": {"pullMode": "ffonly"}}));
}

/// A toolbar button 52 px wide: a label over an icon, maybe a count badge, or a spinner while
/// its action runs.
fn tb(ui: &mut Ui, rect: Rect, icon: Icon, label: &str, labels: bool, tip: &str, enabled: bool, badge: usize, spinning: bool) -> egui::Response {
    let c = p();
    let resp = ui.interact(rect, ui.scope_id().with(("tb", label)), if enabled && !spinning { Sense::click() } else { Sense::hover() });
    if resp.hovered() && enabled {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, c.white(0.05));
    }
    let fade = if enabled { 1.0 } else { 0.35 };
    let show_label = labels || spinning;
    let icon_y = if show_label { rect.center().y + 7.0 } else { rect.center().y };
    if show_label {
        let ink = c.white(0.75).gamma_multiply(fade);
        let galley = super::widgets::fit(ui, label, FontId::proportional(11.0), ink, rect.width());
        ui.painter().galley(pos2(rect.center().x - galley.size().x / 2.0, rect.center().y - 10.0 - galley.size().y / 2.0), galley, ink);
    }
    let ir = Rect::from_center_size(pos2(rect.center().x, icon_y), Vec2::splat(19.0));
    if spinning {
        let t = ui.input(|i| i.time) as f32;
        ui.painter().add(egui::Shape::line(
            (0..=24).map(|k| {
                let a = t * 6.0 + k as f32 / 24.0 * std::f32::consts::TAU * 0.75;
                pos2(ir.center().x + 8.0 * a.cos(), ir.center().y + 8.0 * a.sin())
            }).collect(),
            Stroke::new(2.0, c.white(0.9).gamma_multiply(fade)),
        ));
        ui.ctx().request_repaint();
    } else {
        icons::paint(ui, ir, icon, 19.0, c.white(0.9).gamma_multiply(fade));
    }
    if badge > 0 {
        let text = badge.to_string();
        let w = (text.len() as f32 * 6.0 + 6.0).max(15.0);
        let br = Rect::from_min_size(pos2(ir.right() + 11.0 - w, ir.top() - 5.0), vec2(w, 14.0));
        ui.painter().rect_filled(br, CornerRadius::same(7), ACCENT);
        ui.painter().text(br.center(), Align2::CENTER_CENTER, text, super::fonts::bold(9.5), c.white(1.0));
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled && !spinning, label));
    if tip.is_empty() { resp.tip(label) } else { resp.tip(tip) }
}

impl App {
    pub(crate) fn toolbar(&self, ui: &mut Ui, tab: u64, r: Option<&RepoTab>, later: &mut Vec<Later>) {
        let c = p();
        let locale = self.locale();
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, CornerRadius::ZERO, c.toolbar);
        ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, c.black(0.3)));
        let v = r.and_then(|r| r.view.as_ref());
        let path = v.map(|v| v.path.clone());
        let labels = self.settings.flag("toolbarLabels", true);
        let pull_mode = path.as_deref().map_or_else(|| self.settings.string("pullMode", "ff"), |p| self.pull_mode(p));
        let has_repo_mode = path.as_deref().and_then(|p| self.repo_pref(p, "pullMode")).is_some();
        let detached = v.is_some_and(|v| v.data.head_branch.is_none());
        let head = v.and_then(|v| v.data.refs.iter().find(|r| r.kind == RefKind::Local && Some(&r.name) == v.data.head_branch.as_ref()));
        let has_changes = v.is_some_and(|v| !v.data.status.unstaged.is_empty() || !v.data.status.staged.is_empty());
        let spin = r.and_then(|r| r.spinning);
        let busy = r.is_some_and(|r| self.repo_pull_busy(&r.path));
        // The three parts never overlap. When the window is narrow the pickers give up room
        // first, the branch picker before the repository picker; the button group never slides
        // left over them and Actions and Search never slide left over the group.
        let group_w = 52.0 * 7.0 + 16.0 + 14.0 + 1.0 + 8.0 + 52.0;
        let right_w = 56.0 + 52.0;
        let pickers = (rect.width() - group_w - right_w - 20.0 - 18.0).max(0.0);
        let repo_w = (pickers - 130.0).clamp(80.0, 150.0);
        let branch_w = (pickers - repo_w).clamp(60.0, 130.0);
        // Left: repository and branch pickers.
        let repo_rect = Rect::from_min_size(rect.min, vec2(repo_w, rect.height()));
        let resp = ui.interact(repo_rect, ui.scope_id().with("repo_picker"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(repo_rect, CornerRadius::ZERO, c.white(0.05));
        }
        ui.painter().text(pos2(repo_rect.left() + 17.0, rect.center().y - 9.0), Align2::LEFT_CENTER, "repository", FontId::proportional(11.0), c.white(0.6));
        let name = v.map_or_else(|| "—".to_owned(), |v| v.name.clone());
        let g = super::widgets::fit(ui, &name, super::fonts::bold(15.0), c.white(0.9), repo_rect.width() - 25.0);
        ui.painter().galley(pos2(repo_rect.left() + 17.0, rect.center().y + 1.0), g, c.white(0.9));
        egui::Popup::menu(&resp).show(|ui| {
            let mut entries = vec![Entry::Heading("Open Repositories".into())];
            for t in self.tabs.iter().filter(|t| t.path().is_some()) {
                let id = t.id;
                entries.push(item(t.title.clone(), move |app| app.activate(id)));
            }
            entries.push(Entry::Separator);
            entries.push(Entry::Heading("Recent".into()));
            for rp in self.settings.recents().into_iter().take(8) {
                entries.push(item(rp.clone(), move |app| app.open_repo(&rp)));
            }
            entries.push(Entry::Separator);
            entries.push(item("Open Repo...", |app| app.open_repo_dialog()));
            entries.push(item("Clone Repo...", |app| app.clone_dialog("")));
            entries.push(item("Init Repo...", |app| app.init_dialog()));
            ctxmenu::show(ui, entries, later);
        });
        let chev = Rect::from_min_size(pos2(repo_rect.right() + 2.0, rect.center().y - 8.0), Vec2::splat(16.0));
        icons::paint(ui, chev, Icon::ChevronRight, 16.0, c.white(0.5));
        let branch_rect = Rect::from_min_size(pos2(chev.right() + 2.0, rect.top()), vec2(branch_w, rect.height()));
        let resp = ui.interact(branch_rect, ui.scope_id().with("branch_picker"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(branch_rect, CornerRadius::ZERO, c.white(0.05));
        }
        ui.painter().text(pos2(branch_rect.left() + 8.0, rect.center().y - 9.0), Align2::LEFT_CENTER, "branch", FontId::proportional(11.0), c.white(0.6));
        let bname = v.map_or_else(|| "—".to_owned(), |v| v.data.head_branch.clone().unwrap_or_else(|| format!("{} (detached)", v.data.head_sha.chars().take(6).collect::<String>())));
        let g = super::widgets::fit(ui, &bname, FontId::proportional(15.0), c.white(0.9), branch_rect.width() - 34.0);
        let gw = g.size().x;
        ui.painter().galley(pos2(branch_rect.left() + 8.0, rect.center().y + 1.0), g, c.white(0.9));
        icons::paint(ui, Rect::from_min_size(pos2(branch_rect.left() + 12.0 + gw, rect.center().y + 3.0), Vec2::splat(14.0)), Icon::ChevronDown, 14.0, c.white(0.9));
        // The checked-out branch's pull request was merged: say so beside its name.
        if let Some(head) = v.and_then(|v| v.data.head_branch.as_deref()) {
            let pill = Rect::from_min_size(pos2(branch_rect.right() + 4.0, rect.center().y - 10.0), vec2(96.0, 20.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(pill).layout(egui::Layout::left_to_right(egui::Align::Center)));
            self.merged_pill(&mut child, tab, Some(head), None, later);
        }
        if let Some(v) = v {
            egui::Popup::menu(&resp).show(|ui| {
                let mut entries = vec![Entry::Heading("Checkout a local branch".into())];
                for b in v.data.refs.iter().filter(|r| r.kind == RefKind::Local) {
                    let is_head = Some(&b.name) == v.data.head_branch.as_ref();
                    let name = b.name.clone();
                    let label = if is_head {
                        format!("✓ {name}")
                    } else if super::repo::other_worktree(v, &name).is_some() {
                        format!("{name} (open worktree)")
                    } else {
                        name.clone()
                    };
                    entries.push(item(label, move |app| {
                        if !is_head {
                            app.act(tab, Action::Checkout { target: name, changes: gitcraft_core::actions::CheckoutChanges::Ask, branch: true });
                        }
                    }));
                }
                entries.push(Entry::Separator);
                entries.push(item(format!("Create branch...    {}+B", super::menu::MOD), move |app| app.prompt_branch(tab, None, true)));
                ctxmenu::show(ui, entries, later);
            });
        }
        // Centre: Undo, Redo, Pull, Push, Branch, Stash, Pop | Terminal.
        let picker_column = (rect.width() - gitcraft_core::metrics::TOOLBAR_CENTER_WIDTH as f32 - right_w)
            .clamp(300.0, gitcraft_core::metrics::TOOLBAR_LEFT_MAX as f32);
        let ideal = (rect.center().x - group_w / 2.0).max(rect.left() + picker_column) + 20.0;
        let min_x = branch_rect.right() + 20.0;
        let max_x = rect.right() - right_w - group_w;
        let mut x = ideal.max(min_x).min(max_x.max(min_x));
        let slot = |x: f32, w: f32| Rect::from_min_size(pos2(x, rect.top()), vec2(w, rect.height()));
        // Actions and Search stay at the right edge; where the window is too narrow for every
        // button, the ones that would run under them are cut off (their commands stay in the
        // menus and on their keys).
        let limit = rect.right() - right_w;
        let full_clip = ui.clip_rect();
        ui.set_clip_rect(full_clip.intersect(Rect::from_min_max(full_clip.min, pos2(limit, full_clip.max.y))));
        let undo = v.and_then(|v| v.undo.undo.clone());
        let redo = v.and_then(|v| v.undo.redo.clone());
        if tb(ui, slot(x, 52.0), Icon::Undo2, locale.text("Undo"), labels, &undo.as_ref().map_or("Nothing to undo".into(), |u| format!("Undo {u}")), undo.is_some(), 0, false).clicked() {
            later.push(Box::new(move |app| app.act(tab, Action::Undo)));
        }
        x += 52.0;
        if tb(ui, slot(x, 52.0), Icon::Redo2, locale.text("Redo"), labels, &redo.as_ref().map_or("Nothing to redo".into(), |u| format!("Redo {u}")), redo.is_some(), 0, false).clicked() {
            later.push(Box::new(move |app| app.act(tab, Action::Redo)));
        }
        x += 52.0;
        let pull_tip = locale.text(if detached && pull_mode != "fetch" { "Pull is not available when in a detached HEAD state" } else { PULL_LABELS.iter().find(|(m, _)| *m == pull_mode).map_or("Pull", |(_, l)| l) });
        let pulling = matches!(spin, Some("pull" | "fetch"));
        let pull_label = locale.text(if spin == Some("fetch") { "Fetch" } else { "Pull" });
        if tb(ui, slot(x, 52.0), Icon::ArrowDownToLine, pull_label, labels, pull_tip, !busy && v.is_some() && (!detached || pull_mode == "fetch"), head.map_or(0, |h| h.behind), pulling).clicked() {
            let mode = pull_mode.clone();
            later.push(Box::new(move |app| app.act(tab, if mode == "fetch" { Action::Fetch } else { Action::Pull { mode, autostash: None } })));
        }
        x += 52.0;
        let chev = Rect::from_min_size(pos2(x, rect.top()), vec2(14.0, rect.height()));
        let cresp = ui.interact(chev, ui.scope_id().with("pull_options"), if busy { Sense::hover() } else { Sense::click() });
        if cresp.hovered() && !busy {
            ui.painter().rect_filled(chev, CornerRadius::ZERO, c.white(0.05));
        }
        icons::paint(ui, Rect::from_min_size(pos2(chev.left() + 0.5, rect.top() + 13.0), Vec2::splat(13.0)), Icon::ChevronDown, 13.0, c.white(0.9).gamma_multiply(if busy { 0.35 } else { 1.0 }));
        let cresp = cresp.tip(locale.text("Pull options"));
        egui::Popup::menu(&cresp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            let width = 240.0;
            ui.set_width(width);
            ui.add(egui::Label::new(egui::RichText::new(locale.text("Select the default pull/fetch operation to execute when clicking this button")).size(13.0).color(c.white(0.6))).wrap());
            ui.add_space(4.0);
            for &(m, label) in &PULL_LABELS {
                let label = locale.text(label);
                let selected = pull_mode == m;
                egui::Frame::new().fill(if selected { GOOD.gamma_multiply(0.30) } else { egui::Color32::TRANSPARENT }).show(ui, |ui| {
                    ui.set_min_width(width);
                    ui.add_enabled_ui(!busy, |ui| ui.horizontal(|ui| {
                        let radio = ui.radio(selected, "").on_hover_cursor(egui::CursorIcon::PointingHand).tip(format!("{}: {label}", locale.text("Set as default")));
                        if radio.clicked() {
                            let (m, path) = (m.to_owned(), path.clone());
                            later.push(Box::new(move |app| {
                                save_pull_default(&mut app.settings, path.as_deref().filter(|_| has_repo_mode), m);
                            }));
                        }
                        if ui.add_enabled(v.is_some() && (!detached || m == "fetch"), egui::Button::new(egui::RichText::new(label).size(13.0)).right_text("").frame_when_inactive(false).min_size(vec2(ui.available_width(), 0.0))).clicked() {
                            let mode = m.to_owned();
                            later.push(Box::new(move |app| app.act(tab, if mode == "fetch" { Action::Fetch } else { Action::Pull { mode, autostash: None } })));
                            ui.close();
                        }
                    }));
                });
            }
        });
        x += 16.0;
        let pushing = spin == Some("push");
        if tb(ui, slot(x, 52.0), Icon::ArrowUpFromLine, locale.text("Push"), labels, if detached { "Push is not available when in a detached HEAD state" } else { "Push" }, v.is_some() && !detached, head.map_or(0, |h| h.ahead), pushing).clicked() {
            later.push(Box::new(move |app| app.act(tab, Action::Push { force: false, lease: None, review: None })));
        }
        x += 52.0;
        if tb(ui, slot(x, 52.0), Icon::GitBranch, locale.text("Branch"), labels, &format!("Create branch ({}+B)", super::menu::MOD), v.is_some(), 0, false).clicked() {
            later.push(Box::new(move |app| app.prompt_branch(tab, None, true)));
        }
        x += 52.0;
        let stashing = spin == Some("stash");
        if tb(ui, slot(x, 52.0), Icon::Download, locale.text("Stash"), labels, "Stash changes", has_changes || stashing, 0, stashing).clicked() {
            later.push(Box::new(move |app| app.act(tab, Action::Stash { message: String::new() })));
        }
        x += 52.0;
        if v.is_some() {
            let chev = Rect::from_min_size(pos2(x, rect.top()), vec2(14.0, rect.height()));
            let sresp = ui.interact(chev, ui.scope_id().with("stash_options"), Sense::click());
            icons::paint(ui, chev, Icon::ChevronDown, 12.0, if sresp.hovered() { c.fg } else { c.fg3 });
            let untracked = v.is_some_and(|v| v.data.status.unstaged.iter().any(|f| f.status == '?'));
            let sresp = sresp.tip(locale.text("Stash options"));
            egui::Popup::menu(&sresp).show(|ui| {
                let entries = vec![
                    ctxmenu::off(item("Stash with a title", move |app| app.prompt_stash(tab, "stash")), has_changes),
                    ctxmenu::off(item("Stash and keep staged changes", move |app| app.prompt_stash(tab, "keep")), has_changes),
                    ctxmenu::off(item("Stash untracked files only", move |app| app.prompt_stash(tab, "untracked")), untracked),
                ];
                ctxmenu::show(ui, entries, later);
            });
            x += 14.0;
        }
        let popping = spin == Some("pop");
        if tb(ui, slot(x, 52.0), Icon::Upload, locale.text("Pop"), labels, "Pop the latest stash", v.is_some_and(|v| !v.data.stashes.is_empty()) || popping, 0, popping).clicked() {
            later.push(Box::new(move |app| app.act(tab, Action::StashPop { index: 0, expected: None, without_index: false })));
        }
        x += 52.0 + 4.0;
        ui.painter().rect_filled(Rect::from_center_size(pos2(x, rect.center().y), vec2(1.0, 34.0)), CornerRadius::ZERO, c.white(0.2));
        x += 5.0;
        if tb(ui, slot(x, 52.0), Icon::SquareTerminal, locale.text("Terminal"), labels, "Toggle the terminal panel (Ctrl+`)", v.is_some(), 0, false).clicked() {
            later.push(Box::new(|app| app.settings.toggle("showTerminal", false)));
        }
        ui.set_clip_rect(full_clip);
        // Right: Actions and Search, always at the right edge.
        let actions = slot(limit, 56.0);
        let search = slot(actions.right(), 52.0);
        let aresp = tb(ui, actions, Icon::Zap, "Actions", labels, "", v.is_some(), 0, false);
        if v.is_some() {
            egui::Popup::menu(&aresp).show(|ui| ctxmenu::show(ui, self.actions_menu(tab), later));
        }
        if tb(ui, search, Icon::Search, "Search", labels, &format!("Search commits ({}+F)", super::menu::MOD), v.is_some(), 0, false).clicked() {
            later.push(Box::new(move |app| app.open_search(tab)));
        }
    }

    fn actions_menu(&self, tab: u64) -> Vec<Entry> {
        vec![
            item(format!("Fetch all remotes    {}+L", super::menu::MOD), ctxmenu::act(tab, Action::Fetch)),
            item("Fetch more history…", move |app| app.fetch_more_history(tab)),
            item("Open in File Manager", move |app| app.run(&app.ctx.clone(), "repo.file_manager")),
            item("Open External Terminal", move |app| app.open_terminal(tab)),
            Entry::Separator,
            item("Add remote...", move |app| app.add_remote(tab)),
            item("Repository maintenance…", move |app| app.repository_maintenance(tab)),
            item("Compare commit ranges…", move |app| app.compare_ranges(tab)),
            item("Recover commits from reflog…", move |app| app.reflog_dialog(tab)),
            item("Recover discarded changes…", move |app| app.recover_discarded(tab)),
            item("Activity log", |app| app.log_dialog()),
            Entry::Separator,
            item("Initialize / edit Gitflow...", move |app| app.init_gitflow(tab)),
            item("Add Submodule...", move |app| app.add_submodule(tab)),
            item("Add a new worktree...", move |app| app.add_worktree(tab, None)),
            item("Clean Up Local Branches...", move |app| app.cleanup_dialog(tab)),
            item("Sparse Checkout...", move |app| app.sparse_dialog(tab)),
            Entry::Separator,
            item("Git LFS: enable for this repository", ctxmenu::act(tab, Action::LfsInstall)),
            item("Git LFS: track a pattern...", move |app| app.lfs_track_prompt(tab)),
            item("Git LFS: pull LFS files", ctxmenu::act(tab, Action::LfsPull)),
        ]
    }

    /// Search Commits: the search box is over the graph, so a file or history shown in the
    /// centre makes way for it, unless the file has edits not saved yet.
    pub(crate) fn open_search(&mut self, tab: u64) {
        let Some(r) = self.repo_mut(tab) else { return };
        if r.diff_ui.editing && r.diff_ui.draft.is_some() {
            return self.toast(false, "Save or cancel the changes to this file to search commits.");
        }
        r.ui.diff = None;
        r.ui.history = None;
        r.ui.conflict = None;
        // A pull request or issue in the centre makes way too: the search is over the graph.
        r.ui.pr = None;
        r.ui.issue = None;
        r.ui.search_open = true;
        r.ui.focus_search = true;
    }
}
