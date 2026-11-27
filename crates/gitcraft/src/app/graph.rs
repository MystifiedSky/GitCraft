//! The commit graph: a header with resizable columns, then one 28 px row per commit with its
//! branch labels, the lanes and avatar node, the message, and the optional author, date, sha
//! and changes columns. Only the rows on screen are drawn. The working changes get a "// WIP"
//! row on top; more history loads as the end comes near.

use super::widgets::Tip;
use super::App;
use super::ctxmenu::{self, Entry, Later, item};
use super::icons::{self, Icon};
use super::repo::{RepoTab, Selection};
use super::theme::{ACCENT, BAD, WARN, p};
use super::widgets::{self, Kind, lane_color, mix};
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use gitcraft_core::actions::Action;
use gitcraft_core::git::{GitRef, RefKind};
use gitcraft_core::metrics as m;
use gitcraft_core::session::RepoView;
use std::collections::{HashMap, HashSet};

const ROW: f32 = m::GRAPH_ROW_HEIGHT as f32;
const INNER: f32 = m::GRAPH_ROW_INNER_HEIGHT as f32;
const NODE: f32 = m::COMMIT_NODE_DIAMETER as f32;
const LANE: f32 = m::GRAPH_LANE_WIDTH as f32;
const GUTTER: f32 = m::GRAPH_GUTTER_WIDTH as f32;
const PAD: f32 = m::GRAPH_ZONE_PADDING as f32;
const HEADER: f32 = m::GRAPH_HEADER_ROW_HEIGHT as f32;
const ARC: f32 = m::EDGE_ARC_RADIUS as f32;
const REF_MARGIN: f32 = (m::REF_ZONE_MARGIN_LEFT + m::LEFT_PANEL_TO_GRAPH_GAP) as f32;
const SUMMARY_MARGIN: f32 = m::INLINE_SUMMARY_MARGIN_LEFT as f32;
const MIN_ALPHA: f32 = m::NODE_MIN_ALPHA_PERCENT as f32 / 100.0;

fn commit_row_target(ui: &Ui, rect: Rect, id: egui::Id, summary: &str, selected: bool) -> egui::Response {
    let response = ui.interact(rect, id, Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, summary));
    if response.has_focus() {
        ui.painter().rect_stroke(rect.shrink(1.0), CornerRadius::ZERO, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
    }
    response
}

fn click_modifiers(ui: &Ui, response: &egui::Response) -> egui::Modifiers {
    ui.input(|input| {
        // A quick click and modifier release can arrive in the same frame.
        input.events.iter().rev().find_map(|event| match event {
            egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers }
                if response.interact_rect.contains(*pos) => Some(*modifiers),
            _ => None,
        }).unwrap_or(input.modifiers)
    })
}

#[cfg(test)]
#[test]
fn headless_graph_row_has_a_name_and_keyboard_activation() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let id = egui::Id::NULL.with("test commit");
    let label = "Keep local changes intact";
    let mut activated = false;
    let mut draw = |ui: &mut Ui| {
        let rect = ui.allocate_space(vec2(220.0, ROW)).1;
        let response = commit_row_target(ui, rect, id, label, true);
        activated = response.clicked();
    };
    let mut output = ctx.run_ui(egui::RawInput::default(), &mut draw);
    output.textures_delta.clear();
    let tree = output.platform_output.accesskit_update.expect("Accessibility tree").nodes;
    let (_, row) = tree.iter().find(|(_, node)| node.label() == Some(label)).expect("Named commit row");
    assert_eq!(row.role(), egui::accesskit::Role::Button);
    assert!(row.supports_action(egui::accesskit::Action::Click));

    ctx.memory_mut(|memory| memory.request_focus(id));
    let event = egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    ctx.run_ui(egui::RawInput { events: vec![event], ..Default::default() }, &mut draw).textures_delta.clear();
    assert!(activated);
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Col {
    Refs,
    Graph,
    Message,
    Author,
    Date,
    Sha,
    Changes,
}

impl Col {
    fn key(self) -> &'static str {
        match self {
            Col::Refs => "refs",
            Col::Graph => "graph",
            Col::Message => "message",
            Col::Author => "author",
            Col::Date => "date",
            Col::Sha => "sha",
            Col::Changes => "changes",
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct Columns {
    pub refs: bool,
    pub graph: bool,
    pub description: bool,
    pub author: bool,
    pub date: bool,
    pub sha: bool,
    pub changes: bool,
}

impl Default for Columns {
    fn default() -> Self {
        Columns { refs: true, graph: true, description: true, author: false, date: false, sha: false, changes: false }
    }
}

/// Branch and tag labels per commit, the checked-out branch first, then local, remote, tags.
pub fn refs_by_commit<'a>(refs: &'a [GitRef], head: Option<&str>, hidden: &[String]) -> HashMap<&'a str, Vec<&'a GitRef>> {
    let mut out: HashMap<&str, Vec<&GitRef>> = HashMap::new();
    for r in refs.iter().filter(|r| !hidden.contains(&r.full)) {
        out.entry(r.sha.as_str()).or_default().push(r);
    }
    let order = |r: &GitRef| match r.kind {
        RefKind::Local if Some(r.name.as_str()) == head => 0,
        RefKind::Local => 1,
        RefKind::Remote => 2,
        RefKind::Tag => 3,
    };
    for v in out.values_mut() {
        v.sort_by_key(|r| order(r));
    }
    out
}

/// A local branch paired with its tracking ref while both point at the same commit.
pub fn group_refs<'a>(refs: &[&'a GitRef]) -> Vec<(&'a GitRef, Option<&'a GitRef>)> {
    let mut used: HashSet<*const GitRef> = HashSet::new();
    let mut pairs: HashMap<*const GitRef, &GitRef> = HashMap::new();
    let upstream_of = |local: &GitRef| local.upstream.strip_prefix("refs/remotes/").unwrap_or(&local.upstream).to_owned();
    // A remote branch some local branch names as its upstream belongs to that branch only.
    let configured: HashSet<String> = refs.iter().filter(|r| r.kind == RefKind::Local).map(|r| upstream_of(r)).filter(|u| !u.is_empty()).collect();
    // First the configured upstreams, then, for branches without one, a remote branch of the
    // same name that no branch claims.
    for by_config in [true, false] {
        for local in refs.iter().filter(|r| r.kind == RefKind::Local) {
            let upstream = upstream_of(local);
            if by_config == upstream.is_empty() || pairs.contains_key(&(*local as *const GitRef)) {
                continue;
            }
            let candidates: Vec<&&GitRef> = refs
                .iter()
                .filter(|r| r.kind == RefKind::Remote && r.sha == local.sha && !used.contains(&(**r as *const GitRef)))
                .filter(|r| if by_config { r.name == upstream } else { !configured.contains(&r.name) && r.name.split_once('/').map(|x| x.1) == Some(local.name.as_str()) })
                .collect();
            if candidates.len() == 1 {
                used.insert(*candidates[0] as *const GitRef);
                pairs.insert(*local as *const GitRef, candidates[0]);
            }
        }
    }
    refs.iter().filter(|r| !used.contains(&(**r as *const GitRef))).map(|r| (*r, pairs.get(&(*r as *const GitRef)).copied())).collect()
}

/// Rows marked while a label is hovered: its commit, and for a branch the unlabelled commits below
/// it along the first parent up to the next labelled one.
pub fn hover_segment(repo: &RepoView, index: &HashMap<String, usize>, labelled: &HashSet<&str>, at: usize, tag: bool) -> HashSet<usize> {
    let mut out = HashSet::new();
    let commits = &repo.data.commits;
    if at >= commits.len() {
        return out;
    }
    out.insert(at);
    if tag {
        return out;
    }
    let mut i = at;
    while let Some(parent) = commits[i].parents.first() {
        match index.get(parent) {
            Some(&next) if next > i && !labelled.contains(commits[next].sha.as_str()) => {
                out.insert(next);
                i = next;
            }
            _ => break,
        }
    }
    out
}

/// Widths of the columns for a pane `pane` wide.
struct Layout {
    refs: f32,
    graph: f32,
    msg: f32,
    extras: Vec<(Col, f32)>,
    lanes_w: f32,
}

impl Layout {
    fn content(&self) -> f32 {
        self.refs + self.graph + self.msg + self.extras.iter().map(|e| e.1).sum::<f32>()
    }
}

fn layout(cols: &Columns, widths: &HashMap<String, f32>, lanes: u16, pane: f32) -> Layout {
    let lanes_w = GUTTER + (lanes.max(1) as f32) * LANE + PAD * 2.0;
    let min = |c: Col| match c {
        Col::Refs => m::REF_ZONE_MIN_WIDTH as f32,
        Col::Graph => lanes_w,
        Col::Message => m::MESSAGE_ZONE_MIN_WIDTH as f32,
        Col::Author => 80.0,
        Col::Date => 100.0,
        Col::Sha => 60.0,
        Col::Changes => 80.0,
    };
    let max = |c: Col| min(c).max(if c == Col::Message { 10000.0 } else { 1600.0 });
    let sized = |c: Col, fallback: f32| widths.get(c.key()).filter(|w| w.is_finite()).map_or(fallback, |w| w.clamp(min(c), max(c)));
    let custom = widths.values().any(|w| w.is_finite());
    let extra_w = |c: Col| match c {
        Col::Author => sized(c, m::AUTHOR_ZONE_DEFAULT_WIDTH as f32),
        Col::Date => sized(c, m::DATE_TIME_ZONE_DEFAULT_WIDTH as f32),
        Col::Sha => sized(c, m::SHA_ZONE_WIDTH as f32),
        _ => sized(c, m::CHANGES_ZONE_DEFAULT_WIDTH as f32),
    };
    let wanted: Vec<Col> = [(Col::Author, cols.author), (Col::Date, cols.date), (Col::Sha, cols.sha), (Col::Changes, cols.changes)].into_iter().filter(|(_, on)| *on).map(|(c, _)| c).collect();
    let base_ref = if cols.refs { sized(Col::Refs, m::REF_ZONE_DEFAULT_WIDTH as f32) } else { 0.0 };
    let base_graph = if cols.graph { sized(Col::Graph, (m::GRAPH_ZONE_DEFAULT_WIDTH as f32).min(108.0)) } else { 0.0 };
    let ref_min = if cols.refs { m::REF_ZONE_MIN_WIDTH as f32 } else { 0.0 };
    let graph_min = if cols.graph { lanes_w } else { 0.0 };
    let msg_min = m::MESSAGE_ZONE_MIN_WIDTH as f32;
    let mut extras: Vec<(Col, f32)> = Vec::new();
    for c in wanted {
        let used: f32 = extras.iter().map(|e| e.1).sum::<f32>() + extra_w(c);
        if custom || pane - used - ref_min - graph_min >= msg_min {
            extras.push((c, extra_w(c)));
        }
    }
    let extras_w: f32 = extras.iter().map(|e| e.1).sum();
    let roomy = custom || pane - extras_w >= base_ref + base_graph + m::MESSAGE_ZONE_DEFAULT_WIDTH as f32;
    let graph = if !cols.graph {
        0.0
    } else if roomy {
        base_graph.max(lanes_w)
    } else {
        lanes_w.min(pane - extras_w - ref_min - msg_min).max(0.0)
    };
    let refs = if !cols.refs {
        0.0
    } else if roomy {
        base_ref
    } else {
        ref_min.max((m::REF_ZONE_DEFAULT_WIDTH as f32).min((pane - extras_w) * 0.42).min(pane - extras_w - graph - msg_min)).round()
    };
    let msg = sized(Col::Message, msg_min.max(pane - refs - graph - extras_w));
    Layout { refs, graph, msg, extras, lanes_w }
}

