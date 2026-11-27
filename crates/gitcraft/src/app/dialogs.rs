//! The dialogs over the window: one at a time, centred near the top (a 40 px
//! header with the title and a close button, the body, buttons at the bottom right).
//! Most are forms; what a dialog does when confirmed is a closure run with the app and the
//! values entered. Folder fields browse with the system's folder picker (craft-dialogs).

use super::widgets::Tip;
use super::icons::{self, Icon};
use super::theme::{ACCENT, p};
use super::widgets::{self, Kind, REMOVED};
use super::App;
use egui::{Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use gitcraft_core::actions::Action;
use std::collections::HashMap;

/// What a dialog does when confirmed.
pub type OnSubmit = Box<dyn FnOnce(&mut App, &Values)>;
/// An action that can run more than once (a choice tried again after a failure).
pub type Retry = std::rc::Rc<dyn Fn(&mut App, &Values)>;

/// The values of a dialog's fields, by key.
#[derive(Default, Clone)]
pub struct Values(pub HashMap<String, Value>);

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Text(String),
    Flag(bool),
}

impl Values {
    pub fn text(&self, key: &str) -> String {
        match self.0.get(key) {
            Some(Value::Text(t)) => t.clone(),
            _ => String::new(),
        }
    }

    pub fn flag(&self, key: &str) -> bool {
        matches!(self.0.get(key), Some(Value::Flag(true)))
    }
}

#[derive(Clone)]
pub enum FieldKind {
    Text,
    Password,
    Multiline(usize),
    Check,
    /// (value, label) pairs.
    Select(Vec<(String, String)>),
    /// A folder path with a Browse button.
    Folder,
    /// Text shown as is.
    Note,
}

#[derive(Clone)]
pub struct Field {
    pub key: String,
    pub label: String,
    pub kind: FieldKind,
    pub placeholder: String,
    pub help: String,
    /// Shown only while this check field is on.
    pub when: Option<String>,
}

impl Field {
    pub fn new(key: &str, label: &str, kind: FieldKind) -> Field {
        Field { key: key.into(), label: label.into(), kind, placeholder: String::new(), help: String::new(), when: None }
    }

    pub fn placeholder(mut self, text: &str) -> Field {
        self.placeholder = text.into();
        self
    }

    pub fn help(mut self, text: &str) -> Field {
        self.help = text.into();
        self
    }

    pub fn when(mut self, key: &str) -> Field {
        self.when = Some(key.into());
        self
    }
}

/// A button in the dialog's footer.
pub struct Choice {
    pub label: String,
    pub kind: Kind,
    pub then: OnSubmit,
    /// For a dialog that stays open: the action again, so the button works after a failed try.
    pub again: Option<Retry>,
}

/// Tells dialogs apart, so work a dialog started only changes that same dialog.
static NEXT_TOKEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub enum Body {
    /// Text and buttons.
    Text(String),
    /// Fields in a column (labels above) or a grid (labels on the left, as in Clone and Init).
    Form { intro: String, fields: Vec<Field>, grid: bool },
    /// Read-only text the person can select and copy.
    Info(String),
    /// Lines of text in a scrolling list (activity log).
    List(Vec<(String, String)>),
    /// Rows with a button each, and a filter over them (reflog).
    Pick { hint: String, filter: String, rows: Vec<(String, String)>, button: String, on_pick: std::rc::Rc<dyn Fn(&mut App, usize)> },
    /// Monospace text to read and copy (range diff).
    Mono(String),
    /// Drawn by its owner with the whole app; returns true to close the dialog.
    Custom(Box<dyn FnMut(&mut App, &mut Ui) -> bool>),
}

pub struct Dialog {
    /// This dialog among all dialogs shown.
    pub token: u64,
    pub title: String,
    pub width: f32,
    resizable_height: Option<f32>,
    pub body: Body,
    pub values: Values,
    /// A list down the left side (Clone, Init): (id, label, enabled), and the picked id.
    pub sidebar: Option<(Vec<(String, String, bool)>, String)>,
    pub choices: Vec<Choice>,
    pub cancel: Option<String>,
    pub error: Option<String>,
    /// Running: the buttons are off and the confirm button says this.
    pub busy: Option<String>,
    /// Checked each frame: a reason the confirm button is off.
    pub check: Option<Box<dyn Fn(&Values) -> Option<String>>>,
    /// Keep the dialog open after a choice (it sets `busy` and closes itself).
    pub stay_open: bool,
    first_frame: bool,
    picked: Option<(std::rc::Rc<dyn Fn(&mut App, usize)>, usize)>,
}

impl Dialog {
    pub fn new(title: &str, body: Body) -> Dialog {
        Dialog {
            token: NEXT_TOKEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            title: title.into(),
            width: 440.0,
            resizable_height: None,
            body,
            values: Values::default(),
            sidebar: None,
            choices: Vec::new(),
            cancel: Some("Cancel".into()),
            error: None,
            busy: None,
            check: None,
            stay_open: false,
            first_frame: true,
            picked: None,
        }
    }

    pub fn width(mut self, width: f32) -> Dialog {
        self.width = width;
        self
    }

    /// Give a body that manages its own scrolling a resizable viewport.
    pub fn resizable_height(mut self, height: f32) -> Dialog {
        self.resizable_height = Some(height);
        self
    }

    pub fn value(mut self, key: &str, value: Value) -> Dialog {
        self.values.0.insert(key.into(), value);
        self
    }

    pub fn text_value(self, key: &str, value: &str) -> Dialog {
        self.value(key, Value::Text(value.into()))
    }

    pub fn flag_value(self, key: &str, value: bool) -> Dialog {
        self.value(key, Value::Flag(value))
    }

    pub fn choice(mut self, label: &str, kind: Kind, then: impl FnOnce(&mut App, &Values) + 'static) -> Dialog {
        self.choices.push(Choice { label: label.into(), kind, then: Box::new(then), again: None });
        self
    }

    /// A choice for a dialog that stays open while the work runs and can be tried again.
    pub fn retry_choice(mut self, label: &str, kind: Kind, then: impl Fn(&mut App, &Values) + 'static) -> Dialog {
        let again: Retry = std::rc::Rc::new(then);
        let run = again.clone();
        self.choices.push(Choice { label: label.into(), kind, then: Box::new(move |app, v| run(app, v)), again: Some(again) });
        self
    }

