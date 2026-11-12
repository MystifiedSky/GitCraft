import { QueryClient, useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { api, type Action, type FileChange, type GitRef, type RefView, type RepoView } from "./api";
import { useStore } from "./store";
import { spinButtonFor, withSpin } from "./spin";

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
  // General > Initial Commits in Graph / Show All Commits; UI Customization > Smart Branch Visibility.
  const initial = useStore((s) => (s.settings.showAllCommits ? 10000000 : Math.max(500, s.settings.initialCommits || 2000)));
  const smart = useStore((s) => !!s.settings.smartBranches);
  const view: RefView = { hidden: hidden ?? [], solo: solo ?? [], pinned: pinned ?? null, smart };
  return useQuery({
    queryKey: ["repo", path, limit, view, initial],
    queryFn: async () => {
      const r = await api.openRepo(path!, limit || initial, view);
      latest.set(path!, r);
      return r;
    },
    enabled: !!path,
    // The file watcher already reports every change, so switching back to the window doesn't reload everything.
    refetchOnWindowFocus: false,
    // Keep the graph while changing its view, but never show another repository's data.
    placeholderData: (prev, query) => (query?.queryKey[1] === path ? prev : undefined),
  });
}

export function repoData(path: string): RepoView | undefined {
  return latest.get(path);
}

export async function refresh(path: string) {
  // HEAD changes and file saves also change open diffs, blame and file history.
  // Status can remain "M" while the file's contents change, so its status alone isn't a cache key.
  await Promise.all(["repo", "diff", "versions", "history", "blame", "range", "conflict"].map((kind) =>
    queryClient.invalidateQueries({ queryKey: [kind, path] }),
  ));
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
      } while (task.again);
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
    if (light && ok) await refreshStatus(path);
    else {
      await refresh(path);
    }
    st.setBusy(null);
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
  // Notifications > Enable Desktop Notifications: long network actions that finish while GitCraft is in the background.
  if (set.desktopNotifications && NETWORK.has(action.type) && !document.hasFocus() && "Notification" in window && Notification.permission === "granted") {
    try {
      new Notification(ok ? "GitCraft" : "GitCraft: action failed", { body: `${path.split(/[\\/]/).pop()}: ${message.split("\n")[0]}` });
    } catch {
      /* notifications unavailable */
    }
  }
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
    const r = await api.openRepo(path);
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

/** The original's "Hide all local branches / remotes / tags" and "Show all ..." for a whole left-panel section. */
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