/// The selection moved by the keyboard (j/k, arrows, Home/End; with Shift along the history).
fn keyboard(r: &mut RepoTab, ctx: &egui::Context) {
    let Some(v) = &r.view else { return };
    if ctx.egui_wants_keyboard_input() || super::dialog_open(ctx) {
        return;
    }
    let wip = !v.data.status.staged.is_empty() || !v.data.status.unstaged.is_empty();
    let commits = &v.data.commits;
    let off = usize::from(wip);
    let total = commits.len() + off;
    if total == 0 {
        return;
    }
    let current = match r.ui.selection() {
        Selection::Wip if wip => Some(0),
        Selection::Wip => None,
        Selection::Commit(sha) => r.index.get(&sha).map(|i| i + off),
    };
    let (down, up, home, end, shift, esc) = ctx.input(|i| {
        let k = |key| i.key_pressed(key);
        (k(egui::Key::J) || k(egui::Key::ArrowDown), k(egui::Key::K) || k(egui::Key::ArrowUp), k(egui::Key::Home), k(egui::Key::End), i.modifiers.shift, k(egui::Key::Escape))
    });
    let row_sel = |row: usize| if row < off { Selection::Wip } else { Selection::Commit(commits[row - off].sha.clone()) };
    let next = if esc && (!r.ui.multi.is_empty() || matches!(r.ui.selection(), Selection::Commit(_))) {
        r.ui.multi.clear();
        Some(Selection::Wip)
    } else if home {
        Some(row_sel(0))
    } else if end {
        Some(row_sel(total - 1))
    } else if (down || up) && shift {
        // Along the history: first parent going down, the only child going up.
        match (r.ui.selection(), down) {
            (Selection::Wip, true) => commits.iter().find(|c| c.sha == v.data.head_sha).map(|c| Selection::Commit(c.sha.clone())),
            (Selection::Wip, false) => None,
            (Selection::Commit(sha), true) => r.commit(&sha).and_then(|c| c.parents.first()).filter(|p| r.index.contains_key(*p)).map(|p| Selection::Commit(p.clone())),
            (Selection::Commit(sha), false) => {
                let children: Vec<&str> = commits.iter().filter(|c| c.parents.contains(&sha)).map(|c| c.sha.as_str()).collect();
                if sha == v.data.head_sha && wip {
                    children.is_empty().then_some(Selection::Wip)
                } else if children.len() == 1 {
                    Some(Selection::Commit(children[0].to_owned()))
                } else {
                    None
                }
            }
        }
    } else if down {
        Some(row_sel(current.map_or(0, |c| (c + 1).min(total - 1))))
    } else if up {
        Some(row_sel(current.map_or(0, |c| c.saturating_sub(1))))
    } else {
        None
    };
    if let Some(sel) = next {
        r.ui.select(sel);
    }
}

/// A line from (x1, y1) to (x2, y2); a lane change turns with a rounded corner. `down`: the turn
/// is at the top (the line leaves the node sideways), otherwise at the bottom.
fn curve(points: &mut Vec<Pos2>, a: Pos2, b: Pos2, down: bool) {
    points.clear();
    if (a.x - b.x).abs() < 0.5 {
        points.extend([a, b]);
        return;
    }
    let dir = (b.x - a.x).signum();
    let r = ARC.min((b.x - a.x).abs()).min((b.y - a.y).abs());
    let arc = |c: Pos2, from: f32, to: f32, out: &mut Vec<Pos2>| {
        for k in 0..=8 {
            let t = from + (to - from) * k as f32 / 8.0;
            out.push(pos2(c.x + r * t.cos(), c.y + r * t.sin()));
        }
    };
    use std::f32::consts::{FRAC_PI_2, PI};
    if down {
        // Sideways from a, then down into b.
        points.push(a);
        let center = pos2(b.x - dir * r, a.y + r);
        if dir > 0.0 { arc(center, -FRAC_PI_2, 0.0, points) } else { arc(center, -FRAC_PI_2, -PI, points) }
        points.push(b);
    } else {
        // Down from a, then sideways into b.
        points.push(a);
        let center = pos2(a.x + dir * r, b.y - r);
        if dir > 0.0 { arc(center, PI, FRAC_PI_2, points) } else { arc(center, 0.0, FRAC_PI_2, points) }
        points.push(b);
    }
}

fn dashed_circle(painter: &egui::Painter, c: Pos2, r: f32, stroke: Stroke) {
    let n = ((2.0 * std::f32::consts::PI * r) / 6.0).round().max(6.0) as usize;
    for k in 0..n {
        let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = (k as f32 + 0.5) / n as f32 * std::f32::consts::TAU;
        let pts: Vec<Pos2> = (0..=3).map(|j| {
            let t = a0 + (a1 - a0) * j as f32 / 3.0;
            pos2(c.x + r * t.cos(), c.y + r * t.sin())
        }).collect();
        painter.add(egui::Shape::line(pts, stroke));
    }
}

impl App {
    pub(crate) fn graph_view(&self, ui: &mut Ui, tab: u64, r: &mut RepoTab, later: &mut Vec<Later>) {
        let ctx = ui.ctx().clone();
        keyboard(r, &ctx);
        let Some(view) = r.view.take() else { return };
        self.graph_inner(ui, tab, r, &view, later);
        r.view = Some(view);
    }

