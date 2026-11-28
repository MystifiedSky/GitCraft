//! Every command GitCraft offers, in one place: the command palette lists them, the keyboard
//! handler runs them, and the Keyboard Shortcuts dialog changes their keys. Keys are saved as
//! text ("Ctrl+Shift+S"; Ctrl means ⌘ on a Mac for the defaults).

use super::widgets::Tip;
use super::App;
use std::collections::HashMap;
use super::icons::{self, Icon};
use super::repo::Selection;
use super::theme::{ACCENT, WARN, p};
use super::widgets;
use craft_keymap::Chord;
use egui::{Align2, Color32, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use gitcraft_core::actions::Action;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Repo,
    Navigation,
    Palette,
    Ui,
}

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::Repo => "Repo Actions",
            Group::Navigation => "Navigation",
            Group::Palette => "Command Palette",
            Group::Ui => "UI",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Picker {
    Root,
    OpenRepo,
    Checkout,
    History,
    Blame,
    Theme,
    Profile,
    Workspace,
}

pub struct Command {
    pub id: &'static str,
    pub label: &'static str,
    pub group: Group,
    pub keys: &'static [&'static str],
    /// macOS keys when they differ from the defaults with Ctrl as ⌘.
    pub mac: Option<&'static [&'static str]>,
    pub keywords: &'static str,
    /// Needs an open repository.
    pub repo: bool,
    /// Also runs while typing in a text box.
    pub in_inputs: bool,
    pub picker: Option<Picker>,
}

const fn c(id: &'static str, label: &'static str, group: Group, keys: &'static [&'static str], repo: bool) -> Command {
    Command { id, label, group, keys, mac: None, keywords: "", repo, in_inputs: false, picker: None }
}

const fn inputs(mut cmd: Command) -> Command {
    cmd.in_inputs = true;
    cmd
}

const fn words(mut cmd: Command, keywords: &'static str) -> Command {
    cmd.keywords = keywords;
    cmd
}

const fn picker(mut cmd: Command, p: Picker) -> Command {
    cmd.picker = Some(p);
    cmd
}

const fn mac(mut cmd: Command, keys: &'static [&'static str]) -> Command {
    cmd.mac = Some(keys);
    cmd
}

use Group::{Navigation as N, Palette as P, Repo as R, Ui as U};

