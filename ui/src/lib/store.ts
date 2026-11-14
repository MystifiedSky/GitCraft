import { create } from "zustand";
import { useShallow } from "zustand/react/shallow";
import type { DiffSource, Metrics } from "./api";

export type TabKind = "repo" | "start" | "launchpad";

export interface Tab {
  id: string;
  kind: TabKind;
  path?: string;
  title: string;
}

export type Selection = { type: "wip" } | { type: "commit"; sha: string };

export interface DiffTarget {
  source: DiffSource;
  file: string;
  untracked: boolean;
  parent: string | null;
}

export interface TabUi {
  selection: Selection;
  diff: DiffTarget | null;
  search: string;
  searchOpen: boolean;
  /** Commits picked with Ctrl / Shift + click (2 = compare them). */
  multi: string[];
  /** Comparing a commit with the working folder. */
  compareWorkdir: string | null;
  /** File History / Blame view in the centre. */
  history: { file: string; mode: "history" | "blame"; sha: string | null } | null;
  /** How many commits the graph has loaded (grows when scrolling near the end). */
  limit: number;
  /** Interactive rebase / cherry pick being planned in the centre. */
  rebase: RebasePlan | null;
  /** File open in the merge conflict tool. */
  conflict: string | null;
  /** Pull request / issue open in the centre. */
  pr: number | null;
  issue: number | null;
}

export interface RebaseItem {
  action: import("./api").Step;
  sha: string;
  author: string;
  email: string;
  summary: string;
  /** Full message (summary + body); edited for reword. */
  message: string;
}

export interface RebasePlan {
  mode: "rebase" | "cherry";
  /** Commit (or "--root") the rebase replays onto; unused for cherry pick. */
  base: string;
  onto: string;
  /** Newest first, as shown. */
  items: RebaseItem[];
  original: RebaseItem[];
}

export interface Draft {
  summary: string;
  description: string;
  amend: boolean;
}

/** One entry of the bell's menu. */
export interface Notice {
  id: number;
  text: string;
  at: number;
  readAt: number | null;
}
export type NoticeFilter = "All" | "Read" | "Unread";

/** Most notifications kept (an estimate). */
export const NOTICE_LIMIT = 100;
/** Unread = no readAt, Read = has readAt, All = everything. */
export const noticeVisible = (n: Notice, f: NoticeFilter) => (f === "All" ? true : f === "Unread" ? !n.readAt : !!n.readAt);
/** getUnreadNotificationCountString: unread notifications, "50+" from 50 on. */
export const unreadCountString = (notices: Notice[]) => {
  const n = notices.filter((x) => !x.readAt).length;
  return n >= 50 ? "50+" : String(n);
};

export interface Toast {
  id: number;
  ok: boolean;
  text: string;
}

export type DialogState =
  | null
  | { kind: "prompt"; title: string; label: string; value: string; confirm: string; onSubmit: (v: string, extra?: string) => void; extraLabel?: string }
  | { kind: "confirm"; title: string; text: string; confirm: string; danger?: boolean; onConfirm: () => void }
  | { kind: "clone" }
  | { kind: "init" }
  | { kind: "open" }
  | { kind: "log" }
  | { kind: "shortcuts" }
  | { kind: "about" }
  | { kind: "plan" }
  | { kind: "prefs"; section?: string }
  | { kind: "fileFinder" }
  | {
      kind: "form";
      title: string;
      fields: FormField[];
      confirm: string;
      help?: string;
      onSubmit: (v: Record<string, string | boolean>) => void;
      /** An extra button that fills in the fields (e.g. "Write with AI"). */
      action?: { label: string; run: (v: Record<string, string | boolean>) => Promise<Record<string, string | boolean>> };
    }
  | { kind: "cleanup" }
  | { kind: "sparse" }
  | { kind: "accounts" }
  | { kind: "profiles" }
  | { kind: "workspaces" }
  | { kind: "choice"; title: string; text: string; choices: { label: string; variant?: "primary" | "danger" | "default"; onSelect: () => void }[] }
  | { kind: "info"; title: string; text: string }
  | { kind: "repoManagement" }
  | { kind: "message"; title: string; label: string; value: string; confirm: string; help?: string; generate?: () => Promise<string>; onSubmit: (v: string) => void };