    fn graph_inner(&self, ui: &mut Ui, tab: u64, r: &mut RepoTab, v: &RepoView, later: &mut Vec<Later>) {
        let c = p();
        let path = v.path.clone();
        let cols: Columns = self.settings.get("columns").unwrap_or_default();
        let show_worktree_wip = self.settings.flag("worktreeWipInGraph", true) && v.data.worktrees.len() > 1;
        // The branch being dragged follows the pointer.
        if let Some(dragged) = egui::DragAndDrop::payload::<GitRef>(ui.ctx())
            && let Some(at) = ui.ctx().pointer_latest_pos()
        {
            let layer = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::NULL.with("dragged_ref")));
            let g = layer.layout_no_wrap(dragged.name.clone(), FontId::proportional(12.5), c.white(1.0));
            let r = Rect::from_min_size(at + vec2(14.0, 6.0), g.size() + vec2(14.0, 6.0));
            layer.rect_filled(r, CornerRadius::same(3), ACCENT.gamma_multiply(0.85));
            layer.galley(r.min + vec2(7.0, 3.0), g, c.white(1.0));
        }
        let widths: HashMap<String, f32> = self.settings.get("graphColumnWidths").unwrap_or_default();
        let lazy = self.settings.flag("lazyLoad", true);
        let ghost_on = self.settings.flag("ghostRefs", true);
        let highlight_hover = self.settings.flag("highlightRowsOnHover", true);
        let date_fmt = self.settings.string("dateTimeFormat", "MM/DD/YYYY @ h:mm A");
        let hidden = self.hidden_refs(&path);
        let solo = self.solo_refs(&path);
        let by_sha = refs_by_commit(&v.data.refs, v.data.head_branch.as_deref(), &hidden);
        // The same counts the Agents view shows for the working changes.
        let (wip_total, wip_added, wip_deleted, wip_modified) = super::right::working_changes(&v.data.status);
        let wip = wip_total > 0;
        let off = usize::from(wip);
        let commits = &v.data.commits;
        let total = commits.len() + off + usize::from(v.data.has_more);
        let head_idx = r.index.get(&v.data.head_sha).copied();
        let head_lane = head_idx.and_then(|i| v.graph.rows.get(i)).map_or(0, |row| row.col as usize);
        let lanes = WipLanes::new(wip, self.pinned_ref(&path).is_some(), head_lane, head_idx, commits.len());
        let q = r.ui.search.trim().to_lowercase();
        let matches: Vec<usize> = if q.is_empty() {
            Vec::new()
        } else {
            commits.iter().enumerate().filter(|(_, c)| c.summary.to_lowercase().contains(&q) || c.author.to_lowercase().contains(&q) || c.sha.starts_with(&q) || c.body.to_lowercase().contains(&q)).map(|(i, _)| i).collect()
        };
        let match_set: HashSet<usize> = matches.iter().copied().collect();
        // Stash rows: drawn as a dashed box joined to the commit they were made on by a dashed
        // line, with the stash's menu. (lane, stash row, commit row) for each.
        let stash_of: HashMap<&str, &gitcraft_core::git::Stash> = v.data.stashes.iter().map(|s| (s.sha.as_str(), s)).collect();
        let stash_lines: Vec<(u16, usize, usize)> = commits
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let s = stash_of.get(c.sha.as_str())?;
                Some((v.graph.rows.get(i)?.col, i, *r.index.get(&s.base)?))
            })
            .collect();
        let dashed_at = |row: usize, lane: u16, up: bool| stash_lines.iter().any(|(l, from, to)| *l == lane && if up { row > *from && row <= *to } else { row >= *from && row < *to });

        // ---- Header row (and the bars under it) ----
        let pane_w = ui.available_width();
        let lay = layout(&cols, &widths, v.graph.width + lanes.extra(), pane_w - 12.0);
        let scroll_id = ui.scope_id().with(("graph_scroll", tab));
        let scroll_x = ui.data(|d| d.get_temp::<f32>(scroll_id.with("x"))).unwrap_or(0.0);
        let (head_rect, _) = ui.allocate_exact_size(vec2(pane_w, HEADER), Sense::hover());
        ui.painter().rect_filled(head_rect, CornerRadius::ZERO, c.panel);
        ui.painter().line_segment([head_rect.left_bottom(), head_rect.right_bottom()], Stroke::new(1.0, c.black(0.3)));
        let header_painter = ui.painter().with_clip_rect(head_rect);
        let font = FontId::monospace(10.0);
        let mut x = head_rect.left() - scroll_x;
        let custom = widths.values().any(|w| w.is_finite());
        let mut resize: Option<(Col, Option<f32>)> = None;
        let mut handle = |ui: &mut Ui, col: Col, right: f32, width: f32| {
            let rect = Rect::from_min_max(pos2(right - 3.0, head_rect.top()), pos2(right + 3.0, head_rect.bottom()));
            let id = ui.scope_id().with(("col_resize", col.key()));
            // A slider for keyboards and screen readers: Left / Right resize (Shift: by more),
            // Delete or Backspace resets, as do double-clicks.
            let resp = ui.interact(rect, id, Sense::click_and_drag() | Sense::FOCUSABLE);
            let name = format!("{} column width", col.key());
            resp.widget_info(|| egui::WidgetInfo::slider(true, f64::from(width.round()), &name));
            ui.ctx().accesskit_node_builder(resp.id, |node| node.set_description("Drag, or use the Left and Right arrow keys, to resize. Double-click or press Delete to reset."));
            let focused = resp.has_focus();
            if resp.hovered() || resp.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
            }
            if resp.hovered() || resp.dragged() || focused {
                ui.painter().rect_filled(Rect::from_center_size(rect.center(), vec2(2.0, rect.height())), CornerRadius::ZERO, ACCENT);
            }
            let keys = if focused { ui.input(|i| (i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight), i.modifiers.shift, i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))) } else { (false, false, false, false) };
            let step = if keys.2 { 50.0 } else { 10.0 };
            if resp.double_clicked() || keys.3 {
                resize = Some((col, None));
            } else if resp.dragged() && resp.drag_delta().x != 0.0 {
                resize = Some((col, Some(width + resp.drag_delta().x)));
            } else if keys.0 || keys.1 {
                resize = Some((col, Some(width + if keys.1 { step } else { -step })));
            }
        };
        if cols.refs {
            header_painter.text(pos2(x + 15.0, head_rect.center().y), Align2::LEFT_CENTER, "BRANCH / TAG", font.clone(), c.fg3);
            x += lay.refs;
            handle(ui, Col::Refs, x, lay.refs);
        }
        if cols.graph {
            let rect = Rect::from_min_size(pos2(x, head_rect.top()), vec2(lay.graph, HEADER));
            icons::paint(ui, rect, Icon::Network, 13.0, c.white(0.7));
            // With a pinned branch, a pin at the column's right end jumps to it.
            if let Some(pinned) = self.pinned_ref(&path)
                && let Some(sha) = v.data.refs.iter().find(|g| g.full == pinned || g.name == pinned).map(|g| g.sha.clone())
                && lay.graph >= 48.0
            {
                let pin = Rect::from_min_size(pos2(rect.right() - 22.0, rect.top()), vec2(20.0, HEADER));
                let pr = ui.interact(pin, ui.scope_id().with("pinned_jump"), Sense::click());
                icons::paint(ui, pin, Icon::Pin, 12.0, if pr.hovered() { c.white(1.0) } else { ACCENT });
                if pr.tip("Go to the pinned branch").clicked() {
                    later.push(Box::new(move |app| {
                        if let Some(r) = app.repo_mut(tab) {
                            r.ui.select(Selection::Commit(sha));
                        }
                    }));
                }
            }
            x += lay.graph;
            handle(ui, Col::Graph, x, lay.graph);
        }
        let msg_left = x;
        if lay.msg >= 130.0 {
            let clip = Rect::from_min_max(pos2(x, head_rect.top()), pos2(x + lay.msg - 64.0, head_rect.bottom())).intersect(head_rect);
            ui.painter().with_clip_rect(clip).text(pos2(x + SUMMARY_MARGIN, head_rect.center().y), Align2::LEFT_CENTER, "COMMIT MESSAGE", font.clone(), c.fg3);
        }
        x += lay.msg;
        // Column settings, description toggle and hidden refs at the message column's right end.
        // Kept inside the header however far the columns are scrolled, even in a narrow pane.
        let tools_right = (x - 8.0).max(head_rect.left() + 70.0).min(head_rect.right() - 8.0);
        let mut tx = tools_right;
        let gear = Rect::from_min_size(pos2(tx - 22.0, head_rect.top()), vec2(22.0, HEADER));
        tx -= 22.0;
        let gear_resp = ui.interact(gear, ui.scope_id().with("columns_menu"), Sense::click());
        icons::paint(ui, gear, Icon::Settings2, 13.0, if gear_resp.hovered() { c.white(1.0) } else { c.white(0.7) });
        let gear_resp = gear_resp.tip("Column settings");
        egui::Popup::menu(&gear_resp).show(|ui| {
            let mut entries = vec![Entry::Heading("Columns".into())];
            for (key, label, on) in [("refs", "Branch / Tag", cols.refs), ("graph", "Graph", cols.graph), ("description", "Commit Description", cols.description), ("author", "Author", cols.author), ("date", "Commit Date / Time", cols.date), ("sha", "Sha", cols.sha), ("changes", "Changes", cols.changes)] {
                entries.push(item(format!("{}{label}", if on { "✓ " } else { "   " }), move |app| {
                    let mut cols: Columns = app.settings.get("columns").unwrap_or_default();
                    match key {
                        "refs" => cols.refs = !on,
                        "graph" => cols.graph = !on,
                        "description" => cols.description = !on,
                        "author" => cols.author = !on,
                        "date" => cols.date = !on,
                        "sha" => cols.sha = !on,
                        _ => cols.changes = !on,
                    }
                    app.settings.set("columns", cols);
                }));
            }
            entries.push(Entry::Separator);
            entries.push(item("Reset column widths", |app| app.settings.set("graphColumnWidths", serde_json::json!({}))));
            entries.push(item("Reset columns to default layout", |app| {
                app.settings.set("columns", Columns::default());
                app.settings.set("graphColumnWidths", serde_json::json!({}));
            }));
            entries.push(item("Reset columns to compact layout", |app| {
                app.settings.set("columns", Columns { description: false, ..Columns::default() });
                app.settings.set("graphColumnWidths", serde_json::json!({}));
            }));
            entries.push(Entry::Separator);
            entries.push(item(format!("{}Lazy Load Commits in Graph", if lazy { "✓ " } else { "   " }), move |app| app.settings.set("lazyLoad", !lazy)));
            ctxmenu::show(ui, entries, later);
        });
        let desc = Rect::from_min_size(pos2(tx - 22.0, head_rect.top()), vec2(22.0, HEADER));
        tx -= 22.0;
        let desc_resp = ui.interact(desc, ui.scope_id().with("desc_toggle"), Sense::click()).tip("Show commit description in graph");
        icons::paint(ui, desc, Icon::MessageSquare, 13.0, if cols.description || desc_resp.hovered() { c.white(1.0) } else { c.white(0.5) });
        if desc_resp.clicked() {
            later.push(Box::new(move |app| {
                let mut cols: Columns = app.settings.get("columns").unwrap_or_default();
                cols.description = !cols.description;
                app.settings.set("columns", cols);
            }));
        }
        let hidden_refs: Vec<&GitRef> = v.data.refs.iter().filter(|r| hidden.contains(&r.full)).collect();
        if !hidden_refs.is_empty() {
            let label = format!("{}", hidden_refs.len());
            let w = 30.0 + label.len() as f32 * 6.0;
            let eye = Rect::from_min_size(pos2(tx - w, head_rect.top()), vec2(w, HEADER));
            let eye_resp = ui.interact(eye, ui.scope_id().with("hidden_menu"), Sense::click()).tip("Hidden Branches / Tags");
            let ink = if eye_resp.hovered() { c.white(1.0) } else { WARN };
            icons::paint(ui, Rect::from_min_size(pos2(eye.left() + 4.0, eye.center().y - 6.5), Vec2::splat(13.0)), Icon::EyeOff, 13.0, ink);
            ui.painter().text(pos2(eye.left() + 21.0, eye.center().y), Align2::LEFT_CENTER, label, FontId::proportional(11.0), ink);
            egui::Popup::menu(&eye_resp).show(|ui| {
                let mut entries = vec![Entry::Heading("Hidden Branches / Tags".into())];
                for h in &hidden_refs {
                    let full = h.full.clone();
                    entries.push(item(format!("Show {}", h.name), move |app| app.toggle_ref_list(tab, "hidden", &full)));
                }
                entries.push(Entry::Separator);
                entries.push(item("Show all", move |app| app.clear_list(tab, "hidden")));
                ctxmenu::show(ui, entries, later);
            });
        }
        handle(ui, Col::Message, x, lay.msg);
        for (col, w) in &lay.extras {
            ui.painter().with_clip_rect(head_rect).line_segment([pos2(x, head_rect.top()), pos2(x, head_rect.bottom())], Stroke::new(1.0, c.white(0.05)));
            let title = match col {
                Col::Author => "AUTHOR",
                Col::Date => "COMMIT DATE / TIME",
                Col::Sha => "SHA",
                _ => "CHANGES",
            };
            header_painter.text(pos2(x + 8.0, head_rect.center().y), Align2::LEFT_CENTER, title, font.clone(), c.fg3);
            x += w;
            handle(ui, *col, x, *w);
        }
        if let Some((col, value)) = resize {
            let (refs_now, graph_now) = (lay.refs, lay.graph);
            let (refs_on, graph_on) = (cols.refs, cols.graph);
            let min = match col {
                Col::Refs => m::REF_ZONE_MIN_WIDTH as f32,
                Col::Graph => lay.lanes_w,
                Col::Message => m::MESSAGE_ZONE_MIN_WIDTH as f32,
                Col::Author => 80.0,
                Col::Date => 100.0,
                Col::Sha => 60.0,
                Col::Changes => 80.0,
            };
            later.push(Box::new(move |app| {
                let mut w: HashMap<String, f32> = app.settings.get("graphColumnWidths").unwrap_or_default();
                match value {
                    None => {
                        w.remove(col.key());
                    }
                    Some(value) => {
                        if !custom {
                            if refs_on {
                                w.insert("refs".into(), refs_now);
                            }
                            if graph_on {
                                w.insert("graph".into(), graph_now);
                            }
                        }
                        w.insert(col.key().into(), value.clamp(min, if col == Col::Message { 10000.0 } else { 1600.0_f32.max(min) }).round());
                    }
                }
                app.settings.set("graphColumnWidths", w);
            }));
        }
        let _ = msg_left;

        // ---- Search, filters, solo, bisect and operation bars ----
        if r.ui.search_open {
            self.search_bar(ui, tab, r, v, &matches, later);
        }
        if let Some(f) = &r.ui.history_filter {
            bar(ui, 28.0, c.panel, |ui| {
                let parts: Vec<&str> = [f.author.as_deref(), f.path.as_deref(), f.text.as_deref()].into_iter().flatten().filter(|s| !s.is_empty()).collect();
                ui.add_space(12.0);
                ui.label(egui::RichText::new(format!("Filtered history · {}", parts.join(" · "))).size(12.0).color(c.fg2));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(12.0);
                    if widgets::link(ui, "Clear filters", 12.0).clicked() {
                        later.push(Box::new(move |app| app.set_history_filter(tab, None)));
                    }
                    if widgets::link(ui, "Edit", 12.0).clicked() {
                        later.push(Box::new(move |app| app.edit_history_filter(tab)));
                    }
                });
            });
        }
        if !solo.is_empty() {
            bar(ui, 28.0, WARN.gamma_multiply(0.2), |ui| {
                ui.add_space(12.0);
                let names: Vec<String> = solo.iter().map(|f| f.trim_start_matches("refs/heads/").trim_start_matches("refs/remotes/").trim_start_matches("refs/tags/").to_owned()).collect();
                ui.label(egui::RichText::new(format!("Soloing {} branch{}: {}", solo.len(), if solo.len() == 1 { "" } else { "es" }, names.join(", "))).size(12.0).color(c.fg));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(12.0);
                    if widgets::button(ui, Kind::Default, None, "Stop soloing", 20.0, true).clicked() {
                        later.push(Box::new(move |app| app.clear_solo(tab)));
                    }
                });
            });
        }
        if let Some(b) = &v.data.bisect {
            bar(ui, 30.0, WARN.gamma_multiply(0.1), |ui| {
                ui.add_space(12.0);
                let text = match &b.first_bad {
                    Some(bad) => format!("First bad commit {}", &bad[..10.min(bad.len())]),
                    None => format!("Bisect in progress · {} marked · testing {}", b.tested, &v.data.head_sha[..10.min(v.data.head_sha.len())]),
                };
                ui.label(egui::RichText::new(text).size(12.0).color(c.fg));
                ui.add_space(8.0);
                if b.first_bad.is_none() {
                    for mode in ["good", "bad", "skip"] {
                        let label = format!("{}{}", mode[..1].to_uppercase(), &mode[1..]);
                        if widgets::link(ui, &label, 12.0).clicked() {
                            let head = v.data.head_sha.clone();
                            later.push(Box::new(move |app| app.run_bisect(tab, mode, Some(head), None)));
                        }
                    }
                }
                if widgets::link(ui, "Reset bisect", 12.0).clicked() {
                    later.push(Box::new(move |app| app.run_bisect(tab, "reset", None, None)));
                }
            });
        }
        if let Some(note) = &v.data.state_note {
            state_banner(ui, tab, v, note, later);
        }

        // ---- Rows ----
        let lanes_x = |l: usize, refs: f32| refs + GUTTER + l as f32 * LANE + LANE / 2.0;
        let labelled: HashSet<&str> = by_sha.keys().copied().collect();
        let hover_ref_id = ui.scope_id().with(("hover_ref", tab));
        // The row of the hovered label, and whether that label is a tag (a tag highlights only
        // its commit, a branch its line).
        let hover_ref: Option<(usize, bool)> = ui.data(|d| d.get_temp(hover_ref_id)).flatten();
        let highlighted = match hover_ref {
            Some((i, tag)) if highlight_hover => hover_segment(v, &r.index, &labelled, i, tag),
            _ => HashSet::new(),
        };
        let hover_row = hover_ref.map(|(i, _)| i);
        let hover_active = !highlighted.is_empty();
        let mut new_hover_ref: Option<(usize, bool)> = None;
        // Rows outside a hovered label fade after a 1 s pause, over half a second.
        let clock = ui.input(|i| i.time);
        let since_id = hover_ref_id.with("since");
        let since = match ui.data(|d| d.get_temp::<(Option<usize>, f64)>(since_id)) {
            Some((was, at)) if was == hover_row => at,
            _ => {
                ui.data_mut(|d| d.insert_temp(since_id, (hover_row, clock)));
                clock
            }
        };
        let dim_t = if hover_active { (((clock - since) - 1.0) / 0.5).clamp(0.0, 1.0) as f32 } else { 0.0 };
        if hover_active && dim_t < 1.0 {
            ui.ctx().request_repaint();
        }
        // The stack of every label on a commit, shown while one of its labels is hovered: the
        // row it belongs to and when the pointer was last on it.
        let stack_id = ui.scope_id().with(("ref_stack", tab));
        let mut stack: Option<(usize, f64)> = ui.data(|d| d.get_temp(stack_id)).flatten();
        let selected_idx = match r.ui.selection() {
            Selection::Wip => wip.then_some(0),
            Selection::Commit(sha) => r.index.get(&sha).map(|i| i + off),
        };
        let multi: HashSet<&str> = r.ui.multi.iter().map(String::as_str).collect();
        let content_w = lay.content();
        let mut scroll = egui::ScrollArea::both().id_salt(("graph", tab)).auto_shrink([false, false]).content_margin(0);
        let view_h = ui.available_height();
        if r.ui.reveal {
            r.ui.reveal = false;
            if let Some(sel) = selected_idx {
                let state = egui::scroll_area::State::load(ui.ctx(), ui.scope_id().with(egui::Id::NULL.with(("graph", tab))));
                let top = sel as f32 * ROW;
                if let Some(state) = state {
                    let y = state.clamped_offset().y;
                    if top < y {
                        scroll = scroll.vertical_scroll_offset(top);
                    } else if top + ROW > y + view_h {
                        scroll = scroll.vertical_scroll_offset(top + ROW - view_h);
                    }
                } else if top + ROW > view_h {
                    scroll = scroll.vertical_scroll_offset(top + ROW - view_h);
                }
            }
        }
        let now = chrono::Local::now().timestamp();
        let overflowing = total as f32 * ROW > view_h;
        let timelines = if overflowing { widgets::timeline(&commits.iter().map(|c| c.time).collect::<Vec<_>>(), now) } else { HashMap::new() };
        let mut clicked_row: Option<(usize, egui::Modifiers)> = None;
        let mut want_more = false;
        let mut want_stats: Vec<String> = Vec::new();
        let mut wip_draft: Option<String> = None;
        let mut wip_commit: Option<bool> = None;
        let draft = self.draft(&path);
        let stats = &r.stats;
        // Markers beside the scroll bar for where branches, tags, the checked-out commit, the
        // selection and the search matches sit in the whole list (Preferences > UI Customization;
        // the selection and the matches also show while a search is on).
        let all_markers = self.settings.flag("scrollMarkers", false);
        let show_markers = overflowing && (all_markers || !matches.is_empty());
        if show_markers {
            scroll = scroll.max_width((ui.available_width() - MARKER_STRIP).max(0.0));
        }
        let output = scroll.show_viewport(ui, |ui, viewport| {
            let origin = ui.min_rect().min;
            let (_, _) = ui.allocate_exact_size(vec2(content_w.max(ui.available_width()), (total as f32 * ROW).max(viewport.height())), Sense::hover());
            let painter = ui.painter().clone();
            let first = ((viewport.top() / ROW).floor() as isize - 4).max(0) as usize;
            let last = (((viewport.bottom() / ROW).ceil() as usize) + 4).min(total.saturating_sub(1));
            if total == 0 {
                painter.text(origin + viewport.center().to_vec2(), Align2::CENTER_CENTER, "No commits yet — make the first one on the right.", FontId::proportional(13.0), c.fg3);
                return;
            }
            let full_w = content_w.max(viewport.width() + viewport.left());
            let row_rect = |d: usize| Rect::from_min_size(origin + vec2(0.0, d as f32 * ROW), vec2(full_w, ROW));
            // Row backgrounds, labels and text.
            for d in first..=last {
                let rect = row_rect(d);
                if d < off {
                    let sel = matches!(r.ui.selection(), Selection::Wip) && r.ui.multi.is_empty();
                    let resp = ui.interact(rect, ui.scope_id().with(("wip_row", tab)), Sense::click());
                    if sel {
                        painter.rect_filled(rect, CornerRadius::ZERO, c.selrow);
                    } else if resp.hovered() {
                        painter.rect_filled(rect, CornerRadius::ZERO, c.white(0.03));
                    }
                    if resp.clicked() {
                        clicked_row = Some((d, egui::Modifiers::NONE));
                    }
                    // The "// WIP" box, sized to its text, and the change counts.
                    let left = rect.left() + lay.refs + lay.graph + SUMMARY_MARGIN;
                    let mut text = draft.summary.clone();
                    let font = FontId::proportional(12.5);
                    let text_w = ui.painter().layout_no_wrap(text.clone(), font.clone(), c.fg).size().x;
                    let room = (lay.msg - SUMMARY_MARGIN - m::WIP_BOX_STATS_SPACE as f32).max(100.0);
                    let box_w = (text_w + 35.0).clamp(100.0, room);
                    let box_rect = Rect::from_min_size(pos2(left + 3.0, rect.center().y - 11.0), vec2(box_w, 22.0));
                    let id = ui.scope_id().with(("wip_input", tab));
                    let focused = ui.memory(|mem| mem.has_focus(id));
                    painter.rect(box_rect, CornerRadius::same(3), c.field, Stroke::new(1.0, if focused { ACCENT } else { c.line2 }), StrokeKind::Inside);
                    let edit = egui::TextEdit::singleline(&mut text).id(id).frame(egui::Frame::NONE).font(font).hint_text(egui::RichText::new("// WIP").color(c.fg3)).align(Align2::LEFT_CENTER).desired_width(box_w - 12.0);
                    let er = ui.put(box_rect.shrink2(vec2(6.0, 0.0)), edit);
                    if er.gained_focus() {
                        clicked_row = Some((d, egui::Modifiers::NONE));
                    }
                    if er.changed() {
                        wip_draft = Some(text.clone());
                    }
                    if er.has_focus() {
                        let (enter, shift, cmd) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.modifiers.shift, i.modifiers.command));
                        if enter && cmd {
                            wip_commit = Some(shift);
                        }
                    }
                    widgets::counts(ui, &painter, box_rect.right() + 10.0, rect.center().y, [wip_modified, wip_added, wip_deleted], 13.0);
                    continue;
                }
                let i = d - off;
                let (Some(commit), Some(gr)) = (commits.get(i), v.graph.rows.get(i)) else {
                    if v.data.has_more && i == commits.len() {
                        let center = rect.center_top() + vec2(0.0, ROW / 2.0);
                        let center = pos2(origin.x + viewport.center().x, center.y);
                        if let Some(err) = &r.page_error {
                            let resp = ui.put(Rect::from_center_size(center, vec2(220.0, ROW)), egui::Label::new(egui::RichText::new("Could not load history. Retry").size(12.0).color(c.link)).sense(Sense::click())).tip(err);
                            if resp.clicked() {
                                want_more = true;
                            }
                        } else if !lazy {
                            let resp = ui.put(Rect::from_center_size(center, vec2(220.0, ROW)), egui::Label::new(egui::RichText::new("Load more commits").size(12.0).color(c.link)).sense(Sense::click()));
                            if resp.clicked() {
                                want_more = true;
                            }
                        } else {
                            painter.text(center, Align2::CENTER_CENTER, "Loading more commits…", FontId::proportional(12.0), c.fg3);
                            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
                        }
                    }
                    continue;
                };
                if lazy && v.data.has_more && !r.page_loading && r.page_error.is_none() && i + m::LAZY_LOAD_OFFSET as usize >= commits.len() {
                    want_more = true;
                }
                let color = lane_color(gr.color as usize);
                let sel = (selected_idx == Some(d) && r.ui.multi.is_empty()) || multi.contains(commit.sha.as_str());
                let dim = !q.is_empty() && !match_set.contains(&i);
                let fade = if dim { MIN_ALPHA } else { 1.0 };
                // Hovered anywhere on the row, its labels included.
                let hovered = ui.rect_contains_pointer(rect);
                if sel {
                    painter.rect_filled(rect, CornerRadius::ZERO, c.selrow);
                } else if hovered {
                    painter.rect_filled(rect, CornerRadius::ZERO, c.hoverrow);
                }
                let resp = commit_row_target(ui, rect, ui.scope_id().with(("row", tab, &commit.sha)), &commit.summary, sel);
                if resp.clicked() {
                    clicked_row = Some((d, click_modifiers(ui, &resp)));
                }
                let commit_for_menu = commit.clone();
                match stash_of.get(commit.sha.as_str()) {
                    Some(st) => ctxmenu::attach(&resp, later, || self.stash_menu(tab, st)),
                    None => ctxmenu::attach(&resp, later, || self.commit_menu(tab, v, &commit_for_menu)),
                }
                // Timeline line and label.
                if d > 0 && (!cols.date || !q.is_empty())
                    && let Some(label) = timelines.get(&i)
                {
                    painter.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, c.line));
                    let g = painter.layout_no_wrap(label.clone(), FontId::proportional(10.0), c.fg2);
                    let right = origin.x + viewport.right();
                    let lr = Rect::from_min_size(pos2(right - g.size().x - 12.0, rect.top() - 8.0), g.size() + vec2(12.0, 1.0));
                    painter.rect_filled(lr, CornerRadius::same(2), c.panel);
                    painter.galley(lr.min + vec2(6.0, 0.0), g, c.fg2);
                }
                let node_x = rect.left() + lanes_x(lanes.row(gr.col as usize, i), lay.refs);
                // Labels.
                let refs = by_sha.get(commit.sha.as_str());
                if lay.refs > 0.0 {
                    let zone = Rect::from_min_size(rect.min, vec2(lay.refs, ROW));
                    let mut end_x = zone.left() + REF_MARGIN;
                    if let Some(refs) = refs {
                        let groups = group_refs(refs);
                        let (first, remote) = groups[0];
                        let more = groups.len() - 1;
                        let more_w = if more > 0 { 5.0 + 8.0 + format!("+{more}").len() as f32 * 6.5 } else { 0.0 };
                        let max_w = (zone.width() - REF_MARGIN - 6.0 - more_w).max(20.0);
                        let fill = widgets::mix(color, 0.28);
                        // Branch labels can be dragged onto another branch label: merge, rebase, fast-forward, push or
                        // start a pull request (see `ref_dropped`).
                        let draggable = first.kind != RefKind::Tag;
                        let (label_rect, lresp) = ref_label(ui, &painter, pos2(end_x, rect.center().y), max_w, first, remote, v, fill, fade, ui.scope_id().with(("ref", tab, &first.full)), if draggable { Sense::click_and_drag() } else { Sense::click() });
                        if draggable && lresp.drag_started() {
                            lresp.dnd_set_drag_payload(first.clone());
                        }
                        if let Some(source) = lresp.dnd_hover_payload::<GitRef>().filter(|s| s.full != first.full && first.kind != RefKind::Tag) {
                            let _ = source;
                            painter.rect_stroke(label_rect.expand(2.0), CornerRadius::same(4), Stroke::new(2.0, ACCENT), StrokeKind::Outside);
                        }
                        if let Some(source) = lresp.dnd_release_payload::<GitRef>().filter(|s| s.full != first.full && first.kind != RefKind::Tag) {
                            let (source, target) = ((*source).clone(), first.clone());
                            later.push(Box::new(move |app| app.ref_dropped(tab, source, target)));
                        }
                        end_x = label_rect.right();
                        if lresp.hovered() {
                            new_hover_ref = Some((i, first.kind == RefKind::Tag));
                            stack = Some((i, clock));
                        }
                        if lresp.clicked() {
                            clicked_row = Some((d, click_modifiers(ui, &lresp)));
                        }
                        if lresp.double_clicked() {
                            let target = first.clone();
                            later.push(Box::new(move |app| app.checkout_ref(tab, &target)));
                        }
                        let (f2, r2) = (first.clone(), remote.cloned());
                        let lresp = lresp.tip(describe_ref(first, v) + &r2.as_ref().map(|r| format!("; {}", describe_ref(r, v))).unwrap_or_default());
                        ctxmenu::attach(&lresp, later, || {
                            let mut items = self.branch_menu(tab, v, &f2);
                            if let Some(rem) = &r2 {
                                items.push(Entry::Separator);
                                items.push(ctxmenu::sub(format!("Remote branch: {}", rem.name), self.branch_menu(tab, v, rem)));
                            }
                            items
                        });
                        if more > 0 {
                            painter.line_segment([pos2(end_x, rect.center().y), pos2(end_x + 5.0, rect.center().y)], Stroke::new(2.0, color));
                            let mr = Rect::from_min_size(pos2(end_x + 5.0, rect.center().y - INNER / 2.0), vec2(more_w - 5.0, INNER));
                            painter.rect_filled(mr, CornerRadius::same(2), fill);
                            painter.text(mr.center(), Align2::CENTER_CENTER, format!("+{more}"), FontId::proportional(11.0), c.fg);
                            let mresp = ui.interact(mr, ui.scope_id().with(("more_refs", tab, i)), Sense::click());
                            end_x = mr.right();
                            if mresp.hovered() || mresp.clicked() {
                                new_hover_ref = Some((i, first.kind == RefKind::Tag));
                                stack = Some((i, clock));
                            }
                        }
                        // Another worktree on this commit with uncommitted changes: a pill with their count that opens
                        // it (Preferences > Agents > Show other worktrees' changes in the graph).
                        if show_worktree_wip {
                            for g in refs.iter().filter(|g| g.kind == RefKind::Local) {
                                let Some(wt) = v.data.worktrees.iter().find(|w| w.branch == g.name && !super::same_folder(&w.path, &v.path)) else { continue };
                                // Read again every 10 seconds (the request returns at once while a read is fresh).
                                let entry = r.agents.statuses.get(&wt.path);
                                if entry.is_none_or(|(_, at)| at.elapsed().as_secs() >= 10) {
                                    let p = wt.path.clone();
                                    later.push(Box::new(move |app| app.want_worktree_status(tab, p)));
                                }
                                let changes = match entry {
                                    Some((super::repo::Loaded::Ready(s), _)) => super::right::working_changes(s).0,
                                    _ => 0,
                                };
                                if changes == 0 || end_x + 40.0 > zone.right() {
                                    continue;
                                }
                                let label = format!("✎ {changes}");
                                let g2 = ui.painter().layout_no_wrap(label, FontId::proportional(11.0), c.white(0.95));
                                let pill = Rect::from_min_size(pos2(end_x + 4.0, rect.center().y - INNER / 2.0), vec2(g2.size().x + 10.0, INNER));
                                painter.rect(pill, CornerRadius::same(3), WARN.gamma_multiply(0.25), Stroke::new(1.0, WARN.gamma_multiply(0.8)), StrokeKind::Inside);
                                painter.galley(pos2(pill.left() + 5.0, pill.center().y - g2.size().y / 2.0), g2, c.white(0.95));
                                let presp = ui.interact(pill, ui.scope_id().with(("wt_wip", &wt.path)), Sense::click());
                                if presp.tip(format!("{changes} uncommitted change{} in the worktree at {}. Click to open it.", if changes == 1 { "" } else { "s" }, wt.path)).clicked() {
                                    let p = wt.path.clone();
                                    later.push(Box::new(move |app| app.open_repo(&p)));
                                }
                                end_x = pill.right();
                            }
                        }
                        if more > 0 && stack.is_some_and(|(row, _)| row == i) {
                            let shown = groups.len().min(m::REF_ZONE_MAX_REFS as usize);
                            let widest = groups.iter().take(shown).map(|(g, rem)| ref_width(ui, g, *rem, v)).fold(0.0_f32, f32::max);
                            let screen = ui.ctx().content_rect();
                            let w = (widest + 16.0).max(end_x - label_rect.left()).min(320.0).min(screen.width() - 16.0);
                            let desired = shown as f32 * INNER;
                            let below = screen.bottom() - label_rect.top() - 8.0;
                            let above = label_rect.bottom() - screen.top() - 8.0;
                            let upwards = desired > below && above > below;
                            let h = desired.min(if upwards { above } else { below }).max(INNER);
                            let top = if upwards { label_rect.bottom() - h } else { label_rect.top() };
                            let left = label_rect.left().min(screen.right() - w - 8.0).max(screen.left() + 8.0);
                            let mut stack_tag: Option<bool> = None;
                            egui::Area::new(stack_id.with("area")).order(egui::Order::Foreground).fixed_pos(pos2(left, top)).constrain(false).show(ui.ctx(), |ui| {
                                let (area, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
                                ui.painter().rect_filled(area, CornerRadius::same(2), fill);
                                let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(area).layout(egui::Layout::top_down(egui::Align::Min)));
                                inner.set_clip_rect(area);
                                egui::ScrollArea::vertical().id_salt(stack_id.with("scroll")).max_height(h).auto_shrink([false, true]).show(&mut inner, |ui| {
                                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                                    for (k, (g, rem)) in groups.iter().take(shown).enumerate() {
                                        let draggable = g.kind != RefKind::Tag;
                                        let (row, rresp) = ui.allocate_exact_size(vec2(w, INNER), if draggable { Sense::click_and_drag() } else { Sense::click() });
                                        if draggable && rresp.drag_started() {
                                            rresp.dnd_set_drag_payload((*g).clone());
                                        }
                                        let takes = |source: &GitRef| source.full != g.full && g.kind != RefKind::Tag;
                                        if rresp.dnd_hover_payload::<GitRef>().filter(|source| takes(source)).is_some() {
                                            ui.painter().rect_stroke(row, CornerRadius::ZERO, Stroke::new(2.0, ACCENT), StrokeKind::Inside);
                                        }
                                        if let Some(source) = rresp.dnd_release_payload::<GitRef>().filter(|source| takes(source)) {
                                            let (source, target) = ((*source).clone(), (*g).clone());
                                            later.push(Box::new(move |app| app.ref_dropped(tab, source, target)));
                                        }
                                        if rresp.hovered() {
                                            ui.painter().rect_filled(row, CornerRadius::ZERO, c.white(0.1));
                                            stack_tag = Some(g.kind == RefKind::Tag);
                                        }
                                        let chevron = if k == 0 { 14.0 } else { 0.0 };
                                        let painter = ui.painter().clone();
                                        let (lr, _) = ref_label(ui, &painter, pos2(row.left(), row.center().y), w - chevron, g, *rem, v, Color32::TRANSPARENT, 1.0, ui.scope_id().with(("stack_label", &g.full)), Sense::hover());
                                        if k == 0 {
                                            icons::paint(ui, Rect::from_min_size(pos2(lr.right(), row.center().y - 5.5), Vec2::splat(11.0)), Icon::ChevronDown, 11.0, c.fg.gamma_multiply(0.6));
                                        }
                                        if rresp.clicked() {
                                            clicked_row = Some((d, click_modifiers(ui, &rresp)));
                                        }
                                        if rresp.double_clicked() {
                                            let target = (*g).clone();
                                            later.push(Box::new(move |app| app.checkout_ref(tab, &target)));
                                        }
                                        let (g2, r2) = ((*g).clone(), rem.cloned());
                                        let rresp = rresp.tip(describe_ref(g, v) + &r2.as_ref().map(|r| format!("; {}", describe_ref(r, v))).unwrap_or_default());
                                        ctxmenu::attach(&rresp, later, || {
                                            let mut items = self.branch_menu(tab, v, &g2);
                                            if let Some(rem) = &r2 {
                                                items.push(Entry::Separator);
                                                items.push(ctxmenu::sub(format!("Remote branch: {}", rem.name), self.branch_menu(tab, v, rem)));
                                            }
                                            items
                                        });
                                    }
                                });
                                if ui.rect_contains_pointer(area) {
                                    new_hover_ref = Some((i, stack_tag.unwrap_or(first.kind == RefKind::Tag)));
                                    stack = Some((i, clock));
                                }
                            });
                        }
                    } else if ghost_on && (hovered || sel) {
                        // The branch this commit is on, faint and dashed.
                        let lane = gr.col;
                        let ghost = (0..i).rev().find_map(|j| {
                            let row = v.graph.rows.get(j)?;
                            (row.col == lane).then(|| by_sha.get(commits[j].sha.as_str()).and_then(|v| v.first().copied())).flatten()
                        });
                        if let Some(g) = ghost {
                            let galley = widgets::fit(ui, &g.name, FontId::proportional(12.5), c.fg.gamma_multiply(0.6), zone.width() - REF_MARGIN - 20.0);
                            let gr2 = Rect::from_min_size(pos2(end_x, rect.center().y - INNER / 2.0), vec2(galley.size().x + 10.0, INNER));
                            painter.rect_stroke(gr2, CornerRadius::same(3), Stroke::new(1.0, color.gamma_multiply(0.6)), StrokeKind::Inside);
                            painter.galley(pos2(gr2.left() + 5.0, gr2.center().y - galley.size().y / 2.0), galley, c.fg);
                            end_x = gr2.right();
                        }
                    }
                    if (refs.is_some() || end_x > zone.left() + REF_MARGIN) && lay.graph > 0.0 {
                        let clip = painter.with_clip_rect(Rect::from_min_max(zone.min, pos2(node_x - NODE / 2.0, zone.bottom())));
                        let (y, stroke) = (rect.center().y, Stroke::new(2.0, color.gamma_multiply(fade)));
                        // A gap where the connector crosses the working changes' dashed line.
                        if lanes.crosses_wip(gr.col as usize, i) {
                            let wip_x = rect.left() + lanes_x(lanes.wip_lane, lay.refs);
                            clip.line_segment([pos2(end_x, y), pos2(wip_x - LANE / 4.0, y)], stroke);
                            clip.line_segment([pos2(wip_x + LANE / 4.0, y), pos2(node_x - NODE / 2.0, y)], stroke);
                        } else {
                            clip.line_segment([pos2(end_x, y), pos2(node_x - NODE / 2.0, y)], stroke);
                        }
                    }
                }
                // Commit band behind the node and the message border.
                if lay.graph > 0.0 {
                    let zone = Rect::from_min_size(rect.min + vec2(lay.refs, 0.0), vec2(lay.graph, ROW));
                    let band = Rect::from_min_max(pos2(node_x.max(zone.left()), rect.center().y - INNER / 2.0), pos2(zone.right(), rect.center().y + INNER / 2.0));
                    painter.with_clip_rect(zone).rect_filled(band, CornerRadius::ZERO, mix(color, if sel { 0.22 } else { 0.12 }).gamma_multiply(fade));
                }
                let msg = Rect::from_min_size(rect.min + vec2(lay.refs + lay.graph, 0.0), vec2(lay.msg, ROW));
                painter.rect_filled(Rect::from_min_size(pos2(msg.left(), msg.center().y - INNER / 2.0), vec2(2.0, INNER)), CornerRadius::ZERO, color.gamma_multiply(fade));
                let mut tx = msg.left() + SUMMARY_MARGIN;
                if match_set.contains(&i) {
                    painter.circle_filled(pos2(tx + 3.0, msg.center().y), 3.0, Color32::from_rgb(0xf2, 0xca, 0x33));
                    tx += 12.0;
                }
                let faded = if sel { 0.5 } else { 0.2 };
                let text_alpha = if hover_active && !highlighted.contains(&i) { 1.0 - (1.0 - faded) * dim_t } else { 1.0 } * fade;
                let mut job = egui::text::LayoutJob::default();
                job.append(&commit.summary, 0.0, egui::TextFormat::simple(FontId::proportional(12.5), c.fg.gamma_multiply(text_alpha)));
                if cols.description && !commit.body.is_empty() {
                    let body = commit.body.split(['\r', '\n']).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" | ");
                    job.append(&body, 8.0, egui::TextFormat::simple(FontId::proportional(12.5), c.fg3.gamma_multiply(text_alpha)));
                }
                job.wrap = egui::text::TextWrapping::truncate_at_width((msg.right() - tx - 8.0).max(0.0));
                let g = ui.painter().layout_job(job);
                painter.with_clip_rect(msg).galley(pos2(tx, msg.center().y - g.size().y / 2.0), g, c.fg);
                // Extra columns.
                let mut ex = msg.right();
                for (col, w) in &lay.extras {
                    let cell = Rect::from_min_size(pos2(ex, rect.top()), vec2(*w, ROW));
                    painter.line_segment([cell.left_top(), cell.left_bottom()], Stroke::new(1.0, c.white(0.05)));
                    let clip = painter.with_clip_rect(cell);
                    let font = FontId::proportional(12.0);
                    match col {
                        Col::Author => {
                            widgets::paint_avatar(ui, pos2(cell.left() + 8.0 + 9.0, cell.center().y), 18.0, &commit.author, &commit.email);
                            let g = widgets::fit(ui, &commit.author, font, c.fg.gamma_multiply(fade), w - 40.0);
                            clip.galley(pos2(cell.left() + 32.0, cell.center().y - g.size().y / 2.0), g, c.fg);
                        }
                        Col::Date => {
                            let g = widgets::fit(ui, &widgets::format_date(commit.time, &date_fmt), font, c.fg.gamma_multiply(fade), w - 12.0);
                            clip.galley(pos2(cell.left() + 8.0, cell.center().y - g.size().y / 2.0), g, c.fg);
                        }
                        Col::Sha => {
                            let short: String = commit.sha.chars().take(m::SHORT_SHA_LENGTH as usize).collect();
                            clip.text(pos2(cell.left() + 8.0, cell.center().y), Align2::LEFT_CENTER, short, FontId::monospace(12.0), c.fg2.gamma_multiply(fade));
                        }
                        _ => match stats.get(&commit.sha) {
                            None => {
                                want_stats.push(commit.sha.clone());
                                clip.text(pos2(cell.left() + 8.0, cell.center().y), Align2::LEFT_CENTER, "…", font, c.fg3);
                            }
                            Some(s) => {
                                let files = format!("{} file{}", s.files, if s.files == 1 { "" } else { "s" });
                                let g = painter.layout_no_wrap(files, FontId::proportional(11.0), Color32::from_rgb(0xe2, 0xc0, 0x8d));
                                let gw = g.size().x;
                                clip.galley(pos2(cell.left() + 8.0, cell.center().y - g.size().y / 2.0), g, c.fg);
                                let barr = Rect::from_min_max(pos2(cell.left() + 14.0 + gw, cell.center().y - 3.0), pos2(cell.right() - 6.0, cell.center().y + 3.0));
                                if barr.width() > 20.0 {
                                    clip.rect_filled(barr, CornerRadius::same(1), c.white(0.05));
                                    let sum = (s.added + s.deleted).max(1) as f32;
                                    let aw = barr.width() * s.added as f32 / sum;
                                    let dw = barr.width() * s.deleted as f32 / sum;
                                    clip.rect_filled(Rect::from_min_size(barr.min, vec2(aw, 6.0)), CornerRadius::ZERO, Color32::from_rgb(0x34, 0x7d, 0x39));
                                    clip.rect_filled(Rect::from_min_size(barr.min + vec2(aw, 0.0), vec2(dw, 6.0)), CornerRadius::ZERO, Color32::from_rgb(0xc9, 0x3c, 0x37));
                                }
                            }
                        },
                    }
                    ex += w;
                }
            }
            // Lanes, clipped to the graph column.
            if lay.graph > 0.0 {
                let zone = Rect::from_min_max(pos2(origin.x + lay.refs, origin.y + viewport.top()), pos2(origin.x + lay.refs + lay.graph, origin.y + viewport.bottom()));
                let gp = painter.with_clip_rect(zone.intersect(ui.clip_rect()));
                let mut pts: Vec<Pos2> = Vec::new();
                let lx = |l: usize| origin.x + lanes_x(l, lay.refs);
                // The working changes: a dashed node in a lane of their own, joined to the
                // checked-out commit by a dashed line, drawn also while the node is scrolled away.
                if wip {
                    let x = lx(lanes.wip_lane);
                    let stroke = Stroke::new(2.0, lane_color(head_lane));
                    let yc = origin.y + ROW / 2.0;
                    if first == 0 {
                        dashed_circle(&gp, pos2(x, yc), NODE / 2.0 - 1.0, stroke);
                    }
                    if let Some(h) = head_idx {
                        let start = yc + NODE / 2.0;
                        let to = (origin.y + (h + off) as f32 * ROW + ROW / 2.0).min(origin.y + viewport.bottom() + ROW);
                        // Dashes keep their place as the line scrolls.
                        let mut y = start + ((origin.y + viewport.top() - start).max(0.0) / 6.0).floor() * 6.0;
                        while y < to {
                            gp.line_segment([pos2(x, y), pos2(x, (y + 3.0).min(to))], stroke);
                            y += 6.0;
                        }
                    }
                }
                for d in first.max(off)..=last {
                    let y0 = origin.y + d as f32 * ROW;
                    let yc = y0 + ROW / 2.0;
                    let y1 = y0 + ROW;
                    let i = d - off;
                    let Some(row) = v.graph.rows.get(i) else { continue };
                    let dim = if !q.is_empty() && !match_set.contains(&i) { MIN_ALPHA } else { 1.0 };
                    let _ = dim;
                    // A stash's line to its commit is dashed.
                    let line = |pts: &[Pos2], stroke: Stroke, dashed: bool| {
                        if dashed { gp.extend(egui::Shape::dashed_line(pts, stroke, 3.0, 3.0)) } else { gp.add(egui::Shape::line(pts.to_vec(), stroke)); }
                    };
                    // Lanes enter the row where the row above drew them and leave where this
                    // row does (they differ at the end of the working changes' lane).
                    let (top, here) = (|l: u16| lx(lanes.top(l as usize, i)), |l: u16| lx(lanes.row(l as usize, i)));
                    for [a, b] in &row.up_runs {
                        for lane in *a..=*b {
                            let stroke = Stroke::new(2.0, lane_color(lane as usize));
                            if top(lane) == here(lane) {
                                line(&[pos2(here(lane), y0), pos2(here(lane), yc)], stroke, dashed_at(i, lane, true));
                            } else {
                                curve(&mut pts, pos2(top(lane), y0), pos2(here(lane), yc), false);
                                line(&pts, stroke, dashed_at(i, lane, true));
                            }
                        }
                    }
                    for [a, b] in &row.down_runs {
                        for lane in *a..=*b {
                            line(&[pos2(here(lane), yc), pos2(here(lane), y1)], Stroke::new(2.0, lane_color(lane as usize)), dashed_at(i, lane, false));
                        }
                    }
                    for s in &row.up {
                        curve(&mut pts, pos2(top(s.from), y0), pos2(here(s.to), yc), false);
                        line(&pts, Stroke::new(2.0, lane_color(s.color as usize)), dashed_at(i, s.from, true));
                    }
                    for s in &row.down {
                        curve(&mut pts, pos2(here(s.from), yc), pos2(here(s.to), y1), true);
                        line(&pts, Stroke::new(2.0, lane_color(s.color as usize)), dashed_at(i, s.from, false));
                    }
                }
                // Nodes over the lines.
                for d in first.max(off)..=last {
                    let i = d - off;
                    let (Some(row), Some(commit)) = (v.graph.rows.get(i), commits.get(i)) else { continue };
                    let yc = origin.y + d as f32 * ROW + ROW / 2.0;
                    let x = lx(lanes.row(row.col as usize, i));
                    let color = lane_color(row.color as usize);
                    let fade = if !q.is_empty() && !match_set.contains(&i) { MIN_ALPHA } else { 1.0 };
                    if stash_of.contains_key(commit.sha.as_str()) {
                        // A stash: a dashed box with the stash icon.
                        let b = Rect::from_center_size(pos2(x, yc), Vec2::splat(NODE - 4.0));
                        gp.rect_filled(b, CornerRadius::same(3), c.app);
                        let corners = [b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom(), b.left_top()];
                        gp.extend(egui::Shape::dashed_line(&corners, Stroke::new(1.5, color.gamma_multiply(fade)), 3.0, 2.0));
                        icons::paint(ui, Rect::from_center_size(pos2(x, yc), Vec2::splat(11.0)), Icon::Archive, 11.0, color.gamma_multiply(fade));
                    } else if row.merge {
                        gp.circle_filled(pos2(x, yc), 6.0, color.gamma_multiply(fade));
                    } else {
                        gp.circle_filled(pos2(x, yc), NODE / 2.0, color.gamma_multiply(fade));
                        let center = pos2(x, yc);
                        let inner = m::AVATAR_INNER_DIAMETER as f32;
                        match super::avatars::texture(ui.ctx(), &commit.email) {
                            Some(picture) => super::avatars::paint(&gp, &picture, center, inner, fade),
                            None => {
                                gp.circle_filled(center, inner / 2.0, widgets::author_color(&commit.email.trim().to_lowercase()).gamma_multiply(fade));
                                gp.text(center, Align2::CENTER_CENTER, widgets::initials(&commit.author), FontId::proportional(7.5), Color32::WHITE.gamma_multiply(fade));
                            }
                        }
                    }
                }
            }
        });
        if show_markers {
            let strip = Rect::from_min_size(pos2(output.inner_rect.right(), output.inner_rect.top()), vec2(MARKER_STRIP, output.inner_rect.height()));
            let mut rows = MarkerRows { matches: matches.iter().map(|i| i + off).collect(), selection: selected_idx.into_iter().collect(), ..Default::default() };
            if all_markers {
                let upstreams: HashSet<&str> = v.data.refs.iter().filter(|r| r.kind == RefKind::Local && !r.upstream.is_empty()).map(|r| r.upstream.as_str()).collect();
                for (i, commit) in commits.iter().enumerate() {
                    let row = i + off;
                    if stash_of.contains_key(commit.sha.as_str()) {
                        rows.stashes.push(row);
                        continue;
                    }
                    let refs = by_sha.get(commit.sha.as_str()).map(Vec::as_slice).unwrap_or_default();
                    if commit.sha == v.data.head_sha {
                        rows.head.push(row);
                    } else if refs.iter().any(|r| r.kind == RefKind::Local) {
                        rows.local.push(row);
                    }
                    if refs.iter().any(|r| r.kind == RefKind::Tag) {
                        rows.tags.push(row);
                    }
                    if refs.iter().any(|r| r.kind == RefKind::Remote) {
                        rows.remote.push(row);
                    }
                    if refs.iter().any(|r| r.kind == RefKind::Remote && upstreams.contains(r.name.as_str())) {
                        rows.upstream.push(row);
                    }
                }
            }
            let painter = ui.painter().with_clip_rect(strip);
            painter.rect_filled(strip, CornerRadius::ZERO, c.panel.gamma_multiply(0.6));
            painter.line_segment([strip.left_top(), strip.left_bottom()], Stroke::new(1.0, c.black(0.2)));
            let lane_w = MARKER_STRIP / 3.0;
            for t in marker_ticks(&rows, total, strip.height()) {
                painter.rect_filled(Rect::from_min_size(pos2(strip.left() + t.lane as f32 * lane_w, strip.top() + t.top), vec2(lane_w * t.lanes as f32, t.height)), CornerRadius::ZERO, t.color);
            }
        }
        ui.data_mut(|d| {
            d.insert_temp(scroll_id.with("x"), output.state.clamped_offset().x);
            d.insert_temp(hover_ref_id, new_hover_ref);
        });
        if let Some((_, at)) = stack {
            if clock - at > 0.12 && !egui::Popup::is_any_open(ui.ctx()) {
                stack = None;
            } else {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(130));
            }
        }
        ui.data_mut(|d| d.insert_temp(stack_id, stack));
        // Apply what was clicked or typed.
        if let Some(text) = wip_draft {
            let path = path.clone();
            later.push(Box::new(move |app| {
                let mut d = app.draft(&path);
                d.summary = text;
                app.set_draft(&path, d);
            }));
        }
        if let Some(all) = wip_commit {
            later.push(Box::new(move |app| app.commit_from_wip(tab, all)));
        }
        if let Some((d, mods)) = clicked_row {
            if d < off {
                r.ui.select(Selection::Wip);
                r.ui.reveal = false;
            } else if let Some(cm) = v.data.commits.get(d - off) {
                let sha = cm.sha.clone();
                let cur = match r.ui.selection() {
                    Selection::Commit(s) => Some(s),
                    _ => None,
                };
                if mods.command {
                    let mut base = if r.ui.multi.is_empty() { cur.into_iter().collect() } else { r.ui.multi.clone() };
                    if let Some(pos) = base.iter().position(|x| *x == sha) {
                        base.remove(pos);
                    } else {
                        base.push(sha.clone());
                    }
                    let last = base.last().cloned().unwrap_or(sha);
                    r.ui.multi = if base.len() > 1 { base } else { Vec::new() };
                    r.ui.selection = Some(Selection::Commit(last));
                    r.ui.diff = None;
                    r.ui.compare_workdir = None;
                    r.ui.history = None;
                } else if mods.shift && let Some(a) = cur.and_then(|c| r.index.get(&c).copied()) {
                    let (lo, hi) = (a.min(d - off), a.max(d - off));
                    let picked: Vec<String> = v.data.commits[lo..=hi].iter().map(|c| c.sha.clone()).collect();
                    r.ui.multi = if picked.len() > 1 { picked } else { Vec::new() };
                    r.ui.diff = None;
                    r.ui.compare_workdir = None;
                    r.ui.history = None;
                } else {
                    r.ui.select(Selection::Commit(sha));
                    r.ui.reveal = false;
                }
            }
        }
        if want_more {
            later.push(Box::new(move |app| app.load_more(tab)));
        }
        if !want_stats.is_empty() {
            later.push(Box::new(move |app| app.want_stats(tab, want_stats)));
        }
    }

    fn search_bar(&self, ui: &mut Ui, tab: u64, r: &mut RepoTab, v: &RepoView, matches: &[usize], later: &mut Vec<Later>) {
        let c = p();
        let mut jump: Option<bool> = None;
        let mut close = false;
        bar(ui, 34.0, c.panel, |ui| {
            ui.add_space(8.0);
            let w = (ui.available_width() - 420.0).max(120.0);
            let id = ui.scope_id().with(("search", tab));
            let (rect, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
            let focused = ui.memory(|m| m.has_focus(id));
            ui.painter().rect(rect, CornerRadius::same(3), c.field, Stroke::new(1.0, if focused { ACCENT } else { c.white(0.1) }), StrokeKind::Inside);
            let edit = egui::TextEdit::singleline(&mut r.ui.search).id(id).frame(egui::Frame::NONE).font(FontId::proportional(12.5)).hint_text(egui::RichText::new("Search commits by message, author or sha").color(c.fg3)).align(Align2::LEFT_CENTER).desired_width(w - 12.0);
            let resp = ui.put(rect.shrink2(vec2(6.0, 0.0)), edit);
            if r.ui.focus_search {
                resp.request_focus();
                r.ui.focus_search = false;
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                jump = Some(!ui.input(|i| i.modifiers.shift));
                resp.request_focus();
            }
            if resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
            ui.add_space(8.0);
            if widgets::link(ui, "History filters", 11.5).clicked() {
                later.push(Box::new(move |app| app.edit_history_filter(tab)));
            }
            ui.add_space(8.0);
            if !r.ui.search.trim().is_empty() {
                ui.label(egui::RichText::new(format!("{} in {} loaded commits", matches.len(), v.data.commits.len())).size(11.5).color(c.fg2));
                if v.data.has_more && widgets::link(ui, "Search older commits", 11.5).clicked() {
                    later.push(Box::new(move |app| app.load_more(tab)));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                if widgets::icon_button(ui, Icon::X, 15.0, "Close (Esc)").clicked() {
                    close = true;
                }
                if widgets::icon_button(ui, Icon::ChevronDown, 15.0, "Next (Enter)").clicked() {
                    jump = Some(true);
                }
                if widgets::icon_button(ui, Icon::ChevronUp, 15.0, "Previous (Shift+Enter)").clicked() {
                    jump = Some(false);
                }
            });
        });
        if close {
            r.ui.search_open = false;
            r.ui.search.clear();
        }
        if let Some(forward) = jump
            && !matches.is_empty()
        {
            let cur = match r.ui.selection() {
                Selection::Commit(s) => r.index.get(&s).map(|i| *i as isize).unwrap_or(-1),
                _ => -1,
            };
            let next = if forward {
                matches.iter().copied().find(|x| *x as isize > cur).unwrap_or(matches[0])
            } else {
                matches.iter().rev().copied().find(|x| (*x as isize) < cur).unwrap_or(*matches.last().expect("not empty"))
            };
            let sha = v.data.commits[next].sha.clone();
            r.ui.select(Selection::Commit(sha));
        }
    }

    /// Double-click on a label: check out that branch.
    pub(crate) fn checkout_ref(&mut self, tab: u64, r: &GitRef) {
        let head = self.repo(tab).and_then(|t| t.view.as_ref()).and_then(|v| v.data.head_branch.clone());
        match r.kind {
            RefKind::Local if Some(&r.name) != head.as_ref() => self.act(tab, Action::Checkout { target: r.name.clone(), changes: gitcraft_core::actions::CheckoutChanges::Ask }),
            RefKind::Remote => self.act(tab, Action::CheckoutRemote { remote_branch: r.name.clone(), stash_if_needed: false, changes: gitcraft_core::actions::CheckoutChanges::Ask }),
            _ => {}
        }
    }
}

