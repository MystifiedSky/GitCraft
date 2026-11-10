import { openFavorite } from "@/components/RepoManagement";
// Every command GitCraft offers, in one place: the command palette lists them, the keyboard
// handler runs them, and the shortcut editor changes their keys. Groups follow the original's
// Keyboard Shortcuts headers (Repo Actions, Navigation, Command Palette, UI).
import { api } from "./api";
import { act, confirmThen, promptBranch, promptStash, promptTag, repoData } from "./repo";
import { useStore } from "./store";
import { resetZoom, stepZoom } from "./zoom";
import { addRemote, addSubmodule, addWorktree, initGitflow, openCleanup } from "./manage";
import { promptUpstream } from "./menus";

export type Group = "Repo Actions" | "Navigation" | "Command Palette" | "UI";

export interface Command {
  id: string;
  label: string;
  group: Group;
  /** Default shortcut, e.g. "Ctrl+Shift+S" (Ctrl = Cmd on macOS). */
  keys?: string;
  keywords?: string;
  /** Needs an open repository. */
  repo?: boolean;
  /** Also runs while typing in a text box. */
  inInputs?: boolean;
  /** Opens a picker in the palette instead of running directly. */
  picker?: "openRepo" | "checkout" | "history" | "blame" | "theme" | "profile" | "workspace";
  run?: (path: string | undefined) => void;
}

const st = () => useStore.getState();
const activeTab = () => st().tabs.find((t) => t.id === st().activeId);

function cycleTab(dir: number) {
  const s = st();
  const i = s.tabs.findIndex((t) => t.id === s.activeId);
  s.setActive(s.tabs[(i + dir + s.tabs.length) % s.tabs.length].id);
}

function selectTab(n: number) {
  const s = st();
  const t = s.tabs.filter((x) => x.kind !== "launchpad")[n - 1];
  if (t) s.setActive(t.id);
}