export interface Settings {
  showLeft: boolean;
  showRight: boolean;
  leftWidth: number;
  rightWidth: number;
  pullMode: "ff" | "ffonly" | "rebase" | "fetch";
  zoom: number;
  recents: string[];
  pushAfterCommit: boolean;
  fileView: "path" | "tree";
  fileSort: "path" | "status";
  leftView: "list" | "agents";
  toolbarLabels: boolean;
  defaultCloneFolder: string;
  diffMode: "hunk" | "inline" | "split";
  ignoreWhitespace: boolean;
  wordWrap: boolean;
  columns: GraphColumns;
  /** Hidden and soloed branches / tags per repository (full ref names). */
  hidden: Record<string, string[]>;
  solo: Record<string, string[]>;
  lazyLoad: boolean;
  /** Minutes between background fetches (0 = off); the default is 1. */
  autoFetchMin: number;
  autoPrune: boolean;
  /** "Default Branch Name" for new repositories. */
  defaultBranch: string;
  /** "Skip git hooks by default when committing". */
  skipHooks: boolean;
  /** Custom keyboard shortcuts: command id -> keys ("" = no shortcut). */
  keybindings: Record<string, string>;
  showTerminal: boolean;
  terminalHeight: number;
  theme: string;
  workspaces: Workspace[];
  /** Tab / repository aliases (path -> name). */
  aliases: Record<string, string>;
  /** User-sized repository tabs, in CSS pixels, keyed by repository path. */
  tabWidths: Record<string, number>;
  spellcheck: boolean;
  /** Check GitHub Releases for new versions and download them in the background. */
  autoUpdate: boolean;
  /** Starred repositories (Repository Management, start screen, Open Favorite Repo 1-9). */
  favorites: string[];
  /** Commit template: prefilled summary / description. */
  templateSummary: string;
  templateDescription: string;
  /** External diff / merge tools (git difftool / mergetool names). */
  diffTool: string;
  mergeTool: string;
  /** Show a notice after every successful action (errors always show). */
  notifySuccess: boolean;
  /** "Pin to Left": per repository, the full ref name drawn in lane 0. */
  pinned: Record<string, string>;
  /** "Show ghost branch/tag when hovering over or selecting a commit". */
  ghostRefs: boolean;
  /** Self-managed GitLab host (empty = gitlab.com only). */
  gitlabHost: string;
  // ---- Preferences screen ----
  /** General > Keep submodules up to date (autoUpdateSubmodules: true). */
  autoUpdateSubmodules: boolean;
  /** General > Delete ".orig" files after merging (deleteOrigAfterMerge: false). */
  deleteOrigAfterMerge: boolean;
  /** General > Remember tabs (rememberTabs: true). */
  rememberTabs: boolean;
  /** General > Initial Commits in Graph (maxCommitsInGraph 2000, minimum 500) / Show All Commits. */
  initialCommits: number;
  showAllCommits: boolean;
  /** General > Use extended logging in activity log. */
  extendedLog: boolean;
  /** General > Enable proactive conflict detection with target branch (conflictDetection.enabled: true). */
  conflictDetection: boolean;
  /** General > Use In-App Terminal Instead (of an external terminal). */
  useInAppTerminal: boolean;
  /** Commit > Merge Behavior: squash local merges. */
  squashMerge: boolean;
  /** SSH: use the system SSH agent / config, or this key pair. */
  sshUseAgent: boolean;
  sshPrivateKey: string;
  sshPublicKey: string;
  /** UI Customization. */
  notificationLocation: "top-left" | "top-right" | "bottom-left" | "bottom-right";
  dateFormat: string;
  dateTimeFormat: string;
  dateVerboseFormat: string;
  authorInitials: boolean;
  highlightRowsOnHover: boolean;
  smartBranches: boolean;
  autogroupWorktreeTabs: boolean;
  showAgentsView: boolean;
  /** Notifications > Enable Desktop Notifications (showDesktopNotifications: false). */
  desktopNotifications: boolean;
  /** Editor (editor: fontSize 12, tabSize 4, showLineNumbers, syntaxHighlighting, CRLF on Windows). */
  editorFontFamily: string;
  editorFontSize: number;
  editorLineNumbers: boolean;
  editorSyntax: boolean;
  editorTabSize: number;
  editorEol: "lf" | "crlf";
  monospaceOnly: boolean;
  /** In-App Terminal (cli: fontSize 12, lineHeight 1.2, cursor bar, dim when unfocused, PowerShell). */
  termShell: string;
  termFontFamily: string;
  termFontSize: number;
  termLineHeight: number;
  termCursor: "block" | "underline" | "bar";
  termDim: boolean;
  /** External Tools: editor, terminal, coding agent. */
  externalEditor: string;
  editorPath: string;
  editorFileArgs: string;
  editorRepoArgs: string;
  externalTerminal: string;
  terminalPath: string;
  terminalArgs: string;
  codingAgent: string;
  agentArgs: string;
  /** Agents: setup commands for a new agent worktree, and WIP rows for worktrees. */
  agentSetup: string;
  showWorktreeWip: boolean;
  /** Commit: apply the template; strip # comments (templates.useCommitTemplate true, shouldRemoveComments false). */
  useCommitTemplate: boolean;
  stripComments: boolean;
  /** Experimental > Git Executable (empty = git on PATH). */
  gitPath: string;
  ai: AiSettings;
  integrations: IntegrationSettings;
  /** Repo-specific preferences, per repository path. */
  repoPrefs: Record<string, RepoPrefs>;
}

