import { useEffect } from "react";
import { QueryClient, useQuery } from "@tanstack/react-query";
import { postDesktopNotifications } from "./desktop-notify";
import { invoke } from "@tauri-apps/api/core";
import { api, type Action, type FileChange, type GitRef, type RefView, type RepoView } from "./api";
import { useStore } from "./store";
import { spinButtonFor, withSpin } from "./spin";

export const queryClient = new QueryClient({
  defaultOptions: { queries: { refetchOnWindowFocus: true, staleTime: 2000, retry: false } },
});

/** Latest data per repository, whatever graph view (hidden / solo / pinned) it was read with. */
const latest = new Map<string, RepoView>();

export function refView(path: string): RefView {
  const st = useStore.getState().settings;
  return { hidden: st.hidden[path] ?? [], solo: st.solo[path] ?? [] };
}

function repoRequest(path: string) {
  const s = useStore.getState().settings;
  const initial = Math.min(5000, Math.max(500, s.initialCommits || 2000));
  const view: RefView = { hidden: s.hidden[path] ?? [], solo: s.solo[path] ?? [], pinned: s.pinned?.[path] ?? null, smart: !!s.smartBranches };
  return { initial, view, key: ["repo", path, view, initial] as const };
}

function isOpen(path: string) { return useStore.getState().tabs.some((t) => t.path === path); }

function isActive(path: string) {
  const s = useStore.getState();
  return s.tabs.some((t) => t.id === s.activeId && t.path === path);
}
const deferred = new Map<string, boolean>();
const pageLoads = new Map<string, Promise<void>>();
let nextGeneration = 0;
const generations = new Map<string, number>();
function generationFor(path: string) {
  if (!generations.has(path)) generations.set(path, ++nextGeneration);
  return generations.get(path)!;
}
const actionChanges = new Map<string, { full: boolean }>();
const fullRefreshes = new Map<string, { again: boolean; promise: Promise<void> }>();

export function useRepo(path: string | undefined, active = true) {
  // Subscribe to request inputs without coupling repository reads to unrelated preferences.
  const settings = useStore((s) => s.settings);
  const request = repoRequest(path ?? "");
  const q = useQuery({
    queryKey: request.key,
    queryFn: async () => {
      const generation = generationFor(path!);
      const r = await api.openRepo(path!, request.initial, request.view);
      if (generations.get(path!) === generation && JSON.stringify(repoRequest(path!).key) === JSON.stringify(request.key)) latest.set(path!, r);
      return r;
    },
    enabled: !!path && active,
    staleTime: Infinity,
    gcTime: 60_000,
    refetchOnWindowFocus: false,
    placeholderData: (prev, query) => (query?.queryKey[1] === path ? prev : undefined),
  });
  useEffect(() => {
    if (!path || !active || !deferred.has(path)) return;
    const full = deferred.get(path);
    deferred.delete(path);
    if (full) void refresh(path); else void refreshStatus(path);
  }, [path, active]);
  useEffect(() => {
    if (!path || !active || !(settings.showAllCommits || (q.data?.commits.length ?? 0) < Math.max(500, settings.initialCommits || 2000)) || !q.data?.history_cursor || q.isFetching || q.isPlaceholderData) return;
    // Yield between bounded pages; hiding or closing the tab stops further requests.
    const timer = window.setTimeout(() => void loadMoreRepo(path).catch((e) => useStore.getState().toast(false, String(e))), 25);
    return () => clearTimeout(timer);
  }, [path, active, settings.showAllCommits, settings.initialCommits, q.data?.history_cursor, q.isFetching, q.isPlaceholderData]);
  return q;
}

export function loadMoreRepo(path: string): Promise<void> {
  const request = repoRequest(path);
  const key = JSON.stringify(request.key);
  const pending = pageLoads.get(key);
  if (pending) return pending;
  const before = queryClient.getQueryData<RepoView>(request.key);
  if (!before?.history_cursor || !isActive(path) || deferred.has(path)) return Promise.resolve();
  const cursor = before.history_cursor;
  const generation = generationFor(path);
  const task = (async () => {
    const page = await api.historyPage(path, cursor, request.initial);
    if (generations.get(path) !== generation || !isActive(path) || JSON.stringify(repoRequest(path).key) !== key) return;
    queryClient.setQueryData<RepoView>(request.key, (current) => {
      if (!current || current.history_cursor !== cursor) return current;
      const next = { ...current, commits: [...current.commits, ...page.commits], graph: { rows: [...current.graph.rows, ...page.graph.rows], width: page.graph.width }, has_more: page.has_more, history_cursor: page.history_cursor };
      latest.set(path, next);
      return next;
    });
  })().finally(() => { if (pageLoads.get(key) === task) pageLoads.delete(key); });
  pageLoads.set(key, task);
  return task;
}

