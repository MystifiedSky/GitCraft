//! Small pieces the screens share: the button kinds, section headers, file status marks,
//! avatars, colour mixing and dates.

use super::icons::{self, Icon};
use super::theme::{ACCENT, AI, BAD, GOOD, p};
use egui::{Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

/// The 10 graph lane colours.
pub const LANE_COLORS: [Color32; 10] = [
    Color32::from_rgb(0x23, 0xad, 0xc1),
    Color32::from_rgb(0x56, 0x8f, 0xdf),
    Color32::from_rgb(0xab, 0x78, 0xcf),
    Color32::from_rgb(0xc5, 0x7b, 0xb7),
    Color32::from_rgb(0xd7, 0x7c, 0x9b),
    Color32::from_rgb(0xd4, 0x77, 0x77),
    Color32::from_rgb(0xdc, 0x99, 0x6b),
    Color32::from_rgb(0xd5, 0xbc, 0x6d),
    Color32::from_rgb(0x91, 0xb8, 0x79),
    Color32::from_rgb(0x67, 0xb6, 0xa3),
];

pub const MODIFIED: Color32 = Color32::from_rgb(0xde, 0x9b, 0x43);
pub const REMOVED: Color32 = Color32::from_rgb(0xef, 0x7a, 0x76);
pub const SELECTED_BRANCH: Color32 = Color32::from_rgb(0x15, 0xa0, 0xbf);

pub fn lane_color(i: usize) -> Color32 {
    LANE_COLORS[i % LANE_COLORS.len()]
}

/// `color` over the app background at `amount` (CSS `color-mix`).
pub fn mix(color: Color32, amount: f32) -> Color32 {
    mix_over(color, amount, p().app)
}

pub fn mix_over(color: Color32, amount: f32, bg: Color32) -> Color32 {
    let m = |a: u8, b: u8| (a as f32 * amount + b as f32 * (1.0 - amount)).round() as u8;
    Color32::from_rgb(m(color.r(), bg.r()), m(color.g(), bg.g()), m(color.b(), bg.b()))
}

/// A stable colour for an author, from the lane colours.
pub fn author_color(email: &str) -> Color32 {
    let mut h: u32 = 0;
    for ch in email.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(ch as u32);
    }
    LANE_COLORS[(h % 10) as usize]
}

pub fn initials(name: &str) -> String {
    let parts: Vec<&str> = name.split_whitespace().collect();
    match parts.as_slice() {
        [] => "?".into(),
        [one] => one.chars().take(2).collect::<String>().to_uppercase(),
        [first, .., last] => format!("{}{}", first.chars().next().unwrap_or('?'), last.chars().next().unwrap_or('?')).to_uppercase(),
    }
}

/// An author's round mark: their Gravatar picture, or their initials on their colour.
pub fn paint_avatar(ui: &Ui, center: egui::Pos2, size: f32, name: &str, email: &str) {
    let painter = ui.painter();
    if let Some(picture) = super::avatars::texture(ui.ctx(), email) {
        super::avatars::paint(painter, &picture, center, size, 1.0);
        return;
    }
    painter.circle_filled(center, size / 2.0, author_color(&email.trim().to_lowercase()));
    painter.text(center, Align2::CENTER_CENTER, initials(name), FontId::proportional((size * 0.42).max(7.0)), Color32::WHITE);
}

pub fn avatar(ui: &mut Ui, size: f32, name: &str, email: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_avatar(ui, rect.center(), size, name, email);
    response
}

/// The button kinds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Default,
    Primary,
    Success,
    Danger,
    Ai,
}

/// A button `height` tall with `text` (and an icon), in one of the app's kinds.
pub fn button(ui: &mut Ui, kind: Kind, icon: Option<Icon>, text: &str, height: f32, enabled: bool) -> Response {
    button_labeled(ui, kind, icon, text, text, height, enabled)
}

