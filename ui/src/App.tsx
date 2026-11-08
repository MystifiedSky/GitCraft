import { listen } from "@tauri-apps/api/event";
import { ChevronRight } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api } from "@/lib/api";
import { DEFAULT_METRICS, DEFAULT_SETTINGS, newId, tabUi, useStore, type Settings, type Tab } from "@/lib/store";
import { act, openRepoPath, promptBranch, queryClient, refresh, repoData, useRepo } from "@/lib/repo";
import { resetZoom, setZoom, stepZoom } from "@/lib/zoom";
import { MenuBar, TitleBar } from "./components/TitleBar";
import { TabsBar } from "./components/TabsBar";
import { Toolbar } from "./components/Toolbar";
import { LeftPanel } from "./components/LeftPanel";
import { GraphView } from "./components/GraphView";
import { DiffView } from "./components/DiffView";
import { HistoryView } from "./components/HistoryView";
import { LeftStrip } from "./components/LeftPanel";
import { RightPanel } from "./components/RightPanel";
import { StatusBar } from "./components/StatusBar";
import { LaunchpadScreen, StartScreen } from "./components/StartScreen";
import { Dialogs, Toasts } from "./components/Dialogs";

export default function App() {
  const s = useStore();
  const active = s.tabs.find((t) => t.id === s.activeId) ?? s.tabs[0];
  useStartup();
  useShortcuts();
  useWatchers();

  return (
    <div className="h-full flex flex-col">
      <TitleBar />
      <MenuBar />
      <TabsBar />
      {active.kind === "repo" ? <RepoScreen key={active.id} tab={active} /> : active.kind === "launchpad" ? <LaunchpadScreen /> : <StartScreen />}
      <StatusBar />
      <Dialogs />
      <Toasts />
    </div>
  );
}

function RepoScreen({ tab }: { tab: Tab }) {
  const s = useStore();
  const q = useRepo(tab.path, tab.id);
  const viewing = !!(s.ui[tab.id]?.diff || s.ui[tab.id]?.history);
  const [peek, setPeek] = useState(false);
  useEffect(() => setPeek(false), [viewing]);
  const repo = q.data;
  const ui = s.ui[tab.id];
  const set = s.settings;

  const selectSha = (sha: string) => s.setUi(tab.id, { selection: { type: "commit", sha }, diff: null });

  return (
    <>
      <Toolbar repo={repo} tabId={tab.id} />
      <div className="flex-1 flex min-h-0">
        {set.showLeft && !(viewing && !peek) ? (
          <>
            <div style={{ width: set.leftWidth }} className="shrink-0 min-w-0">
              <LeftPanel repo={repo} tabId={tab.id} onSelectSha={selectSha} />
            </div>
            <Resizer onDrag={(dx) => s.setSettings({ leftWidth: clamp(useStore.getState().settings.leftWidth + dx, 180, 600) })} />
          </>
        ) : (
          // Like the original: while a file is open (or the panel is hidden) the left panel shrinks to icons.
          <LeftStrip repo={repo} onExpand={() => (set.showLeft ? setPeek(true) : s.setSettings({ showLeft: true }))} />
        )}
        <div className="flex-1 min-w-[160px]">
          {q.isError && (
            <div className="h-full flex flex-col items-center justify-center gap-3 text-fg2 px-6 text-center">
              <div className="text-white text-[15px]">This repository could not be opened</div>
              <div className="text-[12.5px]">{String(q.error)}</div>
              <button className="text-link hover:underline" onClick={() => s.closeTab(tab.id)}>
                Close tab
              </button>
            </div>
          )}
          {!repo && q.isLoading && <div className="h-full flex items-center justify-center text-fg3">Loading repository…</div>}
          {repo &&
            (ui?.diff ? (
              <DiffView repo={repo} tabId={tab.id} target={ui.diff} />
            ) : ui?.history ? (
              <HistoryView repo={repo} tabId={tab.id} file={ui.history.file} mode={ui.history.mode} sha={ui.history.sha} />
            ) : (
              <GraphView repo={repo} tabId={tab.id} />
            ))}
        </div>
        {set.showRight && repo && (
          <>
            <Resizer onDrag={(dx) => s.setSettings({ rightWidth: clamp(useStore.getState().settings.rightWidth - dx, 260, 640) })} />
            <div style={{ width: set.rightWidth }} className="shrink-0 min-w-0">
              <RightPanel repo={repo} tabId={tab.id} />
            </div>
          </>
        )}
      </div>
    </>
  );
}

