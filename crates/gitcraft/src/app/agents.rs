//! The Agents view of the left panel: each worktree with its working changes and the coding
//! sessions saved for it, and starting or resuming a session in the in-app terminal.

use super::widgets::Tip;
use super::App;
use super::ctxmenu::{self, Later, item};
use super::dialogs::Dialog;
use super::icons::{self, Icon};
use super::repo::{Loaded, RepoTab};
use super::theme::{ACCENT, BAD, GOOD, WARN, p};
use super::widgets;
use egui::{CornerRadius, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use gitcraft_core::actions::Action;
use gitcraft_core::agents::AgentSession;
use gitcraft_core::git::{WorkStatus, Worktree};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Instant;

pub const AGENT_LABEL: [(&str, &str); 6] = [("claude", "Claude Code"), ("codex", "Codex CLI"), ("copilot", "Copilot CLI"), ("opencode", "OpenCode"), ("gemini", "Gemini CLI"), ("cursor", "Cursor CLI")];

pub fn agent_label(agent: &str) -> Option<&'static str> {
    AGENT_LABEL.iter().find(|(k, _)| *k == agent).map(|(_, l)| *l)
}

/// A folder path compared the way the file system does. Backslashes separate folders only in
/// Windows paths (a drive letter, a `\\server` share, or any path on Windows); elsewhere they are
/// part of a name.
pub fn folder_key(s: &str) -> String {
    let b = s.as_bytes();
    let windows_path = cfg!(windows) || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':') || s.starts_with("\\\\");
    if !windows_path {
        return s.trim_end_matches('/').to_owned();
    }
    let p = s.replace('\\', "/");
    let p = p.strip_prefix("//?/UNC/").map(|r| format!("//{r}")).or_else(|| p.strip_prefix("//?/").map(str::to_owned)).unwrap_or(p);
    let p = p.trim_end_matches('/').to_owned();
    let drive = p.len() >= 3 && p.as_bytes()[0].is_ascii_alphabetic() && &p[1..3] == ":/";
    if drive || p.starts_with("//") { p.to_lowercase() } else { p }
}

pub fn same_folder(a: &str, b: &str) -> bool {
    folder_key(a) == folder_key(b)
}

/// The connection settings for an agent that GitCraft also uses for AI (its path and config folder).
fn connection(ai: &Value, agent: &str) -> Option<Value> {
    let provider = match agent {
        "claude" => "claude_code",
        "codex" => "codex",
        _ => return None,
    };
    if ai["provider"].as_str() == Some(provider) { Some(ai.clone()) } else { ai["connections"].get(provider).cloned() }
}

/// The command line that starts (or resumes) an agent, and the environment it needs. Paths are
/// shell-quoted; the folder is handed to the terminal, never typed as `cd`.
pub fn agent_launch(ai: &Value, shell: &str, args: &str, agent: &str, resume: Option<&str>) -> Result<(String, HashMap<String, String>), String> {
    let program = AGENT_LABEL.iter().find(|(k, _)| *k == agent).map(|(k, _)| *k).ok_or("Choose a coding agent in Preferences > External Tools.")?;
    // Cursor's CLI is installed as cursor-agent.
    let program = if program == "cursor" { "cursor-agent" } else { program };
    if let Some(id) = resume
        && (id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'))
    {
        return Err("This session has an invalid resume id.".into());
    }
    let conn = connection(ai, agent);
    let path = conn.as_ref().and_then(|c| c["cliPath"].as_str()).map(str::trim).unwrap_or("").to_owned();
    let ps = shell == "pwsh" || (cfg!(windows) && (shell.is_empty() || shell == "powershell"));
    let quote = |s: &str| -> Result<String, String> {
        if shell == "cmd" {
            if s.contains(['"', '%', '!', '^', '&', '|', '<', '>', '\r', '\n']) {
                return Err("Choose PowerShell in Terminal Preferences to use this executable path.".into());
            }
            return Ok(format!("\"{s}\""));
        }
        Ok(format!("'{}'", s.replace('\'', if ps { "''" } else { "'\\''" })))
    };
    let executable = if path.is_empty() {
        program.to_owned()
    } else if path.ends_with(".js") {
        format!("node {}", quote(&path)?)
    } else {
        format!("{}{}", if ps { "& " } else { "" }, quote(&path)?)
    };
    let args = match resume {
        Some(id) => format!("{}{id}", if agent == "claude" { "--resume " } else { "resume " }),
        None => args.trim().to_owned(),
    };
    let mut env = HashMap::new();
    if let Some(dir) = conn.as_ref().and_then(|c| c["cliConfigDir"].as_str()).map(str::trim).filter(|d| !d.is_empty()) {
        env.insert(if agent == "claude" { "CLAUDE_CONFIG_DIR" } else { "CODEX_HOME" }.to_owned(), dir.to_owned());
    }
    Ok(([executable, args].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" "), env))
}