export const COMMANDS: Command[] = [
  // Repo Actions
  { id: "repo:fetch", label: "Fetch All", group: "Repo Actions", keys: "Ctrl+L", repo: true, run: (p) => act(p!, { type: "fetch" }) },
  { id: "repo:pull", label: "Pull", group: "Repo Actions", repo: true, run: (p) => act(p!, { type: "pull", mode: st().settings.pullMode === "fetch" ? "ff" : st().settings.pullMode }) },
  { id: "repo:push", label: "Push", group: "Repo Actions", repo: true, run: (p) => act(p!, { type: "push", force: false }) },
  { id: "repo:createBranch", label: "Create Branch", group: "Repo Actions", keys: "Ctrl+B", repo: true, run: (p) => promptBranch(p!, null) },
  { id: "repo:checkout", label: "Checkout", group: "Repo Actions", repo: true, picker: "checkout", keywords: "switch branch" },
  { id: "repo:createTag", label: "Create Tag", group: "Repo Actions", repo: true, run: (p) => promptTag(p!, null) },
  { id: "repo:stash", label: "Stash", group: "Repo Actions", repo: true, run: (p) => promptStash(p!) },
  { id: "repo:pop", label: "Pop Stash", group: "Repo Actions", repo: true, run: (p) => act(p!, { type: "stashPop", index: 0 }) },
  { id: "repo:stageAll", label: "Stage all changes", group: "Repo Actions", keys: "Ctrl+Shift+S", repo: true, run: (p) => act(p!, { type: "stageAll" }, true) },
  { id: "repo:unstageAll", label: "Unstage all changes", group: "Repo Actions", keys: "Ctrl+Shift+U", repo: true, run: (p) => act(p!, { type: "unstageAll" }, true) },
  {
    id: "repo:discardAll",
    label: "Discard all changes",
    group: "Repo Actions",
    repo: true,
    keywords: "reset hard all",
    run: (p) => confirmThen("Discard all changes?", "All uncommitted changes will be discarded. You can bring them back with Undo.", "Discard All", () => act(p!, { type: "discardAll" })),
  },
  { id: "repo:undo", label: "Undo", group: "Repo Actions", keys: "Ctrl+Z", repo: true, run: (p) => act(p!, { type: "undo" }) },
  { id: "repo:redo", label: "Redo", group: "Repo Actions", keys: "Ctrl+Y", repo: true, run: (p) => act(p!, { type: "redo" }) },
  {
    id: "repo:focusCommit",
    label: "Focus commit message",
    group: "Repo Actions",
    keys: "Ctrl+Shift+M",
    repo: true,
    inInputs: true,
    keywords: "view working directory changes wip",
    run: () => {
      const t = activeTab();
      if (!t) return;
      st().setUi(t.id, { selection: { type: "wip" }, diff: null, history: null, rebase: null, conflict: null });
      setTimeout(() => (document.querySelector("[data-active-repository='true'] [data-commit-summary]") as HTMLInputElement | null)?.focus(), 50);
    },
  },
  { id: "repo:searchCommits", label: "Search Commits", group: "Repo Actions", keys: "Ctrl+F", repo: true, run: () => activeTab() && st().setUi(activeTab()!.id, { searchOpen: true }) },
  { id: "repo:history", label: "History", group: "Repo Actions", keys: "Ctrl+Shift+H", repo: true, picker: "history", keywords: "file history" },
  { id: "repo:blame", label: "Blame", group: "Repo Actions", repo: true, picker: "blame" },
  { id: "repo:openInEditor", label: "Open Repo in VS Code", group: "Repo Actions", keys: "Ctrl+Shift+E", repo: true, run: (p) => api.openInEditor(p!) },
  { id: "repo:terminal", label: "Open External Terminal", group: "Repo Actions", keys: "Alt+T", repo: true, keywords: "command line console", run: (p) => api.openTerminal(p!) },
  { id: "repo:fileManager", label: "Open in File Manager", group: "Repo Actions", keys: "Alt+O", repo: true, keywords: "folder explorer finder", run: (p) => api.openFileManager(p!) },
  {
    id: "repo:externalDiff",
    label: "Open in External Diff/Merge Tool",
    group: "Repo Actions",
    keys: "Ctrl+D",
    repo: true,
    run: (p) => {
      const t = activeTab();
      const u = t ? st().ui[t.id] : undefined;
      if (u?.conflict) return act(p!, { type: "externalMerge", file: u.conflict, tool: st().settings.mergeTool });
      if (!u?.diff) return st().toast(false, "Open a file's changes first");
      act(p!, { type: "externalDiff", file: u.diff.file, staged: u.diff.source.kind === "Staged", tool: st().settings.diffTool });
    },
  },
  { id: "repo:gitflow", label: "Configure Gitflow", group: "Repo Actions", repo: true, run: (p) => initGitflow(p!) },
  { id: "repo:lfs", label: "Initialize LFS on this repo", group: "Repo Actions", repo: true, keywords: "configure lfs", run: (p) => act(p!, { type: "lfsInstall" }) },
  { id: "repo:signing", label: "Configure Commit Signing", group: "Repo Actions", keywords: "gpg ssh accounts identity", run: () => st().setDialog({ kind: "prefs", section: "gpg" }) },
  { id: "repo:addRemote", label: "Add Remote", group: "Repo Actions", repo: true, run: (p) => addRemote(p!) },
  { id: "repo:setUpstream", label: "Set Upstream", group: "Repo Actions", repo: true, run: (p) => {
      const r = repoData(p!);
      const b = r?.refs.find((x) => x.kind === "Local" && x.name === r.head_branch);
      if (r && b) promptUpstream(r, b);
    } },
  { id: "repo:addWorktree", label: "Add a new worktree", group: "Repo Actions", repo: true, run: (p) => repoData(p!) && addWorktree(repoData(p!)!) },
  { id: "repo:addSubmodule", label: "Add Submodule", group: "Repo Actions", repo: true, run: (p) => addSubmodule(p!) },
  { id: "repo:cleanup", label: "Clean Up Local Branches", group: "Repo Actions", repo: true, run: openCleanup },
  { id: "ui:integrations", label: "Integrations", group: "UI", keywords: "github gitlab bitbucket azure jira trello connect", run: () => st().setDialog({ kind: "prefs", section: "integrations" }) },
  { id: "repo:sparse", label: "Sparse Checkout", group: "Repo Actions", repo: true, run: () => st().setDialog({ kind: "sparse" }) },

  // Navigation
  { id: "nav:newTab", label: "New Tab", group: "Navigation", keys: "Ctrl+T", inInputs: true, run: () => st().newTab() },
  { id: "nav:closeTab", label: "Close Tab", group: "Navigation", keys: "Ctrl+W", inInputs: true, run: () => st().closeTab(st().activeId) },
  { id: "nav:reopenTab", label: "Reopen Closed Tab", group: "Navigation", keys: "Ctrl+Shift+T", inInputs: true, run: () => st().reopenTab() },
  { id: "nav:nextTab", label: "Jump to the next open tab", group: "Navigation", keys: "Ctrl+Tab", inInputs: true, run: () => cycleTab(1) },
  { id: "nav:prevTab", label: "Jump to the previous open tab", group: "Navigation", keys: "Ctrl+Shift+Tab", inInputs: true, run: () => cycleTab(-1) },
  ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map(
    (n): Command => ({ id: `nav:tab${n}`, label: `Select Tab ${n}`, group: "Navigation", keys: `Ctrl+${n}`, inInputs: true, run: () => selectTab(n) }),
  ),
  { id: "nav:openRepo", label: "Open Repo", group: "Navigation", keys: "Ctrl+O", run: () => st().setDialog({ kind: "open" }) },
  { id: "nav:openRepoPalette", label: "Search for a repository to open", group: "Navigation", keys: "Ctrl+Shift+O", picker: "openRepo" },
  { id: "nav:clone", label: "Clone Repo", group: "Navigation", keys: "Ctrl+N", run: () => st().setDialog({ kind: "clone" }) },
  { id: "nav:init", label: "Init Repo", group: "Navigation", keys: "Ctrl+I", keywords: "initialize create", run: () => st().setDialog({ kind: "init" }) },
  { id: "nav:workspaces", label: "Open Workspaces", group: "Navigation", picker: "workspace" },
  { id: "nav:repoManagement", label: "Open Repo Management", group: "Navigation", keys: "Ctrl+Alt+O", keywords: "repositories favorites recent fetch all pull all", run: () => st().setDialog({ kind: "repoManagement" }) },
  ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map((n): Command => ({ id: `nav:favorite${n}`, label: `Open Favorite Repo ${n}`, group: "Navigation", run: () => openFavorite(n) })),

  // Command Palette
  { id: "palette:toggle", label: "Toggle Command Palette", group: "Command Palette", keys: "Ctrl+Shift+P", inInputs: true, run: () => togglePalette() },

  // UI
  { id: "ui:leftPanel", label: "Toggle Left Panel", group: "UI", keys: "Ctrl+J", run: () => st().setSettings({ showLeft: !st().settings.showLeft }) },
  { id: "ui:detailPanel", label: "Toggle Commit Details Panel", group: "UI", keys: "Ctrl+K", run: () => st().setSettings({ showRight: !st().settings.showRight }) },
  { id: "ui:terminalPanel", label: "Toggle Terminal Panel", group: "UI", keys: "Ctrl+`", inInputs: true, keywords: "in-app terminal command line console", run: () => st().setSettings({ showTerminal: !st().settings.showTerminal }) },
  { id: "ui:zoomIn", label: "Increase Zoom", group: "UI", keys: "Ctrl+=", inInputs: true, run: () => stepZoom(1) },
  { id: "ui:zoomOut", label: "Decrease Zoom", group: "UI", keys: "Ctrl+-", inInputs: true, run: () => stepZoom(-1) },
  { id: "ui:zoomReset", label: "Reset Zoom", group: "UI", keys: "Ctrl+0", inInputs: true, run: resetZoom },
  { id: "ui:toolbarLabels", label: "Toggle Toolbar Labels", group: "UI", run: () => st().setSettings({ toolbarLabels: !st().settings.toolbarLabels }) },
  { id: "ui:theme", label: "Switch Theme", group: "UI", picker: "theme" },
  { id: "ui:profile", label: "Switch to Profile", group: "UI", picker: "profile" },
  { id: "ui:shortcuts", label: "Keyboard Shortcuts", group: "UI", keys: "Ctrl+/", run: () => st().setDialog({ kind: "shortcuts" }) },
  { id: "ui:settings", label: "Settings", group: "UI", keys: "Ctrl+,", keywords: "preferences options", run: () => st().setDialog({ kind: "prefs" }) },
  { id: "ui:activityLog", label: "Activity Log", group: "UI", keywords: "logs", run: () => st().setDialog({ kind: "log" }) },
  { id: "ui:filterLeft", label: "Filter the left panel", group: "UI", keys: "Ctrl+Alt+F", inInputs: true, run: () => (document.querySelector("[data-active-repository='true'] [data-left-filter], [data-start-filter]") as HTMLInputElement | null)?.focus() },
  {
    id: "ui:fullscreen",
    label: "Toggle Full Screen",
    group: "UI",
    keys: "F11",
    inInputs: true,
    run: async () => {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const w = getCurrentWindow();
      w.setFullscreen(!(await w.isFullscreen()));
    },
  },
];

