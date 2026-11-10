import { startBackendSync } from "@/lib/integrations";
import { syncGitCredentials } from "@/lib/hosting";
import { onWatcherChange } from "@/lib/repo";
import { startUpdateChecks } from "@/lib/updates";
import { listen } from "@tauri-apps/api/event";
import { ChevronRight } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api } from "@/lib/api";
import { DEFAULT_METRICS, DEFAULT_SETTINGS, newId, tabUi, useStore, type Profile, type Settings, type Tab } from "@/lib/store";
import { act, openRepoPath, promptBranch, queryClient, refresh, repoData, useRepo } from "@/lib/repo";
import { resetZoom, setZoom, stepZoom } from "@/lib/zoom";
import { MenuBar, TitleBar } from "./components/TitleBar";
import { CommandPalette } from "./components/CommandPalette";
import { dispatchKey } from "@/lib/commands";
import { applyTheme } from "@/lib/prefs";
import { TabsBar } from "./components/TabsBar";
import { Toolbar } from "./components/Toolbar";
import { LeftPanel } from "./components/LeftPanel";
import { GraphView } from "./components/GraphView";
import { DiffView } from "./components/DiffView";
import { HistoryView } from "./components/HistoryView";
import { RebaseView } from "./components/RebaseView";
import { openRebase } from "@/lib/rewrite";
import { ConflictView } from "./components/ConflictView";
import { TerminalPanel } from "./components/TerminalPanel";
import { IssueView, PullRequestView } from "./components/PullRequestView";
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
  useAutoFetch();
  useUpdateChecks();
  const theme = useStore((x) => x.settings.theme);
  useEffect(() => applyTheme(theme), [theme]);

  return (
    <div className="h-full flex flex-col">
      <TitleBar />
      <MenuBar />
      <TabsBar />
      {active.kind === "repo" ? <RepoScreen key={active.id} tab={active} /> : active.kind === "launchpad" ? <LaunchpadScreen /> : <StartScreen />}
      <StatusBar />
      <Dialogs />
      <CommandPalette />
      <Toasts />
    </div>
  );
}

function RepoScreen({ tab }: { tab: Tab }) {
  const s = useStore();
  const q = useRepo(tab.path, tab.id);
  const viewing = !!(s.ui[tab.id]?.diff || s.ui[tab.id]?.history || s.ui[tab.id]?.conflict || s.ui[tab.id]?.rebase);
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
        <div className="flex-1 min-w-[160px] flex flex-col min-h-0">
          <div className="flex-1 min-h-0">
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
            (ui?.pr ? (
              <PullRequestView repo={repo} tabId={tab.id} number={ui.pr} />
            ) : ui?.issue ? (
              <IssueView repo={repo} tabId={tab.id} number={ui.issue} />
            ) : ui?.rebase ? (
              <RebaseView tabId={tab.id} path={repo.path} plan={ui.rebase} />
            ) : ui?.conflict ? (
              <ConflictView repo={repo} tabId={tab.id} file={ui.conflict} />
            ) : ui?.diff ? (
              <DiffView repo={repo} tabId={tab.id} target={ui.diff} />
            ) : ui?.history ? (
              <HistoryView repo={repo} tabId={tab.id} file={ui.history.file} mode={ui.history.mode} sha={ui.history.sha} />
            ) : (
              <GraphView repo={repo} tabId={tab.id} />
            ))}
          </div>
          {set.showTerminal && repo && <TerminalPanel cwd={repo.path} />}
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
      className="w-[2px] shrink-0 bg-line hover:bg-accent cursor-col-resize relative z-10"
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
        const sample = await (opts.shot_view === "conflict" ? api.conflictSample() : api.sampleRepo()).catch(() => null);
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
          else if (opts.shot_view === "conflict") st.setUi(tab.id, { conflict: "settings.txt" });
          else if (opts.shot_view === "palette") st.setPalette("root");
          else if (opts.shot_view === "prefs") st.setDialog({ kind: "prefs" });
          else if (opts.shot_view === "integrations") st.setDialog({ kind: "prefs", section: "integrations" });
          else if (opts.shot_view === "light" || opts.shot_view === "dark-hc") st.setSettings({ theme: opts.shot_view });
          else if (opts.shot_view === "terminal") {
            st.setSettings({ showTerminal: true });
            await new Promise((r) => setTimeout(r, 2500));
          }
          else if (opts.shot_view === "rebase" && repo.commits.length > 4) {
            await openRebase(tab.id, repo.path, repo.commits[4].sha, repo.commits[4].sha.slice(0, st.metrics.sha_len), (items) =>
              items.map((it, i) => (i === 1 ? { ...it, action: "squash" } : i === 2 ? { ...it, action: "reword", message: it.message + " (reworded)" } : i === 3 ? { ...it, action: "drop" } : it)),
            );
          }
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
      const saved = (await api.loadSettings().catch(() => null)) as {
        settings?: Partial<Settings>;
        tabs?: string[];
        active?: number;
        profiles?: Profile[];
        activeProfile?: string;
      } | null;
      if (saved?.settings) useStore.setState({ settings: { ...DEFAULT_SETTINGS, ...saved.settings } });
      if (saved?.profiles?.length) useStore.setState({ profiles: saved.profiles, activeProfile: saved.activeProfile ?? saved.profiles[0].id });
      if (saved?.settings?.zoom && saved.settings.zoom !== 1) setZoom(saved.settings.zoom);
      startBackendSync();
      await syncGitCredentials();
      // General > Remember tabs.
      if (useStore.getState().settings.rememberTabs) for (const p of saved?.tabs ?? []) await openRepoPath(p);
      const tabs = useStore.getState().tabs;
      const repoTabs = tabs.filter((t) => t.kind === "repo");
      if (saved?.active !== undefined && repoTabs[saved.active]) useStore.getState().setActive(repoTabs[saved.active].id);
      // Persist settings and open tabs whenever they change.
      let timer: number | undefined;
      useStore.subscribe((st, prev) => {
        if (st.settings === prev.settings && st.tabs === prev.tabs && st.activeId === prev.activeId && st.profiles === prev.profiles && st.activeProfile === prev.activeProfile) return;
        clearTimeout(timer);
        timer = window.setTimeout(() => {
          const rt = st.tabs.filter((t) => t.kind === "repo");
          api.saveSettings({
            settings: st.settings,
            tabs: rt.map((t) => t.path),
            active: Math.max(0, rt.findIndex((t) => t.id === st.activeId)),
            profiles: st.profiles,
            activeProfile: st.activeProfile,
          });
        }, 400);
      });
    })();
  }, []);
}

