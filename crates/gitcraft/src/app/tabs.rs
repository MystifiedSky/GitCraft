//! The 30 px tabs bar: the start-screen and Launchpad buttons, the open tabs (sized to their
//! title, drag to reorder, drag the right edge to resize a repository tab), New Tab, the tabs
//! list, notifications, preferences and the profile menu.

use super::widgets::Tip;
use super::icons::{self, Icon};
use super::theme::{ACCENT, p};
use super::{App, Tab, TabKind, folder_name};
use egui::{Color32, CornerRadius, FontId, Id, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2};

const HEIGHT: f32 = 30.0;
const MIN_TAB_WIDTH: f32 = 76.0;
const MAX_TAB_WIDTH: f32 = 480.0;
const ICON_TAB: f32 = 38.0;
const SLOT_WIDTH: f32 = 33.0;
const TAB_FONT_SIZE: f32 = 13.0;
const TAB_PADDING: f32 = 12.0;
const TAB_ICON: f32 = 18.0;
const TAB_ICON_GLYPH: f32 = 12.0;
const TAB_GAP: f32 = 4.0;
const TAB_CLOSE: f32 = 17.0;
const TAB_CLOSE_MARGIN: f32 = 6.0;
const PROFILE_FONT_SIZE: f32 = 13.0;
const PROFILE_PADDING: f32 = 8.0;
const PROFILE_MARK: f32 = 18.0;
const PROFILE_GAP: f32 = 6.0;
const PROFILE_CHEVRON: f32 = 13.0;

pub(super) fn tabs_list_id() -> Id {
    Id::unique("tabs_list")
}
/// The colours of the profile marks, by the profile's icon number.
/// The colour of profile picture `i`.
pub(crate) fn profile_color(i: usize) -> Color32 {
    PROFILE_COLORS[i % PROFILE_COLORS.len()]
}

const PROFILE_COLORS: [Color32; 4] = [
    Color32::from_rgb(0x7b, 0xd9, 0x38),
    Color32::from_rgb(0x2e, 0xc4, 0xb6),
    Color32::from_rgb(0xb5, 0x7b, 0xff),
    Color32::from_rgb(0xff, 0x9f, 0x43),
];

/// A tab being dragged to a new place.
pub struct Drag {
    id: u64,
    /// Where on the tab it was picked up.
    grab: f32,
}

/// A repository tab's right edge being dragged.
#[derive(Clone, Copy)]
struct Resize {
    start_x: f32,
    start_width: f32,
    /// Whether an earlier drag set the width, to restore on Escape.
    was_manual: bool,
}

/// A dragged width, clamped to the strip's limits.
fn manual_tab_width(saved: Option<f32>) -> Option<f32> {
    saved.filter(|w| w.is_finite()).map(|w| w.clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH))
}

/// The width a tab needs to show its whole title.
fn natural_tab_width(ui: &Ui, tab: &Tab) -> f32 {
    let galley = ui.painter().layout_no_wrap(tab.title.clone(), FontId::proportional(TAB_FONT_SIZE), Color32::WHITE);
    let icon = if tab.path().is_some() { TAB_ICON } else { 0.0 };
    let padding = TAB_PADDING + icon + TAB_GAP + TAB_CLOSE + TAB_CLOSE_MARGIN;
    (padding + galley.size().x).max(MIN_TAB_WIDTH)
}

/// The width the profile button takes.
fn profile_width(ui: &Ui, name: &str) -> f32 {
    let galley = ui.painter().layout_no_wrap(name.to_owned(), FontId::proportional(PROFILE_FONT_SIZE), Color32::WHITE);
    2.0 * PROFILE_PADDING + PROFILE_MARK + 2.0 * PROFILE_GAP + PROFILE_CHEVRON + galley.size().x
}