export type AiProvider = "anthropic" | "openai" | "codex" | "claude_code" | "google" | "ollama" | "custom";

export interface AiConnection {
  endpoint: string;
  cliPath: string;
  cliConfigDir: string;
  defaultModel: string;
  responseFormat: "auto" | "json_schema" | "json_object" | "text";
  /** Empty feature models inherit the shared default. */
  models: Record<string, string>;
}

export interface AiSettings extends AiConnection {
  enabled: boolean;
  provider: AiProvider;
  instructions: string;
  featureInstructions: Record<string, string>;
  /** Remember connection details when switching providers; keys stay in the OS credential store. */
  connections: Partial<Record<AiProvider, AiConnection>>;
}

export interface IntegrationSettings {
  githubEnterpriseHost: string;
  gitlabHost: string;
  bitbucketUser: string;
  bitbucketServerHost: string;
  bitbucketServerUser: string;
  azureOrg: string;
  jiraSite: string;
  jiraEmail: string;
  jiraServerHost: string;
  trelloKey: string;
}

export interface RepoPrefs {
  /** Issue Tracker: "" = the hosting service's issues; jira, jira-server, trello, azure. */
  tracker?: { kind: string; project: string };
  /** Conflict Prevention for this repository, base branch patterns and excluded branches (one per line). */
  conflictDetection?: boolean;
  baseBranches?: string;
  excludedBranches?: string;
  /** Encoding: default encoding for reading / writing files. */
  encoding?: string;
}

export const DEFAULT_AI: AiSettings = { enabled: true, provider: "anthropic", endpoint: "", cliPath: "", cliConfigDir: "", defaultModel: "", responseFormat: "auto", instructions: "", models: {}, featureInstructions: {}, connections: {} };
export const DEFAULT_INTEGRATIONS: IntegrationSettings = {
  githubEnterpriseHost: "",
  gitlabHost: "",
  bitbucketUser: "",
  bitbucketServerHost: "",
  bitbucketServerUser: "",
  azureOrg: "",
  jiraSite: "",
  jiraEmail: "",
  jiraServerHost: "",
  trelloKey: "",
};

export interface Workspace {
  id: string;
  name: string;
  repos: string[];
}