    pub fn checked(mut self, check: impl Fn(&Values) -> Option<String> + 'static) -> Dialog {
        self.check = Some(Box::new(check));
        self
    }

    pub fn no_cancel(mut self) -> Dialog {
        self.cancel = None;
        self
    }

    pub fn cancel_label(mut self, label: &str) -> Dialog {
        self.cancel = Some(label.into());
        self
    }

    /// Yes / no: `confirm` runs `then`.
    pub fn confirm_with(title: &str, text: &str, confirm: &str, danger: bool, then: impl FnOnce(&mut App, &Values) + 'static) -> Dialog {
        Dialog::new(title, Body::Text(text.into())).choice(confirm, if danger { Kind::Danger } else { Kind::Primary }, then)
    }

    pub fn confirm(title: &str, text: &str, confirm: &str, danger: bool, then: Then) -> Dialog {
        Dialog::confirm_with(title, text, confirm, danger, move |app, _| app.then(then))
    }

    pub fn choice_list(title: &str, text: &str, choices: Vec<(String, Kind, Then)>) -> Dialog {
        let mut d = Dialog::new(title, Body::Text(text.into()));
        for (label, kind, then) in choices {
            d = d.choice(&label, kind, move |app, _| app.then(then));
        }
        d
    }

    pub fn info(title: &str, text: &str) -> Dialog {
        let mut d = Dialog::new(title, Body::Info(text.into())).width(560.0).no_cancel();
        d.choices.push(Choice { label: "Close".into(), kind: Kind::Primary, then: Box::new(|_, _| {}), again: None });
        d
    }

    /// One text field (and maybe a second) and a confirm button.
    pub fn prompt(title: &str, label: &str, value: &str, confirm: &str, then: impl FnOnce(&mut App, &Values) + 'static) -> Dialog {
        Dialog::new(title, Body::Form { intro: String::new(), fields: vec![Field::new("value", label, FieldKind::Text)], grid: false })
            .text_value("value", value)
            .choice(confirm, Kind::Primary, then)
            .checked(|v| v.text("value").trim().is_empty().then(|| String::new()))
    }

    pub fn form(title: &str, fields: Vec<Field>, confirm: &str, then: impl FnOnce(&mut App, &Values) + 'static) -> Dialog {
        Dialog::new(title, Body::Form { intro: String::new(), fields, grid: false }).choice(confirm, Kind::Primary, then)
    }
}

/// Continuations that come from places without access to closures over app state.
pub enum Then {
    Act(u64, Action),
    AddRemote(u64),
    OpenTerminal(u64),
}

/// What a system file dialog was opened for.
pub enum Pick {
    /// Fill this field of the open dialog.
    /// A dialog field (the dialog, by its token, and the field).
    Field(u64, String),
    /// Open the chosen folder as a repository.
    OpenRepo,
    /// Open the chosen repository in a window of its own.
    NewWindow,
    /// Locate a moved repository for this tab.
    Locate(u64),
    /// Apply a patch file to the tab's repository.
    ApplyPatch(u64),
    /// Save the given commits as a patch file.
    SavePatch(u64, Vec<String>),
    /// A preferences field.
    Pref(super::prefs_ui::Target),
}

impl App {
    /// Run a continuation.
    pub(crate) fn then(&mut self, then: Then) {
        match then {
            Then::Act(tab, action) => self.act(tab, action),
            Then::AddRemote(tab) => self.add_remote(tab),
            Then::OpenTerminal(tab) => self.open_terminal(tab),
        }
    }

    pub(crate) fn show_dialog(&mut self, dialog: Dialog) {
        self.dialog = Some(dialog);
    }

    /// Answers from the system file dialogs.
    pub(super) fn take_picks(&mut self) {
        for (then, answer) in self.pickers.take_answers() {
            match answer {
                craft_dialogs::Answer::Paths(mut paths) if !paths.is_empty() => {
                    let path = paths.swap_remove(0).to_string_lossy().into_owned();
                    self.picked(then, path);
                }
                craft_dialogs::Answer::Unavailable(why) => self.toast(false, format!("The file dialog couldn't be shown. {why}")),
                _ => {}
            }
        }
    }

    fn picked(&mut self, then: Pick, path: String) {
        match then {
            // Only into the dialog that asked; one opened since gets nothing.
            Pick::Field(token, key) => {
                if let Some(d) = self.dialog.as_mut().filter(|d| d.token == token) {
                    d.values.0.insert(key, Value::Text(path));
                }
            }
            Pick::OpenRepo => self.open_repo_checked(&path),
            Pick::NewWindow => self.repository_window(&path),
            // The tab stays, with its retry controls, until the folder turns out to be a repository.
            Pick::Locate(tab) => self.spawn(move || match gitcraft_core::git::repo_root(std::path::Path::new(&path)) {
                Some(root) => super::Msg::Located { tab, path: root.to_string_lossy().into_owned() },
                None => super::Msg::Toast(false, format!("{path} is not a Git repository")),
            }),
            Pick::ApplyPatch(tab) => self.act(tab, Action::ApplyPatchFile { file: path }),
            Pick::SavePatch(tab, shas) => self.save_patch_to(tab, shas, path),
            Pick::Pref(target) => {
                if let Some(p) = self.prefs.as_mut() {
                    p.buffers.clear();
                }
                self.set_pref(&target, serde_json::Value::String(path));
            }
        }
    }

    pub(crate) fn pick_folder(&mut self, title: &str, then: Pick) {
        self.pickers.ask(craft_dialogs::FileDialog::folder().title(title), then);
    }

    /// Open a folder as a repository after checking it is one.
    pub(crate) fn open_repo_checked(&mut self, path: &str) {
        let path = path.to_owned();
        self.spawn(move || match gitcraft_core::git::repo_root(std::path::Path::new(&path)) {
            Some(root) => super::Msg::OpenRepo(root.to_string_lossy().into_owned()),
            None => super::Msg::Toast(false, format!("{path} is not a Git repository")),
        });
    }

    /// File > Open Repo: straight to the system folder picker.
    pub(crate) fn open_repo_dialog(&mut self) {
        self.pick_folder("Open a Git repository", Pick::OpenRepo);
    }