pub const COMMANDS: &[Command] = &[
    c("repo:fetch", "Fetch All", R, &["Ctrl+L"], true),
    c("repo:pull", "Pull", R, &[], true),
    c("repo:push", "Push", R, &[], true),
    c("repo:createBranch", "Create Branch", R, &["Ctrl+B"], true),
    words(picker(c("repo:checkout", "Checkout", R, &[], true), Picker::Checkout), "switch branch"),
    c("repo:createTag", "Create Tag", R, &[], true),
    c("repo:stash", "Stash", R, &[], true),
    c("repo:stash-keep-index", "Stash and Keep Staged Changes", R, &[], true),
    c("repo:stash-untracked", "Stash Untracked Files Only", R, &[], true),
    c("repo:pop", "Pop Stash", R, &[], true),
    c("repo:stageAll", "Stage all changes", R, &["Ctrl+Shift+S"], true),
    c("repo:unstageAll", "Unstage all changes", R, &["Ctrl+Shift+U"], true),
    words(c("repo:discardAll", "Discard all changes", R, &[], true), "reset hard all"),
    c("repo:undo", "Undo", R, &["Ctrl+Z"], true),
    c("repo:redo", "Redo", R, &["Ctrl+Y", "Ctrl+Shift+Z"], true),
    words(inputs(c("repo:focusCommit", "Focus commit message", R, &["Ctrl+Shift+M"], true)), "view working directory changes wip"),
    c("repo:searchCommits", "Search Commits", R, &["Ctrl+F"], true),
    words(picker(c("repo:history", "History", R, &["Ctrl+Shift+H"], true), Picker::History), "file history"),
    picker(c("repo:blame", "Blame", R, &[], true), Picker::Blame),
    c("repo:openInEditor", "Open Repo in VS Code", R, &["Ctrl+Shift+E"], true),
    words(c("repo:terminal", "Open External Terminal", R, &["Alt+T"], true), "command line console"),
    words(c("repo:fileManager", "Open in File Manager", R, &["Alt+O"], true), "folder explorer finder"),
    c("repo:externalDiff", "Open in External Diff/Merge Tool", R, &["Ctrl+D"], true),
    c("repo:gitflow", "Configure Gitflow", R, &[], true),
    words(c("repo:lfs", "Initialize LFS on this repo", R, &[], true), "configure lfs"),
    words(c("repo:signing", "Configure Commit Signing", R, &[], false), "gpg ssh accounts identity"),
    c("repo:addRemote", "Add Remote", R, &[], true),
    c("repo:setUpstream", "Set Upstream", R, &[], true),
    c("repo:addWorktree", "Add a new worktree", R, &[], true),
    c("repo:addSubmodule", "Add Submodule", R, &[], true),
    c("repo:rangeDiff", "Compare commit ranges", R, &[], true),
    c("repo:maintenance", "Repository maintenance", R, &[], true),
    c("repo:fetchHistory", "Fetch more repository history", R, &[], true),
    c("repo:reflog", "Recover commits from reflog", R, &[], true),
    c("repo:cleanup", "Clean Up Local Branches", R, &[], true),
    words(c("ui:integrations", "Integrations", U, &[], false), "github gitlab bitbucket azure jira trello connect"),
    c("repo:sparse", "Sparse Checkout", R, &[], true),
    inputs(c("nav:newTab", "New Tab", N, &["Ctrl+T"], false)),
    inputs(c("nav:closeTab", "Close Tab", N, &["Ctrl+W"], false)),
    inputs(c("nav:reopenTab", "Reopen Closed Tab", N, &["Ctrl+Shift+T"], false)),
    inputs(c("nav:tabsList", "Open Tabs List", N, &["Ctrl+Shift+A"], false)),
    mac(inputs(c("nav:nextTab", "Jump to the next open tab", N, &["Ctrl+Tab"], false)), &["Ctrl+Tab", "Meta+Shift+]"]),
    mac(inputs(c("nav:prevTab", "Jump to the previous open tab", N, &["Ctrl+Shift+Tab"], false)), &["Ctrl+Shift+Tab", "Meta+Shift+["]),
    words(inputs(c("nav:moveTabLeft", "Move Tab Left", N, &["Ctrl+Shift+PageUp"], false)), "reorder rearrange"),
    words(inputs(c("nav:moveTabRight", "Move Tab Right", N, &["Ctrl+Shift+PageDown"], false)), "reorder rearrange"),
    inputs(c("nav:tab1", "Select Tab 1", N, &["Ctrl+1"], false)),
    inputs(c("nav:tab2", "Select Tab 2", N, &["Ctrl+2"], false)),
    inputs(c("nav:tab3", "Select Tab 3", N, &["Ctrl+3"], false)),
    inputs(c("nav:tab4", "Select Tab 4", N, &["Ctrl+4"], false)),
    inputs(c("nav:tab5", "Select Tab 5", N, &["Ctrl+5"], false)),
    inputs(c("nav:tab6", "Select Tab 6", N, &["Ctrl+6"], false)),
    inputs(c("nav:tab7", "Select Tab 7", N, &["Ctrl+7"], false)),
    inputs(c("nav:tab8", "Select Tab 8", N, &["Ctrl+8"], false)),
    inputs(c("nav:tab9", "Select Tab 9", N, &["Ctrl+9"], false)),
    c("nav:openRepo", "Open Repo", N, &["Ctrl+O"], false),
    picker(c("nav:openRepoPalette", "Search for a repository to open", N, &["Ctrl+Shift+O"], false), Picker::OpenRepo),
    c("nav:clone", "Clone Repo", N, &["Ctrl+N"], false),
    words(c("nav:init", "Init Repo", N, &["Ctrl+I"], false), "initialize create"),
    picker(c("nav:workspaces", "Open Workspaces", N, &[], false), Picker::Workspace),
    words(c("nav:repoManagement", "Open Repo Management", N, &["Ctrl+Alt+O"], false), "repositories favorites recent fetch all pull all"),
    c("nav:favorite1", "Open Favorite Repo 1", N, &[], false),
    c("nav:favorite2", "Open Favorite Repo 2", N, &[], false),
    c("nav:favorite3", "Open Favorite Repo 3", N, &[], false),
    c("nav:favorite4", "Open Favorite Repo 4", N, &[], false),
    c("nav:favorite5", "Open Favorite Repo 5", N, &[], false),
    c("nav:favorite6", "Open Favorite Repo 6", N, &[], false),
    c("nav:favorite7", "Open Favorite Repo 7", N, &[], false),
    c("nav:favorite8", "Open Favorite Repo 8", N, &[], false),
    c("nav:favorite9", "Open Favorite Repo 9", N, &[], false),
    inputs(c("palette:toggle", "Toggle Command Palette", P, &["Ctrl+Shift+P"], false)),
    c("ui:leftPanel", "Toggle Left Panel", U, &["Ctrl+J"], false),
    c("ui:detailPanel", "Toggle Commit Details Panel", U, &["Ctrl+K"], false),
    mac(words(inputs(c("ui:terminalPanel", "Toggle Terminal Panel", U, &["Ctrl+`"], false)), "in-app terminal command line console"), &["Ctrl+`"]),
    mac(inputs(c("term:new", "New Terminal Tab", U, &["Ctrl+Shift+N"], false)), &["Meta+Alt+N"]),
    mac(inputs(c("term:close", "Close Terminal Tab", U, &["Ctrl+Shift+W"], false)), &["Meta+Alt+W"]),
    mac(inputs(c("term:next", "Next Terminal Tab", U, &["Ctrl+PageDown"], false)), &["Meta+Alt+ArrowRight"]),
    mac(inputs(c("term:prev", "Previous Terminal Tab", U, &["Ctrl+PageUp"], false)), &["Meta+Alt+ArrowLeft"]),
    mac(inputs(c("term:tab1", "Select Terminal Tab 1", U, &["Ctrl+Shift+1"], false)), &["Meta+Alt+1"]),
    mac(inputs(c("term:tab2", "Select Terminal Tab 2", U, &["Ctrl+Shift+2"], false)), &["Meta+Alt+2"]),
    mac(inputs(c("term:tab3", "Select Terminal Tab 3", U, &["Ctrl+Shift+3"], false)), &["Meta+Alt+3"]),
    mac(inputs(c("term:tab4", "Select Terminal Tab 4", U, &["Ctrl+Shift+4"], false)), &["Meta+Alt+4"]),
    mac(inputs(c("term:tab5", "Select Terminal Tab 5", U, &["Ctrl+Shift+5"], false)), &["Meta+Alt+5"]),
    mac(inputs(c("term:tab6", "Select Terminal Tab 6", U, &["Ctrl+Shift+6"], false)), &["Meta+Alt+6"]),
    mac(inputs(c("term:tab7", "Select Terminal Tab 7", U, &["Ctrl+Shift+7"], false)), &["Meta+Alt+7"]),
    mac(inputs(c("term:tab8", "Select Terminal Tab 8", U, &["Ctrl+Shift+8"], false)), &["Meta+Alt+8"]),
    mac(inputs(c("term:tab9", "Select Terminal Tab 9", U, &["Ctrl+Shift+9"], false)), &["Meta+Alt+9"]),
    inputs(c("ui:zoomIn", "Increase Zoom", U, &["Ctrl+="], false)),
    inputs(c("ui:zoomOut", "Decrease Zoom", U, &["Ctrl+-"], false)),
    inputs(c("ui:zoomReset", "Reset Zoom", U, &["Ctrl+0"], false)),
    c("ui:toolbarLabels", "Toggle Toolbar Labels", U, &[], false),
    c("ui:menusInTitleBar", "Toggle Menus in Title Bar", U, &[], false),
    picker(c("ui:theme", "Switch Theme", U, &[], false), Picker::Theme),
    picker(c("ui:profile", "Switch to Profile", U, &[], false), Picker::Profile),
    c("ui:shortcuts", "Keyboard Shortcuts", U, &["Ctrl+/"], false),
    words(c("ui:settings", "Settings", U, &["Ctrl+,"], false), "preferences options"),
    words(c("ui:activityLog", "Activity Log", U, &[], false), "logs"),
    inputs(c("ui:filterLeft", "Filter the left panel", U, &["Ctrl+Alt+F"], false)),
    inputs(c("ui:fullscreen", "Toggle Full Screen", U, &["F11"], false)),
];

pub fn command(id: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|c| c.id == id)
}

/// A shortcut needs Ctrl, ⌘ or Alt, or is an F key.
pub fn is_shortcut(combo: &str) -> bool {
    combo.starts_with("Ctrl+") || combo.starts_with("Meta+") || combo.starts_with("Alt+") || (combo.starts_with('F') && combo[1..].parse::<u8>().is_ok())
}

/// "⌘" or "Win" for Meta.
pub fn label(key: &str) -> String {
    key.split('+').map(|part| if part == "Meta" { if cfg!(target_os = "macos") { "⌘" } else { "Win" } } else { part }).collect::<Vec<_>>().join("+")
}