/** Closing the final tab releases snapshots, queries and pending page results for that repository. */
export function releaseRepo(path: string) {
  generations.delete(path);
  latest.delete(path);
  deferred.delete(path);
  for (const key of pageLoads.keys()) if (JSON.parse(key)[1] === path) pageLoads.delete(key);
  void queryClient.cancelQueries({ predicate: (q) => q.queryKey[1] === path });
  queryClient.removeQueries({ predicate: (q) => q.queryKey[1] === path });
}

export function repoData(path: string): RepoView | undefined {
  return latest.get(path);
}

export async function refresh(path: string): Promise<void> {
  if (!isOpen(path)) return;
  generations.set(path, ++nextGeneration);
  if (!isActive(path)) {
    if (isOpen(path)) deferred.set(path, true);
    return;
  }
  const current = fullRefreshes.get(path);
  if (current) { current.again = true; return current.promise; }
  const task = { again: false, promise: Promise.resolve() };
  fullRefreshes.set(path, task);
  task.promise = (async () => {
    try {
      do {
        task.again = false;
        await Promise.all(["repo", "diff", "versions", "history", "blame", "range", "conflict"].map((kind) =>
          queryClient.invalidateQueries({ queryKey: [kind, path] }, { cancelRefetch: false }),
        ));
      } while (task.again && isActive(path));
    } finally { fullRefreshes.delete(path); }
  })();
  return task.promise;
}

/** Put new file lists into every cached view of the repository, keeping commits and graph as they are
 *  (same objects), so only the file panels re-render. */
function setStatus(path: string, status: RepoView["status"]) {
  queryClient.setQueriesData<RepoView>({ queryKey: ["repo", path] }, (old) => (old ? { ...old, status } : old));
  const l = latest.get(path);
  if (l) latest.set(path, { ...l, status });
}

const statusRefreshes = new Map<string, { again: boolean; promise: Promise<void> }>();

/** Coalesce watcher bursts; an event during a read requests one trailing read instead of concurrent Git processes. */
export function refreshStatus(path: string): Promise<void> {
  if (!isActive(path)) {
    if (isOpen(path) && !deferred.has(path)) deferred.set(path, false);
    return Promise.resolve();
  }
  const current = statusRefreshes.get(path);
  if (current) {
    current.again = true;
    return current.promise;
  }
  const task = { again: false, promise: Promise.resolve() };
  statusRefreshes.set(path, task);
  task.promise = (async () => {
    try {
      do {
        task.again = false;
        setStatus(path, await api.repoStatus(path));
        await Promise.all(["diff", "versions", "blame", "range", "conflict"].map((kind) =>
          queryClient.invalidateQueries({ queryKey: [kind, path] }),
        ));
      } while (task.again && isActive(path));
    } catch {
      await refresh(path);
    } finally {
      statusRefreshes.delete(path);
    }
  })();
  return task.promise;
}

/** File watcher: working-folder edits and index changes refresh the file lists only; anything else
 *  (commits, branches, HEAD) reloads the repository. */
export function onWatcherChange(path: string, full: boolean) {
  if (!isOpen(path)) return;
  const action = actionChanges.get(path);
  if (action) { action.full ||= full; return; }
  if (!full) return void refreshStatus(path);
  // Never suppress an external change just because our own action recently finished.
  void refresh(path);
}

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
  pushBranch: "Pushing…",
  pushTag: "Pushing tag…",
  deleteRemoteTag: "Deleting remote tag…",
  commit: "Committing…",
  checkout: "Checking out…",
  merge: "Merging…",
  rebase: "Rebasing…",
  stash: "Stashing…",
};

/** Run a Git action in the background, show the result and refresh the repository. */
export async function act(path: string, action: Action, quiet = false): Promise<boolean> {
  const st = useStore.getState();
  const set = st.settings;
  // Commit > Merge Behavior: Squash - local merges stage the changes for one commit instead of a merge commit.
  if (action.type === "merge" && set.squashMerge) {
    const head = latest.get(path)?.head_branch ?? "HEAD";
    const ok = await act(path, { type: "mergeSquash", branch: action.branch }, quiet);
    if (ok) st.setDraft(path, { summary: `Squash merge ${action.branch} into ${head}`, description: "", amend: false });
    return ok;
  }
  // Commit > Remove comments from commit messages.
  if (action.type === "commit" && set.stripComments) action = { ...action, stripComments: true };
  // Pull / Push / Stash / Pop show a spinning circle on their toolbar button while this runs (lib/spin.ts).
  const run = action;
  return withSpin(path, spinButtonFor(run, quiet), () => runAct(path, run, quiet));
}

