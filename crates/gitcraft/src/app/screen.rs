//! A repository tab's screen: the toolbar, the left panel (or its strip), the centre (graph,
//! file, history, conflict, rebase, pull request or issue), the terminal under it, and the
//! right panel, with draggable bars between them.

use super::App;
use super::ctxmenu::Later;
use super::right::WorkTab;
use super::theme::p;
use super::widgets;
use egui::{Align2, CornerRadius, FontId, Rect, Ui, pos2, vec2};

const DETAILS_MIN_WIDTH: f32 = 300.0;

impl App {
    pub(super) fn repo_screen(&mut self, ui: &mut Ui, tab: u64, path: &str) {
        self.ensure_repo(tab, path);
        self.tab_shown(tab);
        self.want_side(tab);
        let Some(mut r) = self.repos.remove(&tab) else { return };
        let mut later: Vec<Later> = Vec::new();
        let c = p();
        let full = ui.max_rect();
        // Toolbar.
        let bar = Rect::from_min_size(full.min, vec2(full.width(), 50.0));
        {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar));
            self.toolbar(&mut child, tab, Some(&r), &mut later);
        }
        let body = Rect::from_min_max(pos2(full.left(), bar.bottom()), full.max);
        let viewing = r.ui.viewing();
        if !viewing {
            self.left_peek = false;
        }
        let mut show_left = self.settings.flag("showLeft", true) && (!viewing || self.left_peek);
        let mut show_right = self.settings.flag("showRight", true) && r.view.is_some();
        let bottom = self.settings.flag("rightPanelBottom", false) && show_right;
        let mut left_w = if show_left { self.settings.num("leftWidth", 381.0) as f32 } else { 28.0 };
        let mut right_w = if show_right && !bottom { self.settings.num("rightWidth", 342.0) as f32 } else { 0.0 };
        let fit_right;
        (left_w, right_w, show_left, fit_right) = fit_panels(body.width(), left_w, right_w, show_left, show_right && !bottom, bottom);
        show_right = if bottom { show_right } else { fit_right };
        let left = Rect::from_min_size(body.min, vec2(left_w, body.height()));
        {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(left).layout(egui::Layout::top_down(egui::Align::Min)));
            child.set_clip_rect(left);
            if show_left {
                self.left_panel(&mut child, tab, &mut r, &mut later);
            } else {
                self.left_strip(&mut child, Some(tab), Some(&r), &mut later);
            }
        }
        let mut x = left.right();
        if show_left {
            let split = Rect::from_min_size(pos2(x, body.top()), vec2(2.0, body.height()));
            let dx = widgets::splitter(ui, split, ui.scope_id().with("left_split"), true).drag_delta().x;
            if dx != 0.0 {
                let w = (left_w + dx).clamp(200.0, 600.0);
                later.push(Box::new(move |app| app.settings.set("leftWidth", w.round())));
            }
            x += 2.0;
        }
        let centre_right = if show_right && !bottom { body.right() - right_w - 2.0 } else { body.right() };
        let combined = Rect::from_min_max(pos2(x, body.top()), pos2(centre_right.max(x), body.bottom()));
        let bottom_min = 428.0 + if r.work.tab == WorkTab::Commit && r.work.options_open { 110.0 } else { 0.0 };
        let bottom_h = if bottom { bottom_panel_height(combined.height(), self.settings.num("rightPanelHeight", 350.0) as f32, bottom_min) } else { 0.0 };
        let centre = Rect::from_min_max(combined.min, pos2(combined.right(), combined.bottom() - bottom_h - if bottom { 2.0_f32.min(combined.height()) } else { 0.0 }));
        let right = if bottom {
            Rect::from_min_max(pos2(combined.left(), centre.bottom() + 2.0_f32.min(combined.height())), combined.max)
        } else {
            Rect::from_min_max(pos2(body.right() - right_w, body.top()), body.max)
        };
        // Terminal under the centre.
        // It gives way when the centre is too short for it and the graph above it.
        let show_term = self.settings.flag("showTerminal", false) && r.view.is_some() && centre.height() >= 100.0 + 2.0 + 120.0;
        // Minimized, only the terminal's tab bar stays.
        let minimized = self.settings.flag("terminalMinimized", false);
        let term_h = if !show_term { 0.0 } else if minimized { 27.0 } else { (self.settings.num("terminalHeight", 220.0) as f32).clamp(100.0, (centre.height() - 120.0).max(100.0)) };
        let main = Rect::from_min_max(centre.min, pos2(centre.right(), centre.bottom() - term_h - if show_term { 2.0 } else { 0.0 }));
        {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(main).layout(egui::Layout::top_down(egui::Align::Min)));
            child.set_clip_rect(main);
            child.painter().rect_filled(main, CornerRadius::ZERO, c.app);
            self.centre(&mut child, tab, &mut r, &mut later);
        }
        if show_term {
            let split = Rect::from_min_size(pos2(centre.left(), main.bottom()), vec2(centre.width(), 2.0));
            let dy = if minimized { 0.0 } else { widgets::splitter(ui, split, ui.scope_id().with("term_split"), false).drag_delta().y };
            if dy != 0.0 {
                let h = (term_h - dy).clamp(100.0, 800.0);
                later.push(Box::new(move |app| app.settings.set("terminalHeight", h.round())));
            }
            let term = Rect::from_min_max(pos2(centre.left(), split.bottom()), centre.max);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(term).layout(egui::Layout::top_down(egui::Align::Min)));
            child.set_clip_rect(term);
            self.terminal_panel(&mut child, tab, &r, &mut later);
        }
        if show_right {
            if bottom {
                let split = Rect::from_min_max(pos2(combined.left(), centre.bottom()), pos2(combined.right(), right.top()));
                let response = widgets::splitter(ui, split, ui.scope_id().with("right_bottom_split"), false);
                if let Some(height) = dragged_details_size(&response, bottom_h, false) {
                    let h = bottom_panel_height(combined.height(), height, bottom_min);
                    later.push(Box::new(move |app| app.settings.set("rightPanelHeight", h.round())));
                }
            } else {
                let split = Rect::from_min_size(pos2(right.left() - 2.0, body.top()), vec2(2.0, body.height()));
                let response = widgets::splitter(ui, split, ui.scope_id().with("right_split"), true);
                if let Some(width) = dragged_details_size(&response, right_w, true) {
                    let w = width.clamp(DETAILS_MIN_WIDTH, 640.0);
                    later.push(Box::new(move |app| app.settings.set("rightWidth", w.round())));
                }
            }
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(right).layout(egui::Layout::top_down(egui::Align::Min)));
            child.set_clip_rect(right);
            self.right_panel(&mut child, tab, &mut r, &mut later);
        }
        self.repos.insert(tab, r);
        for f in later {
            f(self);
        }
    }

    /// The centre: an error, a loading note, or what the tab shows.
    fn centre(&self, ui: &mut Ui, tab: u64, r: &mut super::repo::RepoTab, later: &mut Vec<Later>) {
        let c = p();
        let rect = ui.max_rect();
        if r.view.is_none() {
            if let Some(e) = r.error.clone() {
                let filtered = r.ui.history_filter.is_some();
                ui.add_space(rect.height() * 0.35);
                ui.vertical_centered(|ui| {
                    ui.spacing_mut().item_spacing.y = 12.0;
                    ui.label(egui::RichText::new(if filtered { "History filters could not be applied" } else { "This repository could not be opened" }).size(15.0).color(c.white(1.0)));
                    if filtered && widgets::link(ui, "Clear history filters", 13.0).clicked() {
                        later.push(Box::new(move |app| app.set_history_filter(tab, None)));
                    }
                    ui.add(egui::Label::new(egui::RichText::new(&e).size(12.5).color(c.fg2)).wrap());
                    ui.label(egui::RichText::new(&r.path).size(12.0).color(c.fg3));
                    ui.horizontal(|ui| {
                        let w = 270.0;
                        ui.add_space((ui.available_width() - w) / 2.0);
                        ui.spacing_mut().item_spacing.x = 16.0;
                        if widgets::link(ui, "Try again", 13.0).clicked() {
                            later.push(Box::new(move |app| app.load_repo(tab)));
                        }
                        if widgets::link(ui, "Locate folder…", 13.0).clicked() {
                            later.push(Box::new(move |app| app.pick_folder("Locate the repository folder", super::dialogs::Pick::Locate(tab))));
                        }
                        if widgets::link(ui, "Close tab", 13.0).clicked() {
                            later.push(Box::new(move |app| app.close_tab(tab)));
                        }
                    });
                });
            } else {
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, "Loading repository…", FontId::proportional(13.0), c.fg3);
            }
            return;
        }
        if let Some(n) = r.ui.pr {
            self.pr_view(ui, tab, r, n, later);
        } else if let Some(id) = r.ui.issue.clone() {
            self.issue_view(ui, tab, r, &id, later);
        } else if r.ui.rebase.is_some() {
            self.rebase_view(ui, tab, r, later);
        } else if let Some(file) = r.ui.conflict.clone() {
            self.conflict_view(ui, tab, r, &file, later);
        } else if let Some(target) = r.ui.diff.clone() {
            self.diff_view(ui, tab, r, &target, later);
        } else if let Some(h) = r.ui.history.clone() {
            self.history_view(ui, tab, r, &h, later);
        } else {
            self.graph_view(ui, tab, r, later);
        }
    }
}