/// "v0.1.3 (Tag)", "main (Current Branch)".
fn describe_ref(r: &GitRef, v: &RepoView) -> String {
    let kind = match r.kind {
        RefKind::Tag => "Tag",
        RefKind::Remote => "Remote Branch",
        RefKind::Local if Some(&r.name) == v.data.head_branch.as_ref() => "Current Branch",
        RefKind::Local => "Local Branch",
    };
    format!("{} ({kind})", r.name)
}

/// One branch or tag label: check mark for the checked-out branch, the name, kind icons, ahead
/// and behind counts. Returns its rect and response.
#[allow(clippy::too_many_arguments)]
/// How wide a label is with its whole name.
fn ref_width(ui: &Ui, r: &GitRef, remote: Option<&GitRef>, v: &RepoView) -> f32 {
    let head = r.kind == RefKind::Local && Some(&r.name) == v.data.head_branch.as_ref();
    let icons = match r.kind {
        RefKind::Local if remote.is_some() => 2.0,
        _ => 1.0,
    } + if head { 1.0 } else { 0.0 };
    let counts = if r.kind == RefKind::Local && (r.ahead > 0 || r.behind > 0) { format!("↑{}↓{}", r.ahead, r.behind) } else { String::new() };
    let counts_w = if counts.is_empty() { 0.0 } else { ui.painter().layout_no_wrap(counts, FontId::proportional(10.5), p().fg).size().x + 4.0 };
    let name_w = ui.painter().layout_no_wrap(r.name.clone(), FontId::proportional(12.0), p().fg).size().x;
    10.0 + icons * 17.0 + name_w + counts_w
}

