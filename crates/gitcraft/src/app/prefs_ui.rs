//! Preferences, over the whole window below the menu bar: the sections on the left, the chosen
//! section on the right, each setting a row with its label, control and help, as in the desktop
//! app. Repo-Specific Preferences follow for the repository in front.

use super::widgets::Tip;
use super::App;
use super::icons::{self, Icon};
use super::repo::Loaded;
use super::theme::{ACCENT, GOOD, WARN, p};
use super::widgets::{self, Kind, REMOVED};
use egui::{Align2, Color32, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use gitcraft_core::actions::Action;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::Path;

/// Where a preference is kept.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Setting(String),
    /// `repoPrefs[path][key]`.
    Repo(String, String),
    /// Git config: scope, repository (for local), key.
    Config(String, Option<String>, String),
    /// `ai[key]`.
    Ai(String),
    /// `integrations[key]`.
    Integration(String),
}

#[derive(Default)]
pub struct PrefsUi {
    pub section: String,
    /// What text fields hold while they are being edited.
    pub buffers: HashMap<String, String>,
    /// Fields edited but not saved yet (by buffer id): saved when the section changes or
    /// Preferences close, as they would be when the field loses focus.
    pub pending: HashMap<String, (Target, Pending)>,
    pub versions: Option<Loaded<gitcraft_core::prefs::Versions>>,
    pub service: String,
    pub models: Option<(String, Loaded<Vec<gitcraft_core::ai_models::Model>>)>,
    pub test: Option<(String, bool, String)>,
    pub program: Option<(String, String, Result<String, String>)>,
    pub gitflow: Option<gitcraft_core::manage::GitflowConfig>,
    pub lfs: Option<gitcraft_core::manage::LfsInfo>,
    pub sparse: Option<gitcraft_core::manage::SparseInfo>,
    pub submodules: Option<Vec<gitcraft_core::manage::Submodule>>,
}

/// What Preferences read and run, kept on the app rather than in [`PrefsUi`], which is taken out
/// while Preferences are drawn: reads asked for and tasks started while drawing land here.
#[derive(Default)]
pub struct PrefsData {
    pub config: HashMap<String, Loaded<HashMap<String, String>>>,
    pub account: Option<Loaded<gitcraft_core::accounts::AccountInfo>>,
    pub statuses: Option<Loaded<Vec<gitcraft_core::hosting::ServiceStatus>>>,
    /// The preferences task running, which turns its buttons off.
    /// The tasks running, by name: Find Automatically and Test Connection (and the rest) never share one flag.
    pub busy: Vec<String>,
}

/// How a field's edit is saved.
#[derive(Clone, Copy, Debug)]
pub enum Pending {
    Text,
    Number(f64, f64),
}

const EDITORS: [(&str, &str); 11] = super::external_tools::EDITORS;
const AGENTS: [(&str, &str); 7] = [("claude", "Claude Code"), ("codex", "Codex"), ("copilot", "Copilot CLI"), ("opencode", "OpenCode"), ("gemini", "Gemini CLI"), ("cursor", "Cursor CLI"), ("none", "None")];
const DIFF_TOOLS: [&str; 19] = ["vscode", "meld", "kdiff3", "bc", "p4merge", "winmerge", "tortoisemerge", "vimdiff", "araxis", "deltawalker", "diffmerge", "ecmerge", "emerge", "examdiff", "guiffy", "opendiff", "smerge", "tkdiff", "xxdiff"];
const MONO_FONTS: [&str; 11] = ["Cascadia Code", "Cascadia Mono", "Consolas", "Courier New", "Fira Code", "JetBrains Mono", "Lucida Console", "Menlo", "Monaco", "Source Code Pro", "Ubuntu Mono"];
const OTHER_FONTS: [&str; 6] = ["Arial", "Calibri", "Open Sans", "Segoe UI", "Tahoma", "Verdana"];
const ENCODINGS: [(&str, &str); 14] = [
    ("", "Default (UTF-8)"),
    ("utf-8", "UTF-8"),
    ("utf-16le", "UTF-16 LE"),
    ("utf-16be", "UTF-16 BE"),
    ("windows-1252", "Western (Windows 1252)"),
    ("iso-8859-1", "Western (ISO 8859-1)"),
    ("iso-8859-2", "Central European (ISO 8859-2)"),
    ("windows-1251", "Cyrillic (Windows 1251)"),
    ("koi8-r", "Cyrillic (KOI8-R)"),
    ("shift_jis", "Japanese (Shift JIS)"),
    ("euc-jp", "Japanese (EUC-JP)"),
    ("gbk", "Chinese Simplified (GBK)"),
    ("big5", "Chinese Traditional (Big5)"),
    ("euc-kr", "Korean (EUC-KR)"),
];
const AI_FEATURES: [(&str, &str, &str); 6] = [
    ("commit", "Commit Message Generation", "The model GitCraft will use to generate commit messages."),
    ("compose", "Commit Composer", "The model GitCraft will use to compose commits."),
    ("stash", "Stash Message Generation", "The model GitCraft will use to generate stash messages."),
    ("explain", "Explain Changes", "The model GitCraft will use to explain changes."),
    ("pr", "Pull Request", "The model GitCraft will use to generate Pull Request title and description."),
    ("conflict", "Conflict Resolution", "The model GitCraft will use to auto-resolve conflicts."),
];

/// One service of Preferences > Integrations.
pub struct Service {
    pub id: &'static str,
    pub label: &'static str,
    pub hosting: bool,
    pub browser: bool,
    pub gh: bool,
    /// (integrations key, label, placeholder, help).
    pub fields: &'static [(&'static str, &'static str, &'static str, &'static str)],
    pub needs: &'static [&'static str],
    pub token_help: &'static str,
}

pub const SERVICES: [Service; 14] = [
    Service { id: "github", label: "GitHub", hosting: true, browser: true, gh: true, fields: &[], needs: &[], token_help: "A classic token with the repo scope (write:public_key lets GitCraft add SSH keys)." },
    Service { id: "github-enterprise", label: "GitHub Enterprise Server", hosting: true, browser: false, gh: false, fields: &[("githubEnterpriseHost", "Host Domain", "github.mycompany.com", "")], needs: &["githubEnterpriseHost"], token_help: "A personal access token with the repo scope." },
    Service { id: "gitlab", label: "GitLab", hosting: true, browser: true, gh: false, fields: &[], needs: &[], token_help: "A personal access token with the api and read_user scopes." },
    Service { id: "gitlab-self", label: "GitLab Self-Managed", hosting: true, browser: false, gh: false, fields: &[("gitlabHost", "Host Domain", "gitlab.mycompany.com", "")], needs: &["gitlabHost"], token_help: "A personal access token with the api and read_user scopes." },
    Service { id: "bitbucket", label: "Bitbucket", hosting: true, browser: true, gh: false, fields: &[("bitbucketUser", "Atlassian account email", "you@example.com", "Enter your Atlassian account email for a scoped API token. Leave blank for a bearer token or browser sign-in.")], needs: &[], token_help: "Use a scoped Bitbucket API token. Creating repositories requires workspace read and repository admin access. Pushing requires repository read and write access." },
    Service { id: "bitbucket-server", label: "Bitbucket Data Center", hosting: true, browser: false, gh: false, fields: &[("bitbucketServerHost", "Host Domain", "bitbucket.mycompany.com", ""), ("bitbucketServerUser", "Username", "for pushing over HTTPS", "")], needs: &["bitbucketServerHost"], token_help: "An HTTP access token with Project (Admin) and Repositories (Admin) permissions." },
    Service { id: "azure", label: "Azure DevOps", hosting: true, browser: true, gh: false, fields: &[("azureOrg", "Azure Organization", "mycompany", "")], needs: &["azureOrg"], token_help: "A personal access token with Code (read, write & manage), Work Items (read & write), Project and Team (read), Identity (read)." },
    Service { id: "azure-server", label: "Azure DevOps Server 2022", hosting: true, browser: false, gh: false, fields: &[("azureServerCollection", "Collection URL", "https://dev.example.com/tfs/DefaultCollection", "")], needs: &["azureServerCollection"], token_help: "A collection personal access token with Code and Project permissions." },
    Service { id: "gitea", label: "Gitea", hosting: true, browser: false, gh: false, fields: &[("giteaHost", "HTTPS host", "git.example.com", ""), ("giteaUser", "Username", "", "Used for Git over HTTPS.")], needs: &["giteaHost"], token_help: "A personal access token with repository, issue, user and organization permissions for the operations you need." },
    Service { id: "forgejo", label: "Forgejo", hosting: true, browser: false, gh: false, fields: &[("forgejoHost", "HTTPS host", "git.example.com", ""), ("forgejoUser", "Username", "", "Used for Git over HTTPS.")], needs: &["forgejoHost"], token_help: "A personal access token with repository, issue, user and organization permissions for the operations you need." },
    Service { id: "codeberg", label: "Codeberg", hosting: true, browser: false, gh: false, fields: &[("codebergUser", "Username", "", "Used for Git over HTTPS.")], needs: &[], token_help: "A personal access token with repository, issue, user and organization permissions for the operations you need." },
    Service { id: "jira", label: "Jira Cloud", hosting: false, browser: false, gh: false, fields: &[("jiraSite", "Site", "mycompany.atlassian.net", ""), ("jiraEmail", "Account email", "", "")], needs: &["jiraSite", "jiraEmail"], token_help: "An Atlassian API token for the email above." },
    Service { id: "jira-server", label: "Jira Data Center", hosting: false, browser: false, gh: false, fields: &[("jiraServerHost", "Host Domain", "jira.mycompany.com", "")], needs: &["jiraServerHost"], token_help: "A personal access token (Profile > Personal Access Tokens)." },
    Service { id: "trello", label: "Trello", hosting: false, browser: false, gh: false, fields: &[("trelloKey", "API key", "from trello.com/power-ups/admin", "Create a Power-Up to get an API key.")], needs: &["trelloKey"], token_help: "Press the link to authorise GitCraft on Trello, then paste the token it shows." },
];

fn host(h: &str) -> String {
    h.trim().trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/').to_owned()
}

/// Where to make a token for a service.
pub fn token_url(id: &str, i: &Value) -> String {
    let g = |k: &str| i.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
    match id {
        "github" => "https://github.com/settings/tokens/new?scopes=repo,read:org,workflow,write:public_key&description=GitCraft".into(),
        "github-enterprise" => format!("https://{}/settings/tokens/new?scopes=repo,read:org,write:public_key&description=GitCraft", host(&g("githubEnterpriseHost"))),
        "gitlab" => "https://gitlab.com/-/user_settings/personal_access_tokens?name=GitCraft&scopes=api,read_user".into(),
        "gitlab-self" => format!("https://{}/-/user_settings/personal_access_tokens?name=GitCraft&scopes=api,read_user", host(&g("gitlabHost"))),
        "bitbucket" => "https://id.atlassian.com/manage-profile/security/api-tokens".into(),
        "bitbucket-server" => format!("https://{}/plugins/servlet/access-tokens/manage", host(&g("bitbucketServerHost"))),
        "azure" => format!("https://dev.azure.com/{}/_usersSettings/tokens", g("azureOrg").trim()),
        "azure-server" => format!("https://{}/_usersSettings/tokens", host(&g("azureServerCollection"))),
        "gitea" => format!("https://{}/user/settings/applications", host(&g("giteaHost"))),
        "forgejo" => format!("https://{}/user/settings/applications", host(&g("forgejoHost"))),
        "codeberg" => "https://codeberg.org/user/settings/applications".into(),
        "jira" | "jira-server" if id == "jira" => "https://id.atlassian.com/manage-profile/security/api-tokens".into(),
        "jira-server" => format!("https://{}/secure/ViewProfile.jspa", host(&g("jiraServerHost"))),
        _ => format!("https://trello.com/1/authorize?expiration=never&scope=read,write&response_type=token&name=GitCraft&key={}", g("trelloKey").trim()),
    }
}

/// The settings GitCraft uses, with their defaults, for values not saved yet.
fn default_of(key: &str) -> Value {
    match key {
        "autoFetchMin" => json!(super::DEFAULT_AUTO_FETCH_MINUTES),
        "autoPrune" | "autoUpdateSubmodules" | "rememberTabs" | "lazyLoad" | "conflictDetection" | "autoUpdate" | "ghostRefs" | "highlightRowsOnHover" | "toolbarLabels" | "menusInTitleBar" | "showAgentsView" | "autogroupWorktreeTabs" | "spellcheck" | "notifySuccess" | "monospaceOnly" | "editorLineNumbers" | "editorSyntax" | "termDim" | "useCommitTemplate" | "sshUseAgent" | "keepGitconfigUpdated" | "worktreeWipInGraph" => json!(true),
        "defaultBranch" => json!("main"),
        "initialCommits" => json!(2000),
        "pullMode" => json!("ff"),
        "theme" => json!("system"),
        "notificationLocation" => json!("bottom-left"),
        "language" => json!("en"),
        "dateFormat" => json!("MM/DD/YYYY"),
        "dateTimeFormat" => json!("MM/DD/YYYY @ h:mm A"),
        "dateVerboseFormat" => json!("MMMM D, YYYY h:mm A"),
        "editorFontSize" | "termFontSize" => json!(12),
        "editorTabSize" => json!(4),
        "editorEol" => json!(if cfg!(windows) { "crlf" } else { "lf" }),
        "termLineHeight" => json!(1.2),
        "termCursor" => json!("bar"),
        "externalEditor" => json!("vscode"),
        "externalTerminal" => json!("default"),
        "codingAgent" => json!("claude"),
        _ => Value::Null,
    }
}

impl App {
    pub(crate) fn open_prefs(&mut self, section: Option<&str>) {
        let mut ui = PrefsUi { section: section.unwrap_or("general").to_owned(), service: "github".into(), ..Default::default() };
        if section == Some("integrations") || section.is_none() {
            ui.service = "github".into();
        }
        // Fields edited on the screen being replaced are saved first, as when leaving it by the sidebar or Exit.
        if let Some(mut outgoing) = self.prefs.take() {
            self.save_pending(&mut outgoing);
        }
        self.prefs = Some(ui);
        self.prefs_data = PrefsData::default();
    }

    /// Save fields edited but not saved yet (their focus is gone with the section or the screen).
    fn save_pending(&mut self, pui: &mut PrefsUi) {
        for (id, (t, kind)) in std::mem::take(&mut pui.pending) {
            let Some(v) = pui.buffers.remove(&id) else { continue };
            let current = self.pref(&t);
            match kind {
                Pending::Text => {
                    let now = match &current {
                        Value::String(s) => s.clone(),
                        Value::Null => String::new(),
                        v => v.to_string(),
                    };
                    if v != now {
                        self.set_pref(&t, Value::String(v));
                    }
                }
                Pending::Number(min, max) => {
                    let now = current.as_f64().unwrap_or(0.0);
                    let n = v.trim().parse::<f64>().unwrap_or(now).clamp(min, max);
                    if (n - now).abs() > f64::EPSILON {
                        self.set_pref(&t, json!(n));
                    }
                }
            }
        }
    }