pub(super) fn dragged_details_size(response: &egui::Response, size: f32, vertical: bool) -> Option<f32> {
    let delta = response.total_drag_delta()?;
    let id = response.id.with("start_size");
    let started = response.drag_started();
    let start = response.ctx.data_mut(|data| {
        if started {
            data.insert_temp(id, size);
        }
        data.get_temp::<f32>(id)
    })?;
    if started && delta == egui::Vec2::ZERO {
        return None;
    }
    Some(start - if vertical { delta.x } else { delta.y })
}

fn bottom_panel_height(height: f32, preferred: f32, panel_min: f32) -> f32 {
    let available = (height - 2.0).max(0.0);
    let centre_min = 100.0_f32.min(available / 2.0);
    let maximum = available - centre_min;
    preferred.clamp(panel_min.min(maximum), maximum)
}

/// Side panel widths for a body this wide, as the earlier web interface laid them out: the
/// centre keeps up to 440 px when it can, then gives way down to 160 px while both panels
/// shrink towards their smallest (200 and 300 px) in proportion to what they can spare. Only a
/// window too narrow for even that collapses a panel, the left one first, so staging and the
/// commit box stay. In the bottom layout, the centre also has to fit the details panel.
/// Saved widths are left as they are.
fn fit_panels(body: f32, mut left_w: f32, mut right_w: f32, mut show_left: bool, mut show_right: bool, bottom: bool) -> (f32, f32, bool, bool) {
    const COLLAPSED: f32 = 28.0;
    let centre_min = if bottom { DETAILS_MIN_WIDTH } else { 160.0 };
    let min_l = if show_left { 200.0_f32.min(left_w) } else { left_w };
    let min_r = if show_right { DETAILS_MIN_WIDTH.min(right_w) } else { 0.0 };
    // A splitter is drawn beside each panel that is shown (a hidden left panel is only a strip, with none).
    let splitters = |left: bool, right: bool| 4.0 * f32::from(u8::from(left) + u8::from(right));
    let preferred_centre = 440.0_f32.min(body - 504.0).max(centre_min);
    let most = body - centre_min - splitters(show_left, show_right);
    let room = (body - preferred_centre - splitters(show_left, show_right)).max(most.min(min_l + min_r));
    if left_w + right_w > room {
        let (spare_l, spare_r) = ((left_w - min_l).max(0.0), (right_w - min_r).max(0.0));
        let spare = spare_l + spare_r;
        if spare > 0.0 {
            let cut = (left_w + right_w - room).min(spare);
            left_w -= cut * spare_l / spare;
            right_w -= cut * spare_r / spare;
        }
    }
    if left_w + right_w > most && show_left {
        show_left = false;
        left_w = COLLAPSED;
    }
    // The left panel is gone, and with it its splitter: the room for the rest is judged on what is drawn now.
    let most = body - centre_min - splitters(show_left, show_right);
    if left_w + right_w > most && show_right {
        show_right = false;
        right_w = 0.0;
    }
    (left_w, right_w, show_left, show_right)
}