    /// File > Clone Repo.
    pub(crate) fn clone_dialog(&mut self, url: &str) {
        let folder = self.settings.string("defaultCloneFolder", "");
        let fields = vec![
            Field::new("dir", "Where to clone to", FieldKind::Folder).placeholder(if cfg!(windows) { "C:\\Users\\you\\Projects" } else { "~/Projects" }),
            Field::new("url", "URL", FieldKind::Text).placeholder("https://github.com/owner/project.git"),
            Field::new("folder", "Folder name", FieldKind::Text).placeholder("Suggested from the repository URL"),
            Field::new("submodules", "Initialize submodules", FieldKind::Check),
            Field::new("shallow", "Shallow Clone", FieldKind::Check),
            Field::new("limit", "Limit history by", FieldKind::Select(vec![("depth".into(), "Commit depth".into()), ("since".into(), "Since date".into())])).when("shallow"),
            Field::new("depth", "Depth", FieldKind::Text).help("How many commits to clone from the target branch.").when("shallow"),
            Field::new("since", "Since date", FieldKind::Text).placeholder("YYYY-MM-DD").help("Commits older than this date will not be cloned.").when("shallow"),
            Field::new("branch", "Branch to clone", FieldKind::Text).placeholder("default branch").help("The default branch is usually main or master, but you can specify a different one.").when("shallow"),
            Field::new("flags", "Custom flags", FieldKind::Text).help("Any additional flags for git clone, entered exactly as on the command line.").when("shallow"),
            Field::new("sparse", "Sparse Checkout", FieldKind::Check),
            Field::new("blobless", "Blobless clone", FieldKind::Check).help("Fetches trees and commits without file contents at first, reducing clone size. Contents are fetched when needed.").when("sparse"),
            Field::new("paths", "Paths to include", FieldKind::Multiline(5)).placeholder("e.g.\nsrc/\ndocs/\nREADME.md").help("One path per line. Only these paths are checked out; with none, only the files in the top folder.").when("sparse"),
        ];
        let mut sources: Vec<(String, String, bool)> = vec![("url".into(), "Clone with URL".into(), true)];
        sources.extend(SERVICE_SOURCES.iter().map(|(id, label)| (id.to_string(), label.to_string(), true)));
        let mut d = Dialog::new("Clone a Repository", Body::Form { intro: "Clone a Repo".into(), fields, grid: true })
            .width(780.0)
            .text_value("dir", &folder)
            .text_value("url", url)
            .flag_value("submodules", true)
            .text_value("limit", "depth")
            .text_value("depth", "1")
            .checked(|v| {
                if v.text("url").trim().is_empty() || v.text("dir").trim().is_empty() {
                    return Some(String::new());
                }
                if v.flag("shallow") && v.text("limit") == "since" {
                    let since = v.text("since");
                    let valid = chrono::NaiveDate::parse_from_str(since.trim(), "%Y-%m-%d").ok();
                    return match valid {
                        None => Some("Enter a valid date in 'YYYY-MM-DD' format".into()),
                        Some(d) if d > chrono::Local::now().date_naive() => Some("Date cannot be in the future".into()),
                        _ => None,
                    };
                }
                None
            })
            .retry_choice("Clone the repo!", Kind::Primary, |app, v| app.start_clone(v));
        d.sidebar = Some((sources, "url".into()));
        d.stay_open = true;
        self.dialog = Some(d);
    }

    fn start_clone(&mut self, v: &Values) {
        let shallow = v.flag("shallow");
        let since_mode = v.text("limit") == "since";
        let sparse = v.flag("sparse");
        let options = gitcraft_core::session::CloneOptions {
            depth: (shallow && !since_mode).then(|| v.text("depth").trim().parse::<u32>().unwrap_or(1).max(1)),
            since: (shallow && since_mode).then(|| v.text("since").trim().to_owned()),
            branch: shallow.then(|| v.text("branch").trim().to_owned()).filter(|b| !b.is_empty()),
            custom_flags: shallow.then(|| v.text("flags").trim().to_owned()).filter(|b| !b.is_empty()),
            sparse: Some(sparse),
            sparse_paths: sparse.then(|| v.text("paths").lines().map(|l| l.trim().to_owned()).filter(|l| !l.is_empty()).collect()),
            blobless: Some(sparse && v.flag("blobless")),
            folder_name: Some(v.text("folder").trim().to_owned()).filter(|f| !f.is_empty()),
            initialize_submodules: Some(v.flag("submodules")),
        };
        let (url, dir) = (v.text("url"), v.text("dir"));
        let mut token = 0;
        if let Some(d) = self.dialog.as_mut() {
            d.busy = Some("Cloning…".into());
            d.error = None;
            token = d.token;
        }
        self.busy = Some("Cloning…".into());
        let engine = self.engine.clone();
        self.spawn(move || super::Msg::Cloned { token, dir: dir.clone(), result: gitcraft_core::session::clone_repo(&engine, &url, &dir, Some(options)) });
    }

    /// A clone finished. Only the Clone dialog that started it (`token`) closes or shows the
    /// error; when it is gone, the new repository opens anyway and an error is a notice.
    pub(crate) fn cloned(&mut self, token: u64, dir: String, result: Result<String, String>) {
        self.busy = None;
        self.job = None;
        let ours = self.dialog.as_ref().is_some_and(|d| d.token == token);
        match result {
            Ok(path) => {
                self.settings.set("defaultCloneFolder", dir);
                if ours {
                    self.dialog = None;
                }
                self.open_repo(&path);
                self.toast(true, "Clone finished");
            }
            Err(e) => {
                if let Some(d) = self.dialog.as_mut().filter(|_| ours) {
                    d.busy = None;
                    let note = match super::repo::ssh_host_key_problem(&e) {
                        Some(true) => "SSH host key changed. Verify the new fingerprint with the server administrator before updating your SSH known_hosts file.\n",
                        Some(false) => "SSH could not verify the host. Retry in a terminal and accept its key only after confirming the fingerprint with the server administrator.\n",
                        None => "",
                    };
                    d.error = Some(format!("{note}{e}"));
                } else {
                    self.toast(false, e);
                }
            }
        }
    }