/// A button drawn with `text` but named `accessible` for screen readers.
pub fn button_labeled(ui: &mut Ui, kind: Kind, icon: Option<Icon>, text: &str, accessible: &str, height: f32, enabled: bool) -> Response {
    let c = p();
    let size = if height <= 20.5 { 11.0 } else if height <= 24.5 { 12.0 } else if height >= 40.0 { 14.0 } else { 13.0 };
    let pad = if height <= 24.5 { 8.0 } else { 12.0 };
    let text_color = match kind {
        Kind::Danger => REMOVED,
        Kind::Default => c.fg,
        _ => c.white(1.0),
    };
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::proportional(size), text_color);
    let icon_w = icon.map_or(0.0, |_| size + if text.is_empty() { 0.0 } else { 6.0 });
    let width = pad * 2.0 + icon_w + galley.size().x;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), if enabled { Sense::click() } else { Sense::hover() });
    // Screen readers get what egui's own button tells them: a button, its label, whether it works.
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, accessible));
    let hover = enabled && response.hovered();
    let (fill, stroke) = match kind {
        Kind::Default => (c.white(if hover { 0.1 } else { 0.05 }), c.white(0.1)),
        Kind::Primary => (ACCENT.gamma_multiply(if hover { 0.3 } else { 0.2 }), ACCENT.gamma_multiply(0.6)),
        Kind::Success => (GOOD.gamma_multiply(if hover { 0.25 } else { 0.15 }), GOOD),
        Kind::Danger => (if hover { BAD.gamma_multiply(0.1) } else { Color32::TRANSPARENT }, BAD),
        Kind::Ai => (AI.gamma_multiply(if hover { 0.25 } else { 0.15 }), AI),
    };
    let fade = if enabled { 1.0 } else { 0.4 };
    ui.painter().rect(rect, CornerRadius::same(4), fill.gamma_multiply(fade), Stroke::new(1.0, stroke.gamma_multiply(fade)), StrokeKind::Inside);
    let mut x = rect.left() + pad;
    if let Some(icon) = icon {
        icons::paint(ui, Rect::from_min_size(pos2(x, rect.center().y - size / 2.0), Vec2::splat(size)), icon, size, text_color.gamma_multiply(fade));
        x += icon_w;
    }
    ui.painter().galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, text_color.gamma_multiply(fade));
    response
}

/// A small (24 px) button.
pub fn small(ui: &mut Ui, kind: Kind, text: &str) -> Response {
    button(ui, kind, None, text, 24.0, true)
}

/// A bare icon that lights up under the pointer.
pub fn icon_button(ui: &mut Ui, icon: Icon, size: f32, tip: &str) -> Response {
    let c = p();
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size + 6.0), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tip));
    let ink = if response.hovered() { c.white(1.0) } else { c.fg2 };
    icons::paint(ui, rect, icon, size, ink);
    if !tip.is_empty() { response.tip(tip) } else { response }
}

/// "✎ 3  + 1  − 2": counts of modified, added and deleted files from `x`, centred on `y`.
/// Returns the width used.
pub fn counts(ui: &Ui, painter: &egui::Painter, x: f32, y: f32, counts: [usize; 3], size: f32) -> f32 {
    let mut cx = x;
    for (i, (n, color)) in [(counts[0], MODIFIED), (counts[1], GOOD), (counts[2], REMOVED)].into_iter().enumerate() {
        if n == 0 {
            continue;
        }
        let font = super::fonts::semibold(size);
        if i == 0 {
            icons::paint(ui, Rect::from_min_size(pos2(cx, y - size / 2.0), Vec2::splat(size)), Icon::Pencil, size - 1.0, color);
            cx += size + 3.0;
        } else {
            let g = painter.layout_no_wrap(if i == 1 { "+".into() } else { "−".into() }, font.clone(), color);
            let w = g.size().x;
            painter.galley(pos2(cx, y - g.size().y / 2.0), g, color);
            cx += w + 3.0;
        }
        let g = painter.layout_no_wrap(n.to_string(), font, color);
        let w = g.size().x;
        painter.galley(pos2(cx, y - g.size().y / 2.0), g, color);
        cx += w + 10.0;
    }
    cx - x
}