#[cfg(test)]
mod tests {
    use super::{DETAILS_MIN_WIDTH, bottom_panel_height, dragged_details_size, fit_panels};

    #[test]
    fn resizing_details_uses_the_pointer_displacement_from_the_press() {
        use egui::{CentralPanel, Context, Event, Modifiers, PointerButton, RawInput, Rect, Sense, pos2, vec2};

        let ctx = Context::default();
        let press = pos2(160.0, 120.0);
        let divider = Rect::from_min_size(pos2(100.0, 118.0), vec2(120.0, 4.0));
        let frame = |events, size| {
            let mut next = None;
            ctx.run_ui(RawInput { events, ..Default::default() }, |ui| {
                CentralPanel::default().show(ui, |ui| {
                    let response = ui.interact(divider, ui.scope_id().with("details_split"), Sense::drag());
                    next = dragged_details_size(&response, size, false);
                });
            }).textures_delta.clear();
            next
        };

        frame(vec![Event::PointerMoved(pos2(50.0, 60.0))], 428.0);
        let press_events = vec![
            Event::PointerMoved(press),
            Event::PointerButton { pos: press, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE },
        ];
        assert_eq!(frame(press_events, 428.0), None);
        assert_eq!(frame(vec![Event::PointerMoved(pos2(160.0, 105.0))], 428.0), Some(443.0));
        assert_eq!(frame(vec![Event::PointerMoved(pos2(160.0, 90.0))], 443.0), Some(458.0));
    }

