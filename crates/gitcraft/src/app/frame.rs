//! Who draws the window's title bar and edges, decided by `craft-window` for each desktop:
//!
//! - GNOME, KDE, Windows and most other floating desktops: the app draws its own title bar, with
//!   window buttons in the desktop's look and order (Adwaita, Breeze, Windows, generic).
//! - macOS: the system's traffic lights sit over the app's title bar.
//! - Tiling compositors and window managers (Sway, Hyprland, i3, …): the system's frame.
//!
//! The `systemTitleBar` preference gives the frame to the system everywhere (after a restart).
//! `GITCRAFT_TITLEBAR=app` or `=system` (or `CRAFT_TITLEBAR`) wins over that preference.
//!
//! With the `menusInTitleBar` preference (on by default), the File, Edit, View and Help menus
//! share the app's title bar instead of taking a row of their own, as in VS Code.

use super::widgets::Tip;
use super::theme::{BAD, p};
use craft_window::policy::{Backend, CaptionFamily, Captions, Frame, TitleBarPref};
use craft_window::{Button, Layout, Prefix};
use egui::{Color32, CornerRadius, CursorIcon, Id, Pos2, Rect, ResizeDirection, Sense, Stroke, StrokeKind, Ui, ViewportCommand, vec2};
use std::sync::OnceLock;

const HEIGHT: f32 = if cfg!(windows) { 32.0 } else { 36.0 };
pub const RADIUS: u8 = 10;
/// How far in from the window's border the resize handles reach (corners twice as far).
const EDGE: f32 = 9.0;

static FRAME: OnceLock<Frame> = OnceLock::new();

/// Draws the menus in a room of the title bar and returns the room they took.
type MenuRow<'a> = &'a mut dyn FnMut(&mut Ui, Rect) -> Rect;

/// Decide the frame once, before the window is created.
pub fn decide(system_title_bar: bool) -> Frame {
    let pref = if system_title_bar { TitleBarPref::System } else { TitleBarPref::Integrated };
    let mut frame = craft_window::policy::decide_for(Prefix::new("GITCRAFT"), Backend::from_env(), pref).frame;
    if let Frame::Integrated(c) = &mut frame {
        c.layout = all_buttons(&c.layout);
    }
    frame
}

/// The desktop's button order, with minimize and maximize added where the desktop leaves them out
/// (GNOME shows only close by default): next to close, on its side, so the order reads
/// minimize, maximize, close from the inside out.
fn all_buttons(layout: &Layout) -> Layout {
    let mut out = layout.clone();
    let on_left = out.left.contains(&Button::Close) && !out.right.contains(&Button::Close);
    for b in [Button::Minimize, Button::Maximize] {
        if out.left.contains(&b) || out.right.contains(&b) {
            continue;
        }
        let side = if on_left { &mut out.left } else { &mut out.right };
        let close = side.iter().position(|x| *x == Button::Close);
        let maximize = side.iter().position(|x| *x == Button::Maximize);
        match (on_left, close) {
            // Close on the left is outermost, so the others go to its right.
            (true, Some(i)) => side.insert(i + 1, b),
            // On the right, Close is outermost, so Maximize goes before it and Minimize before Maximize.
            (false, Some(i)) => side.insert(maximize.filter(|_| b == Button::Minimize).unwrap_or(i), b),
            (_, None) => side.push(b),
        }
    }
    out
}

/// Keep the decision for the rest of the run and set it on the window to be created.
pub fn install(frame: Frame, viewport: egui::ViewportBuilder) -> egui::ViewportBuilder {
    let viewport = craft_window::policy::apply(viewport, &frame);
    // Round corners need a see-through window; only Linux desktops leave the corners to the app.
    let viewport = viewport.with_transparent(rounded_for(&frame));
    let _ = FRAME.set(frame);
    viewport
}

fn current() -> &'static Frame {
    FRAME.get_or_init(|| Frame::Native)
}

fn captions() -> Option<&'static Captions> {
    match current() {
        Frame::Integrated(c) => Some(c),
        _ => None,
    }
}

fn rounded_for(frame: &Frame) -> bool {
    cfg!(not(any(target_os = "windows", target_os = "macos"))) && matches!(frame, Frame::Integrated(_))
}

/// Whether the window is see-through with round corners the app paints.
pub fn rounded() -> bool {
    rounded_for(current())
}

fn maximized(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.viewport().maximized.unwrap_or(false))
}

fn fullscreen(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.viewport().fullscreen.unwrap_or(false))
}

fn round_now(ctx: &egui::Context) -> bool {
    rounded() && !maximized(ctx) && !fullscreen(ctx)
}