/// The ✎ + − mark of a changed file.
pub fn status_mark(ui: &Ui, rect: Rect, status: char) {
    if status == 'M' {
        let r = Rect::from_center_size(rect.center(), Vec2::splat(14.0));
        ui.painter().rect_filled(r, CornerRadius::same(3), MODIFIED.gamma_multiply(0.2));
        icons::paint(ui, r, Icon::Pencil, 10.0, MODIFIED);
        return;
    }
    let (color, ch) = match status {
        'M' => (MODIFIED, "✎"),
        'A' | '?' => (GOOD, "+"),
        'D' => (BAD, "−"),
        'R' => (ACCENT, "→"),
        'U' => (BAD, "!"),
        _ => (Color32::from_gray(0x99), "•"),
    };
    let r = Rect::from_center_size(rect.center(), Vec2::splat(14.0));
    ui.painter().rect_filled(r, CornerRadius::same(3), color.gamma_multiply(0.2));
    ui.painter().text(r.center(), Align2::CENTER_CENTER, ch, FontId::proportional(10.0), color);
}

/// Link-coloured text that does something; underlined under the pointer.
pub fn link(ui: &mut Ui, text: &str, size: f32) -> Response {
    let c = p();
    let response = ui.add(egui::Label::new(egui::RichText::new(text).size(size).color(c.link)).sense(Sense::click()));
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        let r = response.rect;
        ui.painter().line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, c.link));
    }
    response
}

/// Text cut short with … to fit `width`.
pub fn fit(ui: &Ui, text: &str, font: FontId, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width.max(0.0));
    ui.painter().layout_job(job)
}

/// A chevron that turns from right to down when `open`.
pub fn chevron(ui: &Ui, rect: Rect, open: bool, size: f32, color: Color32) {
    icons::paint(ui, rect, if open { Icon::ChevronDown } else { Icon::ChevronRight }, size, color);
}

/// A collapsing header row `height` tall: chevron, title, and room on the right for `extra`.
/// Returns whether it was clicked.
pub fn section_head(ui: &mut Ui, open: bool, height: f32, title: &str, extra: impl FnOnce(&mut Ui)) -> Response {
    let c = p();
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    // An expandable heading, open or closed.
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::DisclosureTriangle, true, title));
    ui.ctx().accesskit_node_builder(response.id, |node| node.set_expanded(open));
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, c.white(0.05));
    }
    chevron(ui, Rect::from_min_size(pos2(rect.left() + 6.0, rect.center().y - 7.5), Vec2::splat(15.0)), open, 15.0, c.white(0.9));
    ui.painter().text(pos2(rect.left() + 26.0, rect.center().y), Align2::LEFT_CENTER, title, FontId::proportional(13.0), c.white(0.9));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(8.0, 5.0))).layout(egui::Layout::right_to_left(egui::Align::Center)));
    extra(&mut child);
    response
}

/// A checkbox with the label after it.
pub fn checkbox(ui: &mut Ui, value: &mut bool, label: &str, size: f32) -> Response {
    let c = p();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let galley = ui.painter().layout_no_wrap(label.to_owned(), FontId::proportional(size), c.white(0.85));
        let (rect, response) = ui.allocate_exact_size(vec2(13.0 + 8.0 + galley.size().x, galley.size().y.max(16.0)), Sense::click());
        let bx = Rect::from_min_size(pos2(rect.left(), rect.center().y - 6.5), Vec2::splat(13.0));
        let accent = Color32::from_rgb(0x80, 0xb1, 0xea);
        if *value {
            ui.painter().rect_filled(bx, CornerRadius::same(2), accent);
            icons::paint(ui, bx, Icon::Check, 11.0, Color32::from_rgb(0x1c, 0x1e, 0x23));
        } else {
            ui.painter().rect(bx, CornerRadius::same(2), c.field, Stroke::new(1.0, c.white(0.4)), StrokeKind::Inside);
        }
        ui.painter().galley(pos2(bx.right() + 8.0, rect.center().y - galley.size().y / 2.0), galley, c.white(0.85));
        let mut response = response;
        if response.clicked() {
            *value = !*value;
            response.mark_changed();
        }
        let checked = *value;
        response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, checked, label));
        response
    })
    .inner
}

