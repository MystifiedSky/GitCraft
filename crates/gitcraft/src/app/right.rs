//! The right panel: staging and committing (or stashing, or saving a patch) for the working
//! changes, and the details of a selected commit, a comparison of two commits, or what can be
//! done with several picked commits.

use super::widgets::Tip;
use super::App;
use super::ctxmenu::{self, Later};
use super::icons::{self, Icon};
use super::repo::{DiffTarget, Draft, Loaded, RepoTab, Selection};
use super::theme::{ACCENT, AI, BAD, GOOD, p};
use super::widgets::{self, Kind, MODIFIED, REMOVED, SELECTED_BRANCH};
use egui::{Align2, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use gitcraft_core::actions::Action;
use gitcraft_core::git::{DiffSource, FileChange, WORKDIR, WorkStatus};
use std::collections::BTreeMap;

mod spelling;

/// The working-changes panel's own state.
#[derive(Default)]
pub struct WorkUi {
    pub tab: WorkTab,
    pub options_open: bool,
    pub unstaged_closed: bool,
    pub staged_closed: bool,
    pub stash_message: String,
    pub skip_hooks: Option<bool>,
    pub sign_off: bool,
    pub coauthors: String,
    /// The commit running now: the draft and co-authors it was made from, and whether to push
    /// once it succeeds.
    pub submitted: Option<Submitted>,
    /// Folders closed in the tree view.
    pub closed: std::collections::HashSet<String>,
    pub template_applied: bool,
    pub commit_ai_error: Option<String>,
    summary_spelling: spelling::State,
    description_spelling: spelling::State,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum WorkTab {
    #[default]
    Commit,
    Stash,
    Patch,
}

/// Files changed in the index and the working folder, counted once.
pub fn working_changes(s: &WorkStatus) -> (usize, usize, usize, usize) {
    let mut files: BTreeMap<&str, &FileChange> = s.staged.iter().map(|f| (f.path.as_str(), f)).collect();
    for f in &s.unstaged {
        let staged = files.get(f.path.as_str()).copied();
        files.insert(f.path.as_str(), if staged.is_some_and(|x| x.status == 'A' || x.status == 'R') { staged.expect("checked") } else { f });
    }
    let total = files.len();
    let added = files.values().filter(|f| f.status == 'A' || f.status == '?').count();
    let deleted = files.values().filter(|f| f.status == 'D').count();
    (total, added, deleted, total - added - deleted)
}

/// Add Co-authored-by trailers for each `Name <email>` in the comma-separated list.
/// What a commit was made from, kept until its result arrives.
#[derive(Clone, Default)]
pub struct Submitted {
    pub draft: super::repo::Draft,
    pub coauthors: String,
    pub push: bool,
}

pub fn with_coauthors(description: &str, list: &str) -> String {
    let people: Vec<&str> = list.split(',').map(str::trim).filter(|p| {
        let Some((name, rest)) = p.split_once('<') else { return false };
        let Some(email) = rest.strip_suffix('>') else { return false };
        !name.trim().is_empty() && email.contains('@') && !email.contains(char::is_whitespace) && !email.contains(['<', '>'])
    }).collect();
    if people.is_empty() {
        return description.to_owned();
    }
    let trailers: Vec<String> = people.iter().map(|p| format!("Co-authored-by: {p}")).collect();
    if description.trim().is_empty() { trailers.join("\n") } else { format!("{}\n\n{}", description.trim_end(), trailers.join("\n")) }
}

struct TreeNode<'a> {
    name: String,
    path: String,
    dirs: BTreeMap<String, TreeNode<'a>>,
    files: Vec<&'a FileChange>,
    counts: [usize; 3],
}

fn build_tree<'a>(files: &[&'a FileChange]) -> TreeNode<'a> {
    let mut root = TreeNode { name: String::new(), path: String::new(), dirs: BTreeMap::new(), files: Vec::new(), counts: [0; 3] };
    for f in files {
        let parts: Vec<&str> = f.path.split('/').collect();
        let mut node = &mut root;
        for part in &parts[..parts.len() - 1] {
            let path = if node.path.is_empty() { part.to_string() } else { format!("{}/{part}", node.path) };
            node = node.dirs.entry(part.to_string()).or_insert_with(|| TreeNode { name: part.to_string(), path, dirs: BTreeMap::new(), files: Vec::new(), counts: [0; 3] });
        }
        node.files.push(f);
    }
    fn rollup(n: &mut TreeNode) {
        for f in &n.files {
            match f.status {
                'A' | '?' => n.counts[1] += 1,
                'D' => n.counts[2] += 1,
                _ => n.counts[0] += 1,
            }
        }
        let mut add = [0; 3];
        for d in n.dirs.values_mut() {
            rollup(d);
            for i in 0..3 {
                add[i] += d.counts[i];
            }
        }
        for i in 0..3 {
            n.counts[i] += add[i];
        }
    }
    rollup(&mut root);
    root
}

/// What a file row does.
pub struct FileRows<'a> {
    pub files: &'a [FileChange],
    pub active: Option<&'a str>,
    /// The hover button: (label, kind).
    pub button: Option<(&'a str, Kind)>,
}

pub enum FileEvent {
    Open(FileChange),
    Button(FileChange),
}