fn session_status(a: Option<&AgentSession>) -> &'static str {
    match a.map(|a| a.status.as_str()) {
        None => "No saved sessions",
        Some("recent") => "Recent activity",
        Some("finished") => "Last turn finished",
        _ => "No recent activity",
    }
}

/// What the Agents view has read for a tab.
#[derive(Default)]
pub struct AgentsUi {
    pub sessions: Option<Loaded<Vec<AgentSession>>>,
    /// Counts the session scans, so an earlier scan's result is dropped.
    pub request: u64,
    pub read_at: Option<Instant>,
    pub key: String,
    pub statuses: HashMap<String, (Loaded<WorkStatus>, Instant)>,
    /// The status read in flight for each folder; only that read's result is kept.
    pub status_requests: HashMap<String, u64>,
    pub filter: String,
    pub search: bool,
    pub expanded: HashMap<String, bool>,
    /// How the worktree list is ordered: 0 as Git lists them, 1 by latest agent activity, 2 by branch name.
    pub sort: u8,
    /// What the agents' status hooks last reported (see `gitcraft_core::agent_status`), and when it was read.
    pub live: Vec<gitcraft_core::agent_status::AgentState>,
    pub live_at: Option<Instant>,
    /// Permission requests whose command is shown in full.
    pub expanded_requests: std::collections::HashSet<String>,
}

impl App {
    fn launch_agent(&mut self, tab: u64, path: &str, agent: &str, resume: Option<&str>, setup: &[String]) {
        let ai = self.settings.get::<Value>("ai").unwrap_or_default();
        let shell = self.settings.string("termShell", "");
        match agent_launch(&ai, &shell, &self.settings.string("agentArgs", ""), agent, resume) {
            Ok((command, env)) => {
                let sep = if shell == "cmd" { " && " } else { "; " };
                let line = setup.iter().cloned().chain([command]).collect::<Vec<_>>().join(sep);
                self.run_in_terminal(tab, agent_label(agent).unwrap_or("Agent"), &line, path, Some(env));
            }
            Err(e) => self.toast(false, e),
        }
    }

    fn start_agent(&mut self, tab: u64, worktree: bool) {
        let agent = self.settings.string("codingAgent", "claude");
        if agent_label(&agent).is_none() {
            self.open_prefs(Some("tools"));
            return;
        }
        let Some(path) = self.repo(tab).and_then(|r| r.view.as_ref()).map(|v| v.path.clone()) else { return };
        if !worktree {
            self.launch_agent(tab, &path, &agent, None, &[]);
            return;
        }
        let d = Dialog::prompt("Start an agent session in a new worktree", "New branch name", &default_agent_branch(), "Start", move |app, v| {
            let branch = v.text("value").trim().to_owned();
            app.start_agent_in_worktree(tab, branch);
        });
        self.dialog = Some(d);
    }

    /// New Agent Session in one click: a new worktree on a generated branch, with the preferred agent.
    pub(crate) fn start_agent_now(&mut self, tab: u64) {
        if agent_label(&self.settings.string("codingAgent", "claude")).is_none() {
            self.open_prefs(Some("tools"));
            return;
        }
        self.start_agent_in_worktree(tab, default_agent_branch());
    }

    /// Start an agent session for an issue: a worktree on a branch named after it.
    pub(crate) fn start_agent_for_issue(&mut self, tab: u64, number: &str, title: &str) {
        let slug: String = title.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect::<String>().split('-').filter(|p| !p.is_empty()).take(6).collect::<Vec<_>>().join("-");
        let number = number.trim_start_matches('#');
        let suggested = if slug.is_empty() { format!("issue/{number}") } else { format!("issue/{number}-{slug}") };
        let d = Dialog::prompt("Start an agent session for this issue", "New branch name", &suggested, "Start", move |app, v| {
            let branch = v.text("value").trim().to_owned();
            app.start_agent_in_worktree(tab, branch);
        });
        self.dialog = Some(d);
    }