/// Each tab's title width, or a shared width for the longest when the strip is full.
fn fit_tab_widths(natural: &[f32], room: f32) -> Vec<f32> {
    let natural: Vec<f32> = natural.iter().map(|w| w.max(MIN_TAB_WIDTH)).collect();
    let floor = MIN_TAB_WIDTH * natural.len() as f32;
    let total: f32 = natural.iter().sum();
    if total <= room {
        return natural;
    }
    if room <= floor || natural.is_empty() {
        return vec![MIN_TAB_WIDTH; natural.len()];
    }
    // The width given to the longest titles: found by bisection as the total is monotonic.
    let (mut low, mut high) = (MIN_TAB_WIDTH, natural.iter().fold(MIN_TAB_WIDTH, |wide, w| wide.max(*w)));
    for _ in 0..24 {
        let cap = (low + high) / 2.0;
        let used: f32 = natural.iter().map(|w| w.min(cap)).sum();
        if used > room {
            high = cap;
        } else {
            low = cap;
        }
    }
    natural.iter().map(|w| w.min(low)).collect()
}

/// Where a tab's close button goes.
fn tab_close_rect(rect: Rect) -> Rect {
    let center = pos2(rect.right() - TAB_CLOSE_MARGIN - TAB_CLOSE / 2.0, rect.center().y);
    Rect::from_center_size(center, Vec2::splat(TAB_CLOSE))
}

/// A tab's background, branch icon and title.
fn paint_tab(ui: &Ui, rect: Rect, tab: &Tab, fill: Color32, ink: Color32) {
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::ZERO, fill);
    let mut x = rect.left() + TAB_PADDING;
    if tab.path().is_some() {
        let center = pos2(x + TAB_ICON / 2.0, rect.center().y);
        icons::paint(ui, Rect::from_center_size(center, Vec2::splat(TAB_ICON_GLYPH)), Icon::GitBranch, TAB_ICON_GLYPH, ink.gamma_multiply(0.7));
        x += TAB_ICON;
    }
    let title_room = (tab_close_rect(rect).left() - TAB_GAP - x).max(0.0);
    let galley = painter.layout_no_wrap(tab.title.clone(), FontId::proportional(TAB_FONT_SIZE), ink);
    let galley = if galley.size().x > title_room { truncated(ui, &tab.title, title_room, ink) } else { galley };
    painter.galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, ink);
}

/// The thin dark line between tabs.
fn divider(ui: &Ui, x: f32, rect: Rect) {
    ui.painter().line_segment([pos2(x - 0.5, rect.top()), pos2(x - 0.5, rect.bottom())], Stroke::new(1.0, p().black(0.25)));
}

fn icon_tab(ui: &mut Ui, icon: Icon, tip: &str, active: bool) -> egui::Response {
    let c = p();
    let (rect, response) = ui.allocate_exact_size(vec2(ICON_TAB, HEIGHT), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tip));
    if active {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, c.toolbar);
    }
    let ink = if active || response.hovered() { c.white(1.0) } else { c.white(0.8) };
    icons::paint(ui, rect, icon, 15.0, ink);
    divider(ui, rect.right(), rect);
    response.tip(tip)
}

/// The profile mark: a small pixel figure in the profile's colour.
pub(crate) fn profile_mark(ui: &Ui, rect: Rect, color: Color32) {
    let cell = rect.width() / 9.0;
    let at = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(rect.min + vec2(x * cell, y * cell), vec2(w * cell, h * cell));
    let paint = ui.painter();
    for (x, y, w, h) in [(2.0, 1.0, 5.0, 1.0), (1.0, 2.0, 7.0, 4.0), (1.0, 6.0, 2.0, 2.0), (6.0, 6.0, 2.0, 2.0)] {
        paint.rect_filled(at(x, y, w, h), CornerRadius::ZERO, color);
    }
    let eye = Color32::from_rgb(0x1c, 0x1e, 0x23);
    paint.rect_filled(at(2.0, 3.0, 1.0, 1.0), CornerRadius::ZERO, eye);
    paint.rect_filled(at(6.0, 3.0, 1.0, 1.0), CornerRadius::ZERO, eye);
}

