import { QueryClient, useQuery } from "@tanstack/react-query";
import { api, type Action, type FileChange, type GitRef, type RefView, type RepoView } from "./api";
import { useStore } from "./store";

export const queryClient = new QueryClient({
  defaultOptions: { queries: { refetchOnWindowFocus: true, staleTime: 2000, retry: false } },
});

/** Latest data per repository, whatever graph view (limit / hidden / solo) it was read with. */
const latest = new Map<string, RepoView>();

export function refView(path: string): RefView {
  const st = useStore.getState().settings;
  return { hidden: st.hidden[path] ?? [], solo: st.solo[path] ?? [] };
}

export function useRepo(path: string | undefined, tabId: string) {
  const limit = useStore((s) => s.ui[tabId]?.limit ?? 0);
  const hidden = useStore((s) => (path ? s.settings.hidden[path] : undefined));
  const solo = useStore((s) => (path ? s.settings.solo[path] : undefined));
  const pinned = useStore((s) => (path ? s.settings.pinned?.[path] : undefined));
  const view: RefView = { hidden: hidden ?? [], solo: solo ?? [], pinned: pinned ?? null };
  return useQuery({
    queryKey: ["repo", path, limit, view],
    queryFn: async () => {
      const r = await api.openRepo(path!, limit || undefined, view);
      latest.set(path!, r);
      return r;
    },
    enabled: !!path,
    // The file watcher already reports every change, so switching back to the window doesn't reload everything.
    refetchOnWindowFocus: false,
    placeholderData: (prev) => prev,
    initialData: () => (path && limit === 0 && view.hidden.length === 0 && view.solo.length === 0 ? latest.get(path) : undefined),
  });
}

export function repoData(path: string): RepoView | undefined {
  return latest.get(path);
}

/** When this window last finished its own full refresh of each repository (see `onWatcherChange`). */
const lastOwnRefresh = new Map<string, number>();

export function refresh(path: string) {
  return queryClient.invalidateQueries({ queryKey: ["repo", path] });
}

/** Put new file lists into every cached view of the repository, keeping commits and graph as they are
 *  (same objects), so only the file panels re-render. */
function setStatus(path: string, status: RepoView["status"]) {
  queryClient.setQueriesData<RepoView>({ queryKey: ["repo", path] }, (old) => (old ? { ...old, status } : old));
  const l = latest.get(path);
  if (l) latest.set(path, { ...l, status });
}

/** Re-read only the file lists (one `git status`, ~25 ms) instead of the whole repository. */
export async function refreshStatus(path: string) {
  try {
    setStatus(path, await api.repoStatus(path));
    // Open diffs of these files may show different lines now.
    queryClient.invalidateQueries({ queryKey: ["diff", path] });
    queryClient.invalidateQueries({ queryKey: ["versions", path] });
  } catch {
    await refresh(path);
  }
}

/** File watcher: working-folder edits and index changes refresh the file lists only; anything else
 *  (commits, branches, HEAD) reloads the repository, unless this window's own action just did that. */
export function onWatcherChange(path: string, full: boolean) {
  if (!full) return void refreshStatus(path);
  if (Date.now() - (lastOwnRefresh.get(path) ?? 0) < OWN_CHANGE_WINDOW_MS) return;
  void refresh(path);
}
const OWN_CHANGE_WINDOW_MS = 1000;

type Status = RepoView["status"];
const STAGE_TYPES = new Set<Action["type"]>(["stage", "unstage", "stageAll", "unstageAll"]);

/** Actions that only move changes in or out of the index: the file lists are all that can change. */
export function indexOnly(a: Action): boolean {
  return STAGE_TYPES.has(a.type) || (a.type === "patch" && a.op !== "discard");
}

/** What the lists will look like once Git has finished, shown straight away. */
export function predictStatus(s: Status, a: Action): Status | null {
  const move = (from: FileChange[], to: FileChange[], paths: Set<string> | null, toStaged: boolean) => {
    const moving = from.filter((f) => f.status !== "U" && (!paths || paths.has(f.path)));
    if (!moving.length) return null;
    const left = from.filter((f) => !moving.includes(f));
    const kept = to.filter((f) => !moving.some((m) => m.path === f.path));
    const moved = moving.map((f) => ({ ...f, status: toStaged ? (f.status === "?" ? "A" : f.status) : f.status === "A" ? "?" : f.status }));
    // A partly staged file stays listed on the side it leaves only if it still has changes there; unknown until Git
    // answers, so it is shown on the destination side and the real status corrects it a moment later.
    const merged = [...kept, ...moved].sort((x, y) => x.path.localeCompare(y.path));
    return { left, merged };
  };
  switch (a.type) {
    case "stage":
    case "stageAll": {
      const r = move(s.unstaged, s.staged, a.type === "stage" ? new Set(a.paths) : null, true);
      return r && { ...s, unstaged: r.left, staged: r.merged };
    }
    case "unstage":
    case "unstageAll": {
      const r = move(s.staged, s.unstaged, a.type === "unstage" ? new Set(a.paths) : null, false);
      return r && { ...s, staged: r.left, unstaged: r.merged };
    }
    default:
      return null;
  }
}

const BUSY_LABEL: Partial<Record<Action["type"], string>> = {
  stage: "Staging…",
  stageAll: "Staging all changes…",
  unstage: "Unstaging…",
  unstageAll: "Unstaging all changes…",
  patch: "Updating…",
  fetch: "Fetching…",
  pull: "Pulling…",
  push: "Pushing…",
  commit: "Committing…",
  checkout: "Checking out…",
  merge: "Merging…",
  rebase: "Rebasing…",
  stash: "Stashing…",
};