    /// File > Init Repo.
    pub(crate) fn init_dialog(&mut self) {
        let folder = self.settings.string("defaultCloneFolder", "");
        let branch = self.settings.string("defaultBranch", "main");
        let mut templates: Vec<(String, String)> = vec![(String::new(), "Select...".into())];
        templates.extend(gitcraft_core::manage::GITIGNORE_TEMPLATES.iter().map(|(n, _)| (n.to_string(), n.to_string())));
        let fields = vec![
            Field::new("name", "Name", FieldKind::Text).placeholder("my-project"),
            Field::new("dir", "Initialize in", FieldKind::Folder),
            Field::new("full", "Full path", FieldKind::Note),
            Field::new("branch", "Default branch name", FieldKind::Text).placeholder("main"),
            Field::new("ignore", ".gitignore Template (optional)", FieldKind::Select(templates)),
            Field::new("license", "License (optional)", FieldKind::Select(vec![(String::new(), "Select...".into()), ("MIT".into(), "MIT".into())])),
            Field::new("lfs", "Initialize with LFS", FieldKind::Check),
            Field::new("readme", "Add a README", FieldKind::Check),
        ];
        let mut sources: Vec<(String, String, bool)> = vec![("local".into(), "Local Only".into(), true)];
        sources.extend(SERVICE_SOURCES.iter().map(|(id, label)| (id.to_string(), label.to_string(), false)));
        let mut d = Dialog::new("Initialize a Repository", Body::Form { intro: "Initialize a Repo".into(), fields, grid: true })
            .width(780.0)
            .text_value("dir", &folder)
            .text_value("branch", &branch)
            .checked(|v| (v.text("name").trim().is_empty() || v.text("dir").trim().is_empty()).then(String::new))
            .retry_choice("Create Repository", Kind::Primary, move |app, v| {
                let options = gitcraft_core::manage::InitOptions {
                    branch: Some(v.text("branch")).filter(|b| !b.trim().is_empty()).unwrap_or_else(|| branch.clone()),
                    gitignore: Some(v.text("ignore")).filter(|i| !i.is_empty()).into_iter().collect(),
                    license: v.text("license"),
                    readme: v.flag("readme"),
                    lfs: v.flag("lfs"),
                };
                let engine = app.engine.clone();
                let (dir, name) = (v.text("dir"), v.text("name"));
                let mut token = 0;
                if let Some(d) = app.dialog.as_mut() {
                    d.busy = Some("Creating…".into());
                    d.error = None;
                    token = d.token;
                }
                app.spawn(move || super::Msg::Initialized(token, gitcraft_core::session::init_repo(&engine, &dir, &name, Some(options))));
            });
        d.sidebar = Some((sources, "local".into()));
        d.stay_open = true;
        self.dialog = Some(d);
    }

    /// A repository was created. As with a clone, only the dialog that asked (`token`) changes.
    pub(crate) fn initialized(&mut self, token: u64, result: Result<gitcraft_core::manage::InitOutcome, String>) {
        let ours = self.dialog.as_ref().is_some_and(|d| d.token == token);
        match result {
            Ok(outcome) => {
                if ours {
                    self.dialog = None;
                }
                self.open_repo(&outcome.path);
                let ok = outcome.warning.is_none();
                self.toast(ok, outcome.warning.unwrap_or_else(|| "Repository created".into()));
            }
            Err(e) => match self.dialog.as_mut().filter(|_| ours) {
                Some(d) => {
                    d.busy = None;
                    d.error = Some(e);
                }
                None => self.toast(false, e),
            },
        }
    }