#[allow(clippy::too_many_arguments)]
fn ref_label(ui: &mut Ui, painter: &egui::Painter, left_center: Pos2, max_w: f32, r: &GitRef, remote: Option<&GitRef>, v: &RepoView, fill: Color32, fade: f32, id: egui::Id, sense: Sense) -> (Rect, egui::Response) {
    let c = p();
    let head = r.kind == RefKind::Local && Some(&r.name) == v.data.head_branch.as_ref();
    let mut icons_w = 0.0;
    if head {
        icons_w += 17.0;
    }
    let kinds: Vec<Icon> = match r.kind {
        RefKind::Tag => vec![Icon::Tag],
        RefKind::Local if remote.is_some() => vec![Icon::Laptop, Icon::Cloud],
        RefKind::Local => vec![Icon::Laptop],
        RefKind::Remote => vec![Icon::Cloud],
    };
    icons_w += kinds.len() as f32 * 17.0;
    let counts = if r.kind == RefKind::Local && (r.ahead > 0 || r.behind > 0) {
        format!("{}{}", if r.ahead > 0 { format!("↑{}", r.ahead) } else { String::new() }, if r.behind > 0 { format!("↓{}", r.behind) } else { String::new() })
    } else {
        String::new()
    };
    let counts_w = if counts.is_empty() { 0.0 } else { ui.painter().layout_no_wrap(counts.clone(), FontId::proportional(10.5), c.fg).size().x + 4.0 };
    let name = widgets::fit(ui, &r.name, FontId::proportional(12.0), c.fg.gamma_multiply(fade), (max_w - 10.0 - icons_w - counts_w).max(10.0));
    let w = (10.0 + icons_w + name.size().x + counts_w).min(max_w);
    let rect = Rect::from_min_size(pos2(left_center.x, left_center.y - INNER / 2.0), vec2(w, INNER));
    let resp = ui.interact(rect, id, sense);
    painter.rect_filled(rect, CornerRadius::same(2), fill.gamma_multiply(fade));
    let mut x = rect.left() + 5.0;
    let ink = c.fg.gamma_multiply(fade);
    if head {
        icons::paint(ui, Rect::from_min_size(pos2(x, rect.center().y - 6.5), Vec2::splat(13.0)), Icon::Check, 13.0, ink);
        x += 17.0;
    }
    painter.galley(pos2(x, rect.center().y - name.size().y / 2.0), name.clone(), ink);
    x += name.size().x + 4.0;
    for k in kinds {
        icons::paint(ui, Rect::from_min_size(pos2(x, rect.center().y - 6.5), Vec2::splat(13.0)), k, 12.0, ink.gamma_multiply(0.9));
        x += 17.0;
    }
    if !counts.is_empty() {
        painter.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, counts, FontId::proportional(10.5), ink);
    }
    (rect, resp)
}