impl App {
    pub(super) fn tabs_bar(&mut self, ui: &mut Ui) {
        let c = p();
        let shown = egui::Panel::top("tabs_bar")
            .exact_size(HEIGHT)
            .frame(egui::Frame::new().fill(c.tabs))
            .show_separator_line(false)
            .show(ui, |ui| {
                let bar = ui.max_rect();
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let rects = ui.horizontal(|ui| {
                    if icon_tab(ui, Icon::Folder, "Repositories (start screen)", false).clicked() {
                        self.new_tab();
                    }
                    let on_launchpad = self.active_tab().is_some_and(|t| t.kind == TabKind::Launchpad);
                    if icon_tab(ui, Icon::Rocket, "Launchpad", on_launchpad).clicked() {
                        self.open_launchpad();
                    }
                    let rects = self.tab_strip(ui);
                    let new_tip = format!("New tab ({})", self.keys_of("nav:newTab").map(|k| super::keys::label(&k)).unwrap_or_default());
                    if icon_tab(ui, Icon::Plus, &new_tip, false).clicked() {
                        self.new_tab();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        self.profile_button(ui);
                        if icon_tab(ui, Icon::Settings, "Preferences", false).clicked() {
                            self.open_prefs(None);
                        }
                        let unread = self.notices.iter().filter(|n| !n.read).count() + usize::from(self.update_row());
                        let badge = if unread >= 50 { "50+".to_owned() } else { unread.to_string() };
                        let label = if unread > 0 { format!("Notifications ({badge} unread)") } else { "Notifications".to_owned() };
                        let bell = icons::slot(ui, SLOT_WIDTH, Icon::Bell, 15.0, c.white(0.8), c.white(1.0));
                        bell.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
                        let bell = bell.tip(label);
                        if unread > 0 {
                            let text = badge;
                            let w = (text.len() as f32 * 6.0 + 6.0).max(14.0);
                            let br = egui::Rect::from_min_size(egui::pos2(bell.rect.right() - w - 1.0, bell.rect.top() + 3.0), egui::vec2(w, 14.0));
                            ui.painter().rect_filled(br, egui::CornerRadius::same(7), egui::Color32::from_rgb(0x2f, 0x81, 0xf7));
                            ui.painter().text(br.center(), egui::Align2::CENTER_CENTER, text, super::fonts::bold(9.0), egui::Color32::WHITE);
                        }
                        self.notifications(ui, &bell);
                        let list = icons::slot(ui, SLOT_WIDTH, Icon::ChevronDown, 16.0, c.white(0.8), c.white(1.0));
                        let list_tip = self.keys_of("nav:tabsList").map_or_else(|| "Tabs list".to_owned(), |key| format!("Tabs list ({})", super::keys::label(&key)));
                        list.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &list_tip));
                        let list = list.tip(list_tip);
                        egui::Popup::menu(&list).id(tabs_list_id()).align(egui::RectAlign::BOTTOM_END).show(|ui| self.tabs_list(ui));
                    });
                    rects
                }).inner;
                self.drag_ghost(ui, &rects);
                self.drop_dragged_tab(ui, &rects);
                // The bar's lower edge.
                ui.painter().line_segment([bar.left_bottom() - vec2(0.0, 0.5), bar.right_bottom() - vec2(0.0, 0.5)], Stroke::new(1.0, c.black(0.3)));
            });
        // Preferences cover the window from here down, as the tabs stay in view.
        self.below_tabs = shown.response.rect.bottom();
    }

    /// The open tabs, scrolled sideways by the mouse wheel when they do not fit.
    /// The repository a tab's worktree belongs to (its main worktree), when that repository has more than one.
    fn worktree_group(&self, id: u64) -> Option<String> {
        let repo = self.repos.get(&id)?;
        let worktrees = repo.view.as_ref().map(|v| &v.data.worktrees).or_else(|| {
            // An inactive restored tab might not have loaded. A loaded member of the same
            // group already knows its path from Git's worktree list.
            self.repos.values().filter_map(|r| r.view.as_ref()).map(|v| &v.data.worktrees)
                .find(|trees| trees.iter().any(|w| super::same_folder(&w.path, &repo.path)))
        })?;
        if worktrees.len() < 2 { return None; }
        worktrees.first().map(|w| w.path.replace('\\', "/").trim_end_matches('/').to_lowercase())
    }

    /// UI Customization > Automatically group worktree tabs together: worktrees of one repository sit side by side.
    fn group_worktree_tabs(&mut self) {
        if !self.settings.flag("autogroupWorktreeTabs", true) || self.tab_drag.is_some() {
            return;
        }
        let groups: Vec<Option<String>> = self.tabs.iter().map(|t| self.worktree_group(t.id)).collect();
        let mut order: Vec<usize> = Vec::with_capacity(self.tabs.len());
        for i in 0..self.tabs.len() {
            if order.contains(&i) {
                continue;
            }
            order.push(i);
            if let Some(g) = &groups[i] {
                order.extend((i + 1..self.tabs.len()).filter(|j| groups[*j].as_ref() == Some(g)));
            }
        }
        if order.iter().enumerate().any(|(at, i)| at != *i) {
            let mut old: Vec<Option<super::Tab>> = std::mem::take(&mut self.tabs).into_iter().map(Some).collect();
            self.tabs = order.into_iter().filter_map(|i| old[i].take()).collect();
            self.remember_tabs();
        }
    }

    fn tab_strip(&mut self, ui: &mut Ui) -> Vec<(u64, Rect)> {
        self.group_worktree_tabs();
        let (profile, _) = self.settings.active_profile();
        // New Tab and the two right slots sit beside the settings and profile buttons.
        let reserved = 2.0 * ICON_TAB + 2.0 * SLOT_WIDTH + profile_width(ui, &profile);
        let room = (ui.available_width() - reserved).max(MIN_TAB_WIDTH);
        egui::ScrollArea::horizontal()
            .id_salt("tab_strip")
            .max_width(room)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                ui.horizontal_centered(|ui| {
                    let shown: Vec<u64> = self.tabs.iter().filter(|t| t.kind != TabKind::Launchpad).map(|t| t.id).collect();
                    let widths = self.tab_widths(ui, &shown, room);
                    let mut rects = Vec::with_capacity(shown.len());
                    for (i, id) in shown.iter().enumerate() {
                        let first = i == 0;
                        let last = i + 1 == shown.len();
                        rects.push((*id, self.tab(ui, *id, widths[i], first, last)));
                    }
                    rects
                }).inner
            }).inner
    }

    /// Each tab's width, with dragged widths fixed and the others sharing `room`.
    fn tab_widths(&self, ui: &Ui, ids: &[u64], room: f32) -> Vec<f32> {
        let mut manual = Vec::with_capacity(ids.len());
        let mut natural = Vec::with_capacity(ids.len());
        let mut fixed = 0.0;
        for id in ids {
            let tab = self.tabs.iter().find(|t| t.id == *id);
            let saved = tab.and_then(|t| t.path()).and_then(|path| manual_tab_width(self.settings.tab_width(path)));
            fixed += saved.unwrap_or(0.0);
            manual.push(saved);
            natural.push(tab.map_or(MIN_TAB_WIDTH, |tab| natural_tab_width(ui, tab)));
        }
        let auto: Vec<usize> = (0..ids.len()).filter(|&i| manual[i].is_none()).collect();
        let wanted: Vec<f32> = auto.iter().map(|&i| natural[i]).collect();
        let fitted = fit_tab_widths(&wanted, room - fixed);
        let mut widths = natural;
        for (at, &i) in auto.iter().enumerate() {
            widths[i] = fitted[at];
        }
        for (i, saved) in manual.iter().enumerate() {
            if let Some(width) = saved {
                widths[i] = *width;
            }
        }
        widths
    }

    /// One tab; returns where it was drawn.
    fn tab(&mut self, ui: &mut Ui, id: u64, width: f32, first: bool, last: bool) -> Rect {
        let c = p();
        let Some(tab) = self.tabs.iter().find(|t| t.id == id).cloned() else { return Rect::NOTHING };
        let active = id == self.active;
        let (rect, response) = ui.allocate_exact_size(vec2(width, HEIGHT), Sense::click_and_drag());
        response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, active, &tab.title));
        let dragging = self.tab_drag.as_ref().is_some_and(|d| d.id == id);
        if dragging {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            ui.painter().rect_filled(rect, CornerRadius::ZERO, c.black(0.1));
            divider(ui, rect.right(), rect);
            return rect;
        }
        let hovered = response.hovered() || response.contains_pointer();
        let fill = if active { c.toolbar } else if hovered { c.white(0.05) } else { Color32::TRANSPARENT };
        let ink = if active { c.white(1.0) } else { c.white(0.75) };
        paint_tab(ui, rect, &tab, fill, ink);

        let close = tab_close_rect(rect);
        let close_response = ui.interact(close, Id::NULL.with(("tab_close", id)), Sense::click());
        close_response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Close {}", tab.title)));
        if active || hovered || close_response.hovered() {
            if close_response.hovered() {
                ui.painter().rect_filled(close, CornerRadius::same(4), c.white(0.15));
            }
            icons::paint(ui, close, Icon::X, 13.0, ink);
        }
        divider(ui, rect.right(), rect);
        // Tabs of worktrees of one repository share a coloured line along their bottom.
        if self.settings.flag("autogroupWorktreeTabs", true)
            && let Some(group) = self.worktree_group(id)
        {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            group.hash(&mut h);
            let color = super::widgets::lane_color(h.finish() as usize % 16);
            let underline = Rect::from_min_max(pos2(rect.left() + 1.0, rect.bottom() - 8.0), pos2(rect.right() - 1.0, rect.bottom() - 5.0));
            ui.painter().rect_filled(underline, CornerRadius::ZERO, color);
        }
        let response = response.tip(tab.path().unwrap_or(&tab.title).to_owned());

        if close_response.clicked() || response.middle_clicked() {
            self.close_tab(id);
            return rect;
        }
        if response.clicked() {
            self.active = id;
            self.remember_tabs();
        }
        if response.drag_started() && !self.resize_hovered(ui, rect) {
            let grab = ui.ctx().pointer_interact_pos().map_or(0.0, |p| p.x - rect.left());
            self.tab_drag = Some(Drag { id, grab });
            self.active = id;
        }
        if let Some(path) = tab.path() {
            self.resize_handle(ui, rect, path.to_owned(), &tab.title);
        }
        response.context_menu(|ui| {
            if ui.button("Close tab").clicked() {
                self.close_tab(id);
            }
            if ui.button("Close other tabs").clicked() {
                let others: Vec<u64> = self.tabs.iter().filter(|t| t.id != id).map(|t| t.id).collect();
                self.close_tabs(others);
            }
            if ui.button("Close tabs to the right").clicked() {
                let at = self.tabs.iter().position(|t| t.id == id).unwrap_or(0);
                let right: Vec<u64> = self.tabs[at + 1..].iter().map(|t| t.id).collect();
                self.close_tabs(right);
            }
            ui.separator();
            if ui.button("Rename tab").clicked() {
                ui.close();
                self.rename_tab_dialog(id);
            }
            if ui.add_enabled(!self.closed.is_empty(), egui::Button::new("Reopen Closed Tab")).clicked() {
                self.reopen_tab();
            }
            ui.separator();
            let shortcut = |command| self.keys_of(command).map_or_else(String::new, |key| super::keys::label(&key));
            let left_key = shortcut("nav:moveTabLeft");
            let right_key = shortcut("nav:moveTabRight");
            if ui.add_enabled(!first, egui::Button::new("Move Tab Left").shortcut_text(left_key)).clicked() {
                self.shift_tab(id, -1);
            }
            if ui.add_enabled(!last, egui::Button::new("Move Tab Right").shortcut_text(right_key)).clicked() {
                self.shift_tab(id, 1);
            }
        });
        rect
    }

    /// The tab that follows the pointer while it is dragged.
    fn drag_ghost(&self, ui: &Ui, rects: &[(u64, Rect)]) {
        let Some(drag) = &self.tab_drag else { return };
        let Some(tab) = self.tabs.iter().find(|t| t.id == drag.id) else { return };
        let Some((_, slot)) = rects.iter().find(|(id, _)| *id == drag.id) else { return };
        let Some(pointer) = ui.ctx().pointer_interact_pos().or_else(|| ui.input(|i| i.pointer.latest_pos())) else { return };
        let rect = Rect::from_min_size(pos2(pointer.x - drag.grab, slot.top()), vec2(slot.width(), HEIGHT));
        let c = p();
        ui.painter().rect_filled(rect.translate(vec2(1.0, 2.0)), CornerRadius::ZERO, c.black(0.25));
        paint_tab(ui, rect, tab, c.toolbar, c.white(1.0));
        icons::paint(ui, tab_close_rect(rect), Icon::X, 13.0, c.white(1.0));
        ui.painter().rect_stroke(rect.shrink(1.0), CornerRadius::ZERO, Stroke::new(1.0, ACCENT), egui::StrokeKind::Inside);
    }

    fn resize_hovered(&self, ui: &Ui, rect: Rect) -> bool {
        ui.ctx().pointer_interact_pos().is_some_and(|p| p.x >= rect.right() - 5.0)
    }

    /// The 5 px handle on a repository tab's right edge.
    fn resize_handle(&mut self, ui: &mut Ui, rect: Rect, path: String, title: &str) {
        let handle = Rect::from_min_max(pos2(rect.right() - 5.0, rect.top()), rect.right_bottom());
        let id = Id::NULL.with(("tab_resize", &path));
        let response = ui.interact(handle, id, Sense::click_and_drag()).tip(format!("Drag to resize the {title} tab. Escape cancels. Double-click to fit the title."));
        let memory: Option<Resize> = ui.data(|d| d.get_temp(id));
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
        }
        if response.drag_started() {
            let start_x = ui.ctx().pointer_interact_pos().map_or(rect.right(), |p| p.x);
            let was_manual = self.settings.tab_width(&path).is_some();
            ui.data_mut(|d| d.insert_temp(id, Resize { start_x, start_width: rect.width(), was_manual }));
        }
        if let Some(start) = memory {
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                if start.was_manual {
                    self.settings.set_tab_width(&path, start.start_width);
                } else {
                    self.settings.clear_tab_width(&path);
                }
                ui.data_mut(|d| d.remove::<Resize>(id));
            } else if response.dragged() {
                let x = ui.ctx().pointer_interact_pos().map_or(start.start_x, |p| p.x);
                if let Some(width) = manual_tab_width(Some(start.start_width + x - start.start_x)) {
                    self.settings.set_tab_width(&path, width);
                }
            } else if response.drag_stopped() {
                ui.data_mut(|d| d.remove::<Resize>(id));
            }
        }
        if response.double_clicked() {
            self.settings.clear_tab_width(&path);
        }
        if response.hovered() || memory.is_some() {
            ui.painter().line_segment([pos2(rect.right() - 1.0, rect.top()), pos2(rect.right() - 1.0, rect.bottom())], Stroke::new(2.0, ACCENT));
        }
    }

    /// While a tab is dragged, move it past each neighbour the pointer crosses the middle of
    /// (all of them, however far it went in one frame), the release included.
    fn drop_dragged_tab(&mut self, ui: &Ui, rects: &[(u64, Rect)]) {
        let Some(drag) = &self.tab_drag else { return };
        let (dragged, grab) = (drag.id, drag.grab);
        let released = !ui.input(|i| i.pointer.primary_down());
        let pointer = ui.ctx().pointer_interact_pos().or_else(|| ui.input(|i| i.pointer.latest_pos()));
        if let Some(pointer) = pointer
            && let Some(&(_, own)) = rects.iter().find(|(id, _)| *id == dragged)
        {
            let left = pointer.x - grab;
            let right = left + own.width();
            // The furthest neighbour passed: the last one to the right, the first to the left.
            let mut target = None;
            for (id, rect) in rects {
                if *id == dragged {
                    continue;
                }
                if rect.left() > own.left() && right > rect.center().x {
                    target = Some(*id);
                }
                if rect.left() < own.left() && left < rect.center().x && target.is_none() {
                    target = Some(*id);
                }
            }
            if let Some(target) = target {
                let a = self.tabs.iter().position(|t| t.id == dragged);
                let b = self.tabs.iter().position(|t| t.id == target);
                if let (Some(a), Some(b)) = (a, b) {
                    let tab = self.tabs.remove(a);
                    self.tabs.insert(b, tab);
                }
            }
        }
        if released {
            self.tab_drag = None;
            self.remember_tabs();
        }
    }

    fn rename_tab_dialog(&mut self, id: u64) {
        use super::dialogs::{Dialog, Field, FieldKind};
        let Some(tab) = self.tabs.iter().find(|t| t.id == id) else { return };
        let field = Field::new("name", "Tab name", FieldKind::Text);
        self.dialog = Some(Dialog::form("Rename tab", vec![field], "Rename", move |app, values| {
            let Some(tab) = app.tabs.iter_mut().find(|t| t.id == id) else { return };
            let name = values.text("name");
            let name = name.trim();
            if let TabKind::Repo(path) = &tab.kind {
                app.settings.set_alias(path, name);
                tab.title = if name.is_empty() { folder_name(path) } else { name.to_owned() };
            } else if !name.is_empty() {
                tab.title = name.to_owned();
            }
        }).text_value("name", &tab.title));
    }

    fn tabs_list(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().max_height((ui.ctx().content_rect().height() - 108.0).max(100.0)).show(ui, |ui| {
            ui.label("Open Tabs");
            let mut activate = None;
            for tab in &self.tabs {
                if ui.selectable_label(self.active == tab.id, &tab.title).clicked() {
                    activate = Some(tab.id);
                }
            }
            if let Some(id) = activate {
                self.activate(id);
                ui.close();
            }
            let mut closed = self.closed.iter().rev().filter(|tab| tab.path().is_some()).peekable();
            if closed.peek().is_some() {
                ui.separator();
                ui.label("Closed Recently");
                let reopen = closed.find_map(|tab| ui.button(&tab.title).clicked().then_some(tab.id));
                if let Some(id) = reopen {
                    self.reopen_closed_tab(id);
                    ui.close();
                }
            }
        });
    }

    fn profile_button(&mut self, ui: &mut Ui) {
        let c = p();
        let (name, icon) = self.settings.active_profile();
        let width = profile_width(ui, &name);
        let font = FontId::proportional(PROFILE_FONT_SIZE);
        let galley = ui.painter().layout_no_wrap(name, font, c.white(0.9));
        let (rect, response) = ui.allocate_exact_size(vec2(width, HEIGHT), Sense::click());
        if response.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::ZERO, c.white(0.05));
        }
        let mut x = rect.left() + PROFILE_PADDING;
        let mark = Rect::from_min_size(pos2(x, rect.center().y - PROFILE_MARK / 2.0), Vec2::splat(PROFILE_MARK));
        profile_mark(ui, mark, PROFILE_COLORS[icon % PROFILE_COLORS.len()]);
        x += PROFILE_MARK + PROFILE_GAP;
        let text_y = rect.center().y - galley.size().y / 2.0;
        let text_width = galley.size().x;
        ui.painter().galley(pos2(x, text_y), galley, c.white(0.9));
        x += text_width + PROFILE_GAP;
        let chevron = pos2(x, rect.center().y - PROFILE_CHEVRON / 2.0);
        icons::paint(ui, Rect::from_min_size(chevron, Vec2::splat(PROFILE_CHEVRON)), Icon::ChevronDown, PROFILE_CHEVRON, c.white(0.9));
        egui::Popup::menu(&response).align(egui::RectAlign::BOTTOM_END).show(|ui| {
            ui.label(egui::RichText::new("PROFILES").size(11.0).color(c.fg2));
            let active = self.settings.active_profile_index();
            for (i, profile) in self.settings.profile_names().into_iter().enumerate() {
                let mark = if i == active { "✓ " } else { "   " };
                if ui.button(format!("{mark}{profile}")).clicked() && i != active {
                    self.switch_profile(i);
                }
            }
            ui.separator();
            if ui.button("Manage profiles...").clicked() {
                self.open_prefs(Some("profiles"));
            }
            if ui.button("Accounts...").clicked() {
                self.open_prefs(Some("integrations"));
            }
            if ui.button("Preferences...").clicked() {
                self.open_prefs(None);
            }
        });
    }
}