    /// A new worktree beside the repository on `branch` (created from HEAD), the setup commands, then the agent.
    fn start_agent_in_worktree(&mut self, tab: u64, branch: String) {
        let agent = self.settings.string("codingAgent", "claude");
        let Some((path, name)) = self.repo(tab).and_then(|r| r.view.as_ref()).map(|v| (v.path.clone(), v.name.clone())) else { return };
        if branch.is_empty() {
            return;
        }
        let repo = std::path::Path::new(&path);
        let repo_name = repo.file_name().map_or_else(|| name.clone(), |n| n.to_string_lossy().into_owned());
        let safe: String = branch.chars().map(|c| if c.is_alphanumeric() || c == '_' || c == '.' || c == '-' { c } else { '-' }).collect();
        let sibling = format!("{repo_name}-{safe}");
        let folder = repo.parent().map_or_else(|| std::path::PathBuf::from(&sibling), |p| p.join(&sibling)).to_string_lossy().into_owned();
        let setup: Vec<String> = self.settings.string("agentSetup", "").lines().map(|l| l.trim().to_owned()).filter(|l| !l.is_empty()).collect();
        let engine = self.engine.clone();
        let (p2, f2) = (path.clone(), folder.clone());
        self.busy = Some("Adding worktree…".into());
        self.spawn(move || {
            let r = engine.perform(&p2, Action::WorktreeAdd { path: f2, branch, new_branch: true, start: None });
            super::Msg::Apply(Box::new(move |app: &mut App| {
                app.busy = None;
                if r.ok {
                    // The new worktree starts with this repository's view settings.
                    app.inherit_view(&path, &folder);
                    app.load_repo(tab);
                    app.launch_agent(tab, &folder, &agent, None, &setup);
                } else {
                    app.toast(false, r.message);
                }
            }))
        });
    }

    fn want_agent_sessions(&mut self, tab: u64, roots: Vec<String>, key: String) {
        let Some(path) = self.repo(tab).map(|r| r.path.clone()) else { return };
        let ai = self.settings.get::<Value>("ai").unwrap_or_default();
        let dir = |a: &str| connection(&ai, a).and_then(|c| c["cliConfigDir"].as_str().map(|d| d.trim().to_owned())).filter(|d| !d.is_empty());
        let (claude, codex) = (dir("claude"), dir("codex"));
        // Each scan has a number, and only the latest one's sessions are shown: two scans can
        // look in the same folders.
        let mut request = 0;
        if let Some(r) = self.repo_mut(tab) {
            r.agents.request += 1;
            request = r.agents.request;
            r.agents.read_at = Some(Instant::now());
            if r.agents.key != key || r.agents.sessions.is_none() {
                r.agents.sessions = Some(Loaded::Loading);
            }
            r.agents.key = key;
        }
        self.spawn(move || {
            let mut roots = roots;
            if !roots.contains(&path) {
                roots.push(path);
            }
            let list = gitcraft_core::agents::sessions(&roots, claude.as_deref(), codex.as_deref());
            super::Msg::Apply(Box::new(move |app: &mut App| {
                if let Some(r) = app.repo_mut(tab)
                    && r.agents.request == request
                {
                    r.agents.sessions = Some(Loaded::Ready(list));
                }
            }))
        });
    }

    /// Read what the agents' status hooks reported.
    pub(crate) fn want_agent_status(&mut self, tab: u64) {
        self.spawn(move || {
            let live = gitcraft_core::agent_status::read_all();
            super::Msg::Apply(Box::new(move |app: &mut App| {
                if let Some(r) = app.repo_mut(tab) {
                    r.agents.live = live;
                }
            }))
        });
    }

    pub(crate) fn want_worktree_status(&mut self, tab: u64, path: String) {
        let request = super::unique_id();
        if let Some(r) = self.repo_mut(tab) {
            if r.agents.status_requests.contains_key(&path) {
                return;
            }
            let fresh = r.agents.statuses.get(&path).map(|(l, at)| !matches!(l, Loaded::Loading) && at.elapsed().as_secs() < 10);
            if fresh == Some(true) || matches!(r.agents.statuses.get(&path), Some((Loaded::Loading, _))) {
                return;
            }
            r.agents.status_requests.insert(path.clone(), request);
            let keep = r.agents.statuses.remove(&path).map(|(l, _)| l);
            r.agents.statuses.insert(path.clone(), (keep.filter(|l| matches!(l, Loaded::Ready(_))).unwrap_or(Loaded::Loading), Instant::now()));
            if let Some((l, _)) = r.agents.statuses.get_mut(&path)
                && !matches!(l, Loaded::Ready(_))
            {
                *l = Loaded::Loading;
            }
        }
        self.spawn(move || {
            let r = gitcraft_core::git::try_status(std::path::Path::new(&path));
            super::Msg::Apply(Box::new(move |app: &mut App| {
                if let Some(t) = app.repo_mut(tab) {
                    if t.agents.status_requests.get(&path) != Some(&request) {
                        return;
                    }
                    t.agents.status_requests.remove(&path);
                    t.agents.statuses.insert(path, (match r {
                        Ok(s) => Loaded::Ready(s),
                        Err(e) => Loaded::Failed(e),
                    }, Instant::now()));
                }
            }))
        });
    }