function Resizer({ onDrag }: { onDrag: (dx: number) => void }) {
  return (
    <div
      className="w-[2px] shrink-0 bg-[#2e3034] hover:bg-accent cursor-col-resize relative z-10"
      onPointerDown={(e) => {
        let x = e.clientX;
        const el = e.currentTarget;
        el.setPointerCapture(e.pointerId);
        const move = (ev: PointerEvent) => {
          onDrag(ev.clientX - x);
          x = ev.clientX;
        };
        const up = () => {
          el.removeEventListener("pointermove", move);
          el.removeEventListener("pointerup", up);
        };
        el.addEventListener("pointermove", move);
        el.addEventListener("pointerup", up);
      }}
    />
  );
}

const clamp = (v: number, a: number, b: number) => Math.min(b, Math.max(a, v));

/** Load settings and open tabs; in --screenshot mode open the sample project and report when drawn. */
function useStartup() {
  const done = useRef(false);
  useEffect(() => {
    if (done.current) return;
    done.current = true;
    (async () => {
      const opts = await api.launchOptions().catch(() => ({ screenshot: false, shot_view: null as string | null, version: "0.1.0", metrics: DEFAULT_METRICS }));
      useStore.setState({ screenshot: opts.screenshot, version: opts.version, metrics: opts.metrics });
      if (opts.screenshot) {
        const sample = await api.sampleRepo().catch(() => null);
        if (sample) await openRepoPath(sample);
        // Wait until the repository has drawn, then about 120 frames, then take the picture.
        const t0 = performance.now();
        while (performance.now() - t0 < 15000) {
          const tab = useStore.getState().tabs.find((t) => t.kind === "repo");
          if (tab?.path && repoData(tab.path)) break;
          await new Promise((r) => setTimeout(r, 100));
        }
        // Test aid: open another screen before the picture.
        const tab = useStore.getState().tabs.find((t) => t.kind === "repo");
        const repo = tab?.path ? repoData(tab.path) : undefined;
        if (tab && repo && opts.shot_view) {
          const st = useStore.getState();
          const file = repo.status.unstaged[0];
          if (opts.shot_view === "diff" || opts.shot_view === "split" || opts.shot_view === "inline") {
            st.setSettings({ diffMode: opts.shot_view === "diff" ? "hunk" : opts.shot_view, showLeft: true });
            if (file) st.setUi(tab.id, { diff: { source: { kind: "Unstaged" }, file: file.path, untracked: file.status === "?", parent: null } });
          } else if (opts.shot_view === "history" || opts.shot_view === "blame") {
            st.setUi(tab.id, { history: { file: "src/api.rs", mode: opts.shot_view, sha: null } });
          } else if (opts.shot_view === "compare" && repo.commits.length > 3) {
            st.setUi(tab.id, { multi: [repo.commits[0].sha, repo.commits[3].sha], selection: { type: "commit", sha: repo.commits[0].sha } });
            st.setSettings({ columns: { ...st.settings.columns, author: true, changes: true } });
          } else if (opts.shot_view === "start") st.newTab();
          await new Promise((r) => setTimeout(r, 1500));
        }
        let frames = 0;
        await new Promise<void>((res) => {
          const tick = () => (++frames >= 120 ? res() : requestAnimationFrame(tick));
          requestAnimationFrame(tick);
        });
        await api.screenshotReady();
        return;
      }
      const saved = (await api.loadSettings().catch(() => null)) as { settings?: Partial<Settings>; tabs?: string[]; active?: number } | null;
      if (saved?.settings) useStore.setState({ settings: { ...DEFAULT_SETTINGS, ...saved.settings } });
      if (saved?.settings?.zoom && saved.settings.zoom !== 1) setZoom(saved.settings.zoom);
      for (const p of saved?.tabs ?? []) await openRepoPath(p);
      const tabs = useStore.getState().tabs;
      const repoTabs = tabs.filter((t) => t.kind === "repo");
      if (saved?.active !== undefined && repoTabs[saved.active]) useStore.getState().setActive(repoTabs[saved.active].id);
      // Persist settings and open tabs whenever they change.
      let timer: number | undefined;
      useStore.subscribe((st, prev) => {
        if (st.settings === prev.settings && st.tabs === prev.tabs && st.activeId === prev.activeId) return;
        clearTimeout(timer);
        timer = window.setTimeout(() => {
          const rt = st.tabs.filter((t) => t.kind === "repo");
          api.saveSettings({ settings: st.settings, tabs: rt.map((t) => t.path), active: Math.max(0, rt.findIndex((t) => t.id === st.activeId)) });
        }, 400);
      });
    })();
  }, []);
}