/// The key combination of a key press, in the saved form ("Ctrl+Shift+S").
fn is_modifier_key(key: egui::Key) -> bool {
    use egui::Key::*;
    matches!(key, ShiftLeft | ShiftRight | ControlLeft | ControlRight | AltLeft | AltRight | SuperLeft | SuperRight)
}

pub fn combo_of(key: egui::Key, mods: egui::Modifiers) -> Option<String> {
    if is_modifier_key(key) { return None; }
    let name = match key {
        egui::Key::Escape => "Esc".to_owned(),
        egui::Key::Space => "Space".to_owned(),
        egui::Key::Plus | egui::Key::Equals => "=".to_owned(),
        egui::Key::Minus => "-".to_owned(),
        egui::Key::Backtick => "`".to_owned(),
        egui::Key::OpenBracket => "[".to_owned(),
        egui::Key::CloseBracket => "]".to_owned(),
        // Arrows by name ("ArrowRight"), the form shortcut strings are parsed from.
        egui::Key::ArrowLeft | egui::Key::ArrowRight | egui::Key::ArrowUp | egui::Key::ArrowDown => key.name().to_owned(),
        egui::Key::Num0 | egui::Key::Num1 | egui::Key::Num2 | egui::Key::Num3 | egui::Key::Num4 | egui::Key::Num5 | egui::Key::Num6 | egui::Key::Num7 | egui::Key::Num8 | egui::Key::Num9 => key.name().trim_start_matches("Num").to_owned(),
        other => {
            let n = other.symbol_or_name().to_owned();
            if n.chars().count() == 1 { n.to_uppercase() } else { n }
        }
    };
    // "+" is Shift+= on many layouts and its own key on others: both are the "=" key.
    let mods = if key == egui::Key::Plus { egui::Modifiers { shift: false, ..mods } } else { mods };
    let mut parts = Vec::new();
    if mods.ctrl {
        parts.push("Ctrl".to_owned());
    }
    if mods.mac_cmd {
        parts.push("Meta".to_owned());
    }
    if mods.alt {
        parts.push("Alt".to_owned());
    }
    if mods.shift {
        parts.push("Shift".to_owned());
    }
    parts.push(name);
    Some(parts.join("+"))
}

fn shortcut_key(key: egui::Key, physical_key: Option<egui::Key>, mods: egui::Modifiers) -> egui::Key {
    use egui::Key::*;
    // The number row may arrive as punctuation with Shift or Option held.
    if (mods.ctrl || mods.mac_cmd) && (mods.shift || mods.alt)
        && let Some(digit @ (Num0 | Num1 | Num2 | Num3 | Num4 | Num5 | Num6 | Num7 | Num8 | Num9)) = physical_key
        && !matches!(key, Num0 | Num1 | Num2 | Num3 | Num4 | Num5 | Num6 | Num7 | Num8 | Num9)
    {
        return digit;
    }
    key
}

/// A saved key text as a chord craft-keymap matches.
pub fn chord(text: &str) -> Option<Chord> {
    let text = text.replace("Esc", "Escape").replace("Escapeape", "Escape");
    Chord::parse(&text).ok()
}

impl App {
    /// A command's keys, primary first: the user's choice, or the defaults.
    pub(crate) fn bindings(&self, cmd: &Command) -> Vec<String> {
        let custom: std::collections::HashMap<String, String> = self.settings.get("keybindings").unwrap_or_default();
        if let Some(k) = custom.get(cmd.id) {
            return if k.is_empty() { Vec::new() } else { vec![k.clone()] };
        }
        if cfg!(target_os = "macos") {
            if let Some(m) = cmd.mac {
                return m.iter().map(|s| s.to_string()).collect();
            }
            return cmd.keys.iter().map(|k| k.replacen("Ctrl+", "Meta+", 1)).collect();
        }
        cmd.keys.iter().map(|s| s.to_string()).collect()
    }

    pub(crate) fn keys_of(&self, id: &str) -> Option<String> {
        command(id).and_then(|c| self.bindings(c).into_iter().next())
    }