    /// Draw the open dialog, if any, over everything.
    pub(super) fn draw_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.dialog.take() else { return };
        let c = p();
        let screen = ctx.content_rect();
        let mut close = false;
        let mut chosen: Option<usize> = None;
        let mut browse: Option<(String, String)> = None;
        // The dimmed window behind; a click on it closes the dialog.
        egui::Area::new(egui::Id::NULL.with("dialog_shade")).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, CornerRadius::ZERO, Color32::from_black_alpha(128));
            if response.clicked() && dialog.busy.is_none() {
                close = true;
            }
        });
        let width = dialog.width.min(screen.width() - 32.0);
        let left = screen.center().x - width / 2.0;
        let resizable_height = dialog.resizable_height;
        let stacked = dialog.sidebar.is_some() && width < 510.0;
        let top = screen.top() + if resizable_height.is_some() || stacked {
            (screen.height() * 0.04).clamp(8.0, 40.0)
        } else {
            (screen.height() * 0.18).min((screen.height() - 180.0).max(8.0))
        };
        let has_footer = dialog.cancel.is_some() || !dialog.choices.is_empty();
        let available_height = screen.bottom() - top - 8.0;
        let max_height = if stacked { available_height } else { (screen.height() * 0.75).max(180.0).min(available_height) };
        let token = dialog.token;
        let blocked = dialog.check.as_ref().and_then(|f| f(&dialog.values));
        egui::Area::new(egui::Id::NULL.with("dialog")).order(egui::Order::Tooltip)
            .fixed_pos(pos2(left, top)).constrain(resizable_height.is_none()).show(ctx, |ui| {
            let frame = egui::Frame::new().fill(c.panel).stroke(Stroke::new(1.0, c.white(0.12))).corner_radius(CornerRadius::same(6)).shadow(egui::Shadow { offset: [0, 8], blur: 32, spread: 0, color: Color32::from_black_alpha(110) });
            frame.show(ui, |ui| {
                ui.set_width(width);
                if resizable_height.is_none() {
                    ui.set_max_height(max_height);
                }
                ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
                let footer_height = if has_footer {
                    let labels = dialog.choices.iter().enumerate().rev().map(|(i, choice)| {
                        if dialog.busy.is_some() && i + 1 == dialog.choices.len() { dialog.busy.as_deref().unwrap_or_default() } else { choice.label.as_str() }
                    }).chain(dialog.cancel.as_deref());
                    footer_needed_height(ui, labels, width)
                } else {
                    0.0
                };
                // Header.
                let (head, _) = ui.allocate_exact_size(vec2(width, 40.0), Sense::hover());
                ui.painter().line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0, c.line));
                let title = widgets::fit(ui, &dialog.title, FontId::proportional(14.0), c.white(1.0), width - 56.0);
                ui.painter().galley(pos2(head.left() + 16.0, head.center().y - title.size().y / 2.0), title, c.white(1.0));
                let x = Rect::from_center_size(pos2(head.right() - 24.0, head.center().y), Vec2::splat(22.0));
                let xr = ui.interact(x, ui.scope_id().with("close"), Sense::click());
                icons::paint(ui, x, Icon::X, 16.0, if xr.hovered() { c.white(1.0) } else { c.fg2 });
                if xr.clicked() && dialog.busy.is_none() {
                    close = true;
                }
                let body_height = (max_height - 40.0 - footer_height - 32.0).max(0.0);
                let sidebar_width = if stacked { width } else { 195.0 };
                let sidebar_height = if stacked { (body_height / 3.0).min(100.0) } else { body_height.min(320.0) };
                let body_width = if dialog.sidebar.is_some() && !stacked {
                    width - sidebar_width - ui.spacing().item_spacing.x
                } else {
                    width
                };
                let body_height = if stacked { body_height - sidebar_height } else { body_height };
                let layout = if stacked { egui::Layout::top_down(egui::Align::Min) } else { egui::Layout::left_to_right(egui::Align::Min) };
                ui.with_layout(layout, |ui| {
                    if let Some((items, active)) = dialog.sidebar.as_mut() {
                        // Its own scroll area, so every source stays reachable when the dialog is short.
                        ui.vertical(|ui| {
                            ui.set_width(sidebar_width);
                            egui::ScrollArea::vertical().id_salt(("sources", token)).min_scrolled_height(0.0).max_height(sidebar_height).auto_shrink([false, false]).show(ui, |ui| {
                                let (side, _) = ui.allocate_exact_size(vec2(ui.available_width(), (items.len() as f32 * 31.0 + 2.0).max(sidebar_height)), Sense::hover());
                                ui.painter().rect_filled(side, CornerRadius::ZERO, Color32::from_black_alpha(50));
                                ui.painter().line_segment([side.right_top(), side.right_bottom()], Stroke::new(1.0, c.line));
                                for (i, (id, label, enabled)) in items.iter().enumerate() {
                                    let row = Rect::from_min_size(pos2(side.left(), side.top() + 1.0 + i as f32 * 31.0), vec2(side.width() - 1.0, 31.0));
                                    let r = ui.interact(row, ui.scope_id().with(("source", i)), if *enabled { Sense::click() } else { Sense::hover() });
                                    let on = active == id;
                                    if on {
                                        ui.painter().rect_filled(row, CornerRadius::ZERO, c.selrow);
                                    } else if r.hovered() && *enabled {
                                        ui.painter().rect_filled(row, CornerRadius::ZERO, c.hoverrow);
                                    }
                                    let fade = if *enabled { 1.0 } else { 0.4 };
                                    let icon = match id.as_str() {
                                        "url" => Icon::Globe,
                                        "local" => Icon::Laptop,
                                        s if s.starts_with("github") => Icon::Github,
                                        s if s.starts_with("gitlab") => Icon::Gitlab,
                                        _ => Icon::Cloud,
                                    };
                                    icons::paint(ui, Rect::from_min_size(pos2(row.left() + 12.0, row.center().y - 7.0), Vec2::splat(14.0)), icon, 14.0, c.white(0.8).gamma_multiply(fade));
                                    ui.painter().text(pos2(row.left() + 36.0, row.center().y), Align2::LEFT_CENTER, label, FontId::proportional(13.0), if on { c.white(1.0) } else { c.fg.gamma_multiply(fade) });
                                    if !*enabled {
                                        r.tip("Creating a repository on a service isn't available yet. Initialize it here, then add the remote.");
                                    } else if r.clicked() {
                                        *active = id.clone();
                                    }
                                }
                            });
                        });
                    }
                    ui.vertical(|ui| {
                        ui.set_width(body_width);
                        let margin = if resizable_height.is_some() { 12 } else { 16 };
                        egui::Frame::new().inner_margin(egui::Margin::same(margin)).show(ui, |ui| {
                            let draw_body = |ui: &mut Ui| {
                                ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
                                let source = dialog.sidebar.as_ref().map(|(_, a)| a.clone()).unwrap_or_default();
                                if source != "url" && source != "local" && !source.is_empty() {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(RichText::new("Browsing this service's repositories isn't available yet. Paste the repository URL below.").size(12.0).color(c.fg3));
                                    });
                                    ui.add_space(8.0);
                                }
                                if let Body::Custom(draw) = &mut dialog.body {
                                    if draw(self, ui) {
                                        close = true;
                                    }
                                } else {
                                    body(ui, &mut dialog, &mut browse, &mut chosen);
                                }
                                if let Some(e) = &dialog.error {
                                    ui.add_space(8.0);
                                    ui.add(egui::Label::new(RichText::new(e).size(12.0).color(REMOVED)).wrap());
                                }
                                if let Some(reason) = blocked.as_ref().filter(|r| !r.is_empty()) {
                                    ui.add_space(6.0);
                                    ui.label(RichText::new(reason).size(12.0).color(REMOVED));
                                }
                            };
                            if let Some(height) = resizable_height {
                                let max_height = (screen.bottom() - top - 40.0 - 2.0 * f32::from(margin)
                                    - footer_height - 12.0).max(1.0);
                                let width = ui.available_width();
                                egui::Resize::default().id_salt(("dialog_body", token))
                                    .resizable([false, true]).with_stroke(false)
                                    .default_size(vec2(width, height))
                                    .min_size(vec2(width, 280.0_f32.min(max_height)))
                                    .max_size(vec2(width, max_height))
                                    .show(ui, |ui| ui.push_id(token, draw_body).inner);
                            } else {
                                egui::ScrollArea::vertical().id_salt(("body", token)).min_scrolled_height(0.0).max_height(body_height).auto_shrink([false, true]).show(ui, draw_body);
                            }
                        });
                    });
                });
                // Footer.
                if has_footer {
                    ui.add_space(0.0);
                    let (foot, _) = ui.allocate_exact_size(vec2(width, footer_height), Sense::hover());
                    let mut row = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(pos2(foot.left() + 16.0, foot.top() + 8.0), pos2(foot.right() - 16.0, foot.bottom() - 8.0))).layout(egui::Layout::right_to_left(egui::Align::Min).with_main_wrap(true)));
                    row.spacing_mut().item_spacing = vec2(8.0, 8.0);
                    let running = dialog.busy.is_some();
                    for i in (0..dialog.choices.len()).rev() {
                        let ch = &dialog.choices[i];
                        let full = if running && i == dialog.choices.len() - 1 { dialog.busy.clone().unwrap_or_default() } else { ch.label.clone() };
                        let label = footer_label(&row, &full, width - 32.0);
                        let enabled = !running && (blocked.is_none() || ch.kind != Kind::Primary);
                        let response = widgets::button(&mut row, ch.kind, None, &label, 28.0, enabled);
                        let response = if label == full { response } else { response.on_hover_text(&full) };
                        if response.clicked() {
                            chosen = Some(i);
                        }
                    }
                    if let Some(cancel) = &dialog.cancel {
                        let label = footer_label(&row, cancel, width - 32.0);
                        let response = widgets::button(&mut row, Kind::Default, None, &label, 28.0, !running);
                        let response = if label == *cancel { response } else { response.on_hover_text(cancel) };
                        if response.clicked() {
                            close = true;
                        }
                    }
                }
            });
        });
        if dialog.busy.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if dialog.first_frame {
            dialog.first_frame = false;
        }
        if let Some((pick, i)) = dialog.picked.take() {
            pick(self, i);
            return;
        }
        if let Some((key, title)) = browse {
            self.dialog = Some(dialog);
            let token = self.dialog.as_ref().map_or(0, |d| d.token);
            self.pick_folder(&title, Pick::Field(token, key));
            return;
        }
        if let Some(i) = chosen {
            let stay = dialog.stay_open;
            let choice = dialog.choices.remove(i);
            let values = dialog.values.clone();
            if stay {
                // Put the choice back: off while the work runs (`busy`), and ready again after a failure.
                let then: OnSubmit = match choice.again.clone() {
                    Some(again) => Box::new(move |app, v| again(app, v)),
                    None => Box::new(|_, _| {}),
                };
                dialog.choices.insert(i, Choice { label: choice.label.clone(), kind: choice.kind, then, again: choice.again.clone() });
                self.dialog = Some(dialog);
            }
            (choice.then)(self, &values);
            return;
        }
        if close {
            return;
        }
        self.dialog = Some(dialog);
    }
}