export interface Profile {
  id: string;
  name: string;
  gitName: string;
  gitEmail: string;
  settings: Partial<Settings> | null;
}

export interface FormField {
  key: string;
  label: string;
  value: string | boolean;
  type?: "text" | "checkbox";
  placeholder?: string;
  /** Show a Browse button that picks a folder. */
  browse?: boolean;
}

export interface GraphColumns {
  refs: boolean;
  graph: boolean;
  description: boolean;
  author: boolean;
  date: boolean;
  sha: boolean;
  changes: boolean;
}

export const DEFAULT_COLUMNS: GraphColumns = { refs: true, graph: true, description: true, author: false, date: false, sha: false, changes: false };

export const DEFAULT_SETTINGS: Settings = {
  showLeft: true,
  showRight: true,
  leftWidth: 381,
  rightWidth: 342,
  pullMode: "ff",
  zoom: 1,
  recents: [],
  pushAfterCommit: false,
  fileView: "tree",
  fileSort: "path",
  leftView: "list",
  toolbarLabels: true,
  defaultCloneFolder: "",
  diffMode: "hunk",
  ignoreWhitespace: false,
  wordWrap: false,
  columns: DEFAULT_COLUMNS,
  hidden: {},
  solo: {},
  lazyLoad: true,
  autoFetchMin: 1,
  autoPrune: true,
  defaultBranch: "main",
  skipHooks: false,
  keybindings: {},
  showTerminal: false,
  terminalHeight: 220,
  theme: "dark",
  workspaces: [],
  aliases: {},
  tabWidths: {},
  spellcheck: true,
  autoUpdate: true,
  favorites: [],
  templateSummary: "",
  templateDescription: "",
  diffTool: "",
  mergeTool: "",
  notifySuccess: true,
  pinned: {},
  ghostRefs: true,
  gitlabHost: "",
  autoUpdateSubmodules: true,
  deleteOrigAfterMerge: false,
  rememberTabs: true,
  initialCommits: 2000,
  showAllCommits: false,
  extendedLog: false,
  conflictDetection: true,
  useInAppTerminal: false,
  squashMerge: false,
  sshUseAgent: true,
  sshPrivateKey: "",
  sshPublicKey: "",
  notificationLocation: "bottom-left",
  dateFormat: "MM/DD/YYYY",
  dateTimeFormat: "MM/DD/YYYY @ h:mm A",
  dateVerboseFormat: "MMMM D, YYYY h:mm A",
  authorInitials: false,
  highlightRowsOnHover: true,
  smartBranches: false,
  autogroupWorktreeTabs: true,
  showAgentsView: true,
  desktopNotifications: false,
  editorFontFamily: "",
  editorFontSize: 12,
  editorLineNumbers: true,
  editorSyntax: true,
  editorTabSize: 4,
  editorEol: typeof navigator !== "undefined" && /Win/.test(navigator.platform) ? "crlf" : "lf",
  monospaceOnly: true,
  termShell: "powershell",
  termFontFamily: "",
  termFontSize: 12,
  termLineHeight: 1.2,
  termCursor: "bar",
  termDim: true,
  externalEditor: "vscode",
  editorPath: "",
  editorFileArgs: "",
  editorRepoArgs: "",
  externalTerminal: "default",
  terminalPath: "",
  terminalArgs: "",
  codingAgent: "claude",
  agentArgs: "",
  agentSetup: "",
  showWorktreeWip: true,
  useCommitTemplate: true,
  stripComments: false,
  gitPath: "",
  ai: DEFAULT_AI,
  integrations: DEFAULT_INTEGRATIONS,
  repoPrefs: {},
};