/// Corner rounding for the bottom bar, so it follows the window's corners.
pub fn bottom_corners(ctx: &egui::Context) -> CornerRadius {
    if round_now(ctx) { CornerRadius { sw: RADIUS, se: RADIUS, ..CornerRadius::ZERO } } else { CornerRadius::ZERO }
}

/// Rounding of the whole window.
pub fn window_corners(ctx: &egui::Context) -> CornerRadius {
    if round_now(ctx) { CornerRadius::same(RADIUS) } else { CornerRadius::ZERO }
}

/// The width a window button takes in the bar, gap included.
fn slot(family: CaptionFamily) -> f32 {
    match family {
        CaptionFamily::Windows => 46.0,
        CaptionFamily::Breeze => 30.0,
        CaptionFamily::Adwaita | CaptionFamily::Generic => 34.0,
    }
}

fn button_id(kind: Button) -> Id {
    Id::NULL.with(("window_button", kind as u8))
}

/// Where a window button at `x` in the title `bar` takes clicks.
fn button_rect(bar: Rect, x: f32, family: CaptionFamily) -> Rect {
    let w = slot(family);
    match family {
        CaptionFamily::Windows => Rect::from_min_max(Pos2::new(x, bar.top()), Pos2::new(x + w, bar.bottom())),
        _ => Rect::from_center_size(Pos2::new(x + w / 2.0, bar.center().y), vec2(26.0, 26.0)),
    }
}

/// The window buttons' left edges in the title `bar`, as `title_bar` lays them out.
fn button_xs(c: &Captions, bar: Rect) -> Vec<(f32, Button)> {
    let w = slot(c.family);
    let pad = if c.family == CaptionFamily::Windows { 0.0 } else { 7.0 };
    let n = c.layout.right.len() as f32;
    let left = c.layout.left.iter().enumerate().map(|(i, k)| (bar.left() + pad + i as f32 * w, *k));
    let right = c.layout.right.iter().enumerate().map(|(i, k)| (bar.right() - pad - (n - i as f32) * w, *k));
    left.chain(right).collect()
}

fn window_button(ui: &mut Ui, bar: Rect, x: f32, family: CaptionFamily, kind: Button, is_max: bool) {
    let rect = button_rect(bar, x, family);
    let centre = Pos2::new(x + slot(family) / 2.0, bar.center().y);
    let response = ui.interact(rect, button_id(kind), Sense::click());
    let hovered = response.hovered();
    let close = kind == Button::Close;
    let paint = ui.painter();
    let mut ink = p().fg;
    match family {
        CaptionFamily::Windows => {
            if hovered {
                paint.rect_filled(rect, CornerRadius::ZERO, if close { Color32::from_rgb(196, 43, 28) } else { p().white(0.1) });
                if close {
                    ink = Color32::WHITE;
                }
            }
        }
        CaptionFamily::Breeze => {
            // Breeze: bare symbols; a filled circle on hover, red for close.
            if hovered {
                paint.circle_filled(centre, 10.0, if close { BAD } else { p().white(0.1) });
                if close {
                    ink = Color32::WHITE;
                }
            }
        }
        CaptionFamily::Adwaita | CaptionFamily::Generic => {
            let fill = match (close, hovered) {
                (true, true) => BAD,
                (_, true) => p().white(0.1),
                _ => p().panel2,
            };
            paint.circle_filled(centre, 12.0, fill);
            if close && hovered {
                ink = Color32::WHITE;
            }
        }
    }
    let stroke = Stroke::new(if family == CaptionFamily::Windows { 1.0 } else { 1.5 }, ink);
    let s = if family == CaptionFamily::Windows { 5.0 } else { 4.5 };
    match kind {
        Button::Minimize => {
            let y = if family == CaptionFamily::Windows { 0.0 } else { 3.0 };
            paint.line_segment([centre + vec2(-s, y), centre + vec2(s, y)], stroke);
        }
        Button::Maximize if is_max => {
            paint.rect_stroke(Rect::from_center_size(centre + vec2(-1.0, 1.0), vec2(8.0, 8.0)), 1.0, stroke, StrokeKind::Middle);
            paint.line_segment([centre + vec2(0.0, -4.0), centre + vec2(5.0, -4.0)], stroke);
            paint.line_segment([centre + vec2(5.0, -4.0), centre + vec2(5.0, 2.0)], stroke);
        }
        Button::Maximize => {
            paint.rect_stroke(Rect::from_center_size(centre, vec2(2.0 * s, 2.0 * s)), 1.0, stroke, StrokeKind::Middle);
        }
        Button::Close => {
            paint.line_segment([centre + vec2(-s, -s), centre + vec2(s, s)], stroke);
            paint.line_segment([centre + vec2(-s, s), centre + vec2(s, -s)], stroke);
        }
    }
    let tip = match kind {
        Button::Minimize => "Minimize",
        Button::Maximize if is_max => "Restore",
        Button::Maximize => "Maximize",
        Button::Close => "Close",
    };
    let response = response.tip(tip);
    if response.clicked() {
        ui.ctx().send_viewport_cmd(match kind {
            Button::Minimize => ViewportCommand::Minimized(true),
            Button::Maximize => ViewportCommand::Maximized(!is_max),
            Button::Close => ViewportCommand::Close,
        });
    }
}