    /// Run a command by its id.
    pub(crate) fn run_command(&mut self, id: &str) {
        let Some(cmd) = command(id) else { return self.run(&self.ctx.clone(), id) };
        let tab = self.active;
        let repo_tab = self.active_tab().and_then(|t| t.path()).is_some();
        if cmd.repo && !repo_tab {
            return self.toast(false, "Open a repository first");
        }
        if let Some(p) = cmd.picker {
            return self.open_palette(p);
        }
        let ctx = self.ctx.clone();
        let head_branch = self.repo(tab).and_then(|r| r.view.as_ref()).and_then(|v| v.data.head_branch.clone());
        match id {
            "repo:fetch" => self.act(tab, Action::Fetch),
            "repo:pull" => {
                let mode = self.repo(tab).map(|r| self.pull_mode(&r.path)).unwrap_or_else(|| "ff".into());
                let mode = if mode == "fetch" { "ff".into() } else { mode };
                self.act(tab, Action::Pull { mode, autostash: None })
            }
            "repo:push" => self.act(tab, Action::Push { force: false, lease: None, review: None }),
            "repo:createBranch" => self.prompt_branch(tab, None, true),
            "repo:createTag" => self.prompt_tag(tab, None, false, String::new()),
            "repo:stash" => self.prompt_stash(tab, "stash"),
            "repo:stash-keep-index" => self.prompt_stash(tab, "keep"),
            "repo:stash-untracked" => self.prompt_stash(tab, "untracked"),
            "repo:pop" => self.act(tab, Action::StashPop { index: 0, expected: None, without_index: false }),
            "repo:stageAll" => self.act_quiet(tab, Action::StageAll, true),
            "repo:unstageAll" => self.act_quiet(tab, Action::UnstageAll, true),
            "repo:discardAll" => self.confirm_act(tab, "Discard all changes?", "All uncommitted changes will be discarded. You can bring them back with Undo.", "Discard All", Action::DiscardAll),
            "repo:undo" => self.act(tab, Action::Undo),
            "repo:redo" => self.act(tab, Action::Redo),
            "repo:focusCommit" => self.focus_commit_summary(tab),
            "repo:searchCommits" => self.open_search(tab),
            "repo:openInEditor" => self.open_in_editor(tab, None),
            "repo:terminal" => self.open_terminal(tab),
            "repo:fileManager" => self.run(&ctx, "repo.file_manager"),
            "repo:externalDiff" => {
                let Some(r) = self.repo(tab) else { return };
                if let Some(file) = r.ui.conflict.clone() {
                    let tool = self.settings.string("mergeTool", "");
                    return self.act(tab, Action::ExternalMerge { file, tool });
                }
                match r.ui.diff.clone() {
                    None => self.toast(false, "Open a file's changes first"),
                    Some(d) if matches!(d.source, gitcraft_core::git::DiffSource::Staged | gitcraft_core::git::DiffSource::Unstaged) => {
                        let tool = self.settings.string("diffTool", "");
                        self.act(tab, Action::ExternalDiff { file: d.file, staged: d.source == gitcraft_core::git::DiffSource::Staged, tool })
                    }
                    Some(_) => self.toast(false, "External comparison of saved commits, ranges and stashes is not available. Use the in-app diff for this selection."),
                }
            }
            "repo:gitflow" => self.init_gitflow(tab),
            "repo:lfs" => self.act(tab, Action::LfsInstall),
            "repo:signing" => self.open_prefs(Some("gpg")),
            "repo:addRemote" => self.add_remote(tab),
            "repo:setUpstream" => {
                if let Some(b) = head_branch {
                    self.prompt_upstream_for(tab, &b)
                }
            }
            "repo:addWorktree" => self.add_worktree(tab, None),
            "repo:addSubmodule" => self.add_submodule(tab),
            "repo:rangeDiff" => self.compare_ranges(tab),
            "repo:maintenance" => self.repository_maintenance(tab),
            "repo:fetchHistory" => self.fetch_more_history(tab),
            "repo:reflog" => self.reflog_dialog(tab),
            "repo:cleanup" => self.cleanup_dialog(tab),
            "ui:integrations" => self.open_prefs(Some("integrations")),
            "repo:sparse" => self.sparse_dialog(tab),
            "nav:newTab" => self.new_tab(),
            "nav:closeTab" => self.close_tab(self.active),
            "nav:reopenTab" => self.reopen_tab(),
            "nav:tabsList" => {
                if self.dialog.is_none() {
                    egui::Popup::toggle_id(&self.ctx, super::tabs::tabs_list_id());
                }
            }
            "nav:nextTab" => self.cycle_tab(1),
            "nav:prevTab" => self.cycle_tab(-1),
            "nav:moveTabLeft" => self.shift_tab(self.active, -1),
            "nav:moveTabRight" => self.shift_tab(self.active, 1),
            "nav:openRepo" => self.open_repo_dialog(),
            "nav:clone" => self.clone_dialog(""),
            "nav:init" => self.init_dialog(),
            "nav:repoManagement" => self.repo_management(),
            "palette:toggle" => {
                if self.palette.is_some() {
                    self.palette = None;
                } else {
                    self.open_palette(Picker::Root);
                }
            }
            "ui:leftPanel" => self.settings.toggle("showLeft", true),
            "ui:detailPanel" => self.settings.toggle("showRight", true),
            "ui:terminalPanel" => self.settings.toggle("showTerminal", false),
            "term:new" | "term:close" | "term:next" | "term:prev" => self.terminal_command(id),
            t if t.starts_with("term:tab") => self.terminal_command(id),
            "ui:zoomIn" => self.step_zoom(&ctx, true),
            "ui:zoomOut" => self.step_zoom(&ctx, false),
            "ui:zoomReset" => self.set_zoom(&ctx, 1.0),
            "ui:toolbarLabels" => self.settings.toggle("toolbarLabels", true),
            "ui:menusInTitleBar" => self.settings.toggle("menusInTitleBar", true),
            "ui:shortcuts" => self.shortcuts_dialog(),
            "ui:settings" => self.open_prefs(None),
            "ui:activityLog" => self.log_dialog(),
            "ui:filterLeft" => self.focus_left_filter.set(true),
            "ui:fullscreen" => {
                let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
            }
            other => {
                if let Some(n) = other.strip_prefix("nav:tab").and_then(|n| n.parse::<usize>().ok()) {
                    let shown: Vec<u64> = self.tabs.iter().filter(|t| t.kind != super::TabKind::Launchpad).map(|t| t.id).collect();
                    if let Some(id) = shown.get(n - 1) {
                        self.activate(*id);
                    }
                } else if let Some(n) = other.strip_prefix("nav:favorite").and_then(|n| n.parse::<usize>().ok()) {
                    let favorites: Vec<String> = self.settings.get("favorites").unwrap_or_default();
                    match favorites.get(n - 1) {
                        Some(path) => self.open_repo(&path.clone()),
                        None => self.toast(false, format!("There is no favorite repository {n}.")),
                    }
                }
            }
        }
    }

    pub(crate) fn cycle_tab(&mut self, dir: isize) {
        if self.tabs.is_empty() {
            return;
        }
        let i = self.tabs.iter().position(|t| t.id == self.active).unwrap_or(0) as isize;
        let n = self.tabs.len() as isize;
        let next = self.tabs[((i + dir) % n + n) as usize % n as usize].id;
        self.activate(next);
    }