/** Look for GitCraft updates shortly after start-up and every few hours (not in screenshot mode). */
function useUpdateChecks() {
  const screenshot = useStore((s) => s.screenshot);
  const auto = useStore((s) => s.settings.autoUpdate);
  useEffect(() => (screenshot || !auto ? undefined : startUpdateChecks()), [screenshot, auto]);
}

/** Auto-fetch every open repository every N minutes (0 = off), pruning if enabled. A failure is
 *  reported once per repository, like the original's "Autofetch failed" notice. */
function useAutoFetch() {
  const minutes = useStore((s) => s.settings.autoFetchMin);
  const prune = useStore((s) => s.settings.autoPrune);
  const screenshot = useStore((s) => s.screenshot);
  const warned = useRef(new Set<string>());
  useEffect(() => {
    if (!minutes || screenshot) return;
    const id = window.setInterval(async () => {
      const paths = [...new Set(useStore.getState().tabs.filter((t) => t.kind === "repo" && t.path).map((t) => t.path!))];
      for (const p of paths) {
        if (!repoData(p)?.remotes.length) continue;
        const r = await api.run(p, { type: "autoFetch", prune }).catch(() => ({ ok: false, message: "Autofetch failed" }));
        if (r.ok) {
          warned.current.delete(p);
          refresh(p);
        } else if (!warned.current.has(p)) {
          warned.current.add(p);
          useStore.getState().toast(false, r.message);
        }
      }
    }, minutes * 60_000);
    return () => clearInterval(id);
  }, [minutes, prune, screenshot]);
}

/** Watch open repositories and refresh when files change. */
function useWatchers() {
  const tabs = useStore((s) => s.tabs);
  const watched = useRef(new Set<string>());
  useEffect(() => {
    const un = listen<import("@/lib/api").JobProgress>("job-progress", (e) => {
      useStore.setState({ job: e.payload.text ? e.payload : null });
    });
    return () => {
      un.then((f) => f());
    };
  }, []);
  useEffect(() => {
    const un = listen<{ path: string; full: boolean }>("repo-changed", (e) => {
      onWatcherChange(e.payload.path, e.payload.full);
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

      // Every Ctrl / Alt / F-key shortcut comes from the command list (and the person's own keys).
      if (dispatchKey(e, typing)) return;
      if (typing || mod || e.altKey) return;
      if (e.key === "Escape" && tab) {
        const u = tabUi(tab.id);
        if (st.dialog) return;
        if (u.pr || u.issue) return run(() => st.setUi(tab.id, { pr: null, issue: null }));
        if (u.rebase) return run(() => st.setUi(tab.id, { rebase: null }));
        if (u.conflict) return run(() => st.setUi(tab.id, { conflict: null }));
        if (u.diff) return run(() => st.setUi(tab.id, { diff: null }));
        if (u.history) return run(() => st.setUi(tab.id, { history: null }));
        if (u.multi.length || u.compareWorkdir) return run(() => st.setUi(tab.id, { multi: [], compareWorkdir: null }));
        if (u.searchOpen) return run(() => st.setUi(tab.id, { searchOpen: false, search: "" }));
        if (u.selection.type === "commit") return run(() => st.setUi(tab.id, { selection: { type: "wip" } }));
      }
      if (path && tab && !st.dialog) {
        const repo = repoData(path);
        if (!repo) return;
        if (tabUi(tab.id).diff || tabUi(tab.id).history || tabUi(tab.id).rebase || tabUi(tab.id).conflict) return; // keys belong to the open view
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