/** Watch open repositories and refresh when files change. */
function useWatchers() {
  const tabs = useStore((s) => s.tabs);
  const watched = useRef(new Set<string>());
  useEffect(() => {
    const un = listen<string>("repo-changed", (e) => {
      refresh(e.payload);
    });
    return () => {
      un.then((f) => f());
    };
  }, []);
  useEffect(() => {
    const now = new Set(tabs.filter((t) => t.kind === "repo" && t.path).map((t) => t.path!));
    for (const p of now) if (!watched.current.has(p)) api.watchRepo(p).catch(() => {});
    for (const p of watched.current) if (!now.has(p)) api.unwatchRepo(p).catch(() => {});
    watched.current = now;
  }, [tabs]);
}

/** Keyboard shortcuts from the original's key bindings (Ctrl on Windows). */
function useShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const st = useStore.getState();
      const tab = st.tabs.find((t) => t.id === st.activeId);
      const path = tab?.kind === "repo" ? tab.path : undefined;
      const mod = e.ctrlKey || e.metaKey;
      const k = e.key.toLowerCase();
      const typing = (e.target as HTMLElement)?.closest?.("input, textarea, select") != null;
      const run = (f: () => void) => {
        e.preventDefault();
        f();
      };

      if (mod && !e.shiftKey && !e.altKey) {
        if (k === "t") return run(st.newTab);
        if (k === "w") return run(() => st.closeTab(st.activeId));
        if (k === "o") return run(() => st.setDialog({ kind: "open" }));
        if (k === "n") return run(() => st.setDialog({ kind: "clone" }));
        if (k === "i") return run(() => st.setDialog({ kind: "init" }));
        if (k === "j") return run(() => st.setSettings({ showLeft: !st.settings.showLeft }));
        if (k === "k") return run(() => st.setSettings({ showRight: !st.settings.showRight }));
        if (k === "=" || k === "+") return run(() => stepZoom(1));
        if (k === "-") return run(() => stepZoom(-1));
        if (k === "0") return run(resetZoom);
        if (k === "/") return run(() => st.setDialog({ kind: "shortcuts" }));
        if (k === ",") return run(() => st.setDialog({ kind: "prefs" }));
        if (k === "tab") return run(() => cycleTab(1));
        if (path) {
          if (k === "b") return run(() => promptBranch(path, null));
          if (k === "f") return run(() => st.setUi(tab!.id, { searchOpen: true }));
          if (k === "l") return run(() => act(path, { type: "fetch" }));
          if (!typing && k === "z") return run(() => act(path, { type: "undo" }));
          if (!typing && k === "y") return run(() => act(path, { type: "redo" }));
        }
      }
      if (mod && e.shiftKey && !e.altKey) {
        if (k === "t") return run(st.reopenTab);
        if (k === "tab") return run(() => cycleTab(-1));
        if (path) {
          if (k === "s") return run(() => act(path, { type: "stageAll" }, true));
          if (k === "u") return run(() => act(path, { type: "unstageAll" }, true));
          if (k === "z" && !typing) return run(() => act(path, { type: "redo" }));
          if (k === "h") return run(() => st.setDialog({ kind: "fileFinder" }));
          if (k === "e") return run(() => api.openInEditor(path));
          if (k === "m")
            return run(() => {
              st.setUi(tab!.id, { selection: { type: "wip" }, diff: null });
              setTimeout(() => (document.querySelector("[data-commit-summary]") as HTMLInputElement | null)?.focus(), 50);
            });
        }
      }
      if (e.altKey && !mod && path && k === "t") return run(() => api.openTerminal(path));
      if (e.altKey && !mod && path && k === "o") return run(() => api.openFileManager(path));
      if (mod && e.altKey && k === "f") return run(() => (window as unknown as { focusLeftFilter?: () => void }).focusLeftFilter?.());
      if (e.key === "F11") return run(async () => {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const w = getCurrentWindow();
        w.setFullscreen(!(await w.isFullscreen()));
      });
      if (typing || mod || e.altKey) return;
      if (e.key === "Escape" && tab) {
        const u = tabUi(tab.id);
        if (st.dialog) return;
        if (u.diff) return run(() => st.setUi(tab.id, { diff: null }));
        if (u.history) return run(() => st.setUi(tab.id, { history: null }));
        if (u.multi.length || u.compareWorkdir) return run(() => st.setUi(tab.id, { multi: [], compareWorkdir: null }));
        if (u.searchOpen) return run(() => st.setUi(tab.id, { searchOpen: false, search: "" }));
        if (u.selection.type === "commit") return run(() => st.setUi(tab.id, { selection: { type: "wip" } }));
      }
      if (path && tab && !st.dialog) {
        const repo = repoData(path);
        if (!repo) return;
        if (tabUi(tab.id).diff || tabUi(tab.id).history) return; // keys belong to the open file view
        const down = e.key === "ArrowDown" || k === "j";
        const up = e.key === "ArrowUp" || k === "k";
        if (down || up) {
          const wip = repo.status.unstaged.length + repo.status.staged.length > 0;
          const u = tabUi(tab.id);
          const ids: (string | null)[] = [...(wip ? [null] : []), ...repo.commits.map((c) => c.sha)];
          const cur = u.selection.type === "wip" ? (wip ? 0 : -1) : ids.indexOf(u.selection.sha);
          const next = Math.min(ids.length - 1, Math.max(0, cur + (down ? 1 : -1)));
          const id = ids[next];
          return run(() => st.setUi(tab.id, { selection: id === null || id === undefined ? { type: "wip" } : { type: "commit", sha: id }, diff: null, multi: [], compareWorkdir: null }));
        }
        if (k === "s" && tabUi(tab.id).diff?.source.kind === "Unstaged") {
          const f = tabUi(tab.id).diff!.file;
          return run(() => act(path, { type: "stage", paths: [f] }, true));
        }
        if (k === "u" && tabUi(tab.id).diff?.source.kind === "Staged") {
          const f = tabUi(tab.id).diff!.file;
          return run(() => act(path, { type: "unstage", paths: [f] }, true));
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}

function cycleTab(dir: number) {
  const st = useStore.getState();
  const i = st.tabs.findIndex((t) => t.id === st.activeId);
  const n = st.tabs.length;
  st.setActive(st.tabs[(i + dir + n) % n].id);
}

// Keep imports used for side effects.
void queryClient;
void newId;