/// The room the menus take when they share the title `bar`: after the icon or the traffic
/// lights and any left-hand buttons, and before the buttons on the right.
fn menu_area(bar: Rect) -> Rect {
    let (left, right) = match current() {
        Frame::Integrated(c) => {
            let w = slot(c.family);
            let pad = if c.family == CaptionFamily::Windows { 0.0 } else { 7.0 };
            let icon = if cfg!(windows) { 32.0 } else { 0.0 };
            (pad + icon + c.layout.left.len() as f32 * w, pad + c.layout.right.len() as f32 * w)
        }
        Frame::Inset(inset) => (inset.leading, 8.0),
        Frame::Native => (0.0, 0.0),
    };
    let x = bar.left() + left + 4.0;
    Rect::from_min_max(Pos2::new(x, bar.top()), Pos2::new((bar.right() - right - 4.0).max(x), bar.bottom()))
}

fn menus_id() -> Id {
    Id::NULL.with("title_bar_menus")
}

/// The menus' room in this frame's title bar, when they share it: the resize handles step aside
/// there, so a click at the top of a menu is not a resize.
fn menus_this_pass(ctx: &egui::Context) -> Option<Rect> {
    ctx.data(|d| d.get_temp::<(u64, Rect)>(menus_id())).filter(|(pass, _)| *pass == ctx.cumulative_pass_nr()).map(|(_, rect)| rect)
}

/// Draw the title and controls in the desktop's usual positions. Dragging the empty part moves
/// the window. A double click does what the system does.
///
/// `menus` draws the menu row in the room [`menu_area`] leaves, after the empty area so the
/// menus take their own clicks; the title text gives way to them. Returns whether the bar was
/// drawn: not in full screen, nor when the system draws the frame.
pub fn title_bar(ui: &mut Ui, title: &str, menus: Option<MenuRow<'_>>) -> bool {
    let height = match current() {
        Frame::Native => return false,
        // Tall enough that the traffic lights sit centred in it.
        Frame::Inset(inset) => (inset.centre_from_top * 2.0).max(28.0),
        Frame::Integrated(_) => HEIGHT,
    };
    if fullscreen(ui.ctx()) {
        return false;
    }
    let is_max = maximized(ui.ctx());
    let corner = if round_now(ui.ctx()) { RADIUS } else { 0 };
    egui::Panel::top("title_bar")
        .exact_size(height)
        .frame(egui::Frame::new())
        .show_separator_line(false)
        .show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
            ui.painter().rect_filled(rect, CornerRadius { nw: corner, ne: corner, sw: 0, se: 0 }, p().titlebar);
            ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, p().black(0.3)));
            // Before the controls, so they sit on top and take their own clicks.
            craft_window::title_bar::empty_area(ui, rect);
            let font = egui::FontId::proportional(13.5);
            if cfg!(windows) {
                let icon = Rect::from_center_size(egui::pos2(rect.left() + 16.0, rect.center().y), vec2(16.0, 16.0));
                egui::Image::from_bytes("bytes://gitcraft-caption.png", &include_bytes!("../../../../icons/128x128.png")[..])
                    .paint_at(ui, icon);
            }
            if let Some(draw) = menus {
                let area = menu_area(rect);
                let mut menu_ui = ui.new_child(egui::UiBuilder::new().max_rect(area));
                let used = draw(&mut menu_ui, area);
                let pass = ui.ctx().cumulative_pass_nr();
                ui.ctx().data_mut(|d| d.insert_temp(menus_id(), (pass, used)));
            } else if cfg!(windows) {
                let caption_width = captions().map_or(0.0, |c| c.layout.right.len() as f32 * slot(c.family));
                let width = (rect.width() - caption_width - 40.0).max(0.0);
                let title = super::widgets::fit(ui, title, font, p().fg, width);
                let pos = egui::pos2(rect.left() + 32.0, rect.center().y - title.size().y / 2.0);
                ui.painter().galley(pos, title, p().fg);
            } else {
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, title, font, p().fg);
            }
            if let Some(c) = captions() {
                for (x, kind) in button_xs(c, rect) {
                    window_button(ui, rect, x, c.family, kind, is_max);
                }
            }
        });
    true
}