/// A single-line field with the app's look: field colour, accent border while focused.
pub fn field(ui: &mut Ui, text: &mut String, hint: &str, width: f32, height: f32, font: f32) -> Response {
    let c = p();
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let id = ui.auto_id_with(("field", hint));
    let focused = ui.memory(|m| m.has_focus(id));
    ui.painter().rect(rect, CornerRadius::same(3), c.field, Stroke::new(1.0, if focused { ACCENT } else { c.white(0.1) }), StrokeKind::Inside);
    let edit = egui::TextEdit::singleline(text)
        .id(id)
        .frame(egui::Frame::NONE)
        .hint_text(egui::RichText::new(hint).color(c.fg3))
        .font(FontId::proportional(font))
        .align(Align2::LEFT_CENTER)
        .desired_width(rect.width() - 10.0);
    ui.put(rect.shrink2(vec2(5.0, 0.0)), edit)
}

/// A thin draggable bar between panels.
pub fn splitter(ui: &mut Ui, rect: Rect, id: egui::Id, vertical: bool) -> Response {
    let c = p();
    let response = ui.interact(rect.expand2(if vertical { vec2(2.0, 0.0) } else { vec2(0.0, 2.0) }), id, Sense::drag());
    let lit = response.hovered() || response.dragged();
    ui.painter().rect_filled(rect, CornerRadius::ZERO, if lit { ACCENT } else { c.line });
    if lit {
        ui.ctx().set_cursor_icon(if vertical { egui::CursorIcon::ResizeHorizontal } else { egui::CursorIcon::ResizeVertical });
    }
    response
}

/// A date in a format of the UI Customization preferences (`MM/DD/YYYY @ h:mm A`).
pub fn format_date(unix: i64, fmt: &str) -> String {
    use chrono::{Datelike, Local, TimeZone, Timelike};
    let Some(d) = Local.timestamp_opt(unix, 0).single() else { return String::new() };
    let h12 = if d.hour() % 12 == 0 { 12 } else { d.hour() % 12 };
    let token = |t: &str| -> String {
        match t {
            "YYYY" => d.year().to_string(),
            "YY" => format!("{:02}", d.year() % 100),
            "MMMM" => d.format("%B").to_string(),
            "MMM" => d.format("%b").to_string(),
            "MM" => format!("{:02}", d.month()),
            "M" => d.month().to_string(),
            "dddd" => d.format("%A").to_string(),
            "ddd" => d.format("%a").to_string(),
            "DD" => format!("{:02}", d.day()),
            "D" => d.day().to_string(),
            "HH" => format!("{:02}", d.hour()),
            "H" => d.hour().to_string(),
            "hh" => format!("{h12:02}"),
            "h" => h12.to_string(),
            "mm" => format!("{:02}", d.minute()),
            "ss" => format!("{:02}", d.second()),
            "A" => if d.hour() < 12 { "AM" } else { "PM" }.into(),
            "a" => if d.hour() < 12 { "am" } else { "pm" }.into(),
            other => other.into(),
        }
    };
    const TOKENS: [&str; 18] = ["YYYY", "YY", "MMMM", "MMM", "MM", "M", "dddd", "ddd", "DD", "D", "HH", "H", "hh", "h", "mm", "ss", "A", "a"];
    let mut out = String::new();
    let mut rest = fmt;
    while !rest.is_empty() {
        if let Some(stripped) = rest.strip_prefix('[')
            && let Some(end) = stripped.find(']')
        {
            out.push_str(&stripped[..end]);
            rest = &stripped[end + 1..];
            continue;
        }
        match TOKENS.iter().find(|t| rest.starts_with(**t)) {
            Some(t) => {
                out.push_str(&token(t));
                rest = &rest[t.len()..];
            }
            None => {
                let ch = rest.chars().next().unwrap_or(' ');
                out.push(ch);
                rest = &rest[ch.len_utf8()..];
            }
        }
    }
    out
}