/** Same numbers as src/metrics.rs; replaced by the Rust values at start-up. */
export const DEFAULT_METRICS: Metrics = {
  node: 22,
  avatar_inner: 18,
  lane: 22,
  gutter: 28,
  pad: 3,
  graph_zone: 150,
  ref_zone: 130,
  ref_zone_min: 32,
  ref_margin_left: 9,
  ref_margin_right: 6,
  ref_max: 100,
  message_zone: 300,
  message_min: 50,
  summary_margin: 10,
  header: 26,
  row: 28,
  row_inner: 22,
  arc_radius: 11,
  min_alpha: 50,
  sha_len: 6,
  avatar_bg: "#199489",
  lazy_offset: 1400,
  conflict_a: "#15a0bf",
  conflict_b: "#f2ca33",
  conflict_output: "#c517b6",
  time_units: [
    [31556736, "y", "red"],
    [2630016, "M", "red"],
    [604800, "w", "red"],
    [86400, "d", "orange"],
    [3600, "h", "green"],
    [60, "m", "green"],
  ],
  initial_commits: 2000,
  author_zone: 130,
  date_zone: 130,
  sha_zone: 100,
  changes_zone: 200,
  stats_added: "#347d39",
  stats_deleted: "#c93c37",
  stats_files: "#e2c08d",
  summary_counter: 72,
  toolbar_left_max: 369,
  toolbar_center: 461,
  tab_slide_ms: 250,
  wip_stats_space: 75,
  auto_fetch_min: 1,
  zoom_factors: [3, 2.5, 2, 1.75, 1.5, 1.4, 1.3, 1.2, 1.1, 1, 0.9, 0.8],
  default_zoom: 1,
};

let nextId = 1;
export const newId = () => `t${Date.now().toString(36)}${nextId++}`;

const defaultUi = (): TabUi => ({ selection: { type: "wip" }, diff: null, search: "", searchOpen: false, multi: [], compareWorkdir: null, history: null, limit: 0, rebase: null, conflict: null, pr: null, issue: null });

interface Store {
  tabs: Tab[];
  activeId: string;
  closed: Tab[];
  ui: Record<string, TabUi>;
  drafts: Record<string, Draft>;
  settings: Settings;
  toasts: Toast[];
  /** Notification center (the bell): failures stay here after their toast fades. Not saved between runs. */
  notices: Notice[];
  noticeFilter: NoticeFilter;
  /** The bell menu is open (a clicked desktop notification opens it). */
  noticeMenuOpen: boolean;
  setNoticeMenuOpen: (open: boolean) => void;
  setNoticeFilter: (f: NoticeFilter) => void;
  markNotice: (id: number, read: boolean) => void;
  /** "Mark all as read / unread": applies to the notifications the current filter shows. */
  markAllNotices: (read: boolean) => void;
  removeNotice: (id: number) => void;
  dialog: DialogState;
  busy: string | null;
  /** Running clone / fetch / pull / push with its latest progress line. */
  job: import("./api").JobProgress | null;
  /** Command palette: closed, the command list, or one of its pickers. */
  palette: string | null;
  setPalette: (p: string | null) => void;
  profiles: Profile[];
  activeProfile: string;
  renameTab: (id: string, title: string) => void;
  screenshot: boolean;
  version: string;
  metrics: Metrics;

  openRepoTab: (path: string, name: string, group?: string) => void;
  newTab: () => void;
  openLaunchpad: () => void;
  closeTab: (id: string) => void;
  reopenTab: () => void;
  setActive: (id: string) => void;
  /** Move a tab so it sits just before `beforeId` (null: at the end). Dragging tabs and Move Tab Left / Right use it. */
  moveTab: (id: string, beforeId: string | null) => void;
  replaceTab: (id: string, tab: Tab) => void;
  setUi: (id: string, patch: Partial<TabUi>) => void;
  setDraft: (path: string, patch: Partial<Draft>) => void;
  setSettings: (patch: Partial<Settings>) => void;
  toast: (ok: boolean, text: string) => void;
  dropToast: (id: number) => void;
  setDialog: (d: DialogState) => void;
  setBusy: (b: string | null) => void;
}