impl App {
    /// Draw a file list in path or tree view; returns what was clicked.
    fn file_list(&self, ui: &mut Ui, id: egui::Id, rows: FileRows, closed: &mut std::collections::HashSet<String>, menu: &dyn Fn(&FileChange) -> Vec<ctxmenu::Entry>, later: &mut Vec<Later>) -> Option<FileEvent> {
        let c = p();
        let tree = self.settings.string("fileView", "tree") == "tree";
        let by_status = self.settings.string("fileSort", "path") == "status";
        let mut sorted: Vec<&FileChange> = rows.files.iter().collect();
        sorted.sort_by(|a, b| if by_status { a.status.cmp(&b.status).then(a.path.cmp(&b.path)) } else { a.path.cmp(&b.path) });
        let mut event = None;
        let file_row = |ui: &mut Ui, f: &FileChange, indent: usize, label: &str, event: &mut Option<FileEvent>, later: &mut Vec<Later>| {
            let w = ui.available_width();
            let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
            if rows.active == Some(f.path.as_str()) {
                ui.painter().rect_filled(rect, CornerRadius::ZERO, c.selrow);
            } else if resp.hovered() {
                ui.painter().rect_filled(rect, CornerRadius::ZERO, c.hoverrow);
            }
            let x = rect.left() + 12.0 + indent as f32 * 14.0;
            widgets::status_mark(ui, Rect::from_min_size(pos2(x, rect.center().y - 7.0), Vec2::splat(14.0)), f.status);
            let mut right = rect.right() - 8.0;
            let mut button_hit = false;
            if let Some((label, kind)) = rows.button
                && ui.rect_contains_pointer(rect)
            {
                let galley = ui.painter().layout_no_wrap(label.to_owned(), FontId::proportional(11.0), c.fg);
                let bw = galley.size().x + 12.0;
                let br = Rect::from_min_size(pos2(right - bw, rect.center().y - 10.0), vec2(bw, 20.0));
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(br));
                if widgets::button(&mut child, kind, None, label, 20.0, true).clicked() {
                    button_hit = true;
                    *event = Some(FileEvent::Button(f.clone()));
                }
                right -= bw + 6.0;
            }
            let g = widgets::fit(ui, label, FontId::proportional(12.5), c.white(0.85), right - x - 20.0);
            ui.painter().galley(pos2(x + 20.0, rect.center().y - g.size().y / 2.0), g, c.fg);
            let resp = resp.tip(&f.path);
            if resp.clicked() && !button_hit {
                *event = Some(FileEvent::Open(f.clone()));
            }
            let f2 = f.clone();
            ctxmenu::attach(&resp, later, || menu(&f2));
        };
        if !tree {
            for f in &sorted {
                file_row(ui, f, 0, &f.path, &mut event, later);
            }
            return event;
        }
        let root = build_tree(&sorted);
        let mut all_dirs = Vec::new();
        fn collect(n: &TreeNode, out: &mut Vec<String>) {
            for d in n.dirs.values() {
                out.push(d.path.clone());
                collect(d, out);
            }
        }
        collect(&root, &mut all_dirs);
        if !all_dirs.is_empty() {
            let any_open = all_dirs.iter().any(|d| !closed.contains(d));
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                let resp = ui.add(egui::Label::new(egui::RichText::new(if any_open { "Collapse All" } else { "Expand All" }).size(11.5).color(c.fg2)).sense(Sense::click()));
                if resp.clicked() {
                    if any_open {
                        closed.extend(all_dirs.iter().cloned());
                    } else {
                        closed.clear();
                    }
                }
            });
        }
        let mut stack: Vec<(&TreeNode, usize)> = vec![(&root, 0)];
        // Depth-first: folders first (by name), then files.
        fn walk<'a>(n: &'a TreeNode<'a>, depth: usize, out: &mut Vec<(Option<&'a TreeNode<'a>>, Option<&'a FileChange>, usize)>, closed: &std::collections::HashSet<String>) {
            for d in n.dirs.values() {
                out.push((Some(d), None, depth));
                if !closed.contains(&d.path) {
                    walk(d, depth + 1, out, closed);
                }
            }
            for f in &n.files {
                out.push((None, Some(f), depth));
            }
        }
        stack.clear();
        let mut flat = Vec::new();
        walk(&root, 0, &mut flat, closed);
        let mut toggle: Option<String> = None;
        for (dir, file, depth) in flat {
            if let Some(d) = dir {
                let w = ui.available_width();
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(rect, CornerRadius::ZERO, c.hoverrow);
                }
                let x = rect.left() + 10.0 + depth as f32 * 14.0;
                let is_closed = closed.contains(&d.path);
                widgets::chevron(ui, Rect::from_min_size(pos2(x, rect.center().y - 6.0), Vec2::splat(12.0)), !is_closed, 12.0, c.fg2);
                icons::paint(ui, Rect::from_min_size(pos2(x + 18.0, rect.center().y - 6.5), Vec2::splat(13.0)), Icon::Folder, 13.0, c.fg2);
                let g = ui.painter().layout_no_wrap(d.name.clone(), FontId::proportional(12.5), c.fg2);
                let gw = g.size().x;
                ui.painter().galley(pos2(x + 37.0, rect.center().y - g.size().y / 2.0), g, c.fg2);
                if is_closed {
                    widgets::counts(ui, ui.painter(), x + 37.0 + gw + 6.0, rect.center().y, d.counts, 11.5);
                }
                if resp.clicked() {
                    toggle = Some(d.path.clone());
                }
            } else if let Some(f) = file {
                let name = f.path.rsplit('/').next().unwrap_or(&f.path).to_owned();
                file_row(ui, f, depth, &name, &mut event, later);
            }
        }
        if let Some(t) = toggle
            && !closed.remove(&t)
        {
            closed.insert(t);
        }
        let _ = id;
        event
    }

    pub(crate) fn right_panel(&self, ui: &mut Ui, tab: u64, r: &mut RepoTab, later: &mut Vec<Later>) {
        let c = p();
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, CornerRadius::ZERO, c.panel);
        let Some(v) = r.view.as_ref() else { return };
        let (total, ..) = working_changes(&v.data.status);
        let inspecting = matches!(r.ui.selection(), Selection::Commit(_)) || r.ui.multi.len() > 1 || r.ui.compare_workdir.is_some();
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        if inspecting && total > 0 {
            let w = ui.available_width();
            let (bar, _) = ui.allocate_exact_size(vec2(w, 36.0), Sense::hover());
            ui.painter().rect_filled(bar, CornerRadius::ZERO, ACCENT.gamma_multiply(0.4));
            let text = format!("{total} file change{} in working directory", if total == 1 { "" } else { "s" });
            let g = ui.painter().layout_no_wrap(text, FontId::proportional(12.0), c.white(1.0));
            let bw = 100.0;
            let x0 = bar.center().x - (g.size().x + 8.0 + bw) / 2.0;
            ui.painter().galley(pos2(x0, bar.center().y - g.size().y / 2.0), g.clone(), c.white(1.0));
            let br = Rect::from_min_size(pos2(x0 + g.size().x + 8.0, bar.center().y - 10.0), vec2(bw, 20.0));
            let resp = ui.interact(br, ui.scope_id().with("view_changes"), Sense::click());
            ui.painter().rect(br, CornerRadius::same(2), if resp.hovered() { c.white(0.15) } else { egui::Color32::TRANSPARENT }, Stroke::new(1.0, c.white(0.4)), StrokeKind::Inside);
            ui.painter().text(br.center(), Align2::CENTER_CENTER, "View Changes", FontId::proportional(12.0), c.white(1.0));
            if resp.clicked() {
                r.ui.select(Selection::Wip);
                r.ui.conflict = None;
                r.ui.pr = None;
                r.ui.issue = None;
            }
        }
        let bottom = self.settings.flag("rightPanelBottom", false);
        let tip = self.locale().text(if bottom { "Move to right" } else { "Move to bottom" });
        let button = Rect::from_min_size(pos2(rect.right() - 30.0, ui.cursor().top() + 3.0), Vec2::splat(22.0));
        let response = ui.interact(button, ui.scope_id().with("move_right_panel"), Sense::click());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tip));
        let ink = if response.hovered() { c.white(1.0) } else { c.fg2 };
        ui.painter().rect(button, CornerRadius::same(3), c.white(if response.hovered() { 0.15 } else { 0.05 }), Stroke::new(1.0, c.white(0.4)), StrokeKind::Inside);
        icons::paint(ui, button, if bottom { Icon::ArrowBigRight } else { Icon::ArrowBigDown }, 13.0, ink);
        if response.tip(tip).clicked() {
            later.push(Box::new(move |app| app.settings.set("rightPanelBottom", !bottom)));
        }
        if let Some(from) = r.ui.compare_workdir.clone() {
            return self.compare_panel(ui, tab, r, &from, WORKDIR, later);
        }
        if r.ui.multi.len() == 2 {
            let pos = |x: &str| r.index.get(x).copied().unwrap_or(0);
            let mut pair = r.ui.multi.clone();
            pair.sort_by_key(|x| std::cmp::Reverse(pos(x)));
            return self.compare_panel(ui, tab, r, &pair[0], &pair[1], later);
        }
        if r.ui.multi.len() > 2 {
            // A run of consecutive commits: their combined changes below the actions for the selection.
            if let Some((from, to)) = consecutive_range(r) {
                let full = ui.available_rect_before_wrap();
                let selection_height = 48.0 + 30.0 * r.ui.multi.len() as f32;
                let top_h = selection_height.min((full.height() * 0.4).clamp(150.0, 320.0)).min(full.height());
                let mut top = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(full.min, vec2(full.width(), top_h))).layout(egui::Layout::top_down(egui::Align::Min)));
                top.set_clip_rect(Rect::from_min_size(full.min, vec2(full.width(), top_h)));
                self.multi_panel(&mut top, tab, r, true, later);
                let mut bottom = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(pos2(full.left(), full.top() + top_h), full.max)).layout(egui::Layout::top_down(egui::Align::Min)));
                return self.compare_panel(&mut bottom, tab, r, &from, &to, later);
            }
            return self.multi_panel(ui, tab, r, false, later);
        }
        match r.ui.selection() {
            Selection::Commit(sha) => self.commit_details(ui, tab, r, &sha, later),
            Selection::Wip => self.work_panel(ui, tab, r, later),
        }
    }

    fn panel_header(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
        let c = p();
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
        ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, c.black(0.2)));
        let content = Rect::from_min_max(pos2(rect.left() + 8.0, rect.top()), pos2((rect.right() - 38.0).max(rect.left() + 8.0), rect.bottom()));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(content).layout(egui::Layout::left_to_right(egui::Align::Center)));
        child.spacing_mut().item_spacing.x = 8.0;
        add(&mut child);
    }

    fn ai_button(ui: &mut Ui, tip: &str) -> egui::Response {
        let c = p();
        let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
        ui.painter().rect(rect, CornerRadius::same(3), AI.gamma_multiply(if resp.hovered() { 0.25 } else { 0.15 }), Stroke::new(1.0, AI.gamma_multiply(0.8)), StrokeKind::Inside);
        icons::paint(ui, rect, Icon::Sparkles, 13.0, c.white(0.8));
        resp.tip(tip)
    }

    fn recompose_button(ui: &mut Ui, tab: u64, base: String, count: usize, later: &mut Vec<Later>) {
        let button = Self::ai_button(ui, "Recompose with AI");
        button.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Recompose with AI"));
        if button.clicked() {
            later.push(Box::new(move |app| app.recompose_commits(tab, base, count)));
        }
    }

    /// Path / Tree switch and the sort button.
    fn view_switch(&self, ui: &mut Ui, later: &mut Vec<Later>) {
        let c = p();
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(w, 38.0), Sense::hover());
        let by_status = self.settings.string("fileSort", "path") == "status";
        let sort = Rect::from_min_size(pos2(rect.left() + 8.0, rect.center().y - 8.0), Vec2::splat(16.0));
        let resp = ui.interact(sort, ui.scope_id().with("sort"), Sense::click());
        icons::paint(ui, sort, Icon::ArrowDownAZ, 16.0, if resp.hovered() { c.white(1.0) } else { c.white(0.75) });
        if resp.tip(if by_status { "Sorted by status (click to sort by path)" } else { "Sorted by path (click to sort by status)" }).clicked() {
            later.push(Box::new(move |app| app.settings.set("fileSort", if by_status { "path" } else { "status" })));
        }
        let tree = self.settings.string("fileView", "tree") == "tree";
        let sw = Rect::from_center_size(rect.center(), vec2(150.0, 24.0));
        ui.painter().rect_stroke(sw, CornerRadius::same(3), Stroke::new(1.0, c.white(0.1)), StrokeKind::Inside);
        for (i, (id, icon, label)) in [("path", Icon::List, "Path"), ("tree", Icon::FolderTree, "Tree")].into_iter().enumerate() {
            let pr = Rect::from_min_size(pos2(sw.left() + i as f32 * 75.0, sw.top()), vec2(75.0, 24.0));
            let on = (id == "tree") == tree;
            let resp = ui.interact(pr, ui.scope_id().with(("fileview", id)), Sense::click());
            ui.painter().rect_filled(pr.shrink(1.0), CornerRadius::ZERO, if on { c.white(0.15) } else { c.white(0.05) });
            let ink = if on || resp.hovered() { c.white(1.0) } else { c.fg2 };
            icons::paint(ui, Rect::from_min_size(pos2(pr.left() + 12.0, pr.center().y - 7.0), Vec2::splat(14.0)), icon, 14.0, ink);
            ui.painter().text(pos2(pr.left() + 32.0, pr.center().y), Align2::LEFT_CENTER, label, FontId::proportional(13.0), ink);
            if resp.clicked() {
                later.push(Box::new(move |app| app.settings.set("fileView", id)));
            }
        }
    }

    fn work_panel(&self, ui: &mut Ui, tab: u64, r: &mut RepoTab, later: &mut Vec<Later>) {
        let c = p();
        let v = r.view.as_ref().expect("view");
        let path = v.path.clone();
        let mut draft = self.draft(&path);
        let mut draft_changed = false;
        let status = &v.data.status;
        let conflicted: Vec<&FileChange> = status.unstaged.iter().filter(|f| f.status == 'U').collect();
        let unstaged: Vec<FileChange> = status.unstaged.iter().filter(|f| f.status != 'U').cloned().collect();
        let staged = status.staged.clone();
        let (total, ..) = working_changes(status);
        let merging = v.data.state_note.as_deref() == Some("merging");
        let (cur, inc) = v.data.conflict_labels.clone().unwrap_or_else(|| (v.data.head_branch.clone().unwrap_or_else(|| "HEAD".into()), "incoming".into()));
        let branch = v.data.head_branch.clone().unwrap_or_else(|| v.data.head_sha.chars().take(6).collect());
        let wu = &mut r.work;
        // Prefill: the merge message, or the commit template.
        if merging && draft.summary.is_empty() && draft.description.is_empty()
            && let Some(msg) = &v.data.merge_msg
        {
            let (first, rest) = msg.split_once('\n').unwrap_or((msg, ""));
            draft.summary = first.to_owned();
            draft.description = rest.trim().to_owned();
            draft_changed = true;
        }
        if !wu.template_applied && self.settings.flag("useCommitTemplate", true) && !merging && !draft.amend && draft.summary.is_empty() && draft.description.is_empty() {
            wu.template_applied = true;
            let (ts, td) = (self.settings.string("templateSummary", ""), self.settings.string("templateDescription", ""));
            if !ts.is_empty() || !td.is_empty() {
                draft.summary = ts;
                draft.description = td;
                draft_changed = true;
            } else if let Ok(Some((s, d))) = gitcraft_core::extras::commit_template(std::path::Path::new(&path)) {
                draft.summary = s;
                draft.description = d;
                draft_changed = true;
            }
        }
        Self::panel_header(ui, |ui| {
            let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 20.0), if total > 0 { Sense::click() } else { Sense::hover() });
            let fade = if total == 0 { 0.3 } else { 1.0 };
            ui.painter().rect(rect, CornerRadius::same(3), if resp.hovered() { BAD.gamma_multiply(0.15) } else { egui::Color32::TRANSPARENT }, Stroke::new(1.0, BAD.gamma_multiply(fade)), StrokeKind::Inside);
            icons::paint(ui, rect, Icon::Trash2, 12.0, REMOVED.gamma_multiply(fade));
            if resp.tip("Discard all changes").clicked() {
                later.push(Box::new(move |app| app.confirm_act(tab, "Discard all changes?", "All uncommitted changes will be discarded. You can bring them back with Undo.", "Discard All", Action::DiscardAll)));
            }
            let right_w = 26.0;
            let mid = ui.available_width() - right_w - 8.0;
            let text = format!("{total} file change{} on", if total == 1 { "" } else { "s" });
            let g = ui.painter().layout_no_wrap(text, FontId::proportional(13.0), c.white(0.9));
            let bg = widgets::fit(ui, &branch, FontId::proportional(13.0), c.white(1.0), (mid - g.size().x - 30.0).max(30.0));
            let (area, _) = ui.allocate_exact_size(vec2(mid, 24.0), Sense::hover());
            let all = g.size().x + 6.0 + bg.size().x + 12.0;
            let x0 = area.center().x - all / 2.0;
            ui.painter().galley(pos2(x0, area.center().y - g.size().y / 2.0), g.clone(), c.white(0.9));
            let pill = Rect::from_min_size(pos2(x0 + g.size().x + 6.0, area.center().y - 10.0), vec2(bg.size().x + 12.0, 20.0));
            ui.painter().rect_filled(pill, CornerRadius::same(3), widgets::mix(SELECTED_BRANCH, 0.45));
            ui.painter().galley(pos2(pill.left() + 6.0, pill.center().y - bg.size().y / 2.0), bg, c.white(1.0));
            if Self::ai_button(ui, "Compose commits with AI").clicked() {
                later.push(Box::new(move |app| app.compose_commits(tab)));
            }
        });
        self.view_switch(ui, later);
        // The lists, above the commit box.
        let box_h = match wu.tab {
            WorkTab::Commit => 300.0 + if wu.options_open { 110.0 } else { 0.0 },
            WorkTab::Stash => 220.0,
            WorkTab::Patch => 230.0,
        };
        const FILE_SECTION_HEIGHT: f32 = 30.0;
        let lists_h = work_list_height(ui.available_height(), box_h);
        let mut events: Vec<(bool, FileEvent)> = Vec::new();
        let active_unstaged = r.ui.diff.as_ref().filter(|d| d.source == DiffSource::Unstaged).map(|d| d.file.clone());
        let active_staged = r.ui.diff.as_ref().filter(|d| d.source == DiffSource::Staged).map(|d| d.file.clone());
        egui::ScrollArea::vertical().id_salt(("work_lists", tab)).max_height(lists_h).min_scrolled_height(lists_h).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            if !conflicted.is_empty() || v.data.state_note.is_some() {
                egui::Frame::new().fill(BAD.gamma_multiply(0.1)).stroke(Stroke::new(1.0, BAD.gamma_multiply(0.5))).corner_radius(CornerRadius::same(3)).outer_margin(egui::Margin { left: 8, right: 8, top: 4, bottom: 8 }).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    egui::Frame::new().inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
                        let title = match v.data.state_note.as_deref() {
                            Some("rebasing") => format!("Rebasing {inc} onto {cur}"),
                            _ if merging => format!("Merging {inc} into {cur}"),
                            Some(n) => format!("{n} in progress"),
                            None => "Conflicts in progress".into(),
                        };
                        ui.label(egui::RichText::new(title).size(12.0).color(c.white(0.9)));
                        if !conflicted.is_empty() {
                            let t = if conflicted.len() == 1 { "A file conflict was found".to_owned() } else { format!("{} file conflicts were found", conflicted.len()) };
                            ui.label(egui::RichText::new(t).size(11.5).color(REMOVED));
                        }
                    });
                    if !conflicted.is_empty() {
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(format!("Conflicted Files ({})", conflicted.len())).size(12.5).color(c.white(0.9)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_space(8.0);
                                if widgets::button(ui, Kind::Default, None, "Mark All Resolved", 20.0, true).clicked() {
                                    let paths: Vec<String> = conflicted.iter().map(|f| f.path.clone()).collect();
                                    later.push(Box::new(move |app| app.act(tab, Action::MarkResolved { paths })));
                                }
                            });
                        });
                        ui.add_space(4.0);
                        for f in &conflicted {
                            let w = ui.available_width();
                            let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
                            if r.ui.conflict.as_deref() == Some(f.path.as_str()) {
                                ui.painter().rect_filled(rect, CornerRadius::ZERO, c.selrow);
                            } else if resp.hovered() {
                                ui.painter().rect_filled(rect, CornerRadius::ZERO, c.hoverrow);
                            }
                            widgets::status_mark(ui, Rect::from_min_size(pos2(rect.left() + 8.0, rect.center().y - 7.0), Vec2::splat(14.0)), 'U');
                            let mut right = rect.right() - 8.0;
                            let mut hit = false;
                            if ui.rect_contains_pointer(rect) {
                                for (label, side) in [("Incoming", "theirs"), ("Current", "ours")] {
                                    let bw = 62.0;
                                    let br = Rect::from_min_size(pos2(right - bw, rect.center().y - 10.0), vec2(bw, 20.0));
                                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(br));
                                    let tip = if side == "ours" { format!("Take current ({cur})") } else { format!("Take incoming ({inc})") };
                                    if widgets::button(&mut child, Kind::Default, None, label, 20.0, true).tip(tip).clicked() {
                                        hit = true;
                                        let path = f.path.clone();
                                        later.push(Box::new(move |app| app.act(tab, Action::TakeSide { paths: vec![path], side: side.into() })));
                                    }
                                    right -= bw + 4.0;
                                }
                            }
                            let g = widgets::fit(ui, &f.path, FontId::proportional(12.5), c.white(0.85), right - rect.left() - 34.0);
                            ui.painter().galley(pos2(rect.left() + 28.0, rect.center().y - g.size().y / 2.0), g, c.fg);
                            let resp = resp.tip("Open in the merge tool");
                            if resp.clicked() && !hit {
                                r.ui.conflict = Some(f.path.clone());
                                r.ui.diff = None;
                            }
                            let (fp, cur2, inc2) = (f.path.clone(), cur.clone(), inc.clone());
                            ctxmenu::attach(&resp, later, || {
                                let (a, b, d, e) = (fp.clone(), fp.clone(), fp.clone(), fp.clone());
                                vec![
                                    ctxmenu::item("Open in the merge tool", move |app| {
                                        if let Some(r) = app.repo_mut(tab) {
                                            r.ui.conflict = Some(a);
                                            r.ui.diff = None;
                                        }
                                    }),
                                    ctxmenu::item(format!("Take current ({cur2})"), ctxmenu::act(tab, Action::TakeSide { paths: vec![b], side: "ours".into() })),
                                    ctxmenu::item(format!("Take incoming ({inc2})"), ctxmenu::act(tab, Action::TakeSide { paths: vec![d], side: "theirs".into() })),
                                    ctxmenu::Entry::Separator,
                                    ctxmenu::item("Mark resolved", ctxmenu::act(tab, Action::MarkResolved { paths: vec![e] })),
                                ]
                            });
                        }
                        ui.add_space(4.0);
                    }
                });
            }
            let resp = widgets::section_head(ui, !wu.unstaged_closed, FILE_SECTION_HEIGHT, &format!("Unstaged Files ({})", unstaged.len()), |ui| {
                if !unstaged.is_empty() && widgets::small(ui, Kind::Success, "Stage All Changes").tip(format!("{}+Shift+S", super::menu::MOD)).clicked() {
                    later.push(Box::new(move |app| app.act_quiet(tab, Action::StageAll, true)));
                }
            });
            if resp.clicked() {
                wu.unstaged_closed = !wu.unstaged_closed;
            }
            if !wu.unstaged_closed {
                if unstaged.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(egui::RichText::new(if total == 0 { "Working directory clean" } else { "No unstaged changes" }).size(12.0).color(c.fg3));
                    });
                    ui.add_space(8.0);
                }
                let menu = |f: &FileChange| self.file_menu(tab, v, &f.path, false, false, None, None);
                if let Some(e) = self.file_list(ui, ui.scope_id().with("unstaged"), FileRows { files: &unstaged, active: active_unstaged.as_deref(), button: Some(("Stage File", Kind::Success)) }, &mut wu.closed, &menu, later) {
                    events.push((false, e));
                }
            }
            let resp = widgets::section_head(ui, !wu.staged_closed, FILE_SECTION_HEIGHT, &format!("Staged Files ({})", staged.len()), |ui| {
                if !staged.is_empty() && widgets::small(ui, Kind::Danger, "Unstage All Changes").tip(format!("{}+Shift+U", super::menu::MOD)).clicked() {
                    later.push(Box::new(move |app| app.act_quiet(tab, Action::UnstageAll, true)));
                }
            });
            if resp.clicked() {
                wu.staged_closed = !wu.staged_closed;
            }
            if !wu.staged_closed && !staged.is_empty() {
                let menu = |f: &FileChange| self.file_menu(tab, v, &f.path, true, false, None, None);
                if let Some(e) = self.file_list(ui, ui.scope_id().with("staged"), FileRows { files: &staged, active: active_staged.as_deref(), button: Some(("Unstage File", Kind::Danger)) }, &mut wu.closed, &menu, later) {
                    events.push((true, e));
                }
            }
        });
        for (is_staged, e) in events {
            match e {
                FileEvent::Open(f) => {
                    // The picked file shows in the centre, in place of an open conflict.
                    r.ui.conflict = None;
                    r.ui.diff = Some(DiffTarget { source: if is_staged { DiffSource::Staged } else { DiffSource::Unstaged }, untracked: f.status == '?', file: f.path, parent: None });
                }
                FileEvent::Button(f) => {
                    let action = if is_staged { Action::Unstage { paths: vec![f.path] } } else { Action::Stage { paths: vec![f.path] } };
                    later.push(Box::new(move |app| app.act_quiet(tab, action, true)));
                }
            }
        }
        work_form_scroll(tab, ui.available_height()).show(ui, |ui| {
            // Grip, then the Commit / Stash / Patch tabs.
            let w = ui.available_width();
            let (grip, _) = ui.allocate_exact_size(vec2(w, 20.0), Sense::hover());
            ui.painter().rect_filled(Rect::from_center_size(grip.center(), vec2(100.0, 4.0)), CornerRadius::same(2), c.white(0.1));
            let (tabs, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
            ui.painter().line_segment([tabs.left_bottom(), tabs.right_bottom()], Stroke::new(1.0, c.white(0.1)));
            let mut tx = tabs.left() + 8.0;
            for (t, icon, label) in [(WorkTab::Commit, Icon::GitCommitHorizontal, "Commit"), (WorkTab::Stash, Icon::Download, "Stash"), (WorkTab::Patch, Icon::FileDown, "Patch")] {
                let on = wu.tab == t;
                let show_label = on || t == WorkTab::Commit;
                let g = ui.painter().layout_no_wrap(label.to_owned(), FontId::proportional(14.0), c.white(1.0));
                let tw = 20.0 + 15.0 + if show_label { 6.0 + g.size().x } else { 0.0 };
                let tr = Rect::from_min_size(pos2(tx, tabs.top()), vec2(tw, 30.0));
                let resp = ui.interact(tr, ui.scope_id().with(("worktab", label)), Sense::click());
                if on {
                    ui.painter().rect(tr, CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 }, c.field, Stroke::new(1.0, c.white(0.1)), StrokeKind::Inside);
                    ui.painter().line_segment([tr.left_bottom() + vec2(1.0, 0.0), tr.right_bottom() - vec2(1.0, 0.0)], Stroke::new(1.0, c.field));
                }
                let ink = if on || resp.hovered() { c.white(1.0) } else { c.white(0.7) };
                icons::paint(ui, Rect::from_min_size(pos2(tr.left() + 10.0, tr.center().y - 7.5), Vec2::splat(15.0)), icon, 15.0, ink);
                if show_label {
                    ui.painter().text(pos2(tr.left() + 31.0, tr.center().y), Align2::LEFT_CENTER, label, FontId::proportional(14.0), ink);
                }
                if resp.tip(label).clicked() {
                    wu.tab = t;
                }
                tx += tw + 2.0;
            }
            let push = self.settings.flag("pushAfterCommit", false);
            let mut commit_now: Option<bool> = None;
            egui::Frame::new().inner_margin(egui::Margin { left: 8, right: 8, top: 10, bottom: 8 }).show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                match wu.tab {
                    WorkTab::Commit => {
                        let mut amend = draft.amend;
                        if widgets::checkbox(ui, &mut amend, "Amend previous commit", 13.5).clicked() {
                            if amend {
                                let (s, d) = gitcraft_core::git::last_commit_message(std::path::Path::new(&path));
                                draft = Draft { summary: s, description: d, amend: true };
                            } else {
                                draft = Draft::default();
                            }
                            draft_changed = true;
                        }
                        let left = m_counter() as i64 - draft.summary.chars().count() as i64;
                        // The box takes the accent colour while either field has the focus.
                        let (summary_id, description_id) = (egui::Id::NULL.with(("summary", tab)), egui::Id::NULL.with(("description", tab)));
                        let focused = ui.memory(|m| m.has_focus(summary_id) || m.has_focus(description_id));
                        let language = self.settings.flag("spellcheck", true).then(|| {
                            if self.settings.string("language", "en") == "es" {
                                spelling::Language::Spanish
                            } else {
                                spelling::Language::English
                            }
                        });
                        let border = if focused { ACCENT.gamma_multiply(0.7) } else { c.white(0.1) };
                        egui::Frame::new().fill(c.field).stroke(Stroke::new(1.0, border)).corner_radius(CornerRadius::same(3)).show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            ui.horizontal(|ui| {
                                ui.add_space(10.0);
                                let w = ui.available_width() - 26.0 - 6.0 - 40.0;
                                let sresp = spelling::edit(ui, &mut draft.summary, summary_id, spelling::Field::Summary(w), language, &mut wu.summary_spelling);
                                if sresp.changed() {
                                    draft_changed = true;
                                }
                                if sresp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command) {
                                    commit_now = Some(ui.input(|i| i.modifiers.shift));
                                }
                                ui.add_sized([40.0, 30.0], egui::Label::new(egui::RichText::new(left.to_string()).size(14.0).color(if left < 0 { REMOVED } else { c.white(0.6) })));
                                if Self::ai_button(ui, "Generate a commit message with AI").clicked() {
                                    later.push(Box::new(move |app| app.ai_commit_message(tab)));
                                }
                            });
                            let dresp = spelling::edit(ui, &mut draft.description, description_id, spelling::Field::Description, language, &mut wu.description_spelling);
                            if dresp.changed() {
                                draft_changed = true;
                            }
                            if dresp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command) {
                                commit_now = Some(ui.input(|i| i.modifiers.shift));
                            }
                        });
                        if let Some(error) = wu.commit_ai_error.as_ref() {
                            ui.add(egui::Label::new(egui::RichText::new(format!("Could not generate a commit message. {error}")).size(12.5).color(REMOVED)).wrap());
                            let mut dismiss = false;
                            ui.horizontal_wrapped(|ui| {
                                if widgets::button(ui, Kind::Default, Some(Icon::Copy), "Copy error", 22.0, true).clicked() {
                                    ui.ctx().copy_text(error.clone());
                                }
                                if error.contains("Preferences > AI")
                                    && widgets::button(ui, Kind::Default, None, "AI Settings", 22.0, true).clicked()
                                {
                                    later.push(Box::new(|app| app.open_prefs(Some("ai"))));
                                }
                                dismiss = widgets::button(ui, Kind::Default, None, "Dismiss", 22.0, true).clicked();
                            });
                            if dismiss {
                                wu.commit_ai_error = None;
                            }
                        }
                        ui.horizontal(|ui| {
                            let (cr, cresp) = ui.allocate_exact_size(vec2(14.0, 20.0), Sense::click());
                            widgets::chevron(ui, cr, wu.options_open, 14.0, c.white(0.7));
                            let resp = ui.add(egui::Label::new(egui::RichText::new("Commit options").size(13.5).color(c.white(0.7))).sense(Sense::click()));
                            if resp.clicked() || cresp.clicked() {
                                wu.options_open = !wu.options_open;
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if widgets::button(ui, Kind::Ai, Some(Icon::Sparkles), "Compose with AI", 24.0, true).tip("Split your changes into focused commits with AI").clicked() {
                                    later.push(Box::new(move |app| app.compose_commits(tab)));
                                }
                                if Self::ai_button(ui, "Explain the working changes with AI").clicked() {
                                    later.push(Box::new(move |app| app.explain_working(tab)));
                                }
                            });
                        });
                        if wu.options_open {
                            ui.horizontal(|ui| {
                                ui.add_space(20.0);
                                let mut push = push;
                                if widgets::checkbox(ui, &mut push, "Push after committing", 12.5).clicked() {
                                    later.push(Box::new(move |app| app.settings.set("pushAfterCommit", push)));
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.add_space(20.0);
                                ui.label(egui::RichText::new("Co-authors:").size(12.5).color(c.white(0.8)));
                                let w = ui.available_width();
                                widgets::field(ui, &mut wu.coauthors, "Name <email>, …", w, 22.0, 12.0).tip("Adds a Co-authored-by line for each person (separate several with commas)");
                            });
                            ui.horizontal(|ui| {
                                ui.add_space(20.0);
                                let mut skip = wu.skip_hooks.unwrap_or_else(|| self.settings.flag("skipHooks", false));
                                if widgets::checkbox(ui, &mut skip, "Skip Git hooks", 12.5).clicked() {
                                    wu.skip_hooks = Some(skip);
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.add_space(20.0);
                                widgets::checkbox(ui, &mut wu.sign_off, "Sign off (DCO)", 12.5).tip("Add a Signed-off-by trailer using your Git identity");
                            });
                        }
                    }
                    WorkTab::Patch => {
                        egui::Frame::new().fill(c.field).stroke(Stroke::new(1.0, c.white(0.1))).corner_radius(CornerRadius::same(3)).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.add(egui::Label::new(egui::RichText::new("Share your uncommitted changes as a .patch file, without pushing a branch. Untracked files must be staged first. Anyone can apply it with GitCraft or git apply.").size(12.5).color(c.fg2)).wrap());
                            ui.add_space(6.0);
                            if widgets::link(ui, "Apply a patch file…", 12.5).clicked() {
                                later.push(Box::new(move |app| app.apply_patch(tab)));
                            }
                        });
                    }
                    WorkTab::Stash => {
                        egui::Frame::new().fill(c.field).stroke(Stroke::new(1.0, c.white(0.1))).corner_radius(CornerRadius::same(3)).show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            ui.horizontal(|ui| {
                                ui.add_space(10.0);
                                let w = ui.available_width() - 26.0 - 8.0;
                                ui.add_sized([w, 34.0], egui::TextEdit::singleline(&mut wu.stash_message).frame(egui::Frame::NONE).font(FontId::proportional(15.0)).hint_text(egui::RichText::new("Stash title").color(c.white(0.7))).align(Align2::LEFT_CENTER));
                                if Self::ai_button(ui, "Name the stash with AI").clicked() {
                                    later.push(Box::new(move |app| app.ai_stash_title(tab)));
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.add_space(10.0);
                                ui.label(egui::RichText::new("Staged and unstaged changes, including new files, are stashed.").size(12.0).color(c.fg3));
                            });
                            ui.add_space(8.0);
                        });
                    }
                }
                // The big green button.
                let (label, enabled) = match wu.tab {
                    WorkTab::Patch if total == 0 => ("No Changes for a Patch".to_owned(), false),
                    WorkTab::Patch => (format!("Save {total} File Change{} as a Patch", if total == 1 { "" } else { "s" }), true),
                    WorkTab::Stash if total == 0 => ("No Changes to Stash".to_owned(), false),
                    WorkTab::Stash => (format!("Stash Changes to {total} File{}", if total == 1 { "" } else { "s" }), true),
                    _ if !conflicted.is_empty() => (format!("Resolve {} Conflict{} to Commit", conflicted.len(), if conflicted.len() == 1 { "" } else { "s" }), false),
                    _ if merging && !draft.summary.trim().is_empty() => ("Commit and Merge".to_owned(), true),
                    _ if staged.is_empty() && !draft.amend => ("Stage Changes to Commit".to_owned(), false),
                    _ if draft.summary.trim().is_empty() => ("Type a Message to Commit".to_owned(), false),
                    _ if draft.amend => (format!("Amend Previous Commit{}", if push { " and Push" } else { "" }), true),
                    _ => (format!("Commit Changes to {} File{}{}", staged.len(), if staged.len() == 1 { "" } else { "s" }, if push { " and Push" } else { "" }), true),
                };
                let w = ui.available_width();
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 40.0), if enabled { Sense::click() } else { Sense::hover() });
                let (fill, stroke, ink) = if enabled {
                    (GOOD.gamma_multiply(if resp.hovered() { 0.35 } else { 0.25 }), GOOD, c.white(1.0))
                } else {
                    (GOOD.gamma_multiply(0.1), GOOD.gamma_multiply(0.25), c.white(0.45))
                };
                ui.painter().rect(rect, CornerRadius::same(3), fill, Stroke::new(1.0, stroke), StrokeKind::Inside);
                let g = ui.painter().layout_no_wrap(label, super::fonts::semibold(13.5), ink);
                let x0 = rect.center().x - (g.size().x + 23.0) / 2.0;
                icons::paint(ui, Rect::from_min_size(pos2(x0, rect.center().y - 7.5), Vec2::splat(15.0)), Icon::GitCommitHorizontal, 15.0, ink);
                ui.painter().galley(pos2(x0 + 23.0, rect.center().y - g.size().y / 2.0), g, ink);
                if resp.clicked() {
                    match wu.tab {
                        WorkTab::Patch => {
                            let name = super::patches::patch_name(if draft.summary.is_empty() { &branch } else { &draft.summary });
                            later.push(Box::new(move |app| app.save_patch(tab, Vec::new(), &name)));
                        }
                        WorkTab::Stash => {
                            // The title stays while Git runs and if it fails; it is cleared once the stash is made.
                            let message = wu.stash_message.clone();
                            later.push(Box::new(move |app| app.act(tab, Action::Stash { message })));
                        }
                        WorkTab::Commit => commit_now = Some(false),
                    }
                }
                if let Some(all) = commit_now
                    && wu.tab == WorkTab::Commit
                    && (all || enabled)
                {
                    // The summary typed this frame is stored first, so the commit uses what was submitted rather than the
                    // draft as it was before the last key press.
                    let (stored_path, typed) = (path.clone(), draft_changed.then(|| draft.clone()));
                    later.push(Box::new(move |app| {
                        if let Some(typed) = typed {
                            app.set_draft(&stored_path, typed);
                        }
                        app.commit_from_wip(tab, all);
                    }));
                }
            });
        });
        if draft_changed {
            later.push(Box::new(move |app| app.set_draft(&path, draft)));
        }
    }

    /// Commit the draft (with `all`, staging everything first), with the panel's options.
    pub(crate) fn commit_from_wip(&mut self, tab: u64, all: bool) {
        self.commit_checked(tab, all, false);
    }

    fn commit_checked(&mut self, tab: u64, all: bool, confirmed: bool) {
        let Some(r) = self.repo(tab) else { return };
        let Some(v) = r.view.as_ref() else { return };
        let path = v.path.clone();
        // One commit at a time: what a commit was made from is cleared only when it succeeds.
        if r.work.submitted.is_some() {
            return self.toast(false, "Wait for the running commit to finish before committing again.");
        }
        if v.data.status.conflicted || v.data.status.unstaged.iter().any(|f| f.status == 'U') {
            return self.toast(false, "Resolve and stage every conflicted file before committing.");
        }
        let draft = self.draft(&path);
        // With nothing staged there is nothing to commit, except during a merge: the merge
        // commit itself records the merge.
        let merging = v.data.state_note.as_deref() == Some("merging");
        if !all && v.data.status.staged.is_empty() && !draft.amend && !merging {
            return;
        }
        if draft.summary.trim().is_empty() {
            return;
        }
        if draft.amend && !confirmed && gitcraft_core::session::head_is_upstream(&path, &v.data.head_sha) {
            self.dialog = Some(super::dialogs::Dialog::confirm_with(
                "Amend a published commit?",
                "This commit is already on its upstream branch. Amending it changes its ID and may require a force push.",
                "Amend commit",
                true,
                move |app, _| app.commit_checked(tab, all, true),
            ));
            return;
        }
        let wu = &r.work;
        let description = with_coauthors(&draft.description, &wu.coauthors);
        let skip = wu.skip_hooks.unwrap_or_else(|| self.settings.flag("skipHooks", false));
        let sign_off = wu.sign_off;
        // The draft and co-authors stay until the commit succeeds; a push after the commit
        // runs only then.
        let submitted = Submitted { draft: draft.clone(), coauthors: wu.coauthors.clone(), push: self.settings.flag("pushAfterCommit", false) };
        self.act(tab, Action::Commit { summary: draft.summary.clone(), description, amend: draft.amend, stage_all: all, skip_hooks: skip, strip_comments: false, sign_off });
        if let Some(r) = self.repo_mut(tab) {
            r.work.submitted = Some(submitted);
        }
    }

    fn commit_details(&self, ui: &mut Ui, tab: u64, r: &mut RepoTab, sha: &str, later: &mut Vec<Later>) {
        let c = p();
        let v = r.view.as_ref().expect("view");
        let loaded = r.commit(sha).cloned();
        let commit = match loaded {
            Some(cm) => cm,
            None => match r.commit_details.get(sha) {
                Some(Loaded::Ready(cm)) => cm.clone(),
                Some(Loaded::Failed(e)) => {
                    ui.label(egui::RichText::new(e).size(12.0).color(c.fg2));
                    return;
                }
                _ => {
                    let s = sha.to_owned();
                    later.push(Box::new(move |app| app.want_commit_details(tab, &s)));
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(egui::RichText::new("Loading commit…").size(12.0).color(c.fg2));
                    });
                    return;
                }
            },
        };
        if !r.commit_files.contains_key(sha) {
            let s = sha.to_owned();
            later.push(Box::new(move |app| app.want_commit_files(tab, &s)));
        }
        let short: String = commit.sha.chars().take(6).collect();
        Self::panel_header(ui, |ui| {
            ui.label(egui::RichText::new("commit:").size(12.5).color(c.fg2));
            let resp = ui.add(egui::Label::new(egui::RichText::new(&short).size(12.5).monospace().color(c.link)).sense(Sense::click())).tip("Copy commit sha");
            let copy = widgets::icon_button(ui, Icon::Copy, 12.0, "Copy commit sha");
            if resp.clicked() || copy.clicked() {
                let s = commit.sha.clone();
                later.push(Box::new(move |app| app.copy(&s)));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, Icon::X, 15.0, "Close (Esc)").clicked() {
                    r.ui.select(Selection::Wip);
                }
                if Self::ai_button(ui, "Explain this commit with AI").clicked() {
                    let (s, summary) = (commit.sha.clone(), commit.summary.clone());
                    later.push(Box::new(move |app| app.explain_commit(tab, &s, &summary)));
                }
            });
        });
        egui::Frame::new().inner_margin(egui::Margin { left: 12, right: 12, top: 12, bottom: 8 }).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
            ui.add(egui::Label::new(egui::RichText::new(&commit.summary).font(super::fonts::semibold(15.0)).color(c.white(1.0))).wrap().selectable(true));
            // The pull request this commit was the head of, once merged.
            ui.horizontal(|ui| self.merged_pill(ui, tab, None, Some(&commit.sha), later));
            if let Some(Loaded::Ready(_)) = r.commit_files.get(sha) {
                let code = r.signatures.get(sha).map(String::as_str).unwrap_or("");
                let label = match code {
                    "G" => Some("Verified signature"),
                    "B" => Some("Bad signature"),
                    "U" => Some("Valid signature, unknown trust"),
                    "X" => Some("Expired signature"),
                    "Y" => Some("Signature key expired"),
                    "R" => Some("Signature key revoked"),
                    "E" => Some("Signature could not be checked"),
                    _ => None,
                };
                if let Some(label) = label {
                    egui::Frame::new().fill(c.white(0.08)).corner_radius(CornerRadius::same(3)).inner_margin(egui::Margin::symmetric(6, 2)).show(ui, |ui| {
                        ui.label(egui::RichText::new(label).size(11.5).color(if code == "G" { GOOD } else { super::theme::WARN }));
                    });
                }
            }
            if !commit.body.is_empty() {
                egui::ScrollArea::vertical().id_salt(("body", tab)).max_height(120.0).show(ui, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(&commit.body).size(13.0).color(c.white(0.75))).wrap().selectable(true));
                });
            }
            if !v.data.remotes.is_empty() {
                self.commit_checks(ui, tab, r.path.as_str(), &commit.sha, later);
            }
        });
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(w, 48.0), Sense::hover());
        ui.painter().line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, c.white(0.05)));
        ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, c.white(0.05)));
        widgets::paint_avatar(ui, pos2(rect.left() + 12.0 + 14.0, rect.center().y), 28.0, &commit.author, &commit.email);
        let fmt = self.settings.string("dateTimeFormat", "MM/DD/YYYY @ h:mm A");
        let parents_w = 130.0;
        let ag = widgets::fit(ui, &commit.author, FontId::proportional(13.0), c.white(1.0), w - 60.0 - parents_w);
        ui.painter().galley(pos2(rect.left() + 48.0, rect.center().y - 16.0), ag, c.white(1.0));
        let dg = widgets::fit(ui, &format!("authored {}", widgets::format_date(commit.time, &fmt)), FontId::proportional(11.5), c.fg3, w - 60.0 - parents_w);
        ui.painter().galley(pos2(rect.left() + 48.0, rect.center().y + 2.0), dg, c.fg3);
        let mut py = rect.center().y - 8.0 * commit.parents.len() as f32;
        for parent in &commit.parents {
            let short: String = parent.chars().take(6).collect();
            let pr = Rect::from_min_size(pos2(rect.right() - 12.0 - parents_w, py), vec2(parents_w, 16.0));
            let resp = ui.interact(pr, ui.scope_id().with(("parent", parent)), Sense::click());
            let ink = if resp.hovered() { c.white(1.0) } else { c.fg3 };
            let sg = ui.painter().layout_no_wrap(short, FontId::monospace(11.5), c.link);
            let sw = sg.size().x;
            ui.painter().galley(pos2(pr.right() - sw, pr.center().y - sg.size().y / 2.0), sg, c.link);
            ui.painter().text(pos2(pr.right() - sw - 4.0, pr.center().y), Align2::RIGHT_CENTER, "parent:", FontId::proportional(11.5), ink);
            if resp.tip("Jump to parent commit").clicked() {
                r.ui.select(Selection::Commit(parent.clone()));
            }
            py += 16.0;
        }
        let files = match r.commit_files.get(sha) {
            Some(Loaded::Ready(f)) => Some(f.iter().map(|f| f.change.clone()).collect::<Vec<FileChange>>()),
            _ => None,
        };
        let storage: std::collections::HashMap<String, String> = match r.commit_files.get(sha) {
            Some(Loaded::Ready(f)) => f.iter().filter_map(|f| f.storage.clone().map(|s| (f.change.path.clone(), s))).collect(),
            _ => Default::default(),
        };
        let counts = files.as_ref().map_or((0, 0, 0), |f| (f.iter().filter(|x| x.status == 'M' || x.status == 'R').count(), f.iter().filter(|x| x.status == 'A').count(), f.iter().filter(|x| x.status == 'D').count()));
        ui.horizontal(|ui| {
            ui.set_height(32.0);
            ui.add_space(12.0);
            ui.spacing_mut().item_spacing.x = 4.0;
            if counts.0 > 0 {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                icons::paint(ui, r, Icon::Pencil, 11.0, MODIFIED);
                ui.label(egui::RichText::new(format!("{} modified", counts.0)).size(12.0).color(MODIFIED));
                ui.add_space(8.0);
            }
            if counts.1 > 0 {
                ui.label(egui::RichText::new(format!("+ {} added", counts.1)).size(12.0).color(GOOD));
                ui.add_space(8.0);
            }
            if counts.2 > 0 {
                ui.label(egui::RichText::new(format!("− {} deleted", counts.2)).size(12.0).color(REMOVED));
            }
        });
        self.view_switch(ui, later);
        let active = r.ui.diff.as_ref().filter(|d| matches!(d.source, DiffSource::Commit(_))).map(|d| d.file.clone());
        let mut open: Option<FileChange> = None;
        egui::ScrollArea::vertical().id_salt(("commit_files", tab, sha)).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            match (&files, r.commit_files.get(sha)) {
                (None, Some(Loaded::Failed(e))) => {
                    ui.label(egui::RichText::new(e).size(12.0).color(REMOVED));
                }
                (None, _) => {
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(egui::RichText::new("Loading files...").size(12.0).color(c.fg3));
                    });
                }
                (Some(f), _) if f.is_empty() => {
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(egui::RichText::new("No file changes to show.").size(12.0).color(c.fg3));
                    });
                }
                (Some(f), _) => {
                    let parent0 = commit.parents.first().cloned();
                    let menu = |fc: &FileChange| {
                        let rev = if fc.status == 'D' { parent0.clone() } else { Some(commit.sha.clone()) };
                        self.file_menu(tab, v, &fc.path, false, true, rev.as_deref(), storage.get(&fc.path).map(String::as_str))
                    };
                    if let Some(FileEvent::Open(fc)) = self.file_list(ui, ui.scope_id().with("commit_files"), FileRows { files: f, active: active.as_deref(), button: None }, &mut r.work.closed, &menu, later) {
                        open = Some(fc);
                    }
                }
            }
        });
        if let Some(fc) = open {
            let store = storage.get(&fc.path).cloned();
            r.ui.diff = Some(DiffTarget {
                source: DiffSource::Commit(store.clone().unwrap_or_else(|| commit.sha.clone())),
                file: fc.path,
                untracked: false,
                parent: if store.is_some() { None } else { commit.parents.first().cloned() },
            });
        }
    }

    fn compare_panel(&self, ui: &mut Ui, tab: u64, r: &mut RepoTab, from: &str, to: &str, later: &mut Vec<Later>) {
        let c = p();
        let key = (from.to_owned(), to.to_owned());
        if !r.range_files.contains_key(&key) {
            let (a, b) = key.clone();
            later.push(Box::new(move |app| app.want_range_files(tab, &a, &b)));
        }
        let recompose = (to != WORKDIR && r.ui.multi.len() == 2)
            .then(|| recompose_base(r)).flatten();
        let combined = to != WORKDIR && r.ui.multi.len() > 2;
        let title = if to == WORKDIR {
            "Viewing diff between a commit and the WIP"
        } else if combined {
            "Combined changes"
        } else {
            "Viewing diff between 2 commits"
        };
        let mut close = false;
        Self::panel_header(ui, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !combined && widgets::icon_button(ui, Icon::X, 15.0, "Close (Esc)").clicked() {
                    close = true;
                }
                if let Some(base) = recompose {
                    Self::recompose_button(ui, tab, base, 2, later);
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(title).size(12.5).color(c.white(1.0))).truncate());
                });
            });
        });
        let short = |x: &str| if x == WORKDIR { "working folder".to_owned() } else { x.chars().take(6).collect() };
        let files = match r.range_files.get(&key) {
            Some(Loaded::Ready(f)) => Some(f.clone()),
            _ => None,
        };
        ui.horizontal(|ui| {
            ui.set_height(32.0);
            ui.add_space(12.0);
            ui.label(egui::RichText::new(short(from)).monospace().size(12.0).color(c.link));
            ui.label(egui::RichText::new(" → ").size(12.0).color(c.fg2));
            ui.label(egui::RichText::new(short(to)).monospace().size(12.0).color(c.link));
            if let Some(f) = &files {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(format!("{} file{} changed", f.len(), if f.len() == 1 { "" } else { "s" })).size(12.0).color(c.fg3));
            }
        });
        self.view_switch(ui, later);
        let v = r.view.as_ref().expect("view");
        let active = r.ui.diff.as_ref().filter(|d| matches!(d.source, DiffSource::Range(..))).map(|d| d.file.clone());
        let mut open = None;
        egui::ScrollArea::vertical().id_salt(("range_files", tab)).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            match (&files, r.range_files.get(&key)) {
                (None, Some(Loaded::Failed(e))) => {
                    ui.label(egui::RichText::new(e).size(12.0).color(REMOVED));
                }
                (None, _) => {
                    ui.label(egui::RichText::new("Loading files...").size(12.0).color(c.fg3));
                }
                (Some(f), _) => {
                    let menu = |fc: &FileChange| {
                        let rev = if fc.status == 'D' { Some(from.to_owned()) } else if to == WORKDIR { None } else { Some(to.to_owned()) };
                        self.file_menu(tab, v, &fc.path, false, true, rev.as_deref(), None)
                    };
                    if let Some(FileEvent::Open(fc)) = self.file_list(ui, ui.scope_id().with("range"), FileRows { files: f, active: active.as_deref(), button: None }, &mut r.work.closed, &menu, later) {
                        open = Some(fc);
                    }
                }
            }
        });
        if let Some(fc) = open {
            r.ui.diff = Some(DiffTarget { source: DiffSource::Range(from.to_owned(), to.to_owned()), file: fc.path, untracked: false, parent: None });
        }
        if close {
            r.ui.multi.clear();
            r.ui.compare_workdir = None;
            r.ui.diff = None;
        }
    }

    fn multi_panel(&self, ui: &mut Ui, tab: u64, r: &mut RepoTab, combined: bool, later: &mut Vec<Later>) {
        let c = p();
        let v = r.view.as_ref().expect("view");
        let picked: Vec<&gitcraft_core::git::Commit> = v.data.commits.iter().filter(|cm| r.ui.multi.contains(&cm.sha)).collect();
        let oldest_first: Vec<String> = picked.iter().rev().map(|cm| cm.sha.clone()).collect();
        let line = super::rebase::head_line(v);
        let on_line = v.data.head_branch.is_some() && r.ui.multi.iter().all(|x| line.contains(x));
        let n = picked.len();
        let mut clear = false;
        Self::panel_header(ui, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, Icon::X, 15.0, "Clear selection (Esc)").clicked() {
                    clear = true;
                }
                let actions = widgets::icon_button(ui, Icon::EllipsisVertical, 16.0, "Selected commit actions");
                egui::Popup::menu(&actions).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                    let multi = &r.ui.multi;
                    let mut entries = vec![
                        ctxmenu::item(format!("Cherry pick {n} commits"), ctxmenu::act(tab, Action::CherryPickMany { shas: oldest_first.clone() })),
                        ctxmenu::item("Interactive Cherry Pick", {
                            let commits = multi.clone();
                            move |app| app.open_cherry_pick(tab, &commits)
                        }),
                    ];
                    if on_line {
                        let squash = multi.clone();
                        let drop = multi.clone();
                        let base = super::rebase::base_for(v, &oldest_first[0]);
                        let onto = v.data.head_branch.clone().unwrap_or_else(|| "HEAD".into());
                        entries.extend([
                            ctxmenu::Entry::Separator,
                            ctxmenu::item(format!("Squash {n} commits"), move |app| app.squash_commits(tab, squash)),
                            ctxmenu::item("Interactive Rebase", move |app| app.open_rebase(tab, &base, &onto, false)),
                            ctxmenu::danger(format!("Drop {n} commits"), move |app| app.drop_commits(tab, drop)),
                        ]);
                    }
                    ctxmenu::show(ui, entries, later);
                });
                if on_line && let Some(base) = recompose_base(r) {
                    Self::recompose_button(ui, tab, base, n, later);
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(format!("{n} commits selected")).size(12.5).color(c.white(1.0))).truncate());
                });
            });
        });
        if !combined {
            egui::Frame::new().inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                ui.add(egui::Label::new(egui::RichText::new("Select consecutive commits to see their combined changes.").size(12.0).color(c.fg3)).wrap());
            });
        } else {
            ui.add_space(8.0);
        }
        egui::ScrollArea::vertical().id_salt(("multi", tab)).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            for cm in &picked {
                let w = ui.available_width();
                let (rect, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
                ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, c.white(0.05)));
                widgets::paint_avatar(ui, pos2(rect.left() + 12.0 + 9.0, rect.center().y), 18.0, &cm.author, &cm.email);
                let short: String = cm.sha.chars().take(6).collect();
                let sg = ui.painter().layout_no_wrap(short, FontId::monospace(11.0), c.fg3);
                let sw = sg.size().x;
                ui.painter().galley(pos2(rect.right() - 12.0 - sw, rect.center().y - sg.size().y / 2.0), sg, c.fg3);
                let g = widgets::fit(ui, &cm.summary, FontId::proportional(12.5), c.white(0.9), w - 60.0 - sw);
                ui.painter().galley(pos2(rect.left() + 38.0, rect.center().y - g.size().y / 2.0), g, c.white(0.9));
            }
        });
        if clear {
            r.ui.multi.clear();
        }
    }
}