    /// A preference's current value.
    fn pref(&self, t: &Target) -> Value {
        match t {
            Target::Setting(k) if k.contains('.') => {
                let mut parts = k.split('.');
                let root = parts.next().unwrap_or("");
                let mut v = if root == "ai" { serde_json::to_value(self.ai_config()).unwrap_or_default() } else { self.settings.get::<Value>(root).unwrap_or_default() };
                for part in parts {
                    v = v.get(part).cloned().unwrap_or(Value::Null);
                }
                v
            }
            Target::Setting(k) => self.settings.get::<Value>(k).unwrap_or_else(|| default_of(k)),
            Target::Repo(path, k) => self.repo_pref(path, k).unwrap_or(Value::Null),
            Target::Config(scope, path, k) => {
                let key = format!("{scope}|{}", path.clone().unwrap_or_default());
                match self.prefs_data.config.get(&key) {
                    Some(Loaded::Ready(m)) => m.get(k).map(|v| Value::String(v.clone())).unwrap_or(Value::Null),
                    _ => Value::Null,
                }
            }
            Target::Ai(k) => serde_json::to_value(self.ai_config()).ok().and_then(|v| v.get(k).cloned()).unwrap_or(Value::Null),
            Target::Integration(k) => self.settings.get::<Value>("integrations").and_then(|v| v.get(k).cloned()).unwrap_or(json!("")),
        }
    }

    /// Change a preference.
    /// Make a newly created GPG key the signing key, and say so only when Git's configuration took it.
    fn set_signing_key(&mut self, fingerprint: String) {
        self.spawn(move || {
            let r = gitcraft_core::accounts::set(Path::new("."), "global", "user.signingkey", &fingerprint);
            super::Msg::Apply(Box::new(move |app: &mut App| {
                app.prefs_data.config.clear();
                app.prefs_data.account = None;
                match r {
                    Ok(()) => app.toast(true, "GPG Key Created Successfully"),
                    Err(e) => app.toast(false, format!("The GPG key was created but is not active: {e}")),
                }
            }))
        });
    }

    pub(crate) fn set_pref(&mut self, t: &Target, value: Value) {
        match t {
            Target::Setting(k) if k.contains('.') => {
                let parts: Vec<&str> = k.split('.').collect();
                let root = parts[0];
                let mut v = if root == "ai" { serde_json::to_value(self.ai_config()).unwrap_or_else(|_| json!({})) } else { self.settings.get::<Value>(root).unwrap_or_else(|| json!({})) };
                {
                    let mut at = &mut v;
                    for part in &parts[1..parts.len() - 1] {
                        if !at.get(*part).is_some_and(Value::is_object) {
                            at[*part] = json!({});
                        }
                        at = &mut at[*part];
                    }
                    at[parts[parts.len() - 1]] = value;
                }
                self.settings.set(root, v);
            }
            Target::Setting(k) => {
                if k == "theme" {
                    if let Some(id) = value.as_str() {
                        self.set_theme(id);
                    }
                } else {
                    self.settings.set(k, value);
                }
            }
            Target::Repo(path, k) => {
                let mut all: Value = self.settings.get("repoPrefs").unwrap_or_else(|| json!({}));
                if !all.is_object() {
                    all = json!({});
                }
                let entry = all.as_object_mut().expect("object").entry(path.clone()).or_insert_with(|| json!({}));
                if value.is_null() {
                    if let Some(o) = entry.as_object_mut() {
                        o.remove(k);
                    }
                } else {
                    entry[k] = value;
                }
                self.settings.set("repoPrefs", all);
            }
            Target::Config(scope, path, k) => {
                let (scope, path, k) = (scope.clone(), path.clone(), k.clone());
                let v = value.as_str().unwrap_or("").to_owned();
                self.spawn(move || {
                    let dir = path.clone().unwrap_or_else(|| ".".into());
                    let r = gitcraft_core::accounts::set(Path::new(&dir), &scope, &k, &v);
                    super::Msg::Apply(Box::new(move |app: &mut App| {
                        if let Err(e) = r {
                            app.toast(false, e);
                        }
                        app.prefs_data.config.clear();
                        app.prefs_data.account = None;
                    }))
                });
            }
            Target::Ai(k) => {
                let mut ai: Value = serde_json::to_value(self.ai_config()).unwrap_or_else(|_| json!({}));
                ai[k] = value;
                self.settings.set("ai", ai);
            }
            Target::Integration(k) => {
                let mut i: Value = self.settings.get("integrations").unwrap_or_else(|| json!({}));
                if !i.is_object() {
                    i = json!({});
                }
                if k == "gitlabHost" {
                    self.settings.set("gitlabHost", value.clone());
                }
                i[k] = value;
                self.settings.set("integrations", i);
                self.prefs_data.statuses = None;
            }
        }
    }

    fn want_config(&mut self, scope: &str, path: Option<String>, keys: &[&str]) {
        let key = format!("{scope}|{}", path.clone().unwrap_or_default());
        let p = &mut self.prefs_data;
        if p.config.contains_key(&key) {
            return;
        }
        p.config.insert(key.clone(), Loaded::Loading);
        let keys: Vec<String> = keys.iter().map(|s| s.to_string()).collect();
        let scope = scope.to_owned();
        self.spawn(move || {
            let m = gitcraft_core::prefs::config_get(path.as_deref().map(Path::new), &scope, &keys);
            super::Msg::Apply(Box::new(move |app: &mut App| {
                app.prefs_data.config.insert(key, Loaded::Ready(m));
            }))
        });
    }

    fn want_account(&mut self) {
        let p = &mut self.prefs_data;
        if p.account.is_some() {
            return;
        }
        p.account = Some(Loaded::Loading);
        self.spawn(|| {
            let info = gitcraft_core::accounts::info(Path::new("."));
            super::Msg::Apply(Box::new(move |app: &mut App| {
                app.prefs_data.account = Some(Loaded::Ready(info));
            }))
        });
    }

    fn want_statuses(&mut self) {
        let p = &mut self.prefs_data;
        if p.statuses.is_some() {
            return;
        }
        p.statuses = Some(Loaded::Loading);
        self.spawn(|| {
            let handles: Vec<_> = SERVICES.iter().map(|s| std::thread::spawn(move || gitcraft_core::hosting::status(s.id))).collect();
            let list: Vec<_> = handles.into_iter().filter_map(|h| h.join().ok()).collect();
            super::Msg::Apply(Box::new(move |app: &mut App| {
                app.prefs_data.statuses = Some(Loaded::Ready(list));
            }))
        });
    }

    /// Run a preferences task away from the window; `done` gets its answer.
    fn prefs_task<T: Send + 'static>(&mut self, busy: &str, job: impl FnOnce() -> Result<T, String> + Send + 'static, done: impl FnOnce(&mut App, T) + Send + 'static) {
        let name = busy.to_owned();
        if !self.prefs_data.busy.contains(&name) {
            self.prefs_data.busy.push(name.clone());
        }
        self.spawn(move || {
            let r = job();
            super::Msg::Apply(Box::new(move |app: &mut App| {
                app.prefs_data.busy.retain(|b| *b != name);
                match r {
                    Ok(v) => done(app, v),
                    Err(e) => app.toast(false, e),
                }
            }))
        });
    }

    /// The Preferences screen, over everything below the tabs bar.
    pub(super) fn draw_prefs(&mut self, ctx: &egui::Context) {
        let Some(mut pui) = self.prefs.take() else { return };
        let c = p();
        let screen = ctx.content_rect();
        let rect = Rect::from_min_max(pos2(screen.left(), self.below_tabs.clamp(screen.top(), screen.bottom())), screen.max);
        let tab_path = self.active_tab().and_then(|t| t.path().map(str::to_owned));
        let tab_title = self.active_tab().map(|t| t.title.clone()).unwrap_or_default();
        let mut close = false;
        let section_before = pui.section.clone();
        // Not constrained to the screen: egui would move an area that ends up taller than the room
        // below the menu bar up over the menus.
        egui::Area::new(egui::Id::NULL.with("prefs")).order(egui::Order::Foreground).fixed_pos(rect.min).constrain(false).show(ctx, |ui| {
            ui.set_clip_rect(rect);
            let (area, _) = ui.allocate_exact_size(rect.size(), Sense::click_and_drag());
            let corners = super::frame::bottom_corners(ctx);
            ui.painter().rect_filled(area, corners, c.app);
            let nav = Rect::from_min_size(area.min, vec2(250.0, area.height()));
            ui.painter().rect_filled(nav, CornerRadius { sw: corners.sw, ..CornerRadius::ZERO }, c.panel);
            ui.painter().line_segment([nav.right_top(), nav.right_bottom()], Stroke::new(1.0, c.black(0.3)));
            let mut nui = ui.new_child(egui::UiBuilder::new().max_rect(nav).layout(egui::Layout::top_down(egui::Align::Min)));
            egui::ScrollArea::vertical().id_salt("prefs_nav").auto_shrink([false, false]).show(&mut nui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let (exit, resp) = ui.allocate_exact_size(vec2(250.0, 40.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(exit, CornerRadius::ZERO, c.hoverrow);
                }
                icons::paint(ui, Rect::from_min_size(pos2(exit.left() + 12.0, exit.center().y - 8.0), Vec2::splat(16.0)), Icon::ArrowLeft, 16.0, c.white(1.0));
                ui.painter().text(pos2(exit.left() + 36.0, exit.center().y), Align2::LEFT_CENTER, "Exit Preferences", FontId::proportional(15.0), c.white(1.0));
                if resp.tip("Exit Preferences (Esc)").clicked() {
                    close = true;
                }
                let heading = |ui: &mut Ui, t: &str, top: f32| {
                    ui.add_space(top);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(egui::RichText::new(t).size(12.0).color(c.fg2));
                    });
                    ui.add_space(4.0);
                };
                heading(ui, "Current profile", 4.0);
                let (prow, _) = ui.allocate_exact_size(vec2(250.0, 32.0), Sense::hover());
                ui.painter().rect_filled(prow, CornerRadius::ZERO, c.white(0.04));
                icons::paint(ui, Rect::from_min_size(pos2(prow.left() + 16.0, prow.center().y - 7.5), Vec2::splat(15.0)), Icon::Users, 15.0, c.white(0.7));
                ui.painter().text(pos2(prow.left() + 42.0, prow.center().y), Align2::LEFT_CENTER, self.settings.active_profile().0, FontId::proportional(13.0), c.white(1.0));
                heading(ui, "Preferences", 16.0);
                let general: [(&str, &str, Icon); 14] = [
                    ("general", "General", Icon::Cog),
                    ("profiles", "Profiles", Icon::Users),
                    ("ssh", "SSH", Icon::ShieldCheck),
                    ("integrations", "Integrations", Icon::Plug),
                    ("gpg", "Commit Signing", Icon::FileSignature),
                    ("ui", "UI Customization", Icon::Paintbrush),
                    ("notifications", "Notifications", Icon::Bell),
                    ("editor", "Editor", Icon::Code),
                    ("cli", "In-App Terminal", Icon::SquareTerminal),
                    ("tools", "External Tools", Icon::Wrench),
                    ("ai", "AI", Icon::Sparkles),
                    ("agents", "Agents", Icon::Bot),
                    ("commit", "Commit", Icon::GitCommitHorizontal),
                    ("experimental", "Experimental", Icon::FlaskConical),
                ];
                let repo: [(&str, &str, Icon); 9] = [
                    ("gitflow", "Gitflow", Icon::ArrowLeftRight),
                    ("githooks", "Git Hooks", Icon::Brackets),
                    ("lfs", "LFS", Icon::FileText),
                    ("encoding", "Encoding", Icon::Type),
                    ("conflict", "Conflict Prevention", Icon::GitMerge),
                    ("issues", "Issue Tracker", Icon::ListTodo),
                    ("sparse", "Sparse Checkout", Icon::FolderTree),
                    ("pull", "Pull and Checkout", Icon::GitMerge),
                    ("submodules", "Submodules", Icon::Layers),
                ];
                let item = |ui: &mut Ui, id: &str, label: &str, icon: Icon, pui: &mut PrefsUi| {
                    let (r, resp) = ui.allocate_exact_size(vec2(250.0, 32.0), Sense::click());
                    let on = pui.section == id;
                    if on {
                        ui.painter().rect_filled(r, CornerRadius::ZERO, c.selrow);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r, CornerRadius::ZERO, c.hoverrow);
                    }
                    icons::paint(ui, Rect::from_min_size(pos2(r.left() + 16.0, r.center().y - 7.5), Vec2::splat(15.0)), icon, 15.0, c.white(0.7));
                    ui.painter().text(pos2(r.left() + 42.0, r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(13.0), if on { c.white(1.0) } else { c.fg });
                    if resp.clicked() {
                        pui.section = id.to_owned();
                    }
                };
                for (id, label, icon) in general {
                    item(ui, id, label, icon, &mut pui);
                }
                if tab_path.is_some() {
                    heading(ui, "Repo-Specific Preferences", 20.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                        icons::paint(ui, r, Icon::Folder, 12.0, c.fg2);
                        ui.label(egui::RichText::new(format!("Repo:{tab_title}")).size(12.0).color(c.fg2));
                    });
                    ui.add_space(4.0);
                    for (id, label, icon) in repo {
                        item(ui, id, label, icon, &mut pui);
                    }
                }
                ui.add_space(16.0);
            });
            // Edits in the section left are saved, as a click elsewhere would save them.
            if pui.section != section_before {
                self.save_pending(&mut pui);
            }
            let body = Rect::from_min_max(pos2(nav.right() + 1.0, area.top()), area.max);
            let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
            bui.set_clip_rect(body);
            egui::ScrollArea::both().id_salt(("prefs_body", pui.section.clone())).auto_shrink([false, false]).show(&mut bui, |ui| {
                egui::Frame::new().inner_margin(egui::Margin { left: 32, right: 32, top: 24, bottom: 32 }).show(ui, |ui| {
                    ui.set_max_width(1000.0_f32.min(body.width() - 64.0));
                    ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
                    let section = pui.section.clone();
                    let is_repo = ["gitflow", "githooks", "lfs", "encoding", "conflict", "issues", "sparse", "pull", "submodules"].contains(&section.as_str());
                    match (is_repo, &tab_path) {
                        (true, Some(path)) => self.repo_section(ui, &mut pui, &section, path),
                        _ => self.general_section(ui, &mut pui, &section),
                    }
                });
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape))
            && self.dialog.as_ref().is_none_or(|dialog| dialog.suspended_for_connection())
            && !ctx.egui_wants_keyboard_input()
        {
            close = true;
        }
        if close {
            self.save_pending(&mut pui);
        } else {
            self.prefs = Some(pui);
        }
    }
}

/// Section title.
fn h(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).font(super::fonts::semibold(18.0)).color(p().white(0.95)));
    ui.add_space(16.0);
}

fn sub(ui: &mut Ui, text: &str) {
    ui.add_space(24.0);
    ui.label(egui::RichText::new(text).font(super::fonts::semibold(14.0)).color(p().white(0.95)));
    ui.add_space(8.0);
}

fn note(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).size(12.5).color(p().fg2)).wrap());
    ui.add_space(12.0);
}