/** The key combination of an event, in the same form as Command.keys ("Ctrl+Shift+S"). */
export function comboOf(e: KeyboardEvent): string | null {
  const named: Record<string, string> = { " ": "Space", "+": "=", Escape: "Esc" };
  let key = named[e.key] ?? e.key;
  if (["Control", "Shift", "Alt", "Meta"].includes(key)) return null;
  // Use the physical key for digits and symbols so Shift doesn't change them ("Ctrl+Shift+1").
  if (/^Digit\d$/.test(e.code)) key = e.code.slice(5);
  else if (e.code === "Backquote") key = "`";
  if (key.length === 1) key = key.toUpperCase();
  const parts = [];
  if (e.ctrlKey || e.metaKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  parts.push(key);
  return parts.join("+");
}

/** Current shortcut of a command: the person's choice, else the default ("" = none). */
export function keysOf(c: Command): string {
  const custom = st().settings.keybindings?.[c.id];
  return custom !== undefined ? custom : c.keys ?? "";
}

export function runCommand(c: Command) {
  const t = activeTab();
  const path = t?.kind === "repo" ? t.path : undefined;
  if (c.repo && !path) return st().toast(false, "Open a repository first");
  if (c.picker) return openPalette(c.picker);
  c.run?.(path);
}

/** Handle a key press with the command list. Returns true if a command ran. */
export function dispatchKey(e: KeyboardEvent, typing: boolean): boolean {
  const combo = comboOf(e);
  if (!combo) return false;
  const hasMod = /Ctrl|Alt/.test(combo) || /^F\d+$/.test(combo);
  if (!hasMod) return false;
  for (const c of COMMANDS) {
    if (keysOf(c).toUpperCase() !== combo.toUpperCase()) continue;
    if (typing && !c.inInputs) return false; // leave editor and text-box shortcuts with their owner
    const terminal = (e.target as HTMLElement | null)?.closest?.(".xterm");
    if (terminal && c.id !== "ui:terminalPanel" && c.id !== "palette:toggle") return false;
    if (st().dialog && c.repo) return false; // a modal must not change the repository behind it
    if (c.repo) {
      const t = activeTab();
      if (t?.kind !== "repo") return false;
    }
    e.preventDefault();
    runCommand(c);
    return true;
  }
  return false;
}

// The palette's open state lives in the store so any command can open a picker.
export function openPalette(mode: Command["picker"] | "root" = "root") {
  st().setPalette(mode ?? "root");
}

export function togglePalette() {
  st().setPalette(st().palette ? null : "root");
}

/** Simple fuzzy score: every query character in order; earlier and adjacent matches score higher. */
export function fuzzy(query: string, text: string): number {
  const q = query.toLowerCase().trim();
  if (!q) return 1;
  const t = text.toLowerCase();
  if (t.includes(q)) return 1000 - t.indexOf(q);
  let score = 0;
  let last = -1;
  for (const ch of q) {
    if (ch === " ") continue;
    const i = t.indexOf(ch, last + 1);
    if (i < 0) return 0;
    score += i === last + 1 ? 5 : 1;
    last = i;
  }
  return score;
}