async function runAct(path: string, action: Action, quiet: boolean): Promise<boolean> {
  const st = useStore.getState();
  const wasMerging = !!latest.get(path)?.state_note;
  st.setBusy(BUSY_LABEL[action.type] ?? "Working…");
  const light = indexOnly(action);
  const before = latest.get(path)?.status;
  if (light && before) {
    // Show the result immediately; Git's answer replaces it a moment later.
    const guess = predictStatus(before, action);
    if (guess) setStatus(path, guess);
  }
  let ok = false;
  actionChanges.set(path, { full: false });
  try {
    const r = await api.run(path, action);
    if (!r.ok && action.type === "push" && !action.force && /rejected|non-fast-forward|fetch first/i.test(r.message)) {
      offerForcePush(path);
      return false;
    }
    if (!r.ok && NETWORK.has(action.type) && signInProblem(r.message)) {
      void offerSignInHelp(path, action, r.message);
      return false;
    }
    if (!r.ok || (!quiet && st.settings.notifySuccess)) st.toast(r.ok, r.message);
    ok = r.ok;
    await afterAction(path, action, r.ok, r.message, wasMerging);
    return r.ok;
  } catch (e) {
    st.toast(false, String(e));
    return false;
  } finally {
    // Keep the busy note until the lists are up to date, so there is always something showing it's working.
    const changes = actionChanges.get(path);
    actionChanges.delete(path);
    try {
      if (light && ok && !changes?.full) await refreshStatus(path);
      else await refresh(path);
    } finally { st.setBusy(null); }
  }
}

const NETWORK = new Set<Action["type"]>(["fetch", "pull", "push", "pushBranch", "pushTag", "deleteRemoteTag", "fetchRemote"]);
const MOVES_HEAD = new Set<Action["type"]>(["pull", "checkout", "checkoutRemote", "merge", "mergeSquash", "rebase", "interactiveRebase", "fastForward", "reset", "stashPop", "stashApply", "cherryPick"]);

/** Preferences that act after a Git action finishes. */
async function afterAction(path: string, action: Action, ok: boolean, message: string, wasMerging: boolean) {
  const set = useStore.getState().settings;
  // General > Keep submodules up to date: "Automatically update all submodules after performing a Git action".
  if (ok && set.autoUpdateSubmodules && MOVES_HEAD.has(action.type) && (latest.get(path)?.submodules.length ?? 0) > 0)
    await api.run(path, { type: "submoduleUpdate", paths: [] }).catch(() => undefined);
  // General > Delete ".orig" files after merging.
  if (ok && set.deleteOrigAfterMerge && (wasMerging && (action.type === "commit" || action.type === "markResolved" || action.type === "continueRebase") || action.type === "merge"))
    await invoke<string[]>("delete_orig_files", { path }).catch(() => undefined);
  // Notifications > Enable Desktop Notifications: a finished network action while GitCraft is in the background. Failures reach the
  // bell (and from there a desktop notification) through the error toast, so only successes are posted here.
  if (ok && NETWORK.has(action.type)) postDesktopNotifications([`${path.split(/[\\/]/).pop()}: ${message.split("\n")[0]}`]);
}