    /// Run the command whose key was pressed. Text fields keep their own keys, a dialog keeps
    /// the repository behind it unchanged.
    pub(super) fn dispatch_keys(&mut self, ctx: &egui::Context) {
        if craft_menu::bar::leave_for_shortcut(ctx) || craft_menu::bar::is_open(ctx) || craft_menu::bar::is_focused(ctx) || self.recording.is_some() {
            return;
        }
        let typing = ctx.egui_wants_keyboard_input();
        let pressed: Vec<_> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key { key, physical_key, pressed: true, modifiers, repeat: false } => Some((*key, *physical_key, *modifiers)),
                    _ => None,
                })
                .collect()
        });
        for (key, physical_key, mods) in pressed {
            if self.file_editor_shortcut(ctx, key, mods) {
                continue;
            }
            if key == egui::Key::H && mods.matches_exact(egui::Modifiers::COMMAND)
                && self.dialog.is_none() && self.open_file_find(true)
            {
                ctx.input_mut(|input| input.consume_key(mods, key));
                continue;
            }
            let find = |key| {
                let combo = combo_of(key, mods).filter(|c| is_shortcut(c))?.to_uppercase();
                COMMANDS.iter().find(|c| self.bindings(c).iter().any(|b| b.to_uppercase() == combo))
            };
            let shortcut = shortcut_key(key, physical_key, mods);
            let Some(cmd) = find(key).or_else(|| if shortcut != key { find(shortcut) } else { None }) else { continue };
            if typing && !cmd.in_inputs {
                continue;
            }
            // A dialog keeps the repository behind it, and the tab it belongs to, as they are.
            if self.dialog.is_some() && (cmd.repo || cmd.id == "nav:closeTab") {
                continue;
            }
            ctx.input_mut(|i| i.consume_key(mods, key));
            if cmd.id == "repo:searchCommits" && self.open_file_find(false) {
                continue;
            }
            self.run_command(cmd.id);
        }
    }

    pub(crate) fn open_palette(&mut self, mode: Picker) {
        let tab = self.active;
        self.palette_request += 1;
        let request = self.palette_request;
        if matches!(mode, Picker::History | Picker::Blame)
            && let Some(path) = self.repo(tab).map(|r| r.path.clone())
        {
            self.palette_files = None;
            self.spawn(move || super::Msg::Tracked { request, result: gitcraft_core::files::tracked_files(std::path::Path::new(&path)) });
        }
        self.palette = Some(Palette { mode, query: String::new(), selected: 0, focus: true, tab, request, enter: false });
    }

    /// The tracked files for the file picker; a list for an earlier picker is dropped.
    pub(super) fn tracked_loaded(&mut self, request: u64, result: Result<Vec<String>, String>) {
        if self.palette.as_ref().is_none_or(|p| p.request != request) {
            return;
        }
        match result {
            Ok(files) => self.palette_files = Some(files),
            Err(e) => {
                self.palette_files = Some(Vec::new());
                self.toast(false, e);
            }
        }
        if let Some(p) = self.palette.as_mut() {
            p.selected = 0;
        }
    }

    /// The command palette and its pickers, over the window near the top.
    pub(super) fn draw_palette(&mut self, ctx: &egui::Context) {
        let Some(mut pal) = self.palette.take() else { return };
        let c = p();
        // The tab the palette was opened over.
        let tab = pal.tab;
        let path = self.active_tab().and_then(|t| t.path().map(str::to_owned));
        // (key, label, hint, shortcut, action)
        let mut items: Vec<(String, String, String, String, PaletteAction)> = Vec::new();
        match pal.mode {
            Picker::Root => {
                for cmd in COMMANDS.iter().filter(|c| !c.repo || path.is_some()) {
                    let hint = format!("{} {}", cmd.group.title(), cmd.keywords);
                    let action = match cmd.picker {
                        Some(p) => PaletteAction::Picker(p),
                        None => PaletteAction::Command(cmd.id),
                    };
                    let label = self.command_label(cmd);
                    items.push((cmd.id.to_owned(), label, hint, self.keys_of(cmd.id).map(|k| label_of(&k)).unwrap_or_default(), action));
                }
                // Every other editor found on this computer.
                if path.is_some() {
                    let preferred = self.settings.string("externalEditor", "vscode");
                    for tool in installed_editors().iter().copied().filter(|t| *t != preferred) {
                        let name = super::external_tools::editor_name(tool);
                        items.push((format!("open-in:{tool}"), format!("Open Repo in {name}"), format!("{} editor ide", Group::Repo.title()), String::new(), PaletteAction::OpenIn(tool)));
                    }
                }
            }
            Picker::OpenRepo => {
                let mut all: Vec<String> = self.tabs.iter().filter_map(|t| t.path().map(str::to_owned)).collect();
                for r in self.settings.recents() {
                    if !all.contains(&r) {
                        all.push(r);
                    }
                }
                for p in all {
                    items.push((p.clone(), super::folder_name(&p), p.clone(), String::new(), PaletteAction::Open(p)));
                }
            }
            Picker::Checkout => {
                if let Some(v) = self.repo(tab).and_then(|r| r.view.as_ref()) {
                    for r in v.data.refs.iter().filter(|r| r.kind != gitcraft_core::git::RefKind::Tag && !(r.kind == gitcraft_core::git::RefKind::Local && Some(&r.name) == v.data.head_branch.as_ref())) {
                        let remote = r.kind == gitcraft_core::git::RefKind::Remote;
                        items.push((r.full.clone(), r.name.clone(), if remote { "remote branch".into() } else { "local branch".into() }, String::new(), PaletteAction::Checkout(r.name.clone(), remote)));
                    }
                }
            }
            Picker::History | Picker::Blame => {
                for f in self.palette_files.iter().flatten() {
                    let name = f.rsplit('/').next().unwrap_or(f).to_owned();
                    items.push((f.clone(), name, f.clone(), String::new(), PaletteAction::History(f.clone(), pal.mode == Picker::Blame)));
                }
            }
            Picker::Theme => {
                let current = self.settings.theme();
                for t in super::theme::Theme::ALL {
                    items.push((t.id().into(), t.label().into(), if current == t.id() { "current".into() } else { String::new() }, String::new(), PaletteAction::Theme(t.id())));
                }
            }
            Picker::Profile => {
                let active = self.settings.active_profile_index();
                for (i, name) in self.settings.profile_names().into_iter().enumerate() {
                    items.push((format!("p{i}"), name, if i == active { "current".into() } else { String::new() }, String::new(), PaletteAction::Profile(i)));
                }
            }
            Picker::Workspace => {
                let ws: Vec<serde_json::Value> = self.settings.get("workspaces").unwrap_or_default();
                for (i, w) in ws.iter().enumerate() {
                    let name = w["name"].as_str().unwrap_or("Workspace").to_owned();
                    let n = w["repos"].as_array().map_or(0, Vec::len);
                    items.push((format!("w{i}"), name, format!("{n} repositories"), String::new(), PaletteAction::Workspace(i)));
                }
            }
        }
        let q = pal.query.clone();
        let mut scored: Vec<(i64, usize)> = items.iter().enumerate().map(|(i, it)| ((fuzzy(&q, &it.1) * 2).max(fuzzy(&q, &it.2)), i)).filter(|(s, _)| *s > 0).collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored.truncate(200);
        let shown: Vec<usize> = scored.into_iter().map(|(_, i)| i).collect();
        pal.selected = pal.selected.min(shown.len().saturating_sub(1));
        let placeholder = match pal.mode {
            Picker::Root => "Search for commands and actions (e.g., Open Repo)",
            Picker::OpenRepo => "Search for a repository to open",
            Picker::Checkout => "Search for a branch to checkout",
            Picker::History => "Search for a file to view its history / blame",
            Picker::Blame => "Search for a file to view who did what (i.e. its blame)",
            Picker::Theme => "Search for a theme to switch to",
            Picker::Profile => "Search for a profile to switch to",
            Picker::Workspace => "Search for a workspace to open",
        };
        let screen = ctx.content_rect();
        let width = 560.0_f32.min(screen.width() * 0.9);
        let mut close = false;
        let mut run: Option<usize> = None;
        let mut back = false;
        egui::Area::new(egui::Id::NULL.with("palette_shade")).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
            let (_, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            if resp.clicked() {
                close = true;
            }
        });
        egui::Area::new(egui::Id::NULL.with("palette")).order(egui::Order::Tooltip).fixed_pos(pos2(screen.center().x - width / 2.0, screen.top() + 64.0)).show(ctx, |ui| {
            egui::Frame::new().fill(c.menu_bg).stroke(Stroke::new(1.0, c.white(0.15))).corner_radius(CornerRadius::same(6)).shadow(egui::Shadow { offset: [0, 8], blur: 32, spread: 0, color: Color32::from_black_alpha(110) }).show(ui, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let (field, _) = ui.allocate_exact_size(vec2(width, 38.0), Sense::hover());
                ui.painter().rect_filled(field, CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, c.field);
                ui.painter().line_segment([field.left_bottom(), field.right_bottom()], Stroke::new(1.0, c.white(0.1)));
                let resp = ui.put(field.shrink2(vec2(12.0, 0.0)), egui::TextEdit::singleline(&mut pal.query).frame(egui::Frame::NONE).font(FontId::proportional(13.5)).hint_text(egui::RichText::new(placeholder).color(c.fg3)).align(Align2::LEFT_CENTER).desired_width(width - 24.0));
                if pal.focus {
                    resp.request_focus();
                    pal.focus = false;
                }
                // The results shown were found before this frame's typing; an Enter typed with it
                // runs on the next frame, against the results for the new text.
                let pending = std::mem::take(&mut pal.enter);
                if resp.changed() {
                    pal.selected = 0;
                }
                let (up, down, enter, esc, backspace) = ui.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape), i.key_pressed(egui::Key::Backspace)));
                if down {
                    pal.selected = (pal.selected + 1).min(shown.len().saturating_sub(1));
                }
                if up {
                    pal.selected = pal.selected.saturating_sub(1);
                }
                if (enter || pending) && resp.changed() {
                    pal.enter = true;
                } else if (enter || pending) && !shown.is_empty() {
                    run = Some(shown[pal.selected.min(shown.len() - 1)]);
                }
                if esc {
                    close = true;
                }
                if backspace && pal.query.is_empty() && pal.mode != Picker::Root && !resp.changed() {
                    back = true;
                }
                egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
                    ui.add_space(4.0);
                    if matches!(pal.mode, Picker::History | Picker::Blame) && self.palette_files.is_none() {
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(egui::RichText::new("Loading files…").size(12.0).color(c.fg3));
                        });
                    } else if shown.is_empty() {
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(egui::RichText::new("No matches").size(12.5).color(c.fg3));
                        });
                    }
                    for (row, &i) in shown.iter().enumerate() {
                        let it = &items[i];
                        let (rect, r) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click());
                        // Only a moving pointer picks a row, so the arrow keys work under a still one.
                        if r.hovered() && ui.input(|i| i.pointer.is_moving()) {
                            pal.selected = row;
                        }
                        let on = row == pal.selected;
                        if on {
                            ui.painter().rect_filled(rect, CornerRadius::ZERO, ACCENT);
                            if down || up {
                                r.scroll_to_me(None);
                            }
                        }
                        let ink = if on { c.white(1.0) } else { c.fg };
                        let sub = if on { c.white(0.8) } else { c.fg3 };
                        let sc_w = if it.3.is_empty() { 0.0 } else { ui.painter().layout_no_wrap(it.3.clone(), FontId::monospace(11.0), sub).size().x + 12.0 };
                        let g = widgets::fit(ui, &it.1, FontId::proportional(13.0), ink, width - 24.0 - sc_w);
                        let gw = g.size().x;
                        ui.painter().galley(pos2(rect.left() + 12.0, rect.center().y - g.size().y / 2.0), g, ink);
                        if pal.mode != Picker::Root && !it.2.is_empty() {
                            let h = widgets::fit(ui, &it.2, FontId::proportional(11.5), sub, width - 36.0 - gw - sc_w);
                            ui.painter().galley(pos2(rect.left() + 20.0 + gw, rect.center().y - h.size().y / 2.0), h, sub);
                        }
                        if !it.3.is_empty() {
                            ui.painter().text(pos2(rect.right() - 12.0, rect.center().y), Align2::RIGHT_CENTER, &it.3, FontId::monospace(11.0), sub);
                        }
                        if r.clicked() {
                            run = Some(i);
                        }
                    }
                    ui.add_space(4.0);
                });
            });
        });
        if back {
            pal.mode = Picker::Root;
            pal.selected = 0;
        }
        // Dismissing wins over a selection made in the same frame.
        if let Some(i) = run.filter(|_| !close) {
            let action = items.swap_remove(i).4;
            match action {
                PaletteAction::Picker(p) => {
                    self.open_palette(p);
                    return;
                }
                // A dialog opened meanwhile owns the keyboard: the chosen command is dropped, not run behind it.
                PaletteAction::Command(id) => {
                    if self.dialog.is_none() {
                        self.run_command(id);
                    }
                }
                PaletteAction::Open(path) => self.open_repo(&path),
                PaletteAction::Checkout(name, remote) => self.act(tab, if remote { Action::CheckoutRemote { remote_branch: name, stash_if_needed: false, changes: gitcraft_core::actions::CheckoutChanges::Ask } } else { Action::Checkout { target: name, changes: gitcraft_core::actions::CheckoutChanges::Ask, branch: true } }),
                PaletteAction::History(file, blame) => self.open_history(tab, &file, if blame { super::repo::HistoryMode::Blame } else { super::repo::HistoryMode::History }),
                PaletteAction::Theme(id) => self.set_theme(id),
                PaletteAction::Profile(i) => self.switch_profile(i),
                PaletteAction::Workspace(i) => self.open_workspace(i),
                PaletteAction::OpenIn(tool) => {
                    if let Some(path) = self.repo(tab).map(|r| r.path.clone())
                        && let Err(e) = gitcraft_core::external::open_editor(&path, None, tool, "", "", "")
                    {
                        self.toast(false, e);
                    }
                }
            }
            return;
        }
        if !close {
            self.palette = Some(pal);
        }
    }

    /// A command's name as the menu shows it: the editor command names the editor chosen in Preferences.
    fn command_label(&self, cmd: &Command) -> String {
        if cmd.id == "repo:openInEditor" {
            format!("Open Repo in {}", super::external_tools::editor_name(&self.settings.string("externalEditor", "vscode")))
        } else {
            cmd.label.to_owned()
        }
    }

    pub(crate) fn shortcuts_dialog(&mut self) {
        self.shortcuts = Some(ShortcutsUi { first: true, ..Default::default() });
    }

    /// Keyboard Shortcuts: every command and its keys; click a key to record a new one.
    pub(super) fn draw_shortcuts(&mut self, ctx: &egui::Context) {
        let Some(mut s) = self.shortcuts.take() else { return };
        let c = p();
        // Recording takes the next key press.
        if let Some(id) = s.recording.clone() {
            let pressed = ctx.input(|i| i.events.iter().find_map(|e| match e {
                egui::Event::Key { key, physical_key, pressed: true, repeat: false, modifiers }
                    if !is_modifier_key(*key) => Some((*key, *physical_key, *modifiers)),
                _ => None,
            }));
            if let Some((key, physical_key, mods)) = pressed {
                ctx.input_mut(|i| i.consume_key(mods, key));
                if key == egui::Key::Escape {
                    s.recording = None;
                } else if key == egui::Key::Backspace {
                    self.set_keys(&id, Some(String::new()));
                    s.recording = None;
                } else if let Some(combo) = combo_of(shortcut_key(key, physical_key, mods), mods).filter(|c| is_shortcut(c)) {
                    self.set_keys(&id, Some(combo));
                    s.recording = None;
                }
            }
        }
        self.recording = s.recording.clone();
        let screen = ctx.content_rect();
        let width = 600.0_f32.min(screen.width() - 32.0);
        let mut close = false;
        egui::Area::new(egui::Id::NULL.with("shortcuts_shade")).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, CornerRadius::ZERO, Color32::from_black_alpha(128));
            if resp.clicked() {
                close = true;
            }
        });
        let custom: std::collections::HashMap<String, String> = self.settings.get("keybindings").unwrap_or_default();
        let mut reset: Option<&'static str> = None;
        let mut record: Option<&'static str> = None;
        egui::Area::new(egui::Id::NULL.with("shortcuts")).order(egui::Order::Tooltip).fixed_pos(pos2(screen.center().x - width / 2.0, screen.top() + screen.height() * 0.18)).show(ctx, |ui| {
            egui::Frame::new().fill(c.panel).stroke(Stroke::new(1.0, c.white(0.12))).corner_radius(CornerRadius::same(6)).show(ui, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let (head, _) = ui.allocate_exact_size(vec2(width, 40.0), Sense::hover());
                ui.painter().text(pos2(head.left() + 16.0, head.center().y), Align2::LEFT_CENTER, "Keyboard Shortcuts", super::fonts::semibold(14.0), c.white(1.0));
                ui.painter().line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0, c.line));
                let x = Rect::from_center_size(pos2(head.right() - 24.0, head.center().y), Vec2::splat(22.0));
                let xr = ui.interact(x, ui.scope_id().with("close"), Sense::click());
                icons::paint(ui, x, Icon::X, 16.0, if xr.hovered() { c.white(1.0) } else { c.fg2 });
                if xr.clicked() {
                    close = true;
                }
                egui::Frame::new().inner_margin(egui::Margin::same(16)).show(ui, |ui| {
                    let w = ui.available_width();
                    let r = widgets::field(ui, &mut s.filter, &format!("Filter shortcuts ({} + F)", super::menu::MOD), w, 32.0, 13.0);
                    if s.first {
                        r.request_focus();
                        s.first = false;
                    }
                    ui.add_space(12.0);
                    let f = s.filter.to_lowercase();
                    // Every command's keys once, and the commands each key combination is on.
                    let bound: HashMap<&str, Vec<String>> = COMMANDS.iter().map(|cmd| (cmd.id, self.bindings(cmd).iter().map(|b| b.to_uppercase()).collect())).collect();
                    let mut on_keys: HashMap<&str, Vec<&Command>> = HashMap::new();
                    for cmd in COMMANDS.iter() {
                        for b in &bound[cmd.id] {
                            on_keys.entry(b.as_str()).or_default().push(cmd);
                        }
                    }
                    egui::ScrollArea::vertical().max_height(screen.height() * 0.75 - 140.0).show(ui, |ui| {
                        for g in [Group::Repo, Group::Navigation, Group::Palette, Group::Ui] {
                            let list: Vec<&Command> = COMMANDS.iter().filter(|cmd| cmd.group == g && (f.is_empty() || self.command_label(cmd).to_lowercase().contains(&f) || self.keys_of(cmd.id).unwrap_or_default().to_lowercase().contains(&f))).collect();
                            if list.is_empty() {
                                continue;
                            }
                            ui.label(egui::RichText::new(g.title().to_uppercase()).font(super::fonts::semibold(12.0)).color(c.fg2));
                            ui.add_space(4.0);
                            for cmd in list {
                                let keys = self.keys_of(cmd.id).unwrap_or_default();
                                let is_custom = custom.contains_key(cmd.id);
                                let clash = bound[cmd.id].iter().filter_map(|b| on_keys.get(b.as_str())).flatten().copied().find(|o| o.id != cmd.id);
                                let (row, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
                                ui.painter().line_segment([row.left_bottom(), row.right_bottom()], Stroke::new(1.0, c.white(0.05)));
                                ui.painter().text(pos2(row.left(), row.center().y), Align2::LEFT_CENTER, self.command_label(cmd), FontId::proportional(13.0), c.fg);
                                let reset_r = Rect::from_min_size(pos2(row.right() - 40.0, row.top()), vec2(40.0, 30.0));
                                if is_custom {
                                    let rr = ui.interact(reset_r, ui.scope_id().with(("reset", cmd.id)), Sense::click());
                                    ui.painter().text(reset_r.center(), Align2::CENTER_CENTER, "Reset", FontId::proportional(11.0), c.link);
                                    if rr.clicked() {
                                        reset = Some(cmd.id);
                                    }
                                }
                                let rec = s.recording.as_deref() == Some(cmd.id);
                                let text = if rec { "Press keys…".to_owned() } else if keys.is_empty() { "—".into() } else { label(&keys) };
                                let tw = ui.painter().layout_no_wrap(text.clone(), FontId::monospace(11.5), c.fg).size().x.max(78.0) + 12.0;
                                let kr = Rect::from_min_size(pos2(reset_r.left() - 8.0 - tw, row.center().y - 10.0), vec2(tw, 20.0));
                                let kresp = ui.interact(kr, ui.scope_id().with(("keys", cmd.id)), Sense::click()).tip("Click, then press the new keys (Esc cancels, Backspace removes)");
                                ui.painter().rect(kr, CornerRadius::same(3), if rec { ACCENT.gamma_multiply(0.2) } else { c.white(0.08) }, Stroke::new(1.0, if rec { ACCENT } else if kresp.hovered() { c.white(0.3) } else { c.white(0.1) }), StrokeKind::Inside);
                                ui.painter().text(kr.center(), Align2::CENTER_CENTER, text, FontId::monospace(11.5), if rec { c.white(1.0) } else { c.fg });
                                if kresp.clicked() {
                                    record = Some(cmd.id);
                                }
                                let mut nx = kr.left() - 8.0;
                                if let Some(other) = clash {
                                    let r = ui.painter().text(pos2(nx, row.center().y), Align2::RIGHT_CENTER, "clash", FontId::proportional(11.0), WARN);
                                    ui.interact(r, ui.scope_id().with(("clash", cmd.id)), Sense::hover()).tip(format!("Also used by {}", other.label));
                                    nx = r.left() - 8.0;
                                }
                                let uses = cmd.id != "ui:terminalPanel" && cmd.id != "palette:toggle";
                                if is_custom && !keys.is_empty() && uses {
                                    let tip = if !cmd.in_inputs { "The editor and terminal keep their own shortcuts." } else { "The terminal keeps its own shortcuts." };
                                    let r = ui.painter().text(pos2(nx, row.center().y), Align2::RIGHT_CENTER, "scope", FontId::proportional(11.0), c.fg3);
                                    ui.interact(r, ui.scope_id().with(("scope", cmd.id)), Sense::hover()).tip(tip);
                                }
                            }
                            ui.add_space(12.0);
                        }
                        ui.add(egui::Label::new(egui::RichText::new("Shortcuts need Ctrl, Alt, or the platform modifier (or an F key). Arrow keys, J / K, S / U and Esc work as before.").size(12.0).color(c.fg3)).wrap());
                    });
                });
            });
        });
        if let Some(id) = reset {
            self.set_keys(id, None);
        }
        if let Some(id) = record {
            s.recording = Some(id.to_owned());
        }
        if s.recording.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if !close {
            self.shortcuts = Some(s);
        } else {
            self.recording = None;
        }
    }

    fn set_keys(&mut self, id: &str, keys: Option<String>) {
        let mut kb: std::collections::HashMap<String, String> = self.settings.get("keybindings").unwrap_or_default();
        match keys {
            Some(k) => {
                kb.insert(id.to_owned(), k);
            }
            None => {
                kb.remove(id);
            }
        }
        self.settings.set("keybindings", kb);
    }
}