/// Once per frame after the UI: on macOS, keeps AppKit from moving the window when a drag
/// starts on a control over the title bar.
pub fn after_ui(ctx: &egui::Context, applied: &mut Option<bool>) {
    #[cfg(target_os = "macos")]
    if matches!(current(), Frame::Inset(_)) {
        craft_window::title_bar::sync_movable(ctx, applied);
    }
    let _ = (ctx, applied);
}

/// Invisible handles along the window's edges for resizing, and a thin outline, when the app
/// draws the frame.
pub fn window_edges(ctx: &egui::Context) {
    let Some(captions) = captions() else { return };
    if maximized(ctx) || fullscreen(ctx) {
        return;
    }
    let full = ctx.content_rect();
    let (w, h) = (full.width(), full.height());
    let at = |x: f32, y: f32, width: f32, height: f32| Rect::from_min_size(full.min + vec2(x, y), vec2(width, height));
    let c = EDGE * 2.0;
    use CursorIcon as C;
    use ResizeDirection as D;
    // Edges first and corners last, as what is added later is on top.
    let zones = [
        (at(c, 0.0, w - 2.0 * c, EDGE), D::North, C::ResizeNorth),
        (at(c, h - EDGE, w - 2.0 * c, EDGE), D::South, C::ResizeSouth),
        (at(0.0, c, EDGE, h - 2.0 * c), D::West, C::ResizeWest),
        (at(w - EDGE, c, EDGE, h - 2.0 * c), D::East, C::ResizeEast),
        (at(0.0, 0.0, c, c), D::NorthWest, C::ResizeNorthWest),
        (at(w - c, 0.0, c, c), D::NorthEast, C::ResizeNorthEast),
        (at(0.0, h - c, c, c), D::SouthWest, C::ResizeSouthWest),
        (at(w - c, h - c, c, c), D::SouthEast, C::ResizeSouthEast),
    ];
    // Over a window button or the menus, they take the click: the handles that reach over
    // them (a top corner, the top edge) step aside while the pointer is there.
    let bar = Rect::from_min_size(full.min, vec2(w, HEIGHT));
    let over_button = ctx.pointer_hover_pos().and_then(|at| button_xs(captions, bar).into_iter().map(|(x, _)| button_rect(bar, x, captions.family)).find(|b| b.contains(at)));
    let menus = menus_this_pass(ctx).filter(|m| ctx.pointer_hover_pos().is_some_and(|p| m.contains(p)));
    let corners = window_corners(ctx);
    // Above everything, Preferences, dialogs and menus included: clicking into another layer
    // raises it over the others of its order, which would otherwise cover the edges.
    let id = Id::NULL.with("window_edges");
    ctx.move_to_top(egui::LayerId::new(egui::Order::Tooltip, id));
    egui::Area::new(id)
        .order(egui::Order::Tooltip)
        .fixed_pos(full.min)
        .show(ctx, |ui| {
            for (i, (rect, dir, cursor)) in zones.into_iter().enumerate() {
                if over_button.is_some_and(|b| b.intersects(rect)) || menus.is_some_and(|m| m.intersects(rect)) {
                    continue;
                }
                let r = ui.interact(rect, Id::NULL.with(("window_edge", i)), Sense::drag());
                if r.hovered() {
                    ctx.set_cursor_icon(cursor);
                }
                if r.drag_started() {
                    ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
                }
            }
            ui.painter().rect_stroke(full.shrink(0.5), corners, Stroke::new(1.0, p().black(0.3)), StrokeKind::Inside);
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use Button::{Close, Maximize, Minimize};

    fn layout(left: &[Button], right: &[Button]) -> Layout {
        Layout { left: left.to_vec(), right: right.to_vec() }
    }

    #[test]
    fn missing_buttons_are_added_next_to_close() {
        // GNOME's default `appmenu:close`.
        assert_eq!(all_buttons(&layout(&[], &[Close])), layout(&[], &[Minimize, Maximize, Close]));
        // macOS-like `close:` on the left.
        assert_eq!(all_buttons(&layout(&[Close], &[])), layout(&[Close, Maximize, Minimize], &[]));
        // A full layout stays as the desktop has it.
        assert_eq!(all_buttons(&layout(&[], &[Minimize, Maximize, Close])), layout(&[], &[Minimize, Maximize, Close]));
        assert_eq!(all_buttons(&layout(&[Close, Minimize, Maximize], &[])), layout(&[Close, Minimize, Maximize], &[]));
        // Only maximize missing.
        assert_eq!(all_buttons(&layout(&[], &[Minimize, Close])), layout(&[], &[Minimize, Maximize, Close]));
    }
}