/** Run a Git action in the background, show the result and refresh the repository. */
export async function act(path: string, action: Action, quiet = false): Promise<boolean> {
  const st = useStore.getState();
  st.setBusy(BUSY_LABEL[action.type] ?? "Working…");
  const light = indexOnly(action);
  const before = latest.get(path)?.status;
  if (light && before) {
    // Show the result immediately; Git's answer replaces it a moment later.
    const guess = predictStatus(before, action);
    if (guess) setStatus(path, guess);
  }
  let ok = false;
  try {
    const r = await api.run(path, action);
    if (!r.ok && action.type === "push" && !action.force && /rejected|non-fast-forward|fetch first/i.test(r.message)) {
      offerForcePush(path);
      return false;
    }
    if (!r.ok || (!quiet && st.settings.notifySuccess)) st.toast(r.ok, r.message);
    ok = r.ok;
    return r.ok;
  } catch (e) {
    st.toast(false, String(e));
    return false;
  } finally {
    // Keep the busy note until the lists are up to date, so there is always something showing it's working.
    if (light && ok) await refreshStatus(path);
    else {
      await refresh(path);
      lastOwnRefresh.set(path, Date.now());
    }
    st.setBusy(null);
  }
}

/** The remote has commits we don't: pull first, or force push (with lease, or without as a last resort). */
function offerForcePush(path: string) {
  const st = useStore.getState();
  const repo = latest.get(path);
  const b = repo?.refs.find((r) => r.kind === "Local" && r.name === repo.head_branch);
  st.setDialog({
    kind: "choice",
    title: "Push was rejected",
    text: `'${b?.name ?? "This branch"}' is behind '${b?.upstream || "the remote"}'. Update your branch by doing a Pull.\n\nForce push is a destructive action and cannot be undone: it replaces the remote branch with yours.`,
    choices: [
      { label: "Pull", variant: "primary", onSelect: () => act(path, { type: "pull", mode: st.settings.pullMode === "fetch" ? "ff" : st.settings.pullMode }) },
      { label: "Force Push", variant: "danger", onSelect: () => act(path, { type: "push", force: true, lease: true }) },
      { label: "Force Push Anyway", variant: "danger", onSelect: () => act(path, { type: "push", force: true, lease: false }) },
    ],
  });
}

export function currentBranch(repo: RepoView | undefined): GitRef | undefined {
  if (!repo?.head_branch) return undefined;
  return repo.refs.find((r) => r.kind === "Local" && r.name === repo.head_branch);
}

/** Opens a folder as a repository tab; reports a friendly error if it isn't one. */
export async function openRepoPath(path: string) {
  const st = useStore.getState();
  try {
    const r = await api.openRepo(path);
    latest.set(r.path, r);
    st.openRepoTab(r.path, r.name);
  } catch (e) {
    st.toast(false, String(e));
  }
}

export function promptBranch(path: string, at: string | null) {
  useStore.getState().setDialog({
    kind: "prompt",
    title: at ? "Create branch here" : "Create branch",
    label: "Branch name",
    value: "",
    confirm: "Create Branch",
    onSubmit: (name) => act(path, { type: "createBranch", name, at, checkout: true }),
  });
}

export function promptTag(path: string, at: string | null) {
  useStore.getState().setDialog({
    kind: "prompt",
    title: "Create tag here",
    label: "Tag name",
    value: "",
    confirm: "Create Tag",
    extraLabel: "Annotation message (optional, makes an annotated tag)",
    onSubmit: (name, msg) => act(path, { type: "createTag", name, at, message: msg ?? null }),
  });
}

export function promptStash(path: string) {
  useStore.getState().setDialog({
    kind: "prompt",
    title: "Stash changes",
    label: "Stash title (optional)",
    value: "",
    confirm: "Stash",
    onSubmit: (message) => act(path, { type: "stash", message }),
  });
}

export function confirmThen(title: string, text: string, confirm: string, onConfirm: () => void, danger = true) {
  useStore.getState().setDialog({ kind: "confirm", title, text, confirm, danger, onConfirm });
}

/** Open File History or Blame for a file in the active tab. */
export function openHistory(file: string, mode: "history" | "blame" = "history") {
  const st = useStore.getState();
  st.setUi(st.activeId, { history: { file, mode, sha: null }, diff: null });
}

/** "Pin to Left" / "Unpin from Left". */
export function togglePinned(path: string, full: string) {
  const st = useStore.getState();
  const pinned = { ...(st.settings.pinned ?? {}) };
  if (pinned[path] === full) delete pinned[path];
  else pinned[path] = full;
  st.setSettings({ pinned });
}

/** Hide a branch or tag from the graph (or show it again). */
export function toggleHidden(path: string, full: string) {
  const st = useStore.getState();
  const cur = st.settings.hidden[path] ?? [];
  const next = cur.includes(full) ? cur.filter((x) => x !== full) : [...cur, full];
  st.setSettings({ hidden: { ...st.settings.hidden, [path]: next } });
}

/** Solo branches: only soloed refs are shown. */
export function toggleSolo(path: string, full: string) {
  const st = useStore.getState();
  const cur = st.settings.solo[path] ?? [];
  const next = cur.includes(full) ? cur.filter((x) => x !== full) : [...cur, full];
  st.setSettings({ solo: { ...st.settings.solo, [path]: next } });
}

export function clearSolo(path: string) {
  const st = useStore.getState();
  st.setSettings({ solo: { ...st.settings.solo, [path]: [] } });
}

export function showAllRefs(path: string) {
  const st = useStore.getState();
  st.setSettings({ hidden: { ...st.settings.hidden, [path]: [] } });
}

export function copy(text: string) {
  navigator.clipboard.writeText(text).then(
    () => useStore.getState().toast(true, "Copied to clipboard"),
    () => useStore.getState().toast(false, "Could not copy"),
  );
}