/// A full-width bar of `height` with a fill, its contents laid out left to right.
fn bar(ui: &mut Ui, height: f32, fill: Color32, add: impl FnOnce(&mut Ui)) {
    let c = p();
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, height), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
    ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, c.black(0.3)));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    add(&mut child);
}

/// "Merging x into y" / "Rebasing …" with Skip, Continue and Abort.
fn state_banner(ui: &mut Ui, tab: u64, v: &RepoView, note: &str, later: &mut Vec<Later>) {
    let c = p();
    let merging = note == "merging";
    let rebasing = note == "rebasing";
    let sequencer = match note {
        "cherry-picking" => Some("cherry-pick"),
        "reverting" => Some("revert"),
        "applying-mailbox" => Some("am"),
        _ => None,
    };
    let text = match &v.data.conflict_labels {
        Some((cur, inc)) if merging => format!("Merging {inc} into {cur}"),
        Some((cur, inc)) if rebasing => format!("Rebasing {inc} onto {cur}"),
        None if merging => "Merge in progress".into(),
        None if rebasing => "Rebase in progress".into(),
        _ => format!("{note} in progress"),
    } + if v.data.status.conflicted { " — merge conflicts detected" } else { "" };
    let recovery = move |command: &'static str| -> Action {
        match sequencer {
            Some(op) => Action::Sequencer { operation: op.into(), command: command.into() },
            None => match command {
                "continue" => Action::ContinueRebase,
                "skip" => Action::SkipRebase,
                _ => Action::AbortRebase,
            },
        }
    };
    bar(ui, 34.0, WARN.gamma_multiply(0.25), |ui| {
        ui.add_space(12.0);
        ui.label(egui::RichText::new(text).size(12.5).color(c.fg));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(12.0);
            ui.spacing_mut().item_spacing.x = 8.0;
            let abort = if merging { "Abort Merge".to_owned() } else { format!("Abort {}", sequencer.unwrap_or("Rebase")) };
            if widgets::small(ui, Kind::Danger, &abort).clicked() {
                let a = if merging { Action::AbortMerge } else { recovery("abort") };
                later.push(Box::new(move |app| app.act(tab, a)));
            }
            if rebasing || sequencer.is_some() {
                if widgets::small(ui, Kind::Success, &format!("Continue {}", sequencer.unwrap_or("Rebase"))).clicked() {
                    let a = recovery("continue");
                    later.push(Box::new(move |app| app.act(tab, a)));
                }
                if widgets::small(ui, Kind::Default, "Skip Commit").clicked() {
                    let a = recovery("skip");
                    later.push(Box::new(move |app| app.act(tab, a)));
                }
            }
        });
    });
    let _ = (BAD, ACCENT);
}