/// Width of a footer button with `label`, as `widgets::button` draws it.
fn footer_button_width(ui: &Ui, label: &str) -> f32 {
    let text = ui.painter().layout_no_wrap(label.to_owned(), FontId::proportional(13.0), Color32::WHITE).size().x;
    text + 24.0
}

/// `label` cut with … so its button fits `available`; the full label belongs in a tooltip.
fn footer_label(ui: &Ui, label: &str, available: f32) -> String {
    if footer_button_width(ui, label) <= available {
        return label.to_owned();
    }
    let max_text = (available - 24.0).max(0.0);
    let mut kept: Vec<char> = label.chars().collect();
    while !kept.is_empty() {
        kept.pop();
        let candidate = format!("{}…", kept.iter().collect::<String>().trim_end());
        if ui.painter().layout_no_wrap(candidate.clone(), FontId::proportional(13.0), Color32::WHITE).size().x <= max_text {
            return candidate;
        }
    }
    "…".into()
}

/// Room the footer needs for `labels`, which wrap within `width` like the buttons do.
fn footer_needed_height<'a>(ui: &Ui, labels: impl Iterator<Item = &'a str>, width: f32) -> f32 {
    let available = (width - 32.0).max(1.0);
    let mut used = 0.0;
    let mut rows = 1;
    for label in labels {
        let button = footer_button_width(ui, label).min(available);
        if used > 0.0 && used + 8.0 + button > available {
            rows += 1;
            used = button;
        } else {
            used += (if used > 0.0 { 8.0 } else { 0.0 }) + button;
        }
    }
    16.0 + rows as f32 * 28.0 + (rows - 1) as f32 * 8.0
}

