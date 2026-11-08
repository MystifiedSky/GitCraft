import { QueryClient, useQuery } from "@tanstack/react-query";
import { api, type Action, type GitRef, type RefView, type RepoView } from "./api";
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
  const view: RefView = { hidden: hidden ?? [], solo: solo ?? [] };
  return useQuery({
    queryKey: ["repo", path, limit, view],
    queryFn: async () => {
      const r = await api.openRepo(path!, limit || undefined, view);
      latest.set(path!, r);
      return r;
    },
    enabled: !!path,
    placeholderData: (prev) => prev,
    initialData: () => (path && limit === 0 && view.hidden.length === 0 && view.solo.length === 0 ? latest.get(path) : undefined),
  });
}

export function repoData(path: string): RepoView | undefined {
  return latest.get(path);
}

export function refresh(path: string) {
  return queryClient.invalidateQueries({ queryKey: ["repo", path] });
}

const BUSY_LABEL: Partial<Record<Action["type"], string>> = {
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
  try {
    const r = await api.run(path, action);
    if (!r.ok || !quiet) st.toast(r.ok, r.message);
    return r.ok;
  } catch (e) {
    st.toast(false, String(e));
    return false;
  } finally {
    st.setBusy(null);
    await refresh(path);
  }
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