/// Width of the strip of markers beside the graph's scroll bar.
const MARKER_STRIP: f32 = 12.0;

/// The rows the marker strip marks, by kind (rows count the WIP row).
#[derive(Default)]
struct MarkerRows {
    stashes: Vec<usize>,
    tags: Vec<usize>,
    remote: Vec<usize>,
    local: Vec<usize>,
    matches: Vec<usize>,
    upstream: Vec<usize>,
    head: Vec<usize>,
    selection: Vec<usize>,
}

/// One tick in the marker strip: top and height in the strip, the first lane and how many.
#[derive(Debug, PartialEq)]
struct Tick {
    top: f32,
    height: f32,
    lane: usize,
    lanes: usize,
    color: Color32,
}

/// The ticks to draw, in drawing order, for `total` rows in a strip `height` tall. Blocks mark
/// stashes, branches, tags, search matches and the checked-out commit across three lanes; a thin line
/// across all of them marks the selection.
fn marker_ticks(rows: &MarkerRows, total: usize, height: f32) -> Vec<Tick> {
    if total == 0 || height <= 0.0 {
        return Vec::new();
    }
    // Sizes scale with the rows per point, within limits: (base in row units, min, max).
    const ROW_UNITS: f32 = 28.0;
    let size = |base: f32, min: f32, max: f32| {
        let h = (height / (total as f32 * ROW_UNITS) * base).clamp(min, max);
        let slice = height / total as f32;
        (h, if h >= slice { 0.0 } else { (slice - h) / 2.0 })
    };
    let block = size(ROW_UNITS, 5.0, 20.0);
    let line = size(ROW_UNITS / 2.0, 2.0, 4.0);
    // (rows, first lane, lanes, colour, (height, offset)), in drawing order.
    let kinds = [
        (&rows.stashes, 0, 1, Color32::from_rgb(0xfb, 0x94, 0xff), block),
        (&rows.tags, 2, 1, Color32::from_rgb(0xaf, 0xfc, 0xff), block),
        (&rows.remote, 2, 1, Color32::from_rgb(0xff, 0xd6, 0x8b), block),
        (&rows.local, 0, 1, Color32::from_rgb(0xff, 0xa5, 0x00), block),
        (&rows.matches, 1, 1, Color32::from_rgb(0xe6, 0xe6, 0x2b), block),
        (&rows.upstream, 1, 2, Color32::from_rgb(0x9b, 0xff, 0x9b), block),
        (&rows.head, 0, 2, Color32::from_rgb(0x16, 0xc2, 0x16), block),
        (&rows.selection, 0, 3, Color32::from_rgb(0x08, 0x21, 0xff), line),
    ];
    let mut out = Vec::new();
    for (list, lane, lanes, color, (h, offset)) in kinds {
        for row in list {
            out.push(Tick { top: *row as f32 * height / total as f32 + offset - h / 2.0, height: h, lane, lanes, color });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn marker_ticks_sit_at_their_rows() {
        let rows = MarkerRows { head: vec![0], tags: vec![50], matches: vec![99], selection: vec![50], ..Default::default() };
        let ticks = marker_ticks(&rows, 100, 500.0);
        // Drawing order: tags, then matches, head, and the selection on top.
        let kinds: Vec<(usize, usize)> = ticks.iter().map(|t| (t.lane, t.lanes)).collect();
        assert_eq!(kinds, vec![(2, 1), (1, 1), (0, 2), (0, 3)]);
        // Five points per row: blocks are 5 tall from the row's top, less half their height.
        assert_eq!(ticks[0].top, 250.0 - 2.5);
        assert_eq!(ticks[0].height, 5.0);
        assert_eq!(ticks[1].top, 495.0 - 2.5);
        // The selection is a thin line, centred in its row's slice.
        assert_eq!(ticks[3].height, 2.5);
        assert_eq!(ticks[3].top, 250.0 + 1.25 - 1.25);
        assert!(marker_ticks(&rows, 0, 500.0).is_empty());
    }

    use super::*;

    #[test]
    fn columns_give_way_to_the_message_in_narrow_panes() {
        let cols = Columns { author: true, date: true, ..Columns::default() };
        let wide = layout(&cols, &HashMap::new(), 3, 1400.0);
        assert_eq!(wide.extras.len(), 2);
        assert_eq!(wide.refs, m::REF_ZONE_DEFAULT_WIDTH as f32);
        let narrow = layout(&cols, &HashMap::new(), 3, 300.0);
        assert!(narrow.extras.is_empty());
        assert!(narrow.msg >= m::MESSAGE_ZONE_MIN_WIDTH as f32);
        assert!(narrow.refs >= m::REF_ZONE_MIN_WIDTH as f32);
    }

    #[test]
    fn curves_turn_with_the_arc_radius() {
        let mut pts = Vec::new();
        curve(&mut pts, pos2(0.0, 0.0), pos2(44.0, 14.0), true);
        assert_eq!(*pts.first().unwrap(), pos2(0.0, 0.0));
        assert_eq!(*pts.last().unwrap(), pos2(44.0, 14.0));
        // The turn ends straight above the target.
        let before_last = pts[pts.len() - 2];
        assert!((before_last.x - 44.0).abs() < 0.01);
    }
}

/// Where the graph's lanes are drawn when there are working changes: their dashed line gets a
/// lane of its own above the checked-out commit (the left one, or the checked-out branch's when
/// a branch is pinned), the lanes beside it move one to the right until that commit, and below
/// it the checked-out branch's lane is the leftmost.
#[derive(Clone, Copy, Debug)]
struct WipLanes {
    wip: bool,
    pinned: bool,
    head_lane: usize,
    wip_lane: usize,
    /// Rows (commit indexes) above the checked-out commit, beside the working changes' lane.
    wip_end: usize,
}

impl WipLanes {
    fn new(wip: bool, pinned: bool, head_lane: usize, head_idx: Option<usize>, commits: usize) -> WipLanes {
        WipLanes { wip, pinned, head_lane, wip_lane: if pinned { head_lane } else { 0 }, wip_end: if wip { head_idx.unwrap_or(commits) } else { 0 } }
    }

    /// Lanes the graph needs besides the history's own.
    fn extra(&self) -> u16 {
        u16::from(self.wip_end > 0)
    }

    fn shifted(&self, lane: usize) -> usize {
        if lane >= self.wip_lane { lane + 1 } else { lane }
    }

    fn history(&self, lane: usize) -> usize {
        if !self.wip || self.pinned || self.head_lane == 0 {
            lane
        } else if lane == self.head_lane {
            0
        } else if lane < self.head_lane {
            lane + 1
        } else {
            lane
        }
    }

    /// Where `lane` is drawn in commit row `i`.
    fn row(&self, lane: usize, i: usize) -> usize {
        if i < self.wip_end { self.shifted(lane) } else { self.history(lane) }
    }

    /// Where `lane` comes in at the top of commit row `i` (from the row above).
    fn top(&self, lane: usize, i: usize) -> usize {
        if self.wip_end > 0 && i <= self.wip_end { self.shifted(lane) } else { self.history(lane) }
    }

    /// Whether row `i`'s label connector to its node crosses the working changes' line.
    fn crosses_wip(&self, lane: usize, i: usize) -> bool {
        i < self.wip_end && lane >= self.wip_lane
    }
}

#[cfg(test)]
mod wip_lane_tests {
    use super::WipLanes;

    #[test]
    fn working_changes_get_a_lane_of_their_own_above_the_checked_out_commit() {
        // Head is the third commit, on lane 1; nothing pinned.
        let l = WipLanes::new(true, false, 1, Some(2), 10);
        assert_eq!((l.wip_lane, l.extra()), (0, 1));
        // Above the head everything moves right past lane 0.
        assert_eq!((l.row(0, 0), l.row(1, 1)), (1, 2));
        // From the head down, the head's lane is leftmost and the lanes left of it move over.
        assert_eq!((l.row(1, 2), l.row(0, 2), l.row(2, 5)), (0, 1, 2));
        // The head row's lines come in from where the row above drew them.
        assert_eq!((l.top(1, 2), l.top(1, 3)), (2, 0));
        assert!(l.crosses_wip(0, 1) && !l.crosses_wip(0, 2));
    }

    #[test]
    fn a_pinned_branch_keeps_its_lane_and_no_changes_change_nothing() {
        let pinned = WipLanes::new(true, true, 2, Some(3), 10);
        assert_eq!(pinned.wip_lane, 2);
        assert_eq!((pinned.row(1, 0), pinned.row(2, 0), pinned.row(2, 4)), (1, 3, 2));
        let clean = WipLanes::new(false, false, 1, Some(2), 10);
        assert_eq!((clean.extra(), clean.row(1, 0), clean.row(1, 5)), (0, 1, 1));
        // Head on the first row: no shifted rows, the head's lane still leftmost.
        let top = WipLanes::new(true, false, 1, Some(0), 10);
        assert_eq!((top.extra(), top.row(1, 0), top.row(0, 0)), (0, 0, 1));
    }
}