/// The timeline labels ("yesterday", "3 weeks ago"): which row (newest first) gets which.
pub fn timeline(times: &[i64], now: i64) -> std::collections::HashMap<usize, String> {
    use chrono::{Datelike, Local, TimeZone};
    let mut marks: Vec<(i64, String)> = Vec::new();
    let plural = |n: i64, one: &str, many: &str| if n == 1 { one.to_owned() } else { many.replace("{0}", &n.to_string()) };
    for n in 1..=23 {
        marks.push((now - 3600 * n, plural(n, "1 hour ago", "{0} hours ago")));
    }
    for n in 1..=6 {
        marks.push((now - 86400 * n, plural(n, "yesterday", "{0} days ago")));
    }
    for n in 1..=4 {
        marks.push((now - 604800 * n, plural(n, "a week ago", "{0} weeks ago")));
    }
    if let Some(today) = Local.timestamp_opt(now, 0).single() {
        let at = |year: i32, month0: i32| -> Option<i64> {
            let (y, m) = (year + month0.div_euclid(12), month0.rem_euclid(12) as u32 + 1);
            let last = (28..=31).rev().find(|d| chrono::NaiveDate::from_ymd_opt(y, m, *d).is_some())?;
            let date = chrono::NaiveDate::from_ymd_opt(y, m, today.day().min(last))?;
            Local.from_local_datetime(&date.and_hms_opt(0, 0, 0)?).earliest().map(|d| d.timestamp())
        };
        for n in 1..=11 {
            if let Some(t) = at(today.year(), today.month0() as i32 - n) {
                marks.push((t, plural(n as i64, "a month ago", "{0} months ago")));
            }
        }
        for n in 1..=6 {
            if let Some(t) = at(today.year() - n, today.month0() as i32) {
                let text = if n == 1 { "a year ago".into() } else if n == 6 { format!("{n}+ years ago") } else { format!("{n} years ago") };
                marks.push((t, text));
            }
        }
    }
    let mut out = std::collections::HashMap::new();
    if times.is_empty() {
        return out;
    }
    for (date, text) in marks {
        if let Some(i) = times.iter().position(|t| *t < date) {
            out.insert(i, text);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_follow_the_format_tokens_and_literals() {
        use chrono::{Local, TimeZone};
        let t = Local.with_ymd_and_hms(2024, 3, 7, 15, 4, 5).unwrap().timestamp();
        assert_eq!(format_date(t, "MM/DD/YYYY @ h:mm A"), "03/07/2024 @ 3:04 PM");
        assert_eq!(format_date(t, "MMMM D, YYYY [at] HH:mm"), "March 7, 2024 at 15:04");
    }

    #[test]
    fn initials_and_mixing() {
        assert_eq!(initials("Ada Byron Lovelace"), "AL");
        assert_eq!(initials("sam"), "SA");
        assert_eq!(initials("  "), "?");
        assert_eq!(mix_over(Color32::WHITE, 0.5, Color32::BLACK), Color32::from_rgb(128, 128, 128));
    }

    #[test]
    fn timeline_marks_the_first_older_row() {
        let now = 1_700_000_000;
        let rows = [now - 10, now - 2 * 86400 - 5, now - 3 * 86400];
        let marks = timeline(&rows, now);
        assert_eq!(marks.get(&1).map(String::as_str), Some("2 days ago"));
        assert!(!marks.contains_key(&0));
    }
}

/// The tooltip's look: white text on a black, rounded box, the same in every theme.
pub fn tooltip_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgb(0, 0, 0))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 6))
        .shadow(egui::Shadow { offset: [0, 2], blur: 8, spread: 0, color: Color32::from_black_alpha(90) })
}

/// Tooltips in GitCraft's style: `.tip(text)` in place of egui's `.on_hover_text(text)`.
pub trait Tip {
    fn tip(self, text: impl Into<egui::WidgetText>) -> Self;
}

impl Tip for Response {
    fn tip(self, text: impl Into<egui::WidgetText>) -> Response {
        let text: egui::WidgetText = text.into();
        describe(&self, text.text());
        let mut tooltip = egui::Tooltip::for_enabled(&self);
        tooltip.popup = tooltip.popup.frame(tooltip_frame());
        tooltip.show(|ui| {
            ui.set_max_width(ui.spacing().tooltip_width);
            ui.style_mut().visuals.override_text_color = Some(Color32::WHITE);
            ui.add(egui::Label::new(text));
        });
        self
    }
}

/// The tooltip text as the widget's accessible description, and its name when it has none (an
/// icon-only button), as egui does for its own tooltips.
fn describe(response: &Response, text: &str) {
    if response.ctx.has_accesskit_node(response.id) {
        response.ctx.accesskit_node_builder(response.id, |node| {
            if node.description().is_none() {
                node.set_description(text);
            }
            if node.role() != egui::accesskit::Role::Label && node.labelled_by().is_empty() && node.label().is_none_or(str::is_empty) {
                node.set_label(text);
            }
        });
    }
}