export const useStore = create<Store>((set, get) => {
  const start: Tab = { id: newId(), kind: "start", title: "New Tab" };
  return {
    tabs: [start],
    activeId: start.id,
    closed: [],
    ui: {},
    drafts: {},
    settings: DEFAULT_SETTINGS,
    toasts: [],
    notices: [],
    noticeFilter: "All",
    noticeMenuOpen: false,
    setNoticeMenuOpen: (noticeMenuOpen) => set({ noticeMenuOpen }),
    setNoticeFilter: (noticeFilter) => set({ noticeFilter }),
    markNotice: (id, read) => set((s) => ({ notices: s.notices.map((n) => (n.id === id ? { ...n, readAt: read ? Date.now() : null } : n)) })),
    markAllNotices: (read) =>
      set((s) => ({ notices: s.notices.map((n) => (noticeVisible(n, s.noticeFilter) ? { ...n, readAt: read ? Date.now() : null } : n)) })),
    removeNotice: (id) => set((s) => ({ notices: s.notices.filter((n) => n.id !== id) })),
    dialog: null,
    busy: null,
    job: null,
    palette: null,
    setPalette: (palette) => set({ palette }),
    profiles: [{ id: "default", name: "Default Profile", gitName: "", gitEmail: "", settings: null }],
    activeProfile: "default",
    renameTab: (id, title) => {
      const s = get();
      const t = s.tabs.find((x) => x.id === id);
      if (!t) return;
      const aliases = { ...s.settings.aliases };
      if (t.path) {
        if (title.trim()) aliases[t.path] = title.trim();
        else delete aliases[t.path];
      }
      set({
        tabs: s.tabs.map((x) => (x.id === id ? { ...x, title: title.trim() || (x.path ? x.path.split(/[\\/]/).pop()! : x.title) } : x)),
        settings: { ...s.settings, aliases },
      });
    },
    screenshot: false,
    version: "0.1.0",
    metrics: DEFAULT_METRICS,

    openRepoTab: (path, name, group) => {
      const s = get();
      const existing = s.tabs.find((t) => t.kind === "repo" && t.path?.toLowerCase() === path.toLowerCase());
      const recents = [path, ...s.settings.recents.filter((r) => r.toLowerCase() !== path.toLowerCase())].slice(0, 20);
      if (existing) {
        set({ activeId: existing.id, settings: { ...s.settings, recents } });
        return;
      }
      const active = s.tabs.find((t) => t.id === s.activeId);
      const tab: Tab = { id: newId(), kind: "repo", path, title: s.settings.aliases?.[path] || name };
      // Opening from a "New Tab" replaces it.
      if (active && active.kind === "start") {
        set({
          tabs: s.tabs.map((t) => (t.id === active.id ? tab : t)),
          activeId: tab.id,
          ui: { ...s.ui, [tab.id]: defaultUi() },
          settings: { ...s.settings, recents },
        });
      } else {
        // UI Customization > Automatically group worktree tabs together: a linked worktree opens next to its repository.
        const norm = (p?: string) => (p ?? "").replace(/\\/g, "/").toLowerCase();
        const at = group && s.settings.autogroupWorktreeTabs !== false ? s.tabs.map((t) => norm(t.path) === norm(group)).lastIndexOf(true) : -1;
        const tabs = at >= 0 ? [...s.tabs.slice(0, at + 1), tab, ...s.tabs.slice(at + 1)] : [...s.tabs, tab];
        set({ tabs, activeId: tab.id, ui: { ...s.ui, [tab.id]: defaultUi() }, settings: { ...s.settings, recents } });
      }
    },
    newTab: () => {
      const tab: Tab = { id: newId(), kind: "start", title: "New Tab" };
      set((s) => ({ tabs: [...s.tabs, tab], activeId: tab.id }));
    },
    openLaunchpad: () => {
      const s = get();
      const lp = s.tabs.find((t) => t.kind === "launchpad");
      if (lp) return set({ activeId: lp.id });
      const tab: Tab = { id: newId(), kind: "launchpad", title: "Launchpad" };
      set({ tabs: [tab, ...s.tabs], activeId: tab.id });
    },
    closeTab: (id) => {
      const s = get();
      const idx = s.tabs.findIndex((t) => t.id === id);
      if (idx < 0) return;
      const tab = s.tabs[idx];
      let tabs = s.tabs.filter((t) => t.id !== id);
      if (tabs.length === 0) tabs = [{ id: newId(), kind: "start", title: "New Tab" }];
      const activeId = s.activeId === id ? tabs[Math.min(idx, tabs.length - 1)].id : s.activeId;
      set({ tabs, activeId, closed: tab.kind === "repo" ? [...s.closed, tab].slice(-10) : s.closed });
    },
    reopenTab: () => {
      const s = get();
      const last = s.closed[s.closed.length - 1];
      if (!last) return;
      set({ closed: s.closed.slice(0, -1) });
      get().openRepoTab(last.path!, last.title);
    },
    setActive: (id) => set({ activeId: id }),
    moveTab: (id, beforeId) =>
      set((s) => {
        const tab = s.tabs.find((t) => t.id === id);
        if (!tab || id === beforeId) return {};
        const rest = s.tabs.filter((t) => t.id !== id);
        const at = beforeId ? rest.findIndex((t) => t.id === beforeId) : -1;
        const tabs = at < 0 ? [...rest, tab] : [...rest.slice(0, at), tab, ...rest.slice(at)];
        return tabs.every((t, i) => t === s.tabs[i]) ? {} : { tabs };
      }),
    replaceTab: (id, tab) => set((s) => ({ tabs: s.tabs.map((t) => (t.id === id ? tab : t)), activeId: tab.id, ui: { ...s.ui, [tab.id]: defaultUi() } })),
    setUi: (id, patch) => set((s) => ({ ui: { ...s.ui, [id]: { ...(s.ui[id] ?? defaultUi()), ...patch } } })),
    setDraft: (path, patch) =>
      set((s) => ({ drafts: { ...s.drafts, [path]: { ...(s.drafts[path] ?? { summary: "", description: "", amend: false }), ...patch } } })),
    setSettings: (patch) => set((s) => ({ settings: { ...s.settings, ...patch } })),
    toast: (ok, text) => {
      const id = Date.now() + Math.random();
      set((s) => {
        const notices = ok ? s.notices : [...s.notices.slice(-(NOTICE_LIMIT - 1)), { id, text, at: Date.now(), readAt: null }];
        return { toasts: [...s.toasts.slice(-3), { id, ok, text }], notices };
      });
      setTimeout(() => get().dropToast(id), ok ? 3500 : 7000);
    },
    dropToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
    setDialog: (dialog) => set({ dialog }),
    setBusy: (busy) => set({ busy }),
  };
});