    pub(crate) fn agents_view(&self, ui: &mut Ui, tab: u64, r: &mut RepoTab, later: &mut Vec<Later>) {
        let c = p();
        let Some(v) = r.view.as_ref() else {
            ui.add_space(8.0);
            ui.label(RichText::new("Open a repository to see its worktrees and saved coding sessions.").size(12.0).color(c.fg3));
            return;
        };
        let mut worktrees: Vec<Worktree> = v.data.worktrees.clone();
        if !worktrees.iter().any(|w| same_folder(&w.path, &v.path)) {
            worktrees.insert(0, Worktree { path: v.path.clone(), branch: v.data.head_branch.clone().unwrap_or_default(), locked: false, prunable: false });
        }
        let roots: Vec<String> = worktrees.iter().map(|w| w.path.clone()).collect();
        let key = roots.join("\n");
        let au = &mut r.agents;
        if au.key != key || au.read_at.is_none_or(|t| t.elapsed().as_secs() >= 10) {
            let (roots, key) = (roots.clone(), key.clone());
            au.read_at = Some(Instant::now());
            later.push(Box::new(move |app| app.want_agent_sessions(tab, roots, key)));
        }
        // Live status from the agents' hooks, read every two seconds.
        if au.live_at.is_none_or(|t| t.elapsed().as_secs() >= 2) {
            au.live_at = Some(Instant::now());
            later.push(Box::new(move |app| app.want_agent_status(tab)));
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(if au.live.is_empty() { 10 } else { 2 }));
        let sessions: Vec<AgentSession> = match &au.sessions {
            Some(Loaded::Ready(l)) => l.clone(),
            _ => Vec::new(),
        };
        let mut grouped: HashMap<String, Vec<AgentSession>> = HashMap::new();
        for s in sessions {
            grouped.entry(folder_key(&s.path)).or_default().push(s);
        }
        let agent = self.settings.string("codingAgent", "claude");
        let agent_name = agent_label(&agent);
        // The same text the session rows are matched by: titles, ids and agents.
        let filter = au.filter.trim().to_lowercase();
        let visible: Vec<&Worktree> = worktrees
            .iter()
            .filter(|w| {
                let titles = grouped.get(&folder_key(&w.path)).map(|g| g.iter().map(|a| format!("{} {} {}", a.title, a.id, a.agent)).collect::<Vec<_>>().join(" ")).unwrap_or_default();
                format!("{} {} {titles}", w.path, w.branch).to_lowercase().contains(&filter)
            })
            .collect();
        let mut visible = visible;
        let latest = |w: &Worktree| grouped.get(&folder_key(&w.path)).and_then(|g| g.iter().map(|a| a.modified).max()).unwrap_or(i64::MIN);
        match au.sort {
            1 => visible.sort_by_key(|w| std::cmp::Reverse(latest(w))),
            2 => visible.sort_by_key(|w| w.branch.to_lowercase()),
            _ => {}
        }
        let agent_items = || -> Vec<ctxmenu::Entry> {
            let mut out = vec![ctxmenu::Entry::Heading("Coding agent".into())];
            for (k, name) in AGENT_LABEL {
                let mark = if agent == k { "✓ " } else { "" };
                out.push(item(format!("{mark}{name}"), move |app| app.settings.set("codingAgent", k)));
            }
            out
        };
        egui::ScrollArea::vertical().id_salt(("agents", tab)).auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin { left: 12, right: 12, top: 8, bottom: 12 }).show(ui, |ui| {
                let w = ui.available_width();
                ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                ui.horizontal(|ui| {
                    ui.set_height(32.0);
                    ui.label(RichText::new("Worktrees").font(super::fonts::semibold(14.0)).color(c.fg));
                    ui.label(RichText::new(worktrees.len().to_string()).size(14.0).color(c.link));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::icon_button(ui, Icon::Search, 15.0, "Filter worktrees and sessions").clicked() {
                            au.search = !au.search;
                            au.filter.clear();
                        }
                        let sort_tip = ["Sort: as Git lists them", "Sort: latest agent activity first", "Sort: by branch name"][usize::from(au.sort.min(2))];
                        if widgets::icon_button(ui, Icon::ArrowDownAZ, 15.0, sort_tip).clicked() {
                            au.sort = (au.sort + 1) % 3;
                        }
                        let (pr, plug) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                        ui.painter().rect(pr, CornerRadius::same(3), ACCENT.gamma_multiply(0.2), Stroke::new(1.0, ACCENT.gamma_multiply(0.7)), StrokeKind::Inside);
                        icons::paint(ui, Rect::from_center_size(pr.center(), Vec2::splat(15.0)), Icon::Plug, 15.0, c.white(1.0));
                        let plug = plug.tip("Choose coding agent");
                        egui::Popup::menu(&plug).show(|ui| {
                            let mut entries = agent_items();
                            entries.push(ctxmenu::Entry::Separator);
                            entries.push(item("Agent preferences", |app| app.open_prefs(Some("tools"))));
                            ctxmenu::show(ui, entries, later);
                        });
                    });
                });
                if au.search {
                    let resp = widgets::field(ui, &mut au.filter, "Filter worktrees and sessions", w, 26.0, 12.0);
                    if !resp.has_focus() && au.filter.is_empty() && ui.memory(|m| m.focused().is_none()) {
                        resp.request_focus();
                    }
                }
                // The split start button.
                let (rect, _) = ui.allocate_exact_size(vec2(w, 36.0), Sense::hover());
                ui.painter().rect_stroke(rect, CornerRadius::same(3), Stroke::new(1.0, c.white(0.35)), StrokeKind::Inside);
                let main = Rect::from_min_max(rect.min, pos2(rect.right() - 30.0, rect.bottom()));
                let more = Rect::from_min_max(pos2(rect.right() - 30.0, rect.top()), rect.max);
                let mr = ui.interact(main, ui.scope_id().with(("agent_start", tab)), Sense::click()).tip("Start in this repository's terminal");
                let mo = ui.interact(more, ui.scope_id().with(("agent_more", tab)), Sense::click()).tip("More agent options");
                for (r, resp) in [(main, &mr), (more, &mo)] {
                    if resp.hovered() {
                        ui.painter().rect_filled(r.shrink(1.0), CornerRadius::same(2), c.white(0.1));
                    }
                }
                ui.painter().line_segment([more.left_top(), more.left_bottom()], Stroke::new(1.0, c.white(0.25)));
                let label = agent_name.map_or("Choose a coding agent".to_owned(), |n| format!("Start {n} Session"));
                let g = widgets::fit(ui, &label, egui::FontId::proportional(13.0), c.fg, main.width() - 40.0);
                let total = g.size().x + 20.0;
                let x0 = main.center().x - total / 2.0;
                icons::paint(ui, Rect::from_min_size(pos2(x0, main.center().y - 7.0), Vec2::splat(14.0)), Icon::Plus, 14.0, c.fg);
                ui.painter().galley(pos2(x0 + 20.0, main.center().y - g.size().y / 2.0), g, c.fg);
                icons::paint(ui, Rect::from_center_size(more.center(), Vec2::splat(15.0)), Icon::ChevronDown, 15.0, c.fg);
                if mr.clicked() {
                    later.push(Box::new(move |app| app.start_agent(tab, false)));
                }
                egui::Popup::menu(&mo).show(|ui| {
                    let mut entries = vec![
                        item("Start in a new worktree now", move |app| app.start_agent_now(tab)),
                        item("Start in new worktree…", move |app| app.start_agent(tab, true)),
                    ];
                    entries.extend(agent_items());
                    ctxmenu::show(ui, entries, later);
                });
                ui.add_space(4.0);
                match &au.sessions {
                    Some(Loaded::Loading) | None => {
                        ui.label(RichText::new("Finding saved sessions…").size(12.0).color(c.fg2));
                    }
                    Some(Loaded::Failed(_)) => {
                        ui.label(RichText::new("Could not read saved sessions.").size(12.0).color(BAD));
                    }
                    _ => {}
                }
                for wt in &visible {
                    let list = grouped.get(&folder_key(&wt.path)).cloned().unwrap_or_default();
                    self.agent_worktree(ui, tab, v, wt, &list, au, &agent, later);
                }
                if visible.is_empty() {
                    ui.label(RichText::new("No matching worktrees or sessions.").size(12.0).color(c.fg2));
                }
                ui.add_space(4.0);
                let mut foot = "Codex CLI and Claude Code session history".to_owned();
                if let Some(n) = agent_name.filter(|_| !["claude", "codex"].contains(&agent.as_str())) {
                    foot.push_str(&format!(" · {n} runs in the terminal."));
                }
                ui.add(egui::Label::new(RichText::new(foot).size(11.0).color(c.fg3)).wrap());
            });
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn agent_worktree(&self, ui: &mut Ui, tab: u64, v: &gitcraft_core::session::RepoView, wt: &Worktree, sessions: &[AgentSession], au: &mut AgentsUi, agent: &str, later: &mut Vec<Later>) {
        let c = p();
        let current = same_folder(&wt.path, &v.path);
        let status: Option<Result<WorkStatus, ()>> = if current {
            Some(Ok(v.data.status.clone()))
        } else {
            match au.statuses.get(&wt.path) {
                Some((Loaded::Ready(s), at)) => {
                    if at.elapsed().as_secs() >= 10 {
                        let p = wt.path.clone();
                        later.push(Box::new(move |app| app.want_worktree_status(tab, p)));
                    }
                    Some(Ok(s.clone()))
                }
                // Tried again after the same pause as a status that was read.
                Some((Loaded::Failed(_), at)) => {
                    if at.elapsed().as_secs() >= 10 {
                        let p = wt.path.clone();
                        later.push(Box::new(move |app| app.want_worktree_status(tab, p)));
                    }
                    Some(Err(()))
                }
                Some((Loaded::Loading, _)) => None,
                None => {
                    let p = wt.path.clone();
                    later.push(Box::new(move |app| app.want_worktree_status(tab, p)));
                    None
                }
            }
        };
        let filter = au.filter.trim().to_lowercase();
        let matching: Vec<&AgentSession> = if filter.is_empty() { sessions.iter().collect() } else { sessions.iter().filter(|a| format!("{} {} {} {} {}", a.title, a.id, a.agent, wt.path, wt.branch).to_lowercase().contains(&filter)).collect() };
        let expanded = au.expanded.get(&wt.path).copied().unwrap_or(false);
        let shown: Vec<&AgentSession> = if expanded || !filter.is_empty() { matching.clone() } else { matching.iter().take(5).copied().collect() };
        let open = {
            let path = wt.path.clone();
            move |app: &mut App| {
                if current {
                    if let Some(r) = app.repo_mut(tab) {
                        r.ui.select(super::repo::Selection::Wip);
                        r.ui.conflict = None;
                        r.ui.pr = None;
                        r.ui.issue = None;
                    }
                } else {
                    app.open_repo(&path);
                }
            }
        };
        let w = ui.available_width();
        egui::Frame::new().stroke(Stroke::new(1.0, if current { ACCENT.gamma_multiply(0.8) } else { c.white(0.2) })).corner_radius(CornerRadius::same(3)).show(ui, |ui| {
            ui.set_width(w);
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            egui::Frame::new().fill(if current { c.white(0.1) } else { c.white(0.05) }).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
                ui.set_width(w - 20.0);
                ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                ui.horizontal(|ui| {
                    if current {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
                        icons::paint(ui, r, Icon::House, 15.0, c.fg2);
                    }
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(13.0), Sense::hover());
                    icons::paint(ui, r, Icon::GitBranch, 13.0, c.fg2);
                    let name = if wt.branch.is_empty() { "(detached HEAD)" } else { wt.branch.as_str() };
                    let resp = ui.add(egui::Label::new(RichText::new(name).font(super::fonts::semibold(14.0)).color(c.white(0.9))).truncate().sense(Sense::click())).tip(if current { "View working changes" } else { "Open worktree" });
                    if resp.clicked() {
                        later.push(Box::new(open.clone()));
                    }
                    self.merged_pill(ui, tab, Some(&wt.branch), None, later);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let dots = widgets::icon_button(ui, Icon::EllipsisVertical, 14.0, "Worktree actions");
                        egui::Popup::menu(&dots).show(|ui| {
                            let (p1, p2, a) = (wt.path.clone(), wt.path.clone(), agent.to_owned());
                            let label = format!("Start {} here", agent_label(agent).unwrap_or("agent"));
                            let p3 = wt.path.clone();
                            let show = item("Show agent terminal", move |app| {
                                if !app.show_terminal_in(tab, &p3) {
                                    app.toast(false, "No terminal is running in this worktree.");
                                }
                            });
                            ctxmenu::show(ui, vec![item(if current { "View Changes" } else { "Open worktree" }, open.clone()), item(label, move |app| app.launch_agent(tab, &p1, &a, None, &[])), show, item("Copy worktree path", move |app| app.copy(&p2))], later);
                        });
                    });
                });
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(13.0), Sense::hover());
                    icons::paint(ui, r, Icon::Folder, 13.0, c.fg2);
                    ui.add(egui::Label::new(RichText::new(super::folder_name(&wt.path)).size(12.0).color(c.fg2)).truncate()).tip(&wt.path);
                    if wt.locked {
                        ui.label(RichText::new("Locked").size(10.0).color(c.fg2));
                    }
                });
                let resp = ui
                    .horizontal(|ui| match &status {
                        Some(Ok(s)) => {
                            let (total, added, deleted, modified) = super::right::working_changes(s);
                            if added > 0 {
                                ui.label(RichText::new(format!("+{added}")).size(12.0).color(GOOD));
                            }
                            if modified > 0 {
                                ui.label(RichText::new(format!("✎{modified}")).size(12.0).color(WARN));
                            }
                            if deleted > 0 {
                                ui.label(RichText::new(format!("−{deleted}")).size(12.0).color(BAD));
                            }
                            if total == 0 {
                                ui.label(RichText::new("No file changes").size(12.0).color(c.fg3));
                            }
                        }
                        Some(Err(())) => {
                            ui.label(RichText::new("Changes unavailable").size(12.0).color(c.fg3));
                        }
                        None => {
                            ui.label(RichText::new("Loading changes…").size(12.0).color(c.fg3));
                        }
                    })
                    .response
                    .interact(Sense::click())
                    .tip("View working changes");
                if resp.clicked() {
                    later.push(Box::new(open.clone()));
                }
            });
            // What the agents running here report: their state, and a permission request to answer.
            let here: Vec<gitcraft_core::agent_status::AgentState> = au.live.iter().filter(|s| same_folder(&s.cwd, &wt.path) || std::path::Path::new(&s.cwd).starts_with(&wt.path)).cloned().collect();
            for state in &here {
                let (label, color) = match state.state.as_str() {
                    "running" => ("Running", ACCENT),
                    "waiting" => ("Waiting for you", WARN),
                    "permission" => ("Asking permission", BAD),
                    _ => ("Done", GOOD),
                };
                ui.horizontal_wrapped(|ui| {
                    egui::Frame::new().fill(color.gamma_multiply(0.2)).stroke(Stroke::new(1.0, color)).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::symmetric(7, 1)).show(ui, |ui| {
                        ui.label(RichText::new(format!("{} · {label}", agent_label(&state.agent).unwrap_or("Agent"))).size(11.5).color(c.white(0.95)));
                    });
                    if !state.message.is_empty() && state.request.is_none() {
                        ui.add(egui::Label::new(RichText::new(&state.message).size(11.5).color(c.fg2)).truncate());
                    }
                });
                if let Some(req) = &state.request {
                    let open_cmd = au.expanded_requests.contains(&req.id);
                    ui.horizontal(|ui| {
                        let (cr, cresp) = ui.allocate_exact_size(vec2(14.0, 18.0), Sense::click());
                        widgets::chevron(ui, cr, open_cmd, 12.0, c.fg2);
                        let head = ui.add(egui::Label::new(RichText::new(format!("Wants to use {}", req.tool)).size(12.0).color(c.white(0.9))).sense(Sense::click()));
                        if cresp.clicked() || head.clicked() {
                            if open_cmd { au.expanded_requests.remove(&req.id) } else { au.expanded_requests.insert(req.id.clone()) };
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            for (allow, text, kind) in [(false, "Deny", widgets::Kind::Danger), (true, "Allow", widgets::Kind::Success)] {
                                if widgets::small(ui, kind, text).clicked() {
                                    let (session, id) = (state.session.clone(), req.id.clone());
                                    later.push(Box::new(move |app| {
                                        if let Err(e) = gitcraft_core::agent_status::answer(&session, &id, allow) {
                                            app.toast(false, e);
                                        }
                                    }));
                                }
                            }
                        });
                    });
                    if open_cmd {
                        egui::Frame::new().fill(c.field).corner_radius(CornerRadius::same(3)).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
                            ui.add(egui::Label::new(RichText::new(&req.detail).monospace().size(11.5).color(c.fg)).wrap());
                        });
                    }
                }
            }
            // The last activity line.
            let (row, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::hover());
            ui.painter().line_segment([row.left_top(), row.right_top()], Stroke::new(1.0, c.white(0.05)));
            let dot = if sessions.first().is_some_and(|a| a.status == "recent") { GOOD } else { c.white(0.4) };
            ui.painter().circle_filled(pos2(row.left() + 14.0, row.center().y), 3.5, dot);
            ui.painter().text(pos2(row.left() + 24.0, row.center().y), egui::Align2::LEFT_CENTER, session_status(sessions.first()), egui::FontId::proportional(12.0), c.fg2);
            let n = sessions.len();
            ui.painter().text(pos2(row.right() - 10.0, row.center().y), egui::Align2::RIGHT_CENTER, format!("{n} session{}", if n == 1 { "" } else { "s" }), egui::FontId::proportional(11.0), c.fg3);
            resp.tip("Status reflects saved activity, not a live process check.");
            let now = chrono::Local::now().timestamp();
            for a in &shown {
                let (row, resp) = ui.allocate_exact_size(vec2(w, 48.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(row, CornerRadius::ZERO, c.hoverrow);
                }
                ui.painter().line_segment([row.left_top(), row.right_top()], Stroke::new(1.0, c.white(0.05)));
                icons::paint(ui, Rect::from_min_size(pos2(row.left() + 10.0, row.top() + 9.0), Vec2::splat(13.0)), Icon::Bot, 13.0, c.fg2);
                let resume_rect = Rect::from_min_size(pos2(row.right() - 54.0, row.top() + 6.0), vec2(46.0, 18.0));
                let title = widgets::fit(ui, &a.title, egui::FontId::proportional(12.0), c.white(0.9), resume_rect.left() - row.left() - 34.0);
                ui.painter().galley(pos2(row.left() + 28.0, row.top() + 15.5 - title.size().y / 2.0), title, c.white(0.9));
                let rr = ui.interact(resume_rect, ui.scope_id().with(("resume", &a.agent, &a.id)), Sense::click());
                ui.painter().text(resume_rect.right_center(), egui::Align2::RIGHT_CENTER, "Resume", egui::FontId::proportional(11.0), c.link);
                if rr.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                let (time, color) = super::history::time_since(a.modified, now);
                let mut job = egui::text::LayoutJob::default();
                let small = |color| egui::TextFormat::simple(egui::FontId::proportional(10.5), color);
                job.append(&format!("{} · {} · ", agent_label(&a.agent).unwrap_or(&a.agent), session_status(Some(a))), 0.0, small(c.fg3));
                job.append(&time, 0.0, small(color));
                let g = ui.fonts_mut(|f| f.layout_job(job));
                ui.painter().galley(pos2(row.left() + 10.0, row.top() + 28.0), g, c.fg3);
                let (path, agent_id, id) = (wt.path.clone(), a.agent.clone(), a.id.clone());
                if rr.clicked() {
                    let (path, agent_id, id) = (path.clone(), agent_id.clone(), id.clone());
                    later.push(Box::new(move |app| app.launch_agent(tab, &path, &agent_id, Some(&id), &[])));
                }
                let resp = resp.tip(&a.title);
                ctxmenu::attach(&resp, later, || {
                    let cmd = format!("{}{id}", if agent_id == "claude" { "claude --resume " } else { "codex resume " });
                    let id2 = id.clone();
                    vec![item("Resume session", move |app| app.launch_agent(tab, &path, &agent_id, Some(&id), &[])), item("Copy resume command", move |app| app.copy(&cmd)), item("Copy session id", move |app| app.copy(&id2))]
                });
            }
            if matching.len() > 5 {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.set_height(30.0);
                    if widgets::link(ui, &if expanded { "Show recent sessions".to_owned() } else { format!("Browse all {} sessions", matching.len()) }, 12.0).clicked() {
                        au.expanded.insert(wt.path.clone(), !expanded);
                    }
                });
            }
        });
        ui.add_space(8.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_compare_like_the_file_system() {
        assert!(same_folder("C:\\Repo\\", "c:/repo"));
        assert!(same_folder("\\\\?\\C:\\Repo", "c:/repo"));
        assert!(!same_folder("/home/a/Repo", "/home/a/repo"));
        // Outside Windows a backslash is part of a folder's name.
        if !cfg!(windows) {
            assert!(!same_folder("/home/a/x\\y", "/home/a/x/y"));
            assert!(same_folder("/home/a/repo/", "/home/a/repo"));
        }
    }

    #[test]
    fn agent_commands_are_quoted() {
        let ai = serde_json::json!({"provider": "codex", "cliPath": "C:\\Program Files\\O'Brien\\codex.exe", "cliConfigDir": "C:\\Agent Config"});
        let (cmd, env) = agent_launch(&ai, "pwsh", "", "codex", Some("id")).unwrap();
        assert_eq!(cmd, "& 'C:\\Program Files\\O''Brien\\codex.exe' resume id");
        assert_eq!(env.get("CODEX_HOME").map(String::as_str), Some("C:\\Agent Config"));
        let (cmd, _) = agent_launch(&serde_json::json!({}), "bash", " --flag ", "claude", None).unwrap();
        assert_eq!(cmd, "claude --flag");
        assert!(agent_launch(&serde_json::json!({}), "bash", "", "claude", Some("x; rm")).is_err());
    }
}

/// A branch name for a new agent session: `agent/` and the time.
fn default_agent_branch() -> String {
    format!("agent/{}", chrono::Utc::now().format("%Y%m%dT%H%M%S"))
}