    #[test]
    fn bottom_panel_height_fits_short_and_normal_windows() {
        assert_eq!(bottom_panel_height(800.0, 350.0, 428.0), 428.0);
        assert_eq!(bottom_panel_height(800.0, 900.0, 428.0), 698.0);
        assert_eq!(bottom_panel_height(100.0, 350.0, 428.0), 49.0);
        assert_eq!(bottom_panel_height(0.0, 350.0, 428.0), 0.0);
    }

    #[test]
    fn bottom_panel_cannot_be_dragged_below_the_work_form_when_room_is_available() {
        assert_eq!(bottom_panel_height(800.0, 0.0, 428.0), 428.0);
        assert_eq!(bottom_panel_height(800.0, 0.0, 538.0), 538.0);
        assert_eq!(bottom_panel_height(500.0, 0.0, 538.0), 398.0);
    }

    #[test]
    fn the_default_window_keeps_both_panels() {
        // The default 930 px window with the default widths: both at their smallest.
        let (l, r, show_l, show_r) = fit_panels(930.0, 381.0, 342.0, true, true, false);
        assert!(show_l && show_r);
        assert_eq!((l.round(), r.round()), (200.0, 300.0));
        // Wide enough: the saved widths as they are.
        assert_eq!(fit_panels(1600.0, 381.0, 342.0, true, true, false), (381.0, 342.0, true, true));
    }

    #[test]
    fn a_narrow_window_collapses_the_left_panel_before_staging() {
        let (_, r, show_l, show_r) = fit_panels(600.0, 381.0, 342.0, true, true, false);
        assert!(!show_l && show_r);
        assert!(r >= 300.0);
        let (_, _, show_l, show_r) = fit_panels(400.0, 381.0, 342.0, true, true, false);
        assert!(!show_l && !show_r);
        // With the left panel hidden by the person, the right one gets the room.
        let (l, r, _, show_r) = fit_panels(700.0, 28.0, 342.0, false, true, false);
        assert!(show_r && l == 28.0 && r >= 300.0);
    }

    #[test]
    fn the_bottom_panel_keeps_room_for_commit_controls() {
        let (left, _, show_left, _) = fit_panels(620.0, 381.0, 0.0, true, false, true);
        assert!(show_left && 620.0 - left - 2.0 >= DETAILS_MIN_WIDTH);
        let (left, _, show_left, _) = fit_panels(400.0, 381.0, 0.0, true, false, true);
        assert!(!show_left && 400.0 - left >= DETAILS_MIN_WIDTH);
    }
}