fn work_list_height(available: f32, box_h: f32) -> f32 {
    let available = available.max(0.0);
    (available - box_h).max(60.0_f32.min(available / 2.0))
}

fn work_form_scroll(tab: u64, height: f32) -> egui::ScrollArea {
    egui::ScrollArea::vertical().id_salt(("work_form", tab)).max_height(height.max(0.0)).min_scrolled_height(0.0).auto_shrink([false, false])
}

fn recompose_base(r: &RepoTab) -> Option<String> {
    let (base, newest) = consecutive_range(r)?;
    (newest == r.view.as_ref()?.data.head_sha).then_some(base)
}

fn m_counter() -> u32 {
    gitcraft_core::metrics::COMMIT_SUMMARY_COUNTER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_work_panel_leaves_room_for_the_form() {
        let available: f32 = 32.0;
        let lists_h = work_list_height(available, 300.0);
        assert!(lists_h < available, "the form must have a visible viewport");
        assert_eq!(lists_h, 16.0);
        assert_eq!(work_list_height(0.0, 300.0), 0.0);
        assert_eq!(work_list_height(600.0, 300.0), 300.0);
        assert_eq!(work_list_height(150.0, 410.0), 60.0);
    }

    #[test]
    fn short_work_form_scrolls_to_its_last_control() {
        egui::__run_test_ui(|ui| {
            let output = work_form_scroll(1, 16.0).vertical_scroll_offset(410.0).show(ui, |ui| {
                ui.add_space(400.0);
                ui.allocate_exact_size(vec2(100.0, 40.0), Sense::click()).0
            });
            assert!(output.inner_rect.height() <= 16.0);
            assert!(output.state.clamped_offset().y > 0.0);
            assert!(output.inner.intersects(output.inner_rect));
        });
    }

    #[test]
    fn coauthor_trailers() {
        assert_eq!(with_coauthors("Body", "Sam <sam@example.invalid>, bad"), "Body\n\nCo-authored-by: Sam <sam@example.invalid>");
        assert_eq!(with_coauthors("", "nobody"), "");
    }

    #[test]
    fn changes_count_each_file_once() {
        let f = |p: &str, s: char| FileChange { path: p.into(), status: s };
        let s = WorkStatus { staged: vec![f("a", 'A')], unstaged: vec![f("a", 'M'), f("b", 'D')], conflicted: false };
        assert_eq!(working_changes(&s), (2, 1, 1, 0));
    }
}

/// The selected commits as one range, when they follow each other (each is the first parent of the next): the parent
/// of the oldest and the newest. `None` for scattered commits or a run that starts at the root commit.
fn consecutive_range(r: &RepoTab) -> Option<(String, String)> {
    let v = r.view.as_ref()?;
    let mut picked: Vec<usize> = r.ui.multi.iter().map(|sha| r.index.get(sha).copied()).collect::<Option<Vec<_>>>()?;
    picked.sort_unstable();
    let commits = &v.data.commits;
    for pair in picked.windows(2) {
        let (newer, older) = (commits.get(pair[0])?, commits.get(pair[1])?);
        if newer.parents.first() != Some(&older.sha) {
            return None;
        }
    }
    let newest = commits.get(*picked.first()?)?;
    let oldest = commits.get(*picked.last()?)?;
    Some((oldest.parents.first()?.clone(), newest.sha.clone()))
}