/** Bring back the saved repository tabs in one step (start-up, General > Remember tabs): names from the alias or the
 *  folder, the saved active tab showing. Nothing is read here; each tab loads its repository when first shown. */
export function restoreTabs(paths: string[], active: number) {
  const st = useStore.getState();
  const tabs: Tab[] = paths.map((path) => ({ id: newId(), kind: "repo", path, title: st.settings.aliases?.[path] || path.split(/[\\/]/).filter(Boolean).pop() || path }));
  if (tabs.length === 0) return;
  const ui = { ...st.ui };
  for (const t of tabs) ui[t.id] = defaultUi();
  // The empty "New Tab" shown before start-up finished is replaced; anything else stays after the restored tabs.
  const keep = st.tabs.filter((t) => t.kind !== "start");
  useStore.setState({ tabs: [...tabs, ...keep], ui, activeId: (tabs[active] ?? tabs[0]).id });
}

export function tabUi(id: string): TabUi {
  return useStore.getState().ui[id] ?? defaultUi();
}

export function useTabUi(id: string): TabUi {
  return useStore((s) => s.ui[id]) ?? defaultUiConst;
}
const defaultUiConst = defaultUi();

/** The whole store except the busy note, job progress and toasts. Those change several times during every
 *  action; big panels (file lists, branch lists, diffs) use this so they don't re-render for them. */
export function usePanelStore() {
  return useStore(
    useShallow((st: Store) => {
      const { busy, job, toasts, ...rest } = st;
      void busy;
      void job;
      void toasts;
      return rest;
    }),
  );
}