/// The dialog's body: text, a form, or a list. Enter in a text field presses the last button.
fn body(ui: &mut Ui, dialog: &mut Dialog, browse: &mut Option<(String, String)>, chosen: &mut Option<usize>) {
    let c = p();
    let first = dialog.first_frame;
    let last = dialog.choices.len().checked_sub(1);
    if let Body::Pick { hint, filter, rows, button, on_pick } = &mut dialog.body {
        let w = ui.available_width();
        let r = widgets::field(ui, filter, hint, w, 30.0, 13.0);
        if first {
            r.request_focus();
        }
        ui.add_space(12.0);
        let q = filter.trim().to_lowercase();
        let mut shown = 0;
        for (i, (title, detail)) in rows.iter().enumerate() {
            if !q.is_empty() && !title.to_lowercase().contains(&q) && !detail.to_lowercase().contains(&q) {
                continue;
            }
            shown += 1;
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(w - 150.0);
                    ui.add(egui::Label::new(RichText::new(title).size(13.0).color(c.fg)).truncate());
                    ui.add(egui::Label::new(RichText::new(detail).size(12.0).color(c.fg3).monospace()).truncate());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::small(ui, Kind::Default, button).clicked() {
                        dialog.picked = Some((on_pick.clone(), i));
                    }
                });
            });
            ui.add_space(4.0);
            let (line, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
            ui.painter().rect_filled(line, CornerRadius::ZERO, c.white(0.08));
            ui.add_space(4.0);
        }
        if shown == 0 {
            ui.label(RichText::new("No matching entries.").size(13.0).color(c.fg3));
        }
        return;
    }
    match &dialog.body {
        Body::Pick { .. } | Body::Custom(_) => {}
        Body::Mono(text) => {
            let mut t = text.clone();
            ui.add(egui::TextEdit::multiline(&mut t).font(FontId::monospace(12.0)).frame(egui::Frame::NONE).desired_width(f32::INFINITY));
        }
        Body::Text(text) => {
            ui.add(egui::Label::new(RichText::new(text).size(13.0).color(c.fg)).wrap());
        }
        Body::Info(text) => {
            let mut t = text.clone();
            ui.add(egui::TextEdit::multiline(&mut t).font(FontId::proportional(13.0)).frame(egui::Frame::NONE).desired_width(f32::INFINITY).interactive(true));
        }
        Body::List(rows) => {
            for (left, right) in rows {
                ui.horizontal(|ui| {
                    ui.set_min_height(22.0);
                    ui.label(RichText::new(left).size(11.5).color(c.fg3).monospace());
                    ui.add(egui::Label::new(RichText::new(right).size(12.5).color(c.fg)).wrap());
                });
            }
        }
        Body::Form { intro, fields, grid } => {
            if !intro.is_empty() {
                ui.label(RichText::new(intro).size(15.0).color(c.white(1.0)));
                ui.add_space(12.0);
            }
            let grid = *grid && ui.available_width() >= 420.0;
            let fields = fields.clone();
            let mut focus_done = !first;
            for f in &fields {
                if let Some(when) = &f.when
                    && !dialog.values.flag(when)
                {
                    continue;
                }
                // Clone's depth / since fields follow the "Limit history by" choice.
                if (f.key == "depth" && dialog.values.text("limit") == "since") || (f.key == "since" && dialog.values.text("limit") != "since") {
                    continue;
                }
                let indent = if grid && f.when.is_some() { 142.0 + 12.0 } else { 0.0 };
                let mut submit = false;
                let mut row = |ui: &mut Ui, values: &mut Values| {
                    let width = ui.available_width();
                    match &f.kind {
                        FieldKind::Text | FieldKind::Password | FieldKind::Folder => {
                            let mut text = values.text(&f.key);
                            let browse_w = if matches!(f.kind, FieldKind::Folder) { 78.0 } else { 0.0 };
                            ui.horizontal(|ui| {
                                let id = ui.scope_id().with(("field", &f.key));
                                let focused = ui.memory(|m| m.has_focus(id));
                                let (rect, _) = ui.allocate_exact_size(vec2((width - browse_w).max(16.0), 32.0), Sense::hover());
                                ui.painter().rect(rect, CornerRadius::same(4), c.field, Stroke::new(1.0, if focused { ACCENT } else { c.white(0.1) }), StrokeKind::Inside);
                                let edit = egui::TextEdit::singleline(&mut text)
                                    .id(id)
                                    .password(matches!(f.kind, FieldKind::Password))
                                    .frame(egui::Frame::NONE)
                                    .font(FontId::proportional(13.0))
                                    .hint_text(RichText::new(&f.placeholder).color(c.fg3))
                                    .align(Align2::LEFT_CENTER)
                                    .desired_width(rect.width() - 16.0);
                                let r = ui.put(rect.shrink2(vec2(8.0, 0.0)), edit);
                                if !focus_done {
                                    r.request_focus();
                                    focus_done = true;
                                }
                                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    submit = true;
                                }
                                if browse_w > 0.0 && widgets::button(ui, Kind::Default, None, "Browse", 28.0, true).clicked() {
                                    *browse = Some((f.key.clone(), f.label.clone()));
                                }
                            });
                            values.0.insert(f.key.clone(), Value::Text(text));
                        }
                        FieldKind::Multiline(rows) => {
                            let mut text = values.text(&f.key);
                            egui::Frame::new().fill(c.field).stroke(Stroke::new(1.0, c.white(0.1))).corner_radius(CornerRadius::same(4)).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
                                ui.add(egui::TextEdit::multiline(&mut text).frame(egui::Frame::NONE).font(FontId::monospace(12.0)).hint_text(RichText::new(&f.placeholder).color(c.fg3)).desired_rows(*rows).desired_width(width - 16.0));
                            });
                            values.0.insert(f.key.clone(), Value::Text(text));
                        }
                        FieldKind::Check => {
                            let mut on = values.flag(&f.key);
                            let label = if grid { "" } else { f.label.as_str() };
                            widgets::checkbox(ui, &mut on, label, 13.0);
                            values.0.insert(f.key.clone(), Value::Flag(on));
                        }
                        FieldKind::Select(options) => {
                            let mut current = values.text(&f.key);
                            let shown = options.iter().find(|(v, _)| *v == current).map(|(_, l)| l.clone()).unwrap_or_default();
                            // Long choices (branch names) can be cut off: the full text shows under the pointer.
                            let combo = egui::ComboBox::from_id_salt(("select", &f.key)).selected_text(shown.clone()).width(width - 8.0).show_ui(ui, |ui| {
                                for (value, label) in options {
                                    ui.selectable_value(&mut current, value.clone(), label).on_hover_text(label);
                                }
                            });
                            if !shown.is_empty() {
                                combo.response.on_hover_text(shown);
                            }
                            values.0.insert(f.key.clone(), Value::Text(current));
                        }
                        FieldKind::Note => {
                            let dir = values.text("dir");
                            let sep = if cfg!(windows) { '\\' } else { '/' };
                            let text = format!("{}{sep}{}", dir.trim_end_matches(['/', '\\']), values.text("name").trim());
                            ui.add(egui::Label::new(RichText::new(text).size(12.5).color(c.fg2)).wrap());
                        }
                    }
                    if !f.help.is_empty() {
                        ui.add_space(2.0);
                        ui.label(RichText::new(&f.help).size(11.0).color(c.fg3));
                    }
                };
                if grid {
                    ui.horizontal_top(|ui| {
                        ui.add_space(indent);
                        let (label_rect, _) = ui.allocate_exact_size(vec2(130.0, 32.0), Sense::hover());
                        let galley = ui.painter().layout(f.label.clone(), FontId::proportional(12.5), c.fg, 130.0);
                        ui.painter().galley(pos2(label_rect.right() - galley.size().x, label_rect.center().y - galley.size().y / 2.0), galley, c.fg);
                        ui.add_space(12.0);
                        ui.vertical(|ui| row(ui, &mut dialog.values));
                    });
                } else {
                    if !matches!(f.kind, FieldKind::Check) {
                        ui.label(RichText::new(&f.label).size(12.0).color(c.fg2));
                        ui.add_space(4.0);
                    }
                    row(ui, &mut dialog.values);
                }
                ui.add_space(12.0);
                // Checked against the values as edited this frame, not as they were before the key press.
                if submit && dialog.busy.is_none() && dialog.check.as_ref().and_then(|f| f(&dialog.values)).is_none() {
                    *chosen = last;
                }
            }
        }
    }
}