/// `text` cut to fit `width`, ending in an ellipsis.
fn truncated(ui: &Ui, text: &str, width: f32, color: Color32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), FontId::proportional(TAB_FONT_SIZE), color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width);
    ui.fonts_mut(|f| f.layout_job(job))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_tab_widths_stay_within_their_limits() {
        assert_eq!(manual_tab_width(None), None);
        assert_eq!(manual_tab_width(Some(10.0)), Some(MIN_TAB_WIDTH));
        assert_eq!(manual_tab_width(Some(9000.0)), Some(MAX_TAB_WIDTH));
        assert_eq!(manual_tab_width(Some(f32::NAN)), None);
    }

    #[test]
    fn tab_widths_fit_titles_and_share_the_room_before_truncating() {
        // Enough room: every tab keeps the width its title asks for.
        let natural = [100.0, 160.0, 220.0];
        assert_eq!(fit_tab_widths(&natural, 600.0), natural.to_vec());
        // A short title still gets a usable tab.
        assert_eq!(fit_tab_widths(&[10.0], 600.0), vec![MIN_TAB_WIDTH]);
        // A title wider than the old ceiling still fits when there is room.
        assert_eq!(fit_tab_widths(&[700.0], 800.0), vec![700.0]);
        // Not enough room: a short title keeps its width, the long ones share the rest, nothing
        // falls under the minimum, and the tabs share exactly what is there instead of scrolling.
        let fitted = fit_tab_widths(&[90.0, 340.0], 300.0);
        assert_eq!(fitted[0], 90.0);
        assert!((fitted[1] - 210.0).abs() < 0.5);
        let fitted = fit_tab_widths(&natural, 300.0);
        assert!(fitted.iter().zip(natural).all(|(w, n)| *w >= MIN_TAB_WIDTH && *w <= n));
        assert_eq!(fitted[1], fitted[2]);
        assert!((fitted.iter().sum::<f32>() - 300.0).abs() < 0.5);
        // Past the minimum widths the strip scrolls rather than squeezing tabs away.
        assert_eq!(fit_tab_widths(&[400.0; 10], 200.0), vec![MIN_TAB_WIDTH; 10]);
    }

    #[test]
    fn longer_titles_ask_for_wider_tabs() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let repo = |title: &str| Tab { id: 1, kind: TabKind::Repo("/repo".into()), title: title.into() };
            let short = natural_tab_width(ui, &repo("app"));
            let long = natural_tab_width(ui, &repo("a-much-longer-repository-name"));
            assert!(short >= MIN_TAB_WIDTH);
            assert!(long > short);
            // The start tab has no branch icon, so the same title needs less room.
            let start = Tab { id: 2, kind: TabKind::Start, title: "New Tab".into() };
            assert!(natural_tab_width(ui, &start) < natural_tab_width(ui, &repo("New Tab")));
        });
        output.textures_delta.clear();
    }

    #[test]
    fn a_dragged_width_can_be_forgotten_to_fit_the_title_again() {
        let mut settings = crate::settings::Store::in_memory();
        settings.set_tab_width("/repo", 222.0);
        assert_eq!(settings.tab_width("/repo"), Some(222.0));
        settings.clear_tab_width("/repo");
        assert_eq!(settings.tab_width("/repo"), None);
    }

    #[test]
    fn repository_tab_rename_persists_when_reopened_and_blank_restores_folder_name() {
        let ctx = eframe::CreationContext::_new_kittest(egui::Context::default());
        let mut app = App::new(&ctx, super::super::StartOptions::default(), crate::settings::Store::in_memory());
        app.open_repo("/fixture/alpha");
        let alpha = app.active;
        app.open_repo("/fixture/beta");
        let beta = app.active;
        let rename = |app: &mut App, id, name: &str| {
            app.rename_tab_dialog(id);
            let mut dialog = app.dialog.take().expect("rename dialog").text_value("name", name);
            (dialog.choices.remove(0).then)(app, &dialog.values);
        };
        rename(&mut app, alpha, "  Alpha review  ");
        assert_eq!(app.settings.alias("/fixture/alpha").as_deref(), Some("Alpha review"));
        app.close_tab_now(alpha);
        app.close_tab_now(beta);
        app.reopen_closed_tab(alpha);
        assert_eq!(app.active_tab().unwrap().title, "Alpha review");
        assert_eq!(app.closed.last().map(|tab| tab.id), Some(beta));
        let reopened = app.active;
        rename(&mut app, reopened, " ");
        assert_eq!(app.settings.alias("/fixture/alpha"), None);
        assert_eq!(app.active_tab().unwrap().title, "alpha");
        app.reopen_tab();
        assert_eq!(app.active_tab().unwrap().title, "beta");
    }

    #[test]
    fn tabs_list_command_toggles_its_popup() {
        let ctx = eframe::CreationContext::_new_kittest(egui::Context::default());
        let mut app = App::new(&ctx, super::super::StartOptions::default(), crate::settings::Store::in_memory());
        app.run_command("nav:tabsList");
        assert!(egui::Popup::is_id_open(&app.ctx, tabs_list_id()));
        app.run_command("nav:tabsList");
        assert!(!egui::Popup::is_id_open(&app.ctx, tabs_list_id()));
    }
}