/// One setting row: label on the left (260 px), the control on the right, help under it.
/// Rows narrower than this put the label above the controls.
const STACK_BELOW: f32 = 480.0;

fn label_width(w: f32) -> f32 {
    (w * 0.36).clamp(140.0, 260.0)
}

/// Where a row's controls start, for lines that line up under them.
fn controls_indent(w: f32) -> f32 {
    if w < STACK_BELOW { 0.0 } else { label_width(w) + 16.0 }
}

fn row(ui: &mut Ui, label: &str, help: &str, warning: Option<&str>, error: Option<&str>, wide: bool, add: impl FnOnce(&mut Ui)) {
    let c = p();
    let w = ui.available_width();
    // In a narrow pane the label goes above its controls; otherwise beside them, its column
    // narrowing with the pane. The controls wrap onto more lines rather than run past the edge.
    let stacked = w < STACK_BELOW;
    let label_w = if stacked { w } else { label_width(w) };
    let cw = if stacked {
        w
    } else {
        let room = w - label_w - 16.0;
        if wide { room } else { room.min(460.0) }
    }
    .max(60.0);
    let controls = |ui: &mut Ui| {
        ui.vertical(|ui| {
            ui.set_width(cw);
            ui.horizontal_wrapped(|ui| {
                ui.set_min_height(28.0);
                ui.set_max_width(cw);
                ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                add(ui);
            });
            let tiny = |ui: &mut Ui, t: &str, color: Color32| {
                ui.add_space(4.0);
                ui.add(egui::Label::new(egui::RichText::new(t).size(12.0).color(color)).wrap());
            };
            if !help.is_empty() {
                tiny(ui, help, c.fg3);
            }
            if let Some(t) = warning {
                tiny(ui, t, WARN);
            }
            if let Some(t) = error {
                tiny(ui, t, REMOVED);
            }
        });
    };
    let label_galley = ui.painter().layout(label.to_owned(), FontId::proportional(13.0), c.fg, label_w - 10.0);
    let label_h = if stacked { label_galley.size().y + 6.0 } else { (label_galley.size().y + 10.0).max(28.0) };
    if stacked {
        let (lr, _) = ui.allocate_exact_size(vec2(w, label_h), Sense::hover());
        ui.painter().galley(lr.left_top(), label_galley, c.fg);
        controls(ui);
    } else {
        ui.horizontal_top(|ui| {
            // Label, gap and controls add up to the row's width exactly, so rows don't widen
            // the page (which would push every following row further right).
            ui.spacing_mut().item_spacing.x = 0.0;
            let (lr, _) = ui.allocate_exact_size(vec2(label_w, label_h), Sense::hover());
            ui.painter().galley(pos2(lr.left(), lr.top() + 5.0), label_galley, c.fg);
            ui.add_space(16.0);
            controls(ui);
        });
    }
    ui.add_space(8.0);
    let (line, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
    ui.painter().rect_filled(line, CornerRadius::ZERO, c.white(0.04));
    ui.add_space(8.0);
}

fn pbutton(ui: &mut Ui, text: &str, kind: Kind, enabled: bool) -> bool {
    widgets::button(ui, kind, None, text, 28.0, enabled).clicked()
}

impl App {
    fn toggle(&mut self, ui: &mut Ui, t: Target, enabled: bool) {
        let mut on = match self.pref(&t) {
            Value::Bool(b) => b,
            Value::String(s) => matches!(s.to_lowercase().as_str(), "true" | "yes" | "on" | "1"),
            _ => false,
        };
        let resp = ui.add_enabled_ui(enabled, |ui| widgets::checkbox(ui, &mut on, "", 13.0)).inner;
        if resp.clicked() && enabled {
            let v = match &t {
                Target::Config(..) => Value::String(if on { "true".into() } else { "false".into() }),
                _ => Value::Bool(on),
            };
            self.set_pref(&t, v);
        }
    }

    /// A text box that saves when it loses focus or on Enter.
    fn text(&mut self, ui: &mut Ui, pui: &mut PrefsUi, t: Target, placeholder: &str, mono: bool) {
        let id = format!("{t:?}");
        let current = match self.pref(&t) {
            Value::String(s) => s,
            Value::Null => String::new(),
            v => v.to_string(),
        };
        let buf = pui.buffers.entry(id.clone()).or_insert_with(|| current.clone());
        let w = ui.available_width();
        let font = if mono { FontId::monospace(12.5) } else { FontId::proportional(13.0) };
        let resp = field(ui, buf, placeholder, w, TextVisibility::Visible, font, &id);
        let commit = resp.lost_focus() || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
        if !resp.has_focus() && !commit && *buf != current {
            *buf = current.clone();
        }
        if resp.has_focus() && *buf != current {
            pui.pending.insert(id.clone(), (t.clone(), Pending::Text));
        }
        if commit {
            let v = buf.clone();
            pui.buffers.remove(&id);
            pui.pending.remove(&id);
            if v != current {
                self.set_pref(&t, Value::String(v));
            }
        }
    }

    fn area(&mut self, ui: &mut Ui, pui: &mut PrefsUi, t: Target, placeholder: &str, rows: usize, mono: bool) {
        let c = p();
        let id = format!("{t:?}");
        let current = self.pref(&t).as_str().unwrap_or("").to_owned();
        let buf = pui.buffers.entry(id.clone()).or_insert_with(|| current.clone());
        let mut lost = false;
        let mut focused = false;
        egui::Frame::new().fill(c.field).stroke(Stroke::new(1.0, c.white(0.1))).corner_radius(CornerRadius::same(3)).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
            let font = if mono { FontId::monospace(12.5) } else { FontId::proportional(13.0) };
            let r = ui.add(egui::TextEdit::multiline(buf).id(egui::Id::NULL.with(("pref_area", &id))).frame(egui::Frame::NONE).font(font).hint_text(egui::RichText::new(placeholder).color(c.fg3)).desired_rows(rows).desired_width(f32::INFINITY));
            lost = r.lost_focus();
            focused = r.has_focus();
        });
        if !focused && !lost && *buf != current {
            *buf = current.clone();
        }
        if focused && *buf != current {
            pui.pending.insert(id.clone(), (t.clone(), Pending::Text));
        }
        if lost {
            let v = buf.clone();
            pui.buffers.remove(&id);
            pui.pending.remove(&id);
            if v != current {
                self.set_pref(&t, Value::String(v));
            }
        }
    }

    fn num(&mut self, ui: &mut Ui, pui: &mut PrefsUi, t: Target, min: f64, max: f64, width: f32) {
        let id = format!("{t:?}");
        let current = self.pref(&t).as_f64().unwrap_or(0.0);
        let shown = if current.fract() == 0.0 { format!("{}", current as i64) } else { format!("{current}") };
        let buf = pui.buffers.entry(id.clone()).or_insert_with(|| shown.clone());
        let resp = field(ui, buf, "", width, TextVisibility::Visible, FontId::proportional(13.0), &id);
        let commit = resp.lost_focus() || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
        if !resp.has_focus() && !commit && *buf != shown {
            *buf = shown.clone();
        }
        if resp.has_focus() && *buf != shown {
            pui.pending.insert(id.clone(), (t.clone(), Pending::Number(min, max)));
        }
        if commit {
            let n = buf.trim().parse::<f64>().unwrap_or(current).clamp(min, max);
            pui.buffers.remove(&id);
            pui.pending.remove(&id);
            if (n - current).abs() > f64::EPSILON {
                self.set_pref(&t, json!(n));
            }
        }
    }

    fn select(&mut self, ui: &mut Ui, t: Target, options: &[(&str, &str)]) {
        let current = match self.pref(&t) {
            Value::String(s) => s,
            Value::Bool(b) => b.to_string(),
            Value::Null => String::new(),
            v => v.to_string(),
        };
        let shown = options.iter().find(|(v, _)| *v == current).map(|(_, l)| l.to_string()).unwrap_or_else(|| if current.is_empty() { options.first().map(|o| o.1.to_owned()).unwrap_or_default() } else { current.clone() });
        let mut chosen: Option<String> = None;
        let w = ui.available_width();
        egui::ComboBox::from_id_salt(format!("{t:?}")).selected_text(shown).width(w - 8.0).show_ui(ui, |ui| {
            for (v, l) in options {
                if ui.selectable_label(*v == current, *l).clicked() {
                    chosen = Some(v.to_string());
                }
            }
        });
        if let Some(v) = chosen {
            let value = match &t {
                Target::Repo(_, k) if (k == "pullAutostash" || k == "pullCheckoutSubmodules") => {
                    if v.is_empty() { Value::Null } else { Value::Bool(v == "true") }
                }
                Target::Repo(_, k) if k == "pullMode" && v.is_empty() => Value::Null,
                _ => Value::String(v),
            };
            self.set_pref(&t, value);
        }
    }

    /// A path with a Browse button (file or folder).
    fn path(&mut self, ui: &mut Ui, pui: &mut PrefsUi, t: Target, placeholder: &str, folder: bool, title: &str) {
        let browse_w = 84.0;
        let w = ui.available_width() - browse_w - 8.0;
        ui.allocate_ui(vec2(w, 28.0), |ui| self.text(ui, pui, t.clone(), placeholder, true));
        if widgets::button(ui, Kind::Default, Some(Icon::FolderOpen), "Browse", 28.0, true).clicked() {
            let dialog = if folder { craft_dialogs::FileDialog::folder() } else { craft_dialogs::FileDialog::open() }.title(title);
            self.pickers.ask(dialog, super::dialogs::Pick::Pref(t));
        }
    }

    fn general_section(&mut self, ui: &mut Ui, pui: &mut PrefsUi, section: &str) {
        let s = |k: &str| Target::Setting(k.into());
        let c = p();
        match section {
            "general" => {
                h(ui, "General");
                self.want_config("global", None, &["core.autocrlf", "core.longpaths"]);
                let fetch = self.settings.num("autoFetchMin", super::DEFAULT_AUTO_FETCH_MINUTES);
                let w = (fetch > 0.0 && fetch < 2.0).then_some("If you have many visible remotes, this may impact performance.");
                row(ui, "Auto-Fetch Interval", "Fetches when a repository opens, then at this interval in minutes. Set to 0 to disable auto-fetch.", w, None, false, |ui| self.num(ui, pui, s("autoFetchMin"), 0.0, 60.0, 90.0));
                row(ui, "Auto-Prune", "", None, None, false, |ui| self.toggle(ui, s("autoPrune"), true));
                row(ui, "Keep submodules up to date", "Automatically update all submodules after performing a Git action", None, None, false, |ui| self.toggle(ui, s("autoUpdateSubmodules"), true));
                let b = self.settings.string("defaultBranch", "main");
                let bad = b.chars().any(|ch| ch.is_whitespace() || "~^:?*[\\".contains(ch)).then_some("That isn't a valid branch name.");
                row(ui, "Default Branch Name", "Sets the default branch name when initializing new repositories.", None, bad, false, |ui| self.text(ui, pui, s("defaultBranch"), "main", false));
                row(ui, "Delete \".orig\" files after merging", "", None, None, false, |ui| self.toggle(ui, s("deleteOrigAfterMerge"), true));
                row(ui, "Git Config Default: core.autocrlf", "Sets core.autocrlf in your global Git Config.", None, None, false, |ui| self.select(ui, Target::Config("global".into(), None, "core.autocrlf".into()), &[("", "(not set)"), ("true", "true"), ("input", "input"), ("false", "false")]));
                if cfg!(windows) {
                    row(ui, "Longpaths", "Sets core.longpaths in your global Git Config. By default, Windows only supports file paths less than 260 characters. Enable support for longer file paths.", None, None, false, |ui| self.toggle(ui, Target::Config("global".into(), None, "core.longpaths".into()), true));
                }
                row(ui, "Use In-App Terminal Instead", "Open External Terminal (Alt+T) and the Terminal button use the built-in terminal.", None, None, false, |ui| self.toggle(ui, s("useInAppTerminal"), true));
                row(ui, "Remember tabs", "", None, None, false, |ui| self.toggle(ui, s("rememberTabs"), true));
                let ic = self.settings.num("initialCommits", 2000.0);
                row(ui, "Initial Commits in Graph", "Minimum 500. If commit lazy loading is enabled, GitCraft will load additional commits if you reach the earliest commit in the Graph.", (ic > 2000.0).then_some("Increasing this value may adversely affect performance."), None, false, |ui| self.num(ui, pui, s("initialCommits"), 500.0, 100000.0, 90.0));
                row(ui, "Lazy Load Commits in Graph", "", None, None, false, |ui| self.toggle(ui, s("lazyLoad"), true));
                let all = self.settings.flag("showAllCommits", false);
                row(ui, "Show All Commits in Graph", "", all.then_some("Enabling this option may adversely affect performance."), None, false, |ui| self.toggle(ui, s("showAllCommits"), true));
                row(ui, "Use extended logging in activity log", "The activity log keeps Git's full output (standard output and errors) for every command.", None, None, false, |ui| self.toggle(ui, s("extendedLog"), true));
                row(ui, "Enable proactive conflict detection with target branch", "Unchecking this will disable proactive conflict detection for all repositories. You can disable an individual repository from Repo-Specific Preferences > Conflict Prevention.", None, None, false, |ui| self.toggle(ui, s("conflictDetection"), true));
                row(ui, "Automatically download GitCraft updates", "Checks GitHub Releases and installs on restart.", None, None, false, |ui| self.toggle(ui, s("autoUpdate"), true));
                row(ui, "Default folder for clone and init", "", None, None, false, |ui| self.path(ui, pui, s("defaultCloneFolder"), "", true, "Default folder"));
                row(ui, "Default pull behaviour", "", None, None, false, |ui| self.select(ui, s("pullMode"), &[("ff", "Pull (fast-forward if possible)"), ("ffonly", "Pull (fast-forward only)"), ("rebase", "Pull (rebase)"), ("fetch", "Fetch All")]));
                row(ui, "Forget all Usernames and Passwords", "Asks Git Credential Manager to forget what it saved for the connected services.", None, None, false, |ui| {
                    if pbutton(ui, "Forget All", Kind::Default, true) {
                        let i: Value = self.settings.get("integrations").unwrap_or_default();
                        let hosts: Vec<String> = ["github.com", "gitlab.com", "bitbucket.org", "dev.azure.com"].iter().map(|s| s.to_string()).chain(["githubEnterpriseHost", "gitlabHost", "bitbucketServerHost", "forgejoHost", "giteaHost"].iter().filter_map(|k| i.get(*k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned))).collect();
                        self.prefs_task("forget", move || Ok(gitcraft_core::prefs::forget_credentials(&hosts)), |app, n| {
                            app.toast(true, if n > 0 { format!("Forgot saved credentials for {n} host{}", if n == 1 { "" } else { "s" }) } else { "GitCraft has no stored user/pass credentials to remove".into() })
                        });
                    }
                });
                row(ui, "Usage data and bug reports", "GitCraft doesn't send usage data or bug reports anywhere.", None, None, false, |ui| {
                    ui.label(egui::RichText::new("Nothing is sent").size(12.5).color(c.fg3));
                });
            }
            "profiles" => self.profiles_section(ui, pui),
            "ssh" => {
                h(ui, "SSH");
                note(ui, "Used when a remote's address starts with git@ or ssh://. With the local SSH agent, Git uses your system's SSH setup (~/.ssh/config and the agent); otherwise it uses the key below.");
                self.want_account();
                let acc = match &self.prefs_data.account {
                    Some(Loaded::Ready(a)) => Some((a.ssh_keygen, a.gcm, a.credential_helper.clone(), a.email.clone(), a.ssh_keys.iter().map(|k| (k.path.clone(), k.kind.clone(), k.public.clone())).collect::<Vec<_>>())),
                    _ => None,
                };
                row(ui, "Use local SSH agent", "", None, None, false, |ui| self.toggle(ui, s("sshUseAgent"), true));
                row(ui, "SSH Private Key", "", None, None, false, |ui| self.path(ui, pui, s("sshPrivateKey"), "~/.ssh/id_ed25519", false, "SSH Private Key"));
                row(ui, "SSH Public Key", "", None, None, false, |ui| self.path(ui, pui, s("sshPublicKey"), "~/.ssh/id_ed25519.pub", false, "SSH Public Key"));
                row(ui, "Generate new Private/Public key", "This will create new keys and configure GitCraft to use them. Add the public key to your services under Integrations.", None, None, false, |ui| {
                    if pbutton(ui, "Generate", Kind::Default, acc.as_ref().is_some_and(|a| a.0)) {
                        let email = acc.as_ref().map(|a| a.3.clone()).filter(|e| !e.is_empty()).unwrap_or_else(|| "gitcraft".into());
                        self.ssh_key_dialog(email);
                    }
                });
                row(ui, "Use default Git Credential Manager", "Select this to have Git use the credential manager in your Git config to access repositories over HTTPS.", None, None, false, |ui| {
                    let mut on = acc.as_ref().is_some_and(|a| a.2 == "manager");
                    let enabled = acc.as_ref().is_some_and(|a| a.1);
                    if ui.add_enabled_ui(enabled, |ui| widgets::checkbox(ui, &mut on, "", 13.0)).inner.clicked() && enabled {
                        self.set_pref(&Target::Config("global".into(), None, "credential.helper".into()), json!(if on { "manager" } else { "" }));
                    }
                });
                if let Some(keys) = acc.as_ref().map(|a| a.4.clone()).filter(|k| !k.is_empty()) {
                    row(ui, "Keys on this computer", "", None, None, true, |ui| {
                        ui.vertical(|ui| {
                            for (path, kind, public) in keys {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&path).monospace().size(12.5).color(c.fg));
                                    ui.label(egui::RichText::new(kind).size(12.5).color(c.fg3));
                                    if pbutton(ui, "Copy public key", Kind::Default, true) {
                                        self.ctx.copy_text(public.clone());
                                        self.toast(true, "Public key copied");
                                    }
                                    if pbutton(ui, "Use", Kind::Default, true) {
                                        let private = path.trim_end_matches(".pub").to_owned();
                                        let public_path = if path.ends_with(".pub") { path.clone() } else { format!("{path}.pub") };
                                        self.settings.set("sshUseAgent", false);
                                        self.settings.set("sshPrivateKey", private);
                                        self.settings.set("sshPublicKey", public_path);
                                    }
                                });
                            }
                        });
                    });
                }
            }
            "integrations" => self.integrations_section(ui, pui),
            "gpg" => self.signing_section(ui, pui),
            "ui" => {
                h(ui, "UI Customization");
                let themes: Vec<(&str, &str)> = super::theme::Theme::ALL.iter().map(|t| (t.id(), t.label())).collect();
                row(ui, "Theme", "", None, None, false, |ui| self.select(ui, s("theme"), &themes));
                row(ui, "Notification Location", "", None, None, false, |ui| self.select(ui, s("notificationLocation"), &[("top-left", "Top Left"), ("top-right", "Top Right"), ("bottom-left", "Bottom Left"), ("bottom-right", "Bottom Right")]));
                let es = self.settings.string("language", "en") == "es";
                row(ui, "Language", if es { "Las pantallas traducidas se muestran en español." } else { "Spanish is available for the main navigation and start screen." }, None, None, false, |ui| self.select(ui, s("language"), &[("en", "English"), ("es", "Español")]));
                sub(ui, "Date/Time");
                let example = chrono::Local.with_ymd_and_hms(2026, 11, 9, 14, 23, 5).single().map(|d| d.timestamp()).unwrap_or(0);
                for (k, label) in [("dateFormat", "Date Format"), ("dateTimeFormat", "Date/Time Format"), ("dateVerboseFormat", "Date verbose Format")] {
                    let fmt = self.pref(&s(k)).as_str().unwrap_or("").to_owned();
                    let help = format!("Example: {}. Tokens: YYYY MM M DD D MMMM MMM dddd HH H hh h mm ss A; [text] is kept as written.", widgets::format_date(example, &fmt));
                    row(ui, label, &help, None, None, false, |ui| self.text(ui, pui, s(k), "", true));
                }
                sub(ui, "Graph");
                row(ui, "Branch visibility in commit graph", "Smart Branch Visibility shows the checked-out branch, its upstream and the branch it will merge into.", None, None, false, |ui| {
                    let smart = self.settings.flag("smartBranches", false);
                    let mut chosen = None;
                    let w = ui.available_width();
                    egui::ComboBox::from_id_salt("smart").selected_text(if smart { "Smart Branch Visibility" } else { "All" }).width(w - 8.0).show_ui(ui, |ui| {
                        if ui.selectable_label(!smart, "All").clicked() {
                            chosen = Some(false);
                        }
                        if ui.selectable_label(smart, "Smart Branch Visibility").clicked() {
                            chosen = Some(true);
                        }
                    });
                    if let Some(v) = chosen {
                        self.settings.set("smartBranches", v);
                        self.reload_all();
                    }
                });
                for (k, label) in [("refs", "Show branches and tags in graph"), ("graph", "Show commit tree in graph"), ("description", "Show commit description in graph"), ("author", "Show commit author in graph"), ("date", "Show commit date/time in graph"), ("sha", "Show commit sha in graph"), ("changes", "Show commit changes in graph")] {
                    row(ui, label, "", None, None, false, |ui| {
                        let mut cols: Value = serde_json::to_value(self.settings.get::<super::graph::Columns>("columns").unwrap_or_default()).unwrap_or_default();
                        let mut on = cols[k].as_bool().unwrap_or(false);
                        if widgets::checkbox(ui, &mut on, "", 13.0).clicked() {
                            cols[k] = json!(on);
                            self.settings.set("columns", cols);
                        }
                    });
                }
                row(ui, "Compact Graph Column", "", None, None, false, |ui| {
                    if pbutton(ui, "Reset columns to default layout", Kind::Default, true) {
                        self.settings.set("columns", super::graph::Columns::default());
                    }
                    if pbutton(ui, "Reset columns to compact layout", Kind::Default, true) {
                        self.settings.set("columns", super::graph::Columns { description: false, ..Default::default() });
                    }
                });
                row(ui, "Display author initials and generic remote icons instead of avatars", "", None, None, false, |ui| self.toggle(ui, s("authorInitials"), true));
                row(ui, "Show ghost branch/tag when hovering over or selecting a commit", "", None, None, false, |ui| self.toggle(ui, s("ghostRefs"), true));
                row(ui, "Highlight associated rows when hovering over a branch", "", None, None, false, |ui| self.toggle(ui, s("highlightRowsOnHover"), true));
                row(ui, "Show markers beside the graph scrollbar", "Coloured ticks for branches, tags, the checked-out commit and the selection. Off by default.", None, None, false, |ui| self.toggle(ui, s("scrollMarkers"), true));
                sub(ui, "Panels");
                row(ui, "Show menus in the title bar", "As in VS Code: the title bar also holds the File, Edit, View and Help menus, saving a row of space. Needs the app's own title bar.", None, None, false, |ui| self.toggle(ui, s("menusInTitleBar"), true));
                row(ui, "Show toolbar icon labels", "", None, None, false, |ui| self.toggle(ui, s("toolbarLabels"), true));
                row(ui, "Show Agents view in left panel", "", None, None, false, |ui| {
                    self.toggle(ui, s("showAgentsView"), true);
                    if !self.settings.flag("showAgentsView", true) && self.settings.string("leftView", "list") != "list" {
                        self.settings.set("leftView", "list");
                    }
                });
                row(ui, "Automatically group worktree tabs together", "", None, None, false, |ui| self.toggle(ui, s("autogroupWorktreeTabs"), true));
                row(ui, "Enable spell checking", "", None, None, false, |ui| self.toggle(ui, s("spellcheck"), true));
            }
            "notifications" => {
                h(ui, "Notifications");
                row(ui, "Enable Desktop Notifications", "Allow GitCraft to send desktop notifications.", None, None, false, |ui| {
                    let before = self.settings.flag("desktopNotifications", false);
                    self.toggle(ui, s("desktopNotifications"), true);
                    if !before && self.settings.flag("desktopNotifications", false) {
                        super::platform::desktop_notification("GitCraft", "Desktop notifications enabled");
                    }
                });
                row(ui, "Show a notice after successful actions", "Errors are always shown.", None, None, false, |ui| self.toggle(ui, s("notifySuccess"), true));
                row(ui, "Receive Marketing Notifications", "GitCraft has no marketing notifications.", None, None, false, |ui| {
                    let mut off = false;
                    ui.add_enabled_ui(false, |ui| widgets::checkbox(ui, &mut off, "", 13.0));
                });
            }
            "editor" => {
                h(ui, "Editor");
                let mono_only = self.settings.flag("monospaceOnly", true);
                let mut fonts: Vec<&str> = MONO_FONTS.to_vec();
                if !mono_only {
                    fonts.extend(OTHER_FONTS);
                }
                fonts.sort();
                let mut options: Vec<(&str, &str)> = vec![("", "Default")];
                options.extend(fonts.iter().map(|f| (*f, *f)));
                row(ui, "Font", "", None, None, false, |ui| self.select(ui, s("editorFontFamily"), &options));
                row(ui, "Show Only Monospace Fonts", "", None, None, false, |ui| self.toggle(ui, s("monospaceOnly"), true));
                row(ui, "Font Size", "", None, None, false, |ui| self.num(ui, pui, s("editorFontSize"), 8.0, 32.0, 90.0));
                row(ui, "Tab Size", "", None, None, false, |ui| self.num(ui, pui, s("editorTabSize"), 1.0, 16.0, 90.0));
                row(ui, "End of Line Character", "Used for new lines you type in the file editor.", None, None, false, |ui| self.select(ui, s("editorEol"), &[("lf", "LF"), ("crlf", "CRLF")]));
                row(ui, "Show Line Numbers", "", None, None, false, |ui| self.toggle(ui, s("editorLineNumbers"), true));
                row(ui, "Syntax Highlighting", "", None, None, false, |ui| self.toggle(ui, s("editorSyntax"), true));
                row(ui, "Word Wrap", "", None, None, false, |ui| self.toggle(ui, s("wordWrap"), true));
            }
            "cli" => {
                h(ui, "In-App Terminal");
                let shells: Vec<(&str, &str)> = if cfg!(windows) {
                    vec![("powershell", "Windows PowerShell"), ("pwsh", "PowerShell 7"), ("cmd", "Command Prompt"), ("gitbash", "Git Bash"), ("wsl", "WSL")]
                } else {
                    vec![("default", "Default shell"), ("bash", "bash"), ("zsh", "zsh"), ("fish", "fish")]
                };
                row(ui, "Default In-App Terminal", "Changes to this setting will only apply to new tabs or after restarting GitCraft", None, None, false, |ui| self.select(ui, s("termShell"), &shells));
                let mut options: Vec<(&str, &str)> = vec![("", "Default")];
                let mut fonts = MONO_FONTS.to_vec();
                fonts.sort();
                options.extend(fonts.iter().map(|f| (*f, *f)));
                row(ui, "Font", "", None, None, false, |ui| self.select(ui, s("termFontFamily"), &options));
                row(ui, "Font Size", "", None, None, false, |ui| self.num(ui, pui, s("termFontSize"), 8.0, 32.0, 90.0));
                row(ui, "Line Height", "", None, None, false, |ui| self.num(ui, pui, s("termLineHeight"), 1.0, 2.0, 90.0));
                row(ui, "Cursor Style", "", None, None, false, |ui| self.select(ui, s("termCursor"), &[("block", "Block"), ("underline", "Underline"), ("bar", "Bar")]));
                row(ui, "Dim terminal when unfocused", "", None, None, false, |ui| self.toggle(ui, s("termDim"), true));
            }
            "tools" => {
                h(ui, "External Tools");
                let mut tools: Vec<(&str, &str)> = vec![("", "Git's configured tool")];
                tools.extend(DIFF_TOOLS.iter().map(|t| (*t, *t)));
                sub(ui, "Merge Tool");
                row(ui, "External Merge Tool", "Opened with Ctrl+D on a conflicted file (git mergetool).", None, None, false, |ui| self.select(ui, s("mergeTool"), &tools));
                sub(ui, "Diff Tool");
                row(ui, "External Diff Tool", "Opened with Ctrl+D on a changed file (git difftool).", None, None, false, |ui| self.select(ui, s("diffTool"), &tools));
                sub(ui, "Editor");
                row(ui, "External Editor", "File > Open Repo in … and Open file in external editor.", None, None, false, |ui| self.select(ui, s("externalEditor"), &EDITORS));
                if self.settings.string("externalEditor", "vscode") == "custom" {
                    row(ui, "Editor Executable", "", None, None, false, |ui| self.path(ui, pui, s("editorPath"), "", false, "Select an editor executable to use"));
                    row(ui, "Open File Command", "Arguments; $FILE is the file, $REPO_PATH the repository folder.", None, None, false, |ui| self.text(ui, pui, s("editorFileArgs"), "\"$FILE\"", true));
                    row(ui, "Open Repo Command", "Arguments; $REPO_PATH is the repository folder.", None, None, false, |ui| self.text(ui, pui, s("editorRepoArgs"), "\"$REPO_PATH\"", true));
                }
                sub(ui, "Terminal");
                let terms: Vec<(&str, &str)> = if cfg!(windows) {
                    vec![("default", "Default (Windows Terminal if installed)"), ("wt", "Windows Terminal"), ("powershell", "Windows PowerShell"), ("pwsh", "PowerShell 7"), ("cmd", "Command Prompt"), ("gitbash", "Git Bash"), ("custom", "Other (custom)")]
                } else if cfg!(target_os = "macos") {
                    vec![("default", "Terminal"), ("iterm", "iTerm"), ("warp", "Warp"), ("ghostty", "Ghostty"), ("custom", "Other (custom)")]
                } else {
                    vec![("default", "Default"), ("custom", "Other (custom)")]
                };
                row(ui, "External Terminal", "", None, None, false, |ui| self.select(ui, s("externalTerminal"), &terms));
                if self.settings.string("externalTerminal", "default") == "custom" {
                    row(ui, "Terminal Executable", "", None, None, false, |ui| self.path(ui, pui, s("terminalPath"), "", false, "Terminal executable"));
                    row(ui, "Terminal Arguments", "$REPO_PATH is the repository folder.", None, None, false, |ui| self.text(ui, pui, s("terminalArgs"), "", true));
                }
                sub(ui, "Coding Agent");
                row(ui, "Coding Agent", "Started from the Agents view in the in-app terminal.", None, None, false, |ui| self.select(ui, s("codingAgent"), &AGENTS));
                // Hooks that let the agents report their status (and Claude Code its permission requests) to the Agents view.
                for (agent, label) in [("claude", "Claude Code"), ("codex", "Codex")] {
                    let installed = if agent == "claude" { gitcraft_core::agent_status::claude_installed(None) } else { gitcraft_core::agent_status::codex_installed(None) };
                    let help = if agent == "claude" {
                        "Adds GitCraft's hooks to Claude Code's settings, so the Agents view shows whether a session is running, waiting for you or done, and lets you allow or deny its permission requests. Your other hooks stay."
                    } else {
                        "Makes GitCraft Codex's notify program, so the Agents view shows when a session is waiting for you. Not changed if Codex already runs another notify program."
                    };
                    row(ui, &format!("{label} status"), help, None, None, false, |ui| {
                        ui.label(egui::RichText::new(if installed { "Reporting to GitCraft" } else { "Not set up" }).size(12.5).color(if installed { GOOD } else { c.fg3 }));
                        if pbutton(ui, if installed { "Remove" } else { "Set Up" }, Kind::Default, true) {
                            let result = match (agent, installed) {
                                ("claude", false) => gitcraft_core::agent_status::install_claude(None),
                                ("claude", true) => gitcraft_core::agent_status::uninstall_claude(None),
                                (_, false) => gitcraft_core::agent_status::install_codex(None),
                                (_, true) => gitcraft_core::agent_status::uninstall_codex(None),
                            };
                            match result {
                                Ok(()) => self.toast(true, if installed { format!("{label} no longer reports to GitCraft") } else { format!("{label} now reports its status to GitCraft; restart running sessions to start") }),
                                Err(e) => self.toast(false, e),
                            }
                        }
                    });
                }
            }
            "ai" => self.ai_section(ui, pui),
            "agents" => {
                h(ui, "Agents");
                row(ui, "Setup commands (1 per line)", "Commands run in sequence when a new agent session starts in a worktree. Use this to install dependencies and prepare your environment. Put each command on a new line.", None, None, true, |ui| self.area(ui, pui, s("agentSetup"), "npm install\npnpm build", 4, true));
                row(ui, "CLI arguments", "Arguments appended to the agent command when starting a session.", None, None, false, |ui| self.text(ui, pui, s("agentArgs"), "", true));
                row(ui, "Show other worktrees' changes in the graph", "A pill on the commit each other worktree has checked out counts its uncommitted changes; click it to open that worktree.", None, None, false, |ui| self.toggle(ui, s("worktreeWipInGraph"), true));
            }
            "commit" => {
                h(ui, "Commit");
                note(ui, "Create and apply a default initial message for commits to maintain consistency in commit summaries and descriptions.");
                row(ui, "Apply this template to commit messages", "", None, None, false, |ui| self.toggle(ui, s("useCommitTemplate"), true));
                row(ui, "Summary", "", None, None, false, |ui| self.text(ui, pui, s("templateSummary"), "e.g. feat: ", false));
                row(ui, "Description", "", None, None, true, |ui| self.area(ui, pui, s("templateDescription"), "", 4, false));
                row(ui, "Remove comments from commit messages", "Lines starting with # are left out of the commit message.", None, None, false, |ui| self.toggle(ui, s("stripComments"), true));
                row(ui, "Push after each commit", "", None, None, false, |ui| self.toggle(ui, s("pushAfterCommit"), true));
                row(ui, "Skip git hooks by default when committing", "", None, None, false, |ui| self.toggle(ui, s("skipHooks"), true));
                sub(ui, "Merge Behavior");
                row(ui, "Squash", "When enabled, merging branches locally will stage all changes without creating a merge commit automatically. You will need to commit manually.", None, None, false, |ui| self.toggle(ui, s("squashMerge"), true));
            }
            _ => {
                h(ui, "Experimental");
                note(ui, "GitCraft always uses the Git executable (it never had a separate Git library), so SSH commit signing and every advanced Git feature are available.");
                sub(ui, "Git Executable");
                if pui.versions.is_none() {
                    pui.versions = Some(Loaded::Loading);
                    self.spawn(|| {
                        let v = gitcraft_core::prefs::versions("gpg");
                        super::Msg::Apply(Box::new(move |app: &mut App| {
                            if let Some(p) = app.prefs.as_mut() {
                                p.versions = Some(Loaded::Ready(v));
                            }
                        }))
                    });
                }
                let (help, warning) = match &pui.versions {
                    Some(Loaded::Ready(v)) => {
                        let ver = v.git.split(|c: char| !c.is_ascii_digit() && c != '.').find(|s| s.matches('.').count() >= 2).unwrap_or("").to_owned();
                        let old = !ver.is_empty() && super::left::natural_cmp(&ver, "2.40.0") == std::cmp::Ordering::Less;
                        let help = format!("The Git executable GitCraft will use to execute Git commands.\n{} · {}\nDetected OpenSSH version: {}", if v.git.is_empty() { "not found" } else { &v.git }, v.git_path, if v.ssh.is_empty() { "not found" } else { &v.ssh });
                        let warning = if old {
                            Some(format!("Selected Git Executable version is lower than recommended: {ver} < 2.40.0. Using this version of Git may produce unexpected results."))
                        } else if v.ssh.is_empty() {
                            Some("OpenSSH not detected. Interacting with SSH remotes while not using an SSH agent might fail.".into())
                        } else {
                            None
                        };
                        (help, warning)
                    }
                    _ => ("Checking…".into(), None),
                };
                row(ui, "Git Executable", &help, warning.as_deref(), None, false, |ui| {
                    let before = self.settings.string("gitPath", "");
                    self.path(ui, pui, s("gitPath"), "git on your PATH", false, "Git executable");
                    if self.settings.string("gitPath", "") != before {
                        pui.versions = None;
                    }
                });
            }
        }
    }

    fn profiles_section(&mut self, ui: &mut Ui, pui: &mut PrefsUi) {
        let c = p();
        h(ui, "Profiles");
        note(ui, "GitCraft uses profiles to store your application preferences as well as git config information. Switching profiles also sets your global Git author name and email.");
        self.want_account();
        egui::Frame::new().fill(c.white(0.04)).corner_radius(CornerRadius::same(4)).inner_margin(egui::Margin::symmetric(16, 12)).show(ui, |ui| {
            ui.set_width(750.0_f32.min(ui.available_width()));
            ui.label(egui::RichText::new("My Profiles").size(15.0).color(c.white(1.0)));
            ui.add_space(4.0);
            let mut keep = self.settings.flag("keepGitconfigUpdated", true);
            if widgets::checkbox(ui, &mut keep, "Keep my .gitconfig updated with my GitCraft Profile preferences", 12.5).clicked() {
                self.settings.set("keepGitconfigUpdated", keep);
            }
            ui.label(egui::RichText::new("When checked, switching profiles updates your global .gitconfig to match, including commit author name and email.").size(11.0).color(c.fg3));
        });
        ui.add_space(16.0);
        if pbutton(ui, "+ Add a Profile", Kind::Default, true) {
            self.add_profile_dialog();
        }
        ui.add_space(16.0);
        let profiles: Vec<Value> = self.settings.get_root("profiles").unwrap_or_default();
        let active = self.settings.get_root::<Value>("activeProfile");
        let mut sorted: Vec<(usize, Value)> = profiles.into_iter().enumerate().collect();
        sorted.sort_by(|a, b| a.1["name"].as_str().unwrap_or("").cmp(b.1["name"].as_str().unwrap_or("")));
        let account = match &self.prefs_data.account {
            Some(Loaded::Ready(a)) => Some((a.name.clone(), a.email.clone())),
            _ => None,
        };
        let w = 890.0_f32.min(ui.available_width());
        let cols = [w * 0.4, w * 0.27, w * 0.27];
        let (head, _) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
        let mut x = head.left() + 12.0;
        for (i, t) in ["Profile Name ▴", "Author Name", "Author Email"].iter().enumerate() {
            ui.painter().text(pos2(x, head.center().y), Align2::LEFT_CENTER, *t, FontId::proportional(12.0), c.fg2);
            x += cols[i];
        }
        ui.painter().line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0, c.white(0.1)));
        let count = sorted.len();
        for (index, prof) in sorted {
            let is_active = active.as_ref() == prof.get("id") || (active.is_none() && index == 0);
            let (r, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
            if is_active {
                ui.painter().rect(r, CornerRadius::ZERO, c.selrow, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
            } else if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::ZERO, c.hoverrow);
            }
            let mut x = r.left() + 12.0;
            let color = super::tabs::profile_color(prof["icon"].as_u64().unwrap_or(0) as usize);
            super::tabs::profile_mark(ui, Rect::from_min_size(pos2(x, r.center().y - 9.0), Vec2::splat(18.0)), color);
            let name = prof["name"].as_str().unwrap_or("Profile").to_owned();
            let ng = ui.painter().layout_no_wrap(name.clone(), FontId::proportional(13.0), c.fg);
            let nw = ng.size().x;
            ui.painter().galley(pos2(x + 24.0, r.center().y - ng.size().y / 2.0), ng, c.fg);
            if is_active {
                ui.painter().text(pos2(x + 30.0 + nw, r.center().y), Align2::LEFT_CENTER, "current", FontId::proportional(11.0), p().good_text());
            }
            x += cols[0];
            let (gn, ge) = if is_active && account.is_some() { account.clone().unwrap_or_default() } else { (prof["gitName"].as_str().unwrap_or("").to_owned(), prof["gitEmail"].as_str().unwrap_or("").to_owned()) };
            ui.painter().text(pos2(x, r.center().y), Align2::LEFT_CENTER, gn, FontId::proportional(13.0), c.fg2);
            x += cols[1];
            ui.painter().text(pos2(x, r.center().y), Align2::LEFT_CENTER, ge, FontId::proportional(13.0), c.fg2);
            let mr = Rect::from_min_size(pos2(r.right() - 32.0, r.center().y - 12.0), Vec2::splat(24.0));
            let m = ui.interact(mr, ui.scope_id().with(("profile_menu", index)), Sense::click());
            icons::paint(ui, mr, Icon::EllipsisVertical, 15.0, if m.hovered() { c.white(1.0) } else { c.fg2 });
            egui::Popup::menu(&m).show(|ui| {
                if ui.add_enabled(!is_active, egui::Button::new("Switch to this profile")).clicked() {
                    self.switch_profile(index);
                }
                if ui.add_enabled(!is_active && count > 1, egui::Button::new(egui::RichText::new("Delete profile").color(REMOVED))).clicked() {
                    self.remove_profile(index);
                }
            });
        }
        ui.add_space(24.0);
        ui.label(egui::RichText::new("Current profile identity (global Git config)").size(12.0).color(c.fg2));
        ui.add_space(8.0);
        self.want_config("global", None, &["user.name", "user.email"]);
        row(ui, "Author Name", "", None, None, false, |ui| self.text(ui, pui, Target::Config("global".into(), None, "user.name".into()), "", false));
        row(ui, "Author Email", "", None, None, false, |ui| self.text(ui, pui, Target::Config("global".into(), None, "user.email".into()), "", false));
    }

    fn integrations_section(&mut self, ui: &mut Ui, pui: &mut PrefsUi) {
        let c = p();
        h(ui, "Integrations");
        note(ui, "Connect your services to see pull requests and issues in the left panel and the Launchpad, clone your repositories, create pull requests, add SSH keys, and let Git push and pull over HTTPS with the same sign-in. Tokens are kept in the system's credential store.");
        self.want_statuses();
        self.want_account();
        let statuses: Vec<gitcraft_core::hosting::ServiceStatus> = match &self.prefs_data.statuses {
            Some(Loaded::Ready(s)) => s.clone(),
            _ => Vec::new(),
        };
        let loading = matches!(self.prefs_data.statuses, Some(Loaded::Loading));
        ui.horizontal_top(|ui| {
            let (list, _) = ui.allocate_exact_size(vec2(230.0, 520.0), Sense::hover());
            ui.painter().rect(list, CornerRadius::same(4), c.white(0.02), Stroke::new(1.0, c.white(0.08)), StrokeKind::Inside);
            let mut y = list.top() + 4.0;
            for kind in [true, false] {
                ui.painter().text(pos2(list.left() + 12.0, y + 12.0), Align2::LEFT_CENTER, if kind { "HOSTING SERVICES" } else { "ISSUE TRACKERS" }, super::fonts::semibold(11.0), c.fg3);
                y += 24.0;
                for s in SERVICES.iter().filter(|s| s.hosting == kind) {
                    let r = Rect::from_min_size(pos2(list.left() + 1.0, y), vec2(228.0, 30.0));
                    let resp = ui.interact(r, ui.scope_id().with(("svc", s.id)), Sense::click());
                    if pui.service == s.id {
                        ui.painter().rect_filled(r, CornerRadius::ZERO, c.selrow);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r, CornerRadius::ZERO, c.hoverrow);
                    }
                    let st = statuses.iter().find(|x| x.id == s.id);
                    let (icon, color) = match st {
                        Some(x) if x.connected && x.error.is_some() => (Icon::TriangleAlert, WARN),
                        Some(x) if x.connected => (Icon::CheckCircle2, GOOD),
                        _ => (Icon::CircleDashed, c.fg3),
                    };
                    icons::paint(ui, Rect::from_min_size(pos2(r.left() + 12.0, r.center().y - 6.5), Vec2::splat(13.0)), icon, 13.0, color);
                    ui.painter().text(pos2(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, s.label, FontId::proportional(13.0), if pui.service == s.id { c.white(1.0) } else { c.fg });
                    if resp.clicked() {
                        pui.service = s.id.into();
                    }
                    y += 30.0;
                }
            }
            ui.add_space(16.0);
            let detail_w = ui.available_width();
            ui.vertical(|ui| {
                ui.set_width(detail_w);
                let svc = SERVICES.iter().find(|s| s.id == pui.service).unwrap_or(&SERVICES[0]);
                let st = statuses.iter().find(|x| x.id == svc.id).cloned();
                let integrations: Value = self.settings.get("integrations").unwrap_or_default();
                let ready = svc.needs.iter().all(|k| !integrations.get(*k).and_then(Value::as_str).unwrap_or("").trim().is_empty());
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(svc.label).font(super::fonts::semibold(16.0)).color(c.white(1.0)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (t, color) = if loading {
                            ("Checking…".to_owned(), c.fg3)
                        } else {
                            match &st {
                                Some(x) if x.error.is_some() => (x.error.clone().unwrap_or_default(), WARN),
                                Some(x) if x.connected => (format!("Connected{}", x.account.as_ref().map(|a| format!(" as {a}")).unwrap_or_default()), GOOD),
                                _ => (format!("{} is not connected", svc.label), c.fg3),
                            }
                        };
                        ui.label(egui::RichText::new(t).size(12.0).color(color));
                    });
                });
                ui.add_space(12.0);
                for (k, label, placeholder, help) in svc.fields {
                    row(ui, label, help, None, None, false, |ui| self.text(ui, pui, Target::Integration(k.to_string()), placeholder, false));
                }
                let connected = st.as_ref().is_some_and(|x| x.connected);
                let busy = !self.prefs_data.busy.is_empty();
                if !connected && (svc.browser || svc.gh) {
                    row(ui, "Connect", if svc.browser { "Opens your browser to sign in (through Git Credential Manager, which Git uses too)." } else { "" }, None, None, false, |ui| {
                        let id = svc.id;
                        let label = svc.label;
                        if svc.browser && pbutton(ui, &if self.prefs_data.busy.iter().any(|b| b == "browser") { "Waiting for the browser…".to_owned() } else { format!("Connect {label}") }, Kind::Primary, !busy && ready) {
                            self.sign_in(id, "browser", label);
                        }
                        if svc.gh && pbutton(ui, "Use GitHub CLI sign-in", Kind::Default, !busy) {
                            self.sign_in(id, "gh", label);
                        }
                    });
                }
                let token_id = format!("token:{}", svc.id);
                let url = token_url(svc.id, &integrations);
                row(ui, if connected { "Replace token" } else { "Personal Access Token" }, svc.token_help, None, None, false, |ui| {
                    let buf = pui.buffers.entry(token_id.clone()).or_default();
                    let w = ui.available_width() - 110.0;
                    field(ui, buf, "paste token", w, TextVisibility::Masked, FontId::proportional(13.0), &token_id);
                    let token = buf.trim().to_owned();
                    if pbutton(ui, "Save Token", Kind::Default, !token.is_empty() && ready && !busy) {
                        pui.buffers.remove(&token_id);
                        let id = svc.id;
                        self.set_token(id, token, "Token saved");
                    }
                });
                ui.horizontal(|ui| {
                    ui.add_space(controls_indent(ui.available_width()));
                    if ui.add_enabled(ready, egui::Label::new(egui::RichText::new("Generate a token here. ↗").size(12.0).color(c.link)).sense(Sense::click())).clicked() {
                        self.open_url(&url);
                    }
                });
                ui.add_space(12.0);
                if connected {
                    row(ui, "Connection", "", None, None, false, |ui| {
                        if pbutton(ui, "Disconnect", Kind::Danger, !busy) {
                            self.set_token(svc.id, String::new(), &format!("Disconnected from {}", svc.label));
                        }
                    });
                }
                if svc.hosting {
                    let key = self.public_key();
                    let help = match &key {
                        Some((path, _)) => format!("This computer's key: {path}"),
                        None => "No SSH settings detected. You can generate an SSH key in Preferences > SSH.".into(),
                    };
                    row(ui, "SSH Key", &help, None, None, true, |ui| {
                        let label = svc.label;
                        let id = svc.id;
                        if pbutton(ui, &format!("Add key to {label}"), Kind::Default, connected && key.is_some() && !busy)
                            && let Some((_, public)) = key.clone()
                        {
                            let title = format!("GitCraft ({})", std::env::consts::OS);
                            self.prefs_task("ssh", move || gitcraft_core::hosting_api::add_ssh_key(id, &title, &public), move |app, ()| app.toast(true, format!("SSH Key added to {label}!")));
                        }
                        if pbutton(ui, &format!("Copy public key and open {label}"), Kind::Default, key.is_some() && ready)
                            && let Some((_, public)) = key.clone()
                        {
                            self.ctx.copy_text(public);
                            self.open_url(&gitcraft_core::hosting_api::ssh_keys_page(id));
                            self.toast(true, "Public key copied - paste it on the page that opened");
                        }
                    });
                }
            });
        });
    }

    fn public_key(&self) -> Option<(String, String)> {
        let Some(Loaded::Ready(acc)) = &self.prefs_data.account else { return None };
        let chosen = self.settings.string("sshPublicKey", "");
        if !chosen.is_empty() {
            if let Ok(public) = gitcraft_core::accounts::read_public_key(&chosen) {
                return Some((chosen, public));
            }
            if let Some(k) = acc.ssh_keys.iter().find(|k| k.path.trim_end_matches(".pub") == chosen.trim_end_matches(".pub")) {
                return Some((k.path.clone(), k.public.clone()));
            }
        }
        acc.ssh_keys.iter().find(|k| k.path.contains("id_ed25519")).or(acc.ssh_keys.first()).map(|k| (k.path.clone(), k.public.clone()))
    }

    fn sign_in(&mut self, id: &'static str, method: &'static str, label: &'static str) {
        self.prefs_task(method, move || {
            let token = if method == "gh" { gitcraft_core::hosting::gh_cli_token()? } else { gitcraft_core::hosting::browser_sign_in(id)? };
            gitcraft_core::hosting::save_sign_in(id, &token, method != "gh")?;
            gitcraft_core::hosting::sync_git_credentials("");
            gitcraft_core::hosting::whoami_id(id)
        }, move |app, _| {
            app.toast(true, if method == "gh" { format!("Connected to {label} with the GitHub CLI sign-in") } else { format!("Connected to {label}") });
            app.prefs_data.statuses = None;
            app.hosting_changed();
            app.refresh_init_connection(id);
        });
    }

    fn set_token(&mut self, id: &'static str, token: String, ok: &str) {
        let ok = ok.to_owned();
        let gitlab = self.settings.string("gitlabHost", "");
        self.prefs_task("token", move || {
            gitcraft_core::hosting::save_secret_manually(&gitcraft_core::hosting::secret_name(id), token.trim())?;
            gitcraft_core::hosting::sync_git_credentials(&gitlab);
            Ok(())
        }, move |app, ()| {
            app.toast(true, ok);
            app.prefs_data.statuses = None;
            app.hosting_changed();
            app.refresh_init_connection(id);
        });
    }

    fn signing_section(&mut self, ui: &mut Ui, pui: &mut PrefsUi) {
        h(ui, "Commit Signing");
        let keys = ["gpg.program", "gpg.format", "user.signingkey", "commit.gpgsign", "tag.gpgSign", "gpg.ssh.program", "gpg.ssh.allowedSignersFile"];
        self.want_config("global", None, &keys);
        self.want_account();
        let path = self.active_tab().and_then(|t| t.path().map(str::to_owned));
        if let Some(p) = &path {
            self.want_config("local", Some(p.clone()), &["gpg.ssh.allowedSignersFile"]);
        }
        let g = |k: &str| Target::Config("global".into(), None, k.into());
        let val = |app: &App, k: &str| app.pref(&g(k)).as_str().unwrap_or("").to_owned();
        let ssh = val(self, "gpg.format") == "ssh";
        let program = Some(val(self, "gpg.program")).filter(|p| !p.is_empty()).unwrap_or_else(|| "gpg".into());
        let signing = val(self, "user.signingkey");
        let acc = match &self.prefs_data.account {
            Some(Loaded::Ready(a)) => Some((a.gpg, a.name.clone(), a.email.clone(), a.gpg_keys.iter().map(|k| (k.id.clone(), k.fingerprint.clone(), k.uid.clone())).collect::<Vec<_>>())),
            _ => None,
        };
        row(ui, "GPG Format", "", None, None, false, |ui| {
            let mut chosen = None;
            let w = ui.available_width();
            egui::ComboBox::from_id_salt("gpg_format").selected_text(if ssh { "SSH" } else { "OPENPGP" }).width(w - 8.0).show_ui(ui, |ui| {
                if ui.selectable_label(!ssh, "OPENPGP").clicked() {
                    chosen = Some("");
                }
                if ui.selectable_label(ssh, "SSH").clicked() {
                    chosen = Some("ssh");
                }
            });
            if let Some(v) = chosen {
                self.set_pref(&g("gpg.format"), json!(v));
            }
        });
        if !ssh {
            let no_gpg = acc.as_ref().is_some_and(|a| !a.0).then_some("Select a GPG program to sign with.");
            row(ui, "GPG Program", "This is usually \"gpg\" or \"gpg2\" depending on the version you are using.\nYou can also browse for the path directly.", no_gpg, None, false, |ui| self.path(ui, pui, g("gpg.program"), "gpg", false, "Path to gpg"));
            let no_key = signing.is_empty().then_some("A key is required for GPG signing.");
            row(ui, "Signing Key", "", no_key, None, false, |ui| {
                let mut options: Vec<(String, String)> = vec![(String::new(), "Select...".into())];
                if let Some(a) = &acc {
                    options.extend(a.3.iter().map(|(id, fpr, uid)| (if fpr.is_empty() { id.clone() } else { fpr.clone() }, format!("{uid} ({id})"))));
                }
                if !signing.is_empty() && !options.iter().any(|(v, _)| *v == signing) {
                    options.push((signing.clone(), signing.clone()));
                }
                let shown = options.iter().find(|(v, _)| *v == signing).map(|(_, l)| l.clone()).unwrap_or_default();
                let mut chosen = None;
                let w = ui.available_width() - 130.0;
                egui::ComboBox::from_id_salt("signing_key").selected_text(shown).width(w).show_ui(ui, |ui| {
                    for (v, l) in &options {
                        if ui.selectable_label(*v == signing, l).clicked() {
                            chosen = Some(v.clone());
                        }
                    }
                });
                if let Some(v) = chosen {
                    self.set_pref(&g("user.signingkey"), json!(v));
                }
                if pbutton(ui, "Reload GPG keys", Kind::Default, true) {
                    self.prefs_data.account = None;
                }
            });
            row(ui, "Copy GPG Public Key", "", None, None, false, |ui| {
                if pbutton(ui, "Copy", Kind::Default, !signing.is_empty()) {
                    let (prog, key) = (program.clone(), signing.clone());
                    self.prefs_task("export", move || gitcraft_core::prefs::gpg_export(&prog, &key), |app, text| {
                        app.ctx.copy_text(text);
                        app.toast(true, "Public GPG key copied successfully");
                    });
                }
            });
            row(ui, "Generate new GPG Key", "This will create a 4096 bit key with the name and email in your current profile that expires in 2 years.\nThis will also be set as your active signing key.", None, None, false, |ui| {
                let buf = pui.buffers.entry("gpg_pass".into()).or_default();
                field(ui, buf, "Passphrase (optional)", 200.0, TextVisibility::Masked, FontId::proportional(13.0), "gpg_pass");
                let busy = self.prefs_data.busy.iter().any(|b| b == "gpg");
                if pbutton(ui, if busy { "Generating…" } else { "Generate" }, Kind::Default, !busy && acc.as_ref().is_some_and(|a| a.0)) {
                    let pass = pui.buffers.remove("gpg_pass").unwrap_or_default();
                    let (prog, name, email) = (program.clone(), acc.as_ref().map(|a| a.1.clone()).unwrap_or_default(), acc.as_ref().map(|a| a.2.clone()).unwrap_or_default());
                    self.prefs_task("gpg", move || gitcraft_core::prefs::gpg_generate(&prog, &name, &email, &pass), |app, fpr| {
                        // Reported once Git has really stored the signing key (see `set_signing_key`).
                        app.set_signing_key(fpr);
                    });
                }
            });
        } else {
            let no_key = signing.is_empty().then_some("A key is required for SSH signing.");
            row(ui, "Signing Key", "The private key will be used for signing (or the SSH agent, for a public key or key::).", no_key, None, false, |ui| self.path(ui, pui, g("user.signingkey"), "~/.ssh/id_ed25519.pub", false, "Path to signing Key"));
            row(ui, "GPG SSH Program", "Usually empty (ssh-keygen).", None, None, false, |ui| self.path(ui, pui, g("gpg.ssh.program"), "", false, "Path to GPG SSH program"));
            let local = path.as_ref().map(|p| self.pref(&Target::Config("local".into(), Some(p.clone()), "gpg.ssh.allowedSignersFile".into()))).and_then(|v| v.as_str().map(str::to_owned)).filter(|v| !v.is_empty());
            let w = local.map(|l| format!("A local 'gpg.ssh.allowedSignersFile' config value in this repository is overriding this setting. Local value: {l}"));
            row(ui, "Allowed Signers File", "File used for SSH signature verification. If empty, it will use the global git configuration value.", w.as_deref(), None, false, |ui| self.path(ui, pui, g("gpg.ssh.allowedSignersFile"), "", false, "Path to allowed signers file"));
        }
        row(ui, "Sign Commits by Default", "", None, None, false, |ui| self.toggle(ui, g("commit.gpgsign"), true));
        row(ui, "Sign Tags by Default", "", None, None, false, |ui| self.toggle(ui, g("tag.gpgSign"), true));
    }

    fn ai_section(&mut self, ui: &mut Ui, pui: &mut PrefsUi) {
        let c = p();
        h(ui, "AI");
        note(ui, "Use your ChatGPT, Claude, or GitHub Copilot subscription, a hosted API, or a local server for GitCraft’s AI helpers.");
        let ai = self.ai_config();
        let a = |k: &str| Target::Ai(k.into());
        let provider = ai.provider.clone();
        let cli = gitcraft_core::ai_cli::Agent::from_provider(&provider).is_some();
        let (cli_name, cli_login, cli_folder, cli_var, cli_docs) = match provider.as_str() {
            "codex" => ("Codex CLI", "codex login", "~/.codex", "CODEX_HOME", "https://developers.openai.com/codex/cli"),
            "copilot" => ("GitHub Copilot CLI", "copilot login", "~/.copilot", "COPILOT_HOME", "https://docs.github.com/en/copilot/how-tos/copilot-cli/set-up-copilot-cli/authenticate-copilot-cli"),
            _ => ("Claude Code", "claude auth login", "~/.claude", "CLAUDE_CONFIG_DIR", "https://code.claude.com/docs/en/quickstart"),
        };
        let label = match provider.as_str() {
            "anthropic" => "Anthropic",
            "openai" => "OpenAI",
            "codex" => "ChatGPT",
            "claude_code" => "Claude",
            "copilot" => "GitHub Copilot",
            "google" => "Google",
            "ollama" => "Ollama",
            _ => "compatible server",
        };
        row(ui, "Enable AI", "Commit and stash messages, commit composition, change explanations, pull request drafts and conflict resolution all use this connection.", None, None, false, |ui| self.toggle(ui, a("enabled"), true));
        row(ui, "Provider", "Requests go directly to this connection when you press an AI button. A local connection keeps your code on your server.", None, None, false, |ui| {
            let options = [("anthropic", "Anthropic API (Claude)"), ("openai", "OpenAI API"), ("codex", "ChatGPT subscription (Codex CLI)"), ("claude_code", "Claude subscription (Claude Code)"), ("copilot", "GitHub Copilot subscription (Copilot CLI)"), ("google", "Google (Gemini)"), ("ollama", "Ollama (local AI)"), ("custom", "Local / OpenAI-compatible API")];
            let shown = options.iter().find(|(v, _)| *v == provider).map(|(_, l)| *l).unwrap_or("");
            let mut chosen = None;
            let w = ui.available_width();
            egui::ComboBox::from_id_salt("ai_provider").selected_text(shown).width(w - 8.0).show_ui(ui, |ui| {
                for (v, l) in options {
                    if ui.selectable_label(v == provider, l).clicked() {
                        chosen = Some(v);
                    }
                }
            });
            if let Some(v) = chosen {
                self.choose_provider(v, None);
                pui.test = None;
            }
        });
        row(ui, "Local AI presets", "Start your local server, select its preset here, then fetch its models below. Presets reset model overrides for the new server.", None, None, false, |ui| {
            for (name, prov, url) in [("Ollama", "ollama", "http://localhost:11434/v1"), ("LM Studio", "custom", "http://localhost:1234/v1"), ("llama.cpp", "custom", "http://localhost:8080/v1")] {
                if pbutton(ui, name, Kind::Default, true) {
                    self.choose_provider(prov, Some(url));
                }
            }
        });
        if cli {
            let sign_in_help = if provider == "copilot" {
                "Install GitHub Copilot CLI, then sign in with GitHub or reuse an existing Copilot CLI sign-in. AI requests use your Copilot allowance.".into()
            } else {
                format!("Install {cli_name}, then run {cli_login} in a terminal and sign in to your {label} account. Requests use that CLI’s allowance. GitCraft passes the context needed for the helper to {cli_name}.")
            };
            row(ui, &format!("{label} sign-in"), &sign_in_help, None, None, false, |ui| {
                if provider == "copilot" {
                    let waiting = self.prefs_data.busy.iter().any(|b| b == "copilot_sign_in");
                    if pbutton(ui, if waiting { "Waiting for GitHub…" } else { "Sign in with GitHub" }, Kind::Primary, !waiting) {
                        let config = self.ai_config();
                        pui.test = None;
                        self.prefs_task("copilot_sign_in", move || gitcraft_core::ai_cli::sign_in_copilot(&config.cli_path, &config.cli_config_dir), |app, ()| {
                            app.toast(true, "Signed in to GitHub Copilot.");
                        });
                    }
                }
                if pbutton(ui, "Setup Guide", Kind::Default, true) {
                    self.open_url(cli_docs);
                }
            });
            let (help, err) = match &pui.program {
                Some((pv, path, r)) if *pv == provider && *path == ai.cli_path => match r {
                    Ok(v) => (format!("Found and verified: {v}"), None),
                    Err(e) => (format!("Find {cli_name} on this computer or browse to its executable."), Some(e.clone())),
                },
                _ => (format!("Find {cli_name} on this computer or browse to its executable."), None),
            };
            row(ui, "Program", &help, None, err.as_deref(), true, |ui| {
                let w = ui.available_width() - 150.0;
                ui.allocate_ui(vec2(w, 28.0), |ui| self.path(ui, pui, a("cliPath"), "Program path", false, &format!("Choose the {cli_name} executable")));
                let finding = self.prefs_data.busy.iter().any(|b| b == "find");
                if pbutton(ui, if finding { "Finding…" } else { "Find Automatically" }, Kind::Default, !finding) {
                    let prov = provider.clone();
                    self.prefs_task("find", move || {
                        let agent = gitcraft_core::ai_cli::Agent::from_provider(&prov).ok_or("This provider does not use a local program.")?;
                        gitcraft_core::ai_cli::detect_program(agent).map(|d| (prov, d.path, d.version))
                    }, |app, (prov, path, version)| {
                        // Saved only for the provider that was asked about: another one may be selected by now.
                        if app.ai_config().provider == prov {
                            app.set_pref(&Target::Ai("cliPath".into()), json!(path.clone()));
                        }
                        if let Some(p) = app.prefs.as_mut() {
                            p.program = Some((prov, path, Ok(version)));
                        }
                    });
                }
            });
            row(ui, &format!("{cli_name} folder"), &format!("Optional. Choose the folder you used when signing in with {cli_var}. Leave empty to use the CLI’s usual folder."), None, None, false, |ui| self.path(ui, pui, a("cliConfigDir"), cli_folder, true, &format!("Choose the {cli_name} config folder")));
        } else {
            let optional = matches!(provider.as_str(), "ollama" | "custom");
            let key_page = match provider.as_str() {
                "anthropic" => Some("https://console.anthropic.com/settings/keys"),
                "openai" => Some("https://platform.openai.com/api-keys"),
                "google" => Some("https://aistudio.google.com/app/apikey"),
                _ => None,
            };
            let help = if optional { "Leave empty for a local server without authentication, or save a key if your server requires one. Keys stay in the system credential store." } else { "Your API key stays in the system credential store." };
            row(ui, if optional { "API Key (optional)" } else { "Provider API Key" }, help, None, None, false, |ui| {
                let buf = pui.buffers.entry("ai_key".into()).or_default();
                let w = ui.available_width() - 190.0;
                field(ui, buf, if optional { "optional key" } else { "paste key" }, w, TextVisibility::Masked, FontId::proportional(13.0), "ai_key");
                let key = buf.trim().to_owned();
                let saving = self.prefs_data.busy.iter().any(|b| b == "key");
                if pbutton(ui, "Save", Kind::Default, !key.is_empty() && !saving) {
                    pui.buffers.remove("ai_key");
                    self.save_ai_key(&provider, key);
                }
                if pbutton(ui, "Remove Key", Kind::Default, !saving) {
                    self.save_ai_key(&provider, String::new());
                }
            });
            if let Some(page) = key_page {
                ui.horizontal(|ui| {
                    ui.add_space(controls_indent(ui.available_width()));
                    if widgets::link(ui, &format!("Get an API key for {label}."), 12.0).clicked() {
                        self.open_url(page);
                    }
                });
                ui.add_space(8.0);
            }
            let help = if provider == "anthropic" { "Optional API base or full Messages URL. Empty uses Anthropic." } else { "Enter a server address, a /v1 API base, or the full /chat/completions URL. Empty uses the provider default; a compatible server needs its own URL." };
            let placeholder = match provider.as_str() {
                "ollama" => "http://localhost:11434/v1",
                "custom" => "http://localhost:1234/v1",
                _ => "provider default",
            };
            row(ui, "API Endpoint URL", help, None, None, false, |ui| self.text(ui, pui, a("endpoint"), placeholder, true));
        }
        // Models.
        let source = format!("{}|{}|{}|{}", ai.provider, ai.endpoint, ai.cli_path, ai.cli_config_dir);
        let (models, models_err, models_loading) = match &pui.models {
            Some((s, Loaded::Ready(m))) if *s == source => (m.clone(), None, false),
            Some((s, Loaded::Failed(e))) if *s == source => (Vec::new(), Some(e.clone()), false),
            Some((s, Loaded::Loading)) if *s == source => (Vec::new(), None, true),
            _ => (Vec::new(), None, false),
        };
        let fallback = if cli { format!("Use {cli_name} default") } else if provider == "anthropic" { "Use Anthropic default".into() } else { "Select a model".into() };
        let mut model_help = "Fetch the models available on this connection. The default is used by every feature unless you choose an override.".to_owned();
        if matches!(&pui.models, Some((s, Loaded::Ready(_))) if *s == source) {
            model_help += &if models.is_empty() { "\nNo models were listed. You can still enter a model ID.".to_owned() } else { format!("\n{} models found.", models.len()) };
        }
        row(ui, if cli { "Default Model (optional)" } else { "Default Model" }, &model_help, None, models_err.as_deref(), true, |ui| {
            let w = ui.available_width() - 140.0;
            ui.allocate_ui(vec2(w, 28.0), |ui| self.model_field(ui, pui, a("defaultModel"), &models, &fallback));
            if pbutton(ui, if models_loading { "Fetching…" } else if models.is_empty() { "Fetch Models" } else { "Refresh Models" }, Kind::Default, !models_loading) {
                let config = self.ai_config();
                pui.models = Some((source.clone(), Loaded::Loading));
                let s2 = source.clone();
                self.spawn(move || {
                    let r = gitcraft_core::ai_models::list(&config);
                    super::Msg::Apply(Box::new(move |app: &mut App| {
                        if let Some(p) = app.prefs.as_mut() {
                            p.models = Some((s2, match r {
                                Ok(m) => Loaded::Ready(m),
                                Err(e) => Loaded::Failed(e),
                            }));
                        }
                    }))
                });
            }
        });
        if provider != "anthropic" && !cli {
            row(ui, "Response Mode", "Automatic tries structured output, then JSON mode, then a JSON prompt only if the server rejects the format. Every mode supports every feature; answers are validated before use.", None, None, false, |ui| self.select(ui, a("responseFormat"), &[("auto", "Automatic (recommended)"), ("json_schema", "Structured output"), ("json_object", "JSON mode"), ("text", "JSON prompt only (maximum compatibility)")]));
        }
        // Settings as a value first, so equal settings always give the same text whatever order a map held its keys in.
        let signature = serde_json::to_value(&ai).and_then(|v| serde_json::to_string(&v)).unwrap_or_default();
        let (help, err) = match &pui.test {
            Some((sig, ok, text)) if *sig == signature => (text.clone(), (!ok).then_some("Connection test failed. Check the settings above and try again.")),
            _ => (if cli { "Checks your subscription sign-in and makes a short test request without repository data.".into() } else { "Tests the default model with the same connection and response mode used by AI features.".into() }, None),
        };
        row(ui, "Test Connection", &help, None, err, false, |ui| {
            let testing = self.prefs_data.busy.iter().any(|b| b == "test");
            if pbutton(ui, if testing { "Testing…" } else { "Test Connection" }, Kind::Default, !testing && ai.enabled) {
                let config = self.ai_config();
                let sig = signature.clone();
                self.prefs_task("test", move || Ok(gitcraft_core::ai::with_config(config, gitcraft_core::ai::test_connection)), move |app, r: Result<String, String>| {
                    let (ok, text) = match r {
                        Ok(t) => (true, t),
                        Err(e) => (false, e),
                    };
                    app.toast(ok, text.clone());
                    if let Some(p) = app.prefs.as_mut() {
                        p.test = Some((sig, ok, text));
                    }
                });
            }
            if testing && pbutton(ui, "Stop AI", Kind::Default, true) {
                self.stop_ai();
            }
        });
        sub(ui, "Global Instructions");
        row(ui, "Custom Instructions", "Applied to every AI feature, together with any feature instructions below.", None, None, true, |ui| self.area(ui, pui, a("instructions"), "", 3, false));
        for (id, title, help) in AI_FEATURES {
            sub(ui, title);
            row(ui, "Model Override (optional)", &format!("{help} Leave empty to use the Default Model."), None, None, true, |ui| {
                let t = Target::Setting(format!("ai.models.{id}"));
                self.model_field(ui, pui, t, &models, "Use Default Model");
            });
            row(ui, "Custom Instructions", "", None, None, true, |ui| {
                let t = Target::Setting(format!("ai.featureInstructions.{id}"));
                self.area(ui, pui, t, "", 2, false);
            });
        }
        let _ = c;
    }

    fn model_field(&mut self, ui: &mut Ui, pui: &mut PrefsUi, t: Target, models: &[gitcraft_core::ai_models::Model], fallback: &str) {
        let t = self.nested_ai(t);
        let manual_id = format!("manual:{t:?}");
        let manual = pui.buffers.contains_key(&manual_id);
        if !models.is_empty() && !manual {
            let current = self.pref(&t).as_str().unwrap_or("").to_owned();
            let mut options: Vec<(String, String)> = vec![(String::new(), fallback.to_owned())];
            options.extend(models.iter().map(|m| (m.id.clone(), if m.name == m.id { m.name.clone() } else { format!("{} ({})", m.name, m.id) })));
            if !current.is_empty() && !models.iter().any(|m| m.id == current) {
                options.push((current.clone(), format!("{current} (saved)")));
            }
            let shown = options.iter().find(|(v, _)| *v == current).map(|(_, l)| l.clone()).unwrap_or_default();
            let mut chosen = None;
            let w = ui.available_width() - 90.0;
            egui::ComboBox::from_id_salt(format!("{t:?}")).selected_text(shown).width(w).show_ui(ui, |ui| {
                for (v, l) in &options {
                    if ui.selectable_label(*v == current, l).clicked() {
                        chosen = Some(v.clone());
                    }
                }
            });
            if let Some(v) = chosen {
                self.set_pref(&t, json!(v));
            }
        } else {
            let w = ui.available_width() - if models.is_empty() { 0.0 } else { 90.0 };
            ui.allocate_ui(vec2(w, 28.0), |ui| self.text(ui, pui, t.clone(), fallback, true));
        }
        if !models.is_empty() && pbutton(ui, if manual { "Use List" } else { "Enter ID" }, Kind::Default, true) {
            if manual {
                pui.buffers.remove(&manual_id);
            } else {
                pui.buffers.insert(manual_id, String::new());
            }
        }
    }

    /// `ai.models.<id>` and `ai.featureInstructions.<id>` live inside the AI settings.
    fn nested_ai(&self, t: Target) -> Target {
        t
    }

    fn choose_provider(&mut self, provider: &str, preset: Option<&str>) {
        let mut ai: Value = serde_json::to_value(self.ai_config()).unwrap_or_else(|_| json!({}));
        let old = ai["provider"].as_str().unwrap_or("anthropic").to_owned();
        let keys = ["endpoint", "cliPath", "cliConfigDir", "defaultModel", "responseFormat", "models"];
        let mut connections = self.settings.get::<Value>("ai").and_then(|v| v.get("connections").cloned()).unwrap_or_else(|| json!({}));
        let mut current = json!({});
        for k in keys {
            current[k] = ai[k].clone();
        }
        connections[&old] = current;
        let saved = connections.get(provider).cloned().unwrap_or_else(|| json!({}));
        let defaults = json!({"endpoint": "", "cliPath": "", "cliConfigDir": "", "defaultModel": "", "responseFormat": "auto", "models": {}});
        for k in keys {
            ai[k] = saved.get(k).cloned().unwrap_or_else(|| defaults[k].clone());
        }
        if let Some(url) = preset {
            ai["endpoint"] = json!(url);
            ai["defaultModel"] = json!("");
            ai["models"] = json!({});
            ai["responseFormat"] = json!("auto");
        }
        ai["provider"] = json!(provider);
        ai["connections"] = connections;
        self.settings.set("ai", ai);
    }

    fn save_ai_key(&mut self, provider: &str, value: String) {
        let name = gitcraft_core::ai::key_name(provider).to_owned();
        let removed = value.is_empty();
        self.prefs_task("key", move || gitcraft_core::hosting::save_secret_manually(&name, value.trim()), move |app, ()| {
            app.toast(true, if removed { "Saved API key removed" } else { "Saved to the system credential store" });
            if let Some(p) = app.prefs.as_mut() {
                p.models = None;
            }
        });
    }

    fn repo_section(&mut self, ui: &mut Ui, pui: &mut PrefsUi, section: &str, path: &str) {
        let c = p();
        let rp = |k: &str| Target::Repo(path.to_owned(), k.to_owned());
        let tab = self.active;
        let dir = path.to_owned();
        match section {
            "gitflow" => {
                h(ui, "Gitflow");
                note(ui, "Gitflow keeps feature, release and hotfix work on separate branches. GitCraft runs it itself, no extra tools needed.");
                let cfg = pui.gitflow.get_or_insert_with(|| gitcraft_core::manage::gitflow_config(Path::new(&dir))).clone();
                row(ui, "Status", "", None, None, false, |ui| {
                    ui.label(egui::RichText::new(if cfg.initialized { "Initialized" } else { "Not initialized" }).size(13.0).color(if cfg.initialized { GOOD } else { c.fg3 }));
                });
                if cfg.initialized {
                    for (l, v) in [("Production branch", &cfg.master), ("Development branch", &cfg.develop), ("Feature prefix", &cfg.feature), ("Release prefix", &cfg.release), ("Hotfix prefix", &cfg.hotfix), ("Version tag prefix", &cfg.versiontag)] {
                        row(ui, l, "", None, None, false, |ui| {
                            ui.label(egui::RichText::new(if v.is_empty() { "(none)" } else { v }).monospace().size(13.0).color(c.fg));
                        });
                    }
                }
                row(ui, if cfg.initialized { "Change the settings" } else { "Initialize Gitflow" }, "", None, None, false, |ui| {
                    if pbutton(ui, if cfg.initialized { "Edit Gitflow" } else { "Initialize Gitflow" }, Kind::Primary, true) {
                        self.init_gitflow(tab);
                        pui.gitflow = None;
                    }
                });
            }
            "githooks" => {
                h(ui, "Git Hooks");
                note(ui, "Git hooks are shell scripts that execute after an event such as a commit or push. Git for Windows runs them with its own sh, so no extra setup is needed.");
                self.want_config("local", Some(dir.clone()), &["core.hooksPath"]);
                row(ui, "Git Hooks directory", "The directory that Git hooks will use for this repository and its linked worktrees. Empty = .git/hooks.", None, None, false, |ui| self.path(ui, pui, Target::Config("local".into(), Some(dir.clone()), "core.hooksPath".into()), ".git/hooks", true, "Select a directory to use"));
            }
            "lfs" => {
                h(ui, "LFS");
                let info = pui.lfs.get_or_insert_with(|| gitcraft_core::manage::lfs_info(Path::new(&dir))).clone();
                if !info.available {
                    note(ui, "Git LFS isn't installed. Git for Windows includes it; elsewhere install git-lfs.");
                    return;
                }
                row(ui, "Git LFS", &info.version, None, None, false, |ui| {
                    if info.enabled {
                        ui.label(egui::RichText::new("Initialized for this repository").size(13.0).color(GOOD));
                    } else if pbutton(ui, "Initialize LFS", Kind::Primary, true) {
                        self.act(tab, Action::LfsInstall);
                        pui.lfs = None;
                    }
                });
                row(ui, "Tracked patterns", "", None, None, true, |ui| {
                    ui.vertical(|ui| {
                        if info.patterns.is_empty() {
                            ui.label(egui::RichText::new("None").size(12.5).color(c.fg3));
                        }
                        for pat in &info.patterns {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(pat).monospace().size(12.5).color(c.fg));
                                if pbutton(ui, "Untrack", Kind::Default, true) {
                                    self.act(tab, Action::LfsUntrack { pattern: pat.clone() });
                                    pui.lfs = None;
                                }
                            });
                        }
                    });
                });
                row(ui, "Track a pattern", "", None, None, false, |ui| {
                    let buf = pui.buffers.entry("lfs_track".into()).or_default();
                    let w = ui.available_width();
                    let r = field(ui, buf, "*.psd", w, TextVisibility::Visible, FontId::monospace(12.5), "lfs_track");
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !buf.trim().is_empty() {
                        let pattern = buf.trim().to_owned();
                        pui.buffers.remove("lfs_track");
                        self.act(tab, Action::LfsTrack { pattern, lockable: false });
                        pui.lfs = None;
                    }
                });
                row(ui, "LFS files", "", None, None, false, |ui| {
                    if pbutton(ui, "Pull LFS files", Kind::Default, true) {
                        self.act(tab, Action::LfsPull);
                    }
                });
            }
            "encoding" => {
                h(ui, "Encoding");
                row(ui, "Default encoding", "Default encoding when reading or writing files in this repo.", None, None, false, |ui| self.select(ui, rp("encoding"), &ENCODINGS));
            }
            "conflict" => {
                h(ui, "Conflict Prevention");
                note(ui, "GitCraft can automatically compare your branch against its \"Base\" branch to see if any conflicts arise while you're working.");
                self.want_config("local", Some(dir.clone()), &["rerere.enabled"]);
                self.want_config("global", None, &["rerere.enabled"]);
                row(ui, "Reuse recorded conflict resolutions", "Remember how you resolve conflicts and reuse the same resolution when Git encounters it again.", None, None, false, |ui| {
                    let local = self.pref(&Target::Config("local".into(), Some(dir.clone()), "rerere.enabled".into()));
                    let global = self.pref(&Target::Config("global".into(), None, "rerere.enabled".into()));
                    let v = local.as_str().filter(|s| !s.is_empty()).or(global.as_str()).unwrap_or("").to_lowercase();
                    let mut on = matches!(v.as_str(), "true" | "yes" | "on" | "1");
                    if widgets::checkbox(ui, &mut on, "", 13.0).clicked() {
                        self.set_pref(&Target::Config("local".into(), Some(dir.clone()), "rerere.enabled".into()), json!(on.to_string()));
                    }
                });
                let global = self.settings.flag("conflictDetection", true);
                row(ui, "Enable proactive conflict detection with target branch", "Unchecking this will disable proactive conflict detection only for this repository.", (!global).then_some("Globally disabled for all repositories. To re-enable, go to Preferences > General."), None, false, |ui| {
                    let mut on = self.repo_pref(&dir, "conflictDetection").and_then(|v| v.as_bool()).unwrap_or(true);
                    if ui.add_enabled_ui(global, |ui| widgets::checkbox(ui, &mut on, "", 13.0)).inner.clicked() && global {
                        self.set_pref(&rp("conflictDetection"), json!(on));
                    }
                });
                row(ui, "Base branches", "One pattern per line. GitCraft compares your branch with the first branch that matches (the remote's default branch when empty).\norigin/main - include a single branch (main) on origin\norigin/** - include all branches on origin\norigin/feature/** - include branches on origin which begin with feature/\n!origin/abc - exclude a single branch (abc) on origin", None, None, true, |ui| self.area(ui, pui, rp("baseBranches"), "origin/main\norigin/develop", 4, true));
                row(ui, "Branches Excluded from Conflict Prevention", "No conflict warnings while one of these branches is checked out (one per line, * allowed).", None, None, true, |ui| self.area(ui, pui, rp("excludedBranches"), "", 3, true));
            }
            "issues" => {
                h(ui, "Issue Tracker");
                note(ui, "Which issues the ISSUES section shows for this repository. Connect the tracker first in Preferences > Integrations.");
                let tracker = self.repo_pref(&dir, "tracker").unwrap_or_else(|| json!({"kind": "", "project": ""}));
                let kind = tracker["kind"].as_str().unwrap_or("").to_owned();
                let project = tracker["project"].as_str().unwrap_or("").to_owned();
                row(ui, "Issue tracker", "", None, None, false, |ui| {
                    let kinds = [("", "The repository's hosting service (GitHub / GitLab / Bitbucket / Azure DevOps)"), ("jira", "Jira Cloud"), ("jira-server", "Jira Data Center"), ("trello", "Trello"), ("azure", "Azure Boards (Azure DevOps)")];
                    let shown = kinds.iter().find(|(v, _)| *v == kind).map(|(_, l)| *l).unwrap_or("");
                    let mut chosen = None;
                    let w = ui.available_width();
                    egui::ComboBox::from_id_salt("tracker").selected_text(shown).width(w - 8.0).show_ui(ui, |ui| {
                        for (v, l) in kinds {
                            if ui.selectable_label(v == kind, l).clicked() {
                                chosen = Some(v);
                            }
                        }
                    });
                    if let Some(v) = chosen {
                        self.set_pref(&rp("tracker"), json!({"kind": v, "project": project}));
                    }
                });
                if !kind.is_empty() {
                    let (label, placeholder, help) = match kind.as_str() {
                        "trello" => ("Board id", "abc123XY", "The id in the board's address (trello.com/b/<id>/...). Empty shows the cards assigned to you."),
                        "azure" => ("Project", "MyProject", "Empty shows the issues assigned to you."),
                        _ => ("Project key", "ABC", "Empty shows the issues assigned to you."),
                    };
                    row(ui, label, help, None, None, false, |ui| {
                        // One draft per repository, so one repository's text is never shown or committed in another's.
                        let id = format!("tracker_project:{dir}");
                        let buf = pui.buffers.entry(id.clone()).or_insert_with(|| project.clone());
                        let w = ui.available_width();
                        let r = field(ui, buf, placeholder, w, TextVisibility::Visible, FontId::proportional(13.0), &id);
                        if r.lost_focus() {
                            let v = buf.trim().to_owned();
                            pui.buffers.remove(&id);
                            self.set_pref(&rp("tracker"), json!({"kind": kind, "project": v}));
                        }
                    });
                }
            }
            "sparse" => {
                h(ui, "Sparse Checkout");
                note(ui, "Keep only some folders of a big repository in your working folder.");
                let info = pui.sparse.get_or_insert_with(|| gitcraft_core::manage::sparse_info(Path::new(&dir))).clone();
                row(ui, "Status", "", None, None, false, |ui| {
                    if info.enabled {
                        ui.label(egui::RichText::new(format!("On: {}", info.patterns.join(", "))).size(13.0).color(GOOD));
                    } else {
                        ui.label(egui::RichText::new("Off").size(13.0).color(c.fg3));
                    }
                });
                row(ui, "Folders", "", None, None, false, |ui| {
                    if pbutton(ui, "Choose folders…", Kind::Primary, true) {
                        self.sparse_dialog(tab);
                        pui.sparse = None;
                    }
                    if info.enabled && pbutton(ui, "Turn off", Kind::Danger, true) {
                        self.act(tab, Action::SparseDisable);
                        pui.sparse = None;
                    }
                });
            }
            "pull" => {
                h(ui, "Pull and Checkout");
                row(ui, "Pull mode", "", None, None, false, |ui| self.select(ui, rp("pullMode"), &[("", "Use the global default"), ("ff", "Fast-forward if possible"), ("ffonly", "Fast-forward only"), ("rebase", "Rebase"), ("fetch", "Fetch only")]));
                row(ui, "Autostash when rebasing a pull", "Temporarily stash tracked working changes and reapply them after the rebase. Git may report conflicts when reapplying.", None, None, false, |ui| self.select(ui, rp("pullAutostash"), &[("", "Use Git configuration"), ("true", "Enabled"), ("false", "Disabled")]));
                row(ui, "Update submodules after pull or checkout", "", None, None, false, |ui| self.select(ui, rp("pullCheckoutSubmodules"), &[("", "Use the global default"), ("true", "Initialize and update recursively"), ("false", "Disabled")]));
            }
            _ => {
                h(ui, "Submodules");
                let list = pui.submodules.get_or_insert_with(|| gitcraft_core::manage::submodules(Path::new(&dir))).clone();
                if list.is_empty() {
                    note(ui, "This repository has no submodules.");
                }
                for m in &list {
                    let state = match m.state.as_str() {
                        "in-sync" => "in sync",
                        "out-of-sync" => "out of sync",
                        "uninitialized" => "not initialized",
                        _ => "conflict",
                    };
                    row(ui, &m.path, &m.url, None, None, false, |ui| {
                        ui.label(egui::RichText::new(state).size(12.5).color(c.fg2));
                        if pbutton(ui, "Update", Kind::Default, true) {
                            self.act(tab, Action::SubmoduleUpdate { paths: vec![m.path.clone()] });
                        }
                        if pbutton(ui, "Remove", Kind::Danger, true) {
                            self.act(tab, Action::SubmoduleRemove { path: m.path.clone() });
                        }
                    });
                }
                row(ui, "Submodules", "", None, None, false, |ui| {
                    if pbutton(ui, "Add Submodule", Kind::Default, true) {
                        self.add_submodule(tab);
                    }
                    if !list.is_empty() && pbutton(ui, "Update all", Kind::Default, true) {
                        self.act(tab, Action::SubmoduleUpdate { paths: Vec::new() });
                    }
                });
            }
        }
    }

    /// Read every open repository again (a preference changed what the graph shows).
    pub(crate) fn reload_all(&mut self) {
        let tabs: Vec<u64> = self.repos.keys().copied().collect();
        for t in tabs {
            self.load_repo(t);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TextVisibility {
    Visible,
    Masked,
}

/// A one-line field in the preferences' look.
fn field(ui: &mut Ui, text: &mut String, hint: &str, width: f32, visibility: TextVisibility, font: FontId, id: &str) -> egui::Response {
    let c = p();
    let (rect, _) = ui.allocate_exact_size(vec2(width.max(40.0), 28.0), Sense::hover());
    let eid = egui::Id::NULL.with(("pref_field", id));
    let focused = ui.memory(|m| m.has_focus(eid));
    ui.painter().rect(rect, CornerRadius::same(3), c.field, Stroke::new(1.0, if focused { ACCENT } else { c.white(0.1) }), StrokeKind::Inside);
    let edit = egui::TextEdit::singleline(text).id(eid).password(visibility == TextVisibility::Masked).frame(egui::Frame::NONE).font(font).hint_text(egui::RichText::new(hint).color(c.fg3)).align(Align2::LEFT_CENTER).desired_width(rect.width() - 16.0);
    ui.put(rect.shrink2(vec2(8.0, 0.0)), edit)
}

use chrono::TimeZone;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_copilot_preferences_show_sign_in_and_models() {
        let ctx = egui::Context::default();
        let cc = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = App::new(&cc, crate::app::StartOptions::default(), crate::settings::Store::in_memory());
        app.choose_provider("copilot", None);
        let mut prefs = PrefsUi::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.ai_section(ui, &mut prefs));
        output.textures_delta.clear();
        let painted: Vec<_> = output.shapes.iter().filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text.galley.text()),
            _ => None,
        }).collect();
        assert!(painted.contains(&"Sign in with GitHub"));
        assert!(painted.contains(&"Fetch Models"));
        assert!(!painted.contains(&"Provider API Key"));
    }

    #[test]
    fn headless_preference_fields_mask_only_credentials() {
        let ctx = egui::Context::default();
        let mut ordinary = String::from("ordinary value");
        let mut credential = String::from("fixture value");
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let font = FontId::proportional(13.0);
            field(ui, &mut ordinary, "", 220.0, TextVisibility::Visible, font.clone(), "ordinary");
            field(ui, &mut credential, "", 220.0, TextVisibility::Masked, font, "masked");
        });
        output.textures_delta.clear();
        let painted: Vec<_> = output.shapes.iter().filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text.galley.text()),
            _ => None,
        }).collect();
        assert!(painted.contains(&ordinary.as_str()));
        assert!(!painted.contains(&credential.as_str()));
        let mask = "•".repeat(credential.chars().count());
        assert!(painted.contains(&mask.as_str()));
        assert_eq!(credential, "fixture value");
    }
}
