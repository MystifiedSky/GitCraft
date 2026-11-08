import { create } from "zustand";
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
}

export interface Draft {
  summary: string;
  description: string;
  amend: boolean;
}

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
  | { kind: "prefs" };

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
  leftView: "list" | "agents";
  toolbarLabels: boolean;
  defaultCloneFolder: string;
}

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
  leftView: "list",
  toolbarLabels: true,
  defaultCloneFolder: "",
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
  summary_counter: 72,
  auto_fetch_min: 1,
  zoom_factors: [3, 2.5, 2, 1.75, 1.5, 1.4, 1.3, 1.2, 1.1, 1, 0.9, 0.8],
  default_zoom: 1,
};

let nextId = 1;
export const newId = () => `t${Date.now().toString(36)}${nextId++}`;

const defaultUi = (): TabUi => ({ selection: { type: "wip" }, diff: null, search: "", searchOpen: false });

interface Store {
  tabs: Tab[];
  activeId: string;
  closed: Tab[];
  ui: Record<string, TabUi>;
  drafts: Record<string, Draft>;
  settings: Settings;
  toasts: Toast[];
  dialog: DialogState;
  busy: string | null;
  screenshot: boolean;
  version: string;
  metrics: Metrics;

  openRepoTab: (path: string, name: string) => void;
  newTab: () => void;
  openLaunchpad: () => void;
  closeTab: (id: string) => void;
  reopenTab: () => void;
  setActive: (id: string) => void;
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
    dialog: null,
    busy: null,
    screenshot: false,
    version: "0.1.0",
    metrics: DEFAULT_METRICS,

    openRepoTab: (path, name) => {
      const s = get();
      const existing = s.tabs.find((t) => t.kind === "repo" && t.path?.toLowerCase() === path.toLowerCase());
      const recents = [path, ...s.settings.recents.filter((r) => r.toLowerCase() !== path.toLowerCase())].slice(0, 20);
      if (existing) {
        set({ activeId: existing.id, settings: { ...s.settings, recents } });
        return;
      }
      const active = s.tabs.find((t) => t.id === s.activeId);
      const tab: Tab = { id: newId(), kind: "repo", path, title: name };
      // Opening from a "New Tab" replaces it, like the original.
      if (active && active.kind === "start") {
        set({
          tabs: s.tabs.map((t) => (t.id === active.id ? tab : t)),
          activeId: tab.id,
          ui: { ...s.ui, [tab.id]: defaultUi() },
          settings: { ...s.settings, recents },
        });
      } else {
        set({ tabs: [...s.tabs, tab], activeId: tab.id, ui: { ...s.ui, [tab.id]: defaultUi() }, settings: { ...s.settings, recents } });
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
    replaceTab: (id, tab) => set((s) => ({ tabs: s.tabs.map((t) => (t.id === id ? tab : t)), activeId: tab.id, ui: { ...s.ui, [tab.id]: defaultUi() } })),
    setUi: (id, patch) => set((s) => ({ ui: { ...s.ui, [id]: { ...(s.ui[id] ?? defaultUi()), ...patch } } })),
    setDraft: (path, patch) =>
      set((s) => ({ drafts: { ...s.drafts, [path]: { ...(s.drafts[path] ?? { summary: "", description: "", amend: false }), ...patch } } })),
    setSettings: (patch) => set((s) => ({ settings: { ...s.settings, ...patch } })),
    toast: (ok, text) => {
      const id = Date.now() + Math.random();
      set((s) => ({ toasts: [...s.toasts.slice(-3), { id, ok, text }] }));
      setTimeout(() => get().dropToast(id), ok ? 3500 : 7000);
    },
    dropToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
    setDialog: (dialog) => set({ dialog }),
    setBusy: (busy) => set({ busy }),
  };
});

export function tabUi(id: string): TabUi {
  return useStore.getState().ui[id] ?? defaultUi();
}

export function useTabUi(id: string): TabUi {
  return useStore((s) => s.ui[id]) ?? defaultUiConst;
}
const defaultUiConst = defaultUi();