const SSH_REFUSED = /Permission denied \(publickey|Could not read from remote repository|Host key verification failed/i;
const HTTPS_REFUSED = /Authentication failed|could not read Username|terminal prompts disabled|returned error: 403|Permission to .* denied|Invalid username or (password|token)/i;

/** Git couldn't sign in to the remote (SSH key refused, or no / wrong HTTPS sign-in). */
export function signInProblem(message: string): "ssh" | "https" | null {
  if (SSH_REFUSED.test(message)) return "ssh";
  if (HTTPS_REFUSED.test(message)) return "https";
  return null;
}

interface RemoteAuth {
  remote: string;
  url: string;
  ssh: boolean;
  host: string;
  https_url: string | null;
  connected: boolean;
}

/** Explain a refused sign-in and offer the fixes: switch an SSH remote to HTTPS (browser sign-in, or the account
 *  connected in Accounts), add this computer's SSH key to the service, or connect an account. */
async function offerSignInHelp(path: string, action: Action, message: string) {
  const st = useStore.getState();
  const gitlabHost = st.settings.gitlabHost ?? "";
  const info = await invoke<RemoteAuth>("remote_auth", { path, gitlabHost }).catch(() => null);
  const service = info?.host === "github.com" ? "GitHub" : info?.host ? info.host : "the server";
  const openAccounts = { label: "Open Accounts", onSelect: () => st.setDialog({ kind: "accounts" }) };
  if (info && info.ssh) {
    const https = info.https_url;
    const keysPage = info.host === "github.com" ? "https://github.com/settings/ssh/new" : `https://${info.host}/-/user_settings/ssh_keys`;
    st.setDialog({
      kind: "choice",
      title: `${service} didn't accept your SSH key`,
      text:
        `The remote "${info.remote}" uses SSH (${info.url}), and ${service} refused the SSH key on this computer. ` +
        `Usually the key just hasn't been added to your ${service} account.\n\n` +
        (https
          ? `Easiest fix: use HTTPS instead. You sign in once (in your browser${info.connected ? ", or with the account connected in Accounts" : ""}) and Push, Pull and Fetch work from then on.\n\n`
          : "") +
        `Or add this computer's SSH key to ${service}: GitCraft copies the key and opens the page to paste it.\n\nGit said: ${message.split("\n")[0]}`,
      choices: [
        ...(https
          ? [
              {
                label: "Use HTTPS (recommended)",
                variant: "primary" as const,
                onSelect: async () => {
                  const ok = await act(path, { type: "remoteEdit", name: info.remote, newName: "", url: https, pushUrl: "" }, true);
                  if (ok) {
                    st.toast(true, `${info.remote} now uses ${https}. Trying again…`);
                    await act(path, action);
                  }
                },
              },
            ]
          : []),
        {
          label: "Add my SSH key",
          onSelect: async () => {
            const acc = await api.accountInfo(path).catch(() => null);
            const key = acc?.ssh_keys.find((k) => /id_ed25519/.test(k.path)) ?? acc?.ssh_keys[0];
            if (!key) return st.setDialog({ kind: "accounts" });
            await navigator.clipboard.writeText(key.public).catch(() => undefined);
            st.toast(true, `Copied ${key.path.split(/[\\/]/).pop()} - paste it on the page that just opened, then try again`);
            await invoke("open_url", { url: keysPage }).catch(() => undefined);
          },
        },
        openAccounts,
      ],
    });
    return;
  }
  st.setDialog({
    kind: "choice",
    title: `Sign in to ${service}`,
    text:
      `${service} didn't accept the sign-in for "${info?.remote ?? "the remote"}" (${info?.url ?? ""}).\n\n` +
      (info?.connected
        ? `GitCraft used the token saved in Accounts. Check it hasn't expired and that it may push (classic token: "repo" scope; fine-grained: Contents read and write).`
        : `Connect your ${service} account in Accounts with a personal access token that may push (classic token: "repo" scope), and GitCraft will use it for Push, Pull and Fetch.`) +
      `\n\nGit said: ${message.split("\n")[0]}`,
    choices: [{ ...openAccounts, variant: "primary" as const }],
  });
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
    const request = repoRequest(path);
    const r = await api.openRepo(path, request.initial, request.view);
    const canonical = repoRequest(r.path);
    if (JSON.stringify(canonical.view) === JSON.stringify(request.view)) queryClient.setQueryData(canonical.key, r);
    latest.set(r.path, r);
    const main = r.worktrees[0]?.path;
    st.openRepoTab(r.path, r.name, main && main.replace(/\\/g, "/").toLowerCase() !== r.path.replace(/\\/g, "/").toLowerCase() ? main : undefined);
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

export function promptTag(path: string, at: string | null, annotated = false, initialName = "") {
  useStore.getState().setDialog({
    kind: "prompt",
    title: annotated ? "Create annotated tag here" : "Create tag here",
    label: "Tag name",
    value: initialName,
    confirm: "Create Tag",
    extraLabel: annotated ? "Annotation message" : "Annotation message (optional, makes an annotated tag)",
    onSubmit: (name, msg) => {
      if (annotated && !msg?.trim()) {
        useStore.getState().toast(false, "Enter an annotation message to create an annotated tag.");
        promptTag(path, at, true, name);
        return;
      }
      void act(path, { type: "createTag", name, at, message: msg ?? null });
    },
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

/** Sent by the graph's "// WIP" box on Ctrl+Enter ({ path, all }); the Commit panel commits. */
export const WIP_COMMIT_EVENT = "gitcraft:wip-commit";

/** "Hide all local branches / remotes / tags" and "Show all ..." for a whole left-panel section. */
export function setHiddenMany(path: string, fulls: string[], hide: boolean) {
  const st = useStore.getState();
  const cur = st.settings.hidden[path] ?? [];
  const next = hide ? [...new Set([...cur, ...fulls])] : cur.filter((x) => !fulls.includes(x));
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