#[test]
fn headless_repository_dialogs_keep_the_form_beside_the_sidebar() {
    let ctx = egui::Context::default();
    let cc = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut settings = crate::settings::Store::in_memory();
    settings.set("defaultCloneFolder", "C:/Projects/Repositories/Example parent folder");
    let mut app = App::new(&cc, super::StartOptions::default(), settings);
    for size in [vec2(1920.0, 1033.0), vec2(760.0, 520.0), vec2(390.0, 410.0)] {
        for (init, source, intro, submit) in [
            (true, "Local Only", "Initialize a Repo", "Create Repository"),
            (false, "Clone with URL", "Clone a Repo", "Clone the repo!"),
        ] {
            if init { app.init_dialog(); } else { app.clone_dialog("https://example.invalid/project.git"); }
            if !init {
                app.dialog.as_mut().unwrap().values.0.insert("shallow".into(), Value::Flag(true));
            }
            let screen = Rect::from_min_size(egui::Pos2::ZERO, size);
            for frame in 0..3 {
                let mut output = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |_| app.draw_dialog(&ctx));
                output.textures_delta.clear();
                if frame < 2 { continue; }
                let text = |label: &str| output.shapes.iter().find_map(|shape| match &shape.shape {
                    egui::Shape::Text(t) if t.galley.text() == label && shape.clip_rect.contains_rect(t.visual_bounding_rect()) => Some(t),
                    _ => None,
                }).unwrap_or_else(|| panic!("{label} must be fully visible at {size:?}"));
                assert_eq!(text(intro).galley.rows.len(), 1, "the heading must have room to fit");
                if size.x > 510.0 {
                    assert!(text(intro).pos.x - text(source).pos.x > 130.0, "the form must have a separate column");
                    assert!(text("Browse").pos.x - text(intro).pos.x > 280.0, "the directory field must have room before Browse");
                } else {
                    assert!(text(intro).pos.y > text(source).pos.y, "the form must stack below the source list");
                    assert!(text("Browse").pos.x - text(intro).pos.x > 180.0);
                }
                assert!(text(submit).pos.y > text("Browse").pos.y);
            }
        }
    }
}

#[test]
fn headless_short_dialog_does_not_constrain_the_next_body() {
    let ctx = egui::Context::default();
    let cc = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = App::new(&cc, super::StartOptions::default(), crate::settings::Store::in_memory());
    let screen = Rect::from_min_size(egui::Pos2::ZERO, vec2(760.0, 520.0));
    let render = |app: &mut App| {
        let mut output = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |_| app.draw_dialog(&ctx));
        output.textures_delta.clear();
        output.shapes
    };
    let bodies = [
        Body::Form { intro: "Fixture content".into(), fields: vec![Field::new("fixture", "Notes", FieldKind::Multiline(6))], grid: false },
        Body::List(vec![("Today".into(), "Fixture content".into())]),
        Body::Info("Fixture content".into()),
        Body::Mono("Fixture content".into()),
        Body::Pick { hint: "Filter".into(), filter: String::new(), rows: vec![("Fixture content".into(), "Details".into())], button: "Open".into(), on_pick: std::rc::Rc::new(|_, _| {}) },
    ];
    for body in bodies {
        app.dialog = Some(Dialog::new("Short dialog", Body::Text("Short body".into())).no_cancel());
        render(&mut app);
        render(&mut app);
        app.dialog = Some(Dialog::new("Longer dialog", body).choice("Continue", Kind::Primary, |_, _| {}));
        let shapes = render(&mut app);
        for label in ["Fixture content", "Cancel", "Continue"] {
            assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == label && s.clip_rect.contains_rect(t.visual_bounding_rect()))), "{label} is clipped");
        }
    }
}

#[test]
fn headless_long_choices_wrap_inside_the_dialog() {
    let ctx = egui::Context::default();
    let cc = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = App::new(&cc, super::StartOptions::default(), crate::settings::Store::in_memory());
    let screen = Rect::from_min_size(egui::Pos2::ZERO, vec2(760.0, 520.0));
    let width = 440.0;
    let labels = [
        "Merge feature/a-very-long-branch-name into main",
        "Rebase feature/a-very-long-branch-name onto main",
        "Merge main into feature/a-very-long-branch-name",
        "Fast-forward main to feature/a-very-long-branch-name",
        "Start a pull request from feature/a-very-long-branch-name into main",
    ];
    let mut d = Dialog::new("Choose what to do with feature/a-very-long-branch-name and main", Body::Text("Choose an action.".into())).width(width);
    for label in labels {
        d = d.choice(label, Kind::Default, |_, _| {});
    }
    app.dialog = Some(d);
    let mut shapes = Vec::new();
    for _ in 0..3 {
        let mut output = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |_| app.draw_dialog(&ctx));
        output.textures_delta.clear();
        shapes = output.shapes;
    }
    let left = screen.center().x - width / 2.0 + 1.0;
    let right = screen.center().x + width / 2.0 - 1.0;
    let mut seen = 0;
    let mut title_seen = false;
    for shape in &shapes {
        let egui::Shape::Text(t) = &shape.shape else { continue };
        let text = t.galley.text();
        if text.starts_with("Choose what to do") {
            let rect = t.visual_bounding_rect();
            assert!(rect.left() >= left && rect.right() <= right, "the title is outside the dialog: {rect:?}");
            title_seen = true;
            continue;
        }
        let shown = text.trim_end_matches('…');
        if text == "Cancel" || labels.iter().any(|label| label.starts_with(shown)) {
            let rect = t.visual_bounding_rect();
            assert!(rect.left() >= left && rect.right() <= right, "{text} is outside the dialog: {rect:?}");
            seen += 1;
        }
    }
    assert!(title_seen, "the title must be drawn");
    assert_eq!(seen, labels.len() + 1, "every choice and Cancel must be drawn");
}

/// Where a repository can come from: the left list of the Clone and Init dialogs.
pub const SERVICE_SOURCES: [(&str, &str); 11] = [
    ("github", "GitHub.com"),
    ("github-enterprise", "GitHub Enterprise Server"),
    ("gitlab", "GitLab.com"),
    ("gitlab-self", "GitLab (Self-Managed)"),
    ("bitbucket", "Bitbucket.org"),
    ("bitbucket-server", "Bitbucket Data Center"),
    ("azure", "Azure DevOps"),
    ("azure-server", "Azure DevOps Server"),
    ("gitea", "Gitea"),
    ("forgejo", "Forgejo"),
    ("codeberg", "Codeberg"),
];