fn label_of(key: &str) -> String {
    label(key)
}

pub enum PaletteAction {
    Picker(Picker),
    Command(&'static str),
    Open(String),
    Checkout(String, bool),
    History(String, bool),
    Theme(&'static str),
    Profile(usize),
    Workspace(usize),
    /// Open the repository in this editor (one found installed), whatever the preferred editor is.
    OpenIn(&'static str),
}

/// The editors found installed, looked for once per run.
fn installed_editors() -> &'static [&'static str] {
    static FOUND: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    FOUND.get_or_init(|| {
        let tools: Vec<&'static str> = super::external_tools::EDITORS.iter().map(|(id, _)| *id).collect();
        gitcraft_core::external::installed_editors(&tools)
    })
}

pub struct Palette {
    pub mode: Picker,
    pub query: String,
    pub selected: usize,
    pub focus: bool,
    /// The tab it was opened over, and which opening this is.
    pub tab: u64,
    pub request: u64,
    /// Enter was pressed with new text; run the choice when its results are in.
    pub enter: bool,
}

#[derive(Default)]
pub struct ShortcutsUi {
    pub filter: String,
    pub recording: Option<String>,
    first: bool,
}

/// Every query character in order; earlier and adjacent matches score higher.
pub fn fuzzy(query: &str, text: &str) -> i64 {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return 1;
    }
    let t = text.to_lowercase();
    if let Some(i) = t.find(&q) {
        return 1000 - t[..i].chars().count() as i64;
    }
    let chars: Vec<char> = t.chars().collect();
    let mut score = 0;
    let mut last: isize = -1;
    for ch in q.chars().filter(|c| *c != ' ') {
        let Some(i) = chars.iter().enumerate().skip((last + 1) as usize).find(|(_, c)| **c == ch).map(|(i, _)| i as isize) else { return 0 };
        score += if i == last + 1 { 5 } else { 1 };
        last = i;
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_key_parses() {
        for cmd in COMMANDS {
            for k in cmd.keys.iter().chain(cmd.mac.unwrap_or(&[]).iter()) {
                assert!(chord(k).is_some(), "{}: {k}", cmd.id);
                assert!(is_shortcut(k), "{}: {k}", cmd.id);
            }
        }
        let mut ids: Vec<&str> = COMMANDS.iter().map(|c| c.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), COMMANDS.len());
    }

    #[test]
    fn fuzzy_prefers_substrings() {
        assert!(fuzzy("open", "Open Repo") > fuzzy("opr", "Open Repo"));
        assert_eq!(fuzzy("xyz", "Open Repo"), 0);
        assert!(fuzzy("", "anything") > 0);
    }

    #[test]
    fn key_presses_read_as_saved() {
        let mods = egui::Modifiers { ctrl: true, shift: true, ..Default::default() };
        assert_eq!(combo_of(egui::Key::S, mods).as_deref(), Some("Ctrl+Shift+S"));
        assert_eq!(combo_of(egui::Key::Num1, egui::Modifiers { ctrl: true, ..Default::default() }).as_deref(), Some("Ctrl+1"));
        let shifted = shortcut_key(egui::Key::Exclamationmark, Some(egui::Key::Num1), mods);
        assert_eq!(combo_of(shifted, mods).as_deref(), Some("Ctrl+Shift+1"));
        assert_eq!(combo_of(egui::Key::ControlLeft, mods), None);
        assert_eq!(shortcut_key(egui::Key::Exclamationmark, Some(egui::Key::Num1), egui::Modifiers::SHIFT), egui::Key::Exclamationmark);
        let ctrl = egui::Modifiers { ctrl: true, ..Default::default() };
        assert_eq!(combo_of(egui::Key::Minus, ctrl).as_deref(), Some("Ctrl+-"));
        assert_eq!(combo_of(egui::Key::Plus, egui::Modifiers { shift: true, ..ctrl }).as_deref(), Some("Ctrl+="));
    }
}

impl App {
    /// Show the working changes with the commit box, the cursor in its summary.
    pub(crate) fn focus_commit_summary(&mut self, tab: u64) {
        self.activate(tab);
        self.settings.set("showRight", true);
        if let Some(r) = self.repo_mut(tab) {
            r.ui.select(Selection::Wip);
            r.ui.rebase = None;
            r.ui.conflict = None;
            r.ui.pr = None;
            r.ui.issue = None;
            r.work.tab = super::right::WorkTab::Commit;
        }
        self.ctx.memory_mut(|m| m.request_focus(egui::Id::NULL.with(("summary", tab))));
        self.focus_summary = Some(tab);
    }
}
