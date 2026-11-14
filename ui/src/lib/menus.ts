import type { MenuEntry } from "@/components/ui/menu";
import type { Commit, GitRef, RepoView, Stash } from "./api";
import { api } from "./api";
import { act, clearSolo, confirmThen, copy, openHistory, promptBranch, promptTag, toggleHidden, togglePinned, toggleSolo } from "./repo";
import { useStore } from "./store";
import { addWorktree } from "./manage";
import { startPullRequest } from "./hosting";
import { explainBranch, patchName, savePatch } from "./ai";
import { baseFor, dropCommits, headLine, moveCommit, openRebase, rewordCommit, squashCommits } from "./rewrite";
import { linkItems } from "./weblinks";
import { editorName } from "@/components/TitleBar";

const activeTab = () => useStore.getState().activeId;

/** Shared by commit rows and branch labels so Reset is always discoverable. */
export function resetMenu(repo: RepoView, sha: string): MenuEntry {
  const head = repo.head_branch ?? "HEAD";
  const reset = (mode: "soft" | "mixed" | "hard") => () => {
    if (mode === "hard")
      confirmThen(`Reset ${head} to this commit?`, "Hard reset throws away all uncommitted changes. Undo can move the branch back, but cannot restore discarded work.", "Reset Hard",
        () => act(repo.path, { type: "reset", sha, mode }));
    else void act(repo.path, { type: "reset", sha, mode });
  };
  return {
    label: `Reset ${head} to this commit`,
    children: [
      { label: "Soft - keep all changes", onSelect: reset("soft") },
      { label: "Mixed - keep working copy but reset index", onSelect: reset("mixed") },
      { label: "Hard - discard all changes", danger: true, onSelect: reset("hard") },
    ],
  };
}

/** Which remote a remote-tracking name (origin/feature/x) belongs to; remote names can hold "/" too. */
export function splitRemote(repo: RepoView, name: string): { remote: string; branch: string } | null {
  const remote = [...repo.remotes].sort((a, b) => b.length - a.length).find((r) => name.startsWith(r + "/"));
  return remote ? { remote, branch: name.slice(remote.length + 1) } : null;
}

/** Prove ancestry from loaded parents; display order alone does not prove a fast-forward is possible. */
export function isLoadedAncestor(repo: RepoView, ancestor: string, tip: string): boolean {
  const parents = new Map(repo.commits.map((c) => [c.sha, c.parents]));
  const pending = [tip];
  const seen = new Set<string>();
  while (pending.length) {
    const sha = pending.pop()!;
    if (sha === ancestor) return true;
    if (seen.has(sha)) continue;
    seen.add(sha);
    pending.push(...(parents.get(sha) ?? []));
  }
  return false;
}

/** Ask which remote branch a local branch should track ("Set Upstream"). */
export function promptUpstream(repo: RepoView, ref: GitRef) {
  const remotes = repo.refs.filter((r) => r.kind === "Remote").map((r) => r.name);
  const guess = ref.upstream || remotes.find((r) => r.endsWith("/" + ref.name)) || `${repo.remotes[0] ?? "origin"}/${ref.name}`;
  useStore.getState().setDialog({
    kind: "prompt",
    title: "Set Upstream",
    label: `What remote/branch should "${ref.name}" push to and pull from? (empty = stop tracking)`,
    value: guess,
    confirm: "Set Upstream",
    onSubmit: (v) => act(repo.path, { type: "setUpstream", branch: ref.name, upstream: v }),
  });
}

export function branchMenu(repo: RepoView, ref: GitRef): MenuEntry[] {
  const path = repo.path;
  const head = repo.head_branch;
  const isHead = ref.kind === "Local" && ref.name === head;
  if (ref.kind === "Tag") return tagMenu(repo, ref);
  const items: MenuEntry[] = [];
  if (ref.kind === "Local") {
    if (!isHead) items.push({ label: `Checkout ${ref.name}`, onSelect: () => act(path, { type: "checkout", target: ref.name }) });
  } else {
    items.push({ label: `Checkout ${ref.name}`, onSelect: () => act(path, { type: "checkoutRemote", remoteBranch: ref.name }) });
  }
  if (head && !isHead) {
    items.push({ label: `Merge ${ref.name} into ${head}`, onSelect: () => act(path, { type: "merge", branch: ref.name }) });
    items.push({
      label: `Squash merge ${ref.name} into ${head}`,
      onSelect: async () => {
        // The commit message for this: "Squash merge {0} into {1}".
        if (await act(path, { type: "mergeSquash", branch: ref.name })) useStore.getState().setDraft(path, { summary: `Squash merge ${ref.name} into ${head}`, description: "", amend: false });
      },
    });
    items.push({ label: `Rebase ${head} onto ${ref.name}`, onSelect: () => act(path, { type: "rebase", onto: ref.name }) });
    items.push({ label: `Interactive Rebase ${head} onto ${ref.name}`, onSelect: () => openRebase(activeTab(), path, ref.name, ref.name) });
    // Fast-forward the checked-out branch when the other ref is strictly ahead of it.
    if (ref.sha !== repo.head_sha && isLoadedAncestor(repo, repo.head_sha, ref.sha)) items.push({ label: `Fast-forward ${head} to ${ref.name}`, onSelect: () => act(path, { type: "fastForward", branch: head, to: ref.name }) });
  }
  items.push({ separator: true });
  if (isHead) items.push({ label: `Push ${ref.name}`, onSelect: () => act(path, { type: "push", force: false }) });
  if (ref.kind === "Local") items.push({ label: `Start a pull request from ${ref.name}`, onSelect: () => startPullRequest(path, ref.name) });
  items.push({ label: "Explain Branch Changes (Preview)", onSelect: () => explainBranch(path, ref.name) });
  if (ref.kind === "Local" && !isHead && ref.upstream && ref.behind > 0 && ref.ahead === 0)
    items.push({ label: `Fast-forward ${ref.name} to ${ref.upstream}`, onSelect: () => act(path, { type: "fastForward", branch: ref.name, to: ref.upstream }) });
  if (ref.kind === "Local") items.push({ label: ref.upstream ? `Set Upstream (now ${ref.upstream})` : "Set Upstream", onSelect: () => promptUpstream(repo, ref) });
  items.push({ label: "Create branch here", onSelect: () => promptBranch(path, ref.sha) });
  items.push(resetMenu(repo, ref.sha));
  items.push({ label: `Create worktree from ${ref.name}`, onSelect: () => addWorktree(repo, undefined, ref.sha) });
  if (repo.head_branch && headLine(repo).includes(ref.sha))
    items.push({ label: "Rename commit", onSelect: () => rewordCommit(repo, ref.sha) });
  items.push({ label: "Create tag here", onSelect: () => promptTag(path, ref.sha) });
  items.push({ separator: true });
  if (ref.kind === "Local") {
    items.push({
      label: `Rename ${ref.name}`,
      onSelect: () =>
        useStore.getState().setDialog({
          kind: "prompt",
          title: `Rename ${ref.name}`,
          label: "New branch name",
          value: ref.name,
          confirm: "Rename",
          onSubmit: (v) => act(path, { type: "renameBranch", old: ref.name, new: v }),
        }),
    });
    items.push({
      label: `Delete ${ref.name}`,
      danger: true,
      disabled: isHead,
      onSelect: () => confirmThen(`Delete ${ref.name}?`, "The branch label is removed. You can undo this with Undo.", "Delete", () => act(path, { type: "deleteBranch", name: ref.name })),
    });
  } else {
    items.push({
      label: `Delete ${ref.name}`,
      danger: true,
      onSelect: () =>
        confirmThen(`Delete ${ref.name} from the remote?`, "This removes the branch on the remote server for everyone.", "Delete", () =>
          act(path, { type: "deleteRemoteBranch", remoteBranch: ref.name }),
        ),
    });
  }
  items.push({ separator: true });
  const st = useStore.getState().settings;
  const hidden = (st.hidden[path] ?? []).includes(ref.full);
  const soloed = (st.solo[path] ?? []).includes(ref.full);
  items.push({ label: hidden ? "Show in the graph" : "Hide in the graph", disabled: isHead && !hidden, onSelect: () => toggleHidden(path, ref.full) });
  items.push({ label: soloed ? "Stop soloing this branch" : "Solo", onSelect: () => toggleSolo(path, ref.full) });
  items.push({ label: st.pinned?.[path] === ref.full ? "Unpin from Left" : "Pin to Left", onSelect: () => togglePinned(path, ref.full) });
  if ((st.solo[path] ?? []).length > 0) items.push({ label: "Stop soloing all branches", onSelect: () => clearSolo(path) });
  items.push({ separator: true });
  items.push({ label: "Copy branch name", onSelect: () => copy(ref.name) });
  items.push({ label: "Copy commit sha", onSelect: () => copy(ref.sha) });
  // "Copy link to branch": for a remote branch or a local branch's upstream.
  const onRemote = splitRemote(repo, ref.kind === "Remote" ? ref.name : ref.upstream);
  if (onRemote) items.push(...linkItems(repo, "Copy link to branch", { kind: "branch", name: onRemote.branch }, [onRemote.remote]));
  return items;
}

export function tagMenu(repo: RepoView, ref: GitRef): MenuEntry[] {
  const path = repo.path;
  return [
    { label: `Checkout the commit at ${ref.name}`, onSelect: () => act(path, { type: "checkout", target: ref.name }) },
    { label: "Create branch here", onSelect: () => promptBranch(path, ref.sha) },
    { separator: true },
    ...(repo.remotes.length ? [
      { label: `Push ${ref.name} to remote`, children: repo.remotes.map((remote) => ({
        label: remote, onSelect: () => act(path, { type: "pushTag" as const, name: ref.name, remote }),
      })) },
      { label: `Delete ${ref.name} from remote`, danger: true, children: repo.remotes.map((remote) => ({
        label: remote, danger: true,
        onSelect: () => confirmThen(`Delete ${ref.name} from ${remote}?`,
          "This deletes the tag on the remote for everyone. Your local tag is kept.",
          "Delete Remote Tag", () => act(path, { type: "deleteRemoteTag" as const, name: ref.name, remote })),
      })) },
      { separator: true },
    ] : []),
    { label: `Delete ${ref.name} locally`, danger: true, onSelect: () => act(path, { type: "deleteTag", name: ref.name }) },
    { label: (useStore.getState().settings.hidden[path] ?? []).includes(ref.full) ? "Show in the graph" : "Hide in the graph", onSelect: () => toggleHidden(path, ref.full) },
    { separator: true },
    { label: "Copy tag name", onSelect: () => copy(ref.name) },
    ...linkItems(repo, "Copy link to this tag", { kind: "tag", name: ref.name }),
  ];
}

/** For a merge, explain which parent's changes will be used instead of silently choosing parent one. */
export function commitActionMenu(repo: RepoView, c: Commit, type: "cherryPick" | "revert", disabled = false): MenuEntry {
  const label = type === "revert" ? "Revert commit" : "Cherry pick commit";
  if (c.parents.length < 2) return { label, disabled, onSelect: () => act(repo.path, { type, sha: c.sha }) };
  return { label, disabled, children: c.parents.map((sha, index) => {
    const parent = repo.commits.find((p) => p.sha === sha);
    return {
      label: `Relative to parent ${index + 1}: ${sha.slice(0, useStore.getState().metrics.sha_len)}${parent ? " — " + parent.summary : ""}`,
      onSelect: () => confirmThen(`${label} relative to parent ${index + 1}?`,
        `${type === "revert" ? "Undo" : "Apply"} the merge's changes compared with parent ${index + 1} (${sha}).`,
        type === "revert" ? "Revert" : "Cherry Pick",
        () => act(repo.path, { type, sha: c.sha, mainline: index + 1 })),
    };
  }) };
}

export function commitMenu(repo: RepoView, c: Commit): MenuEntry[] {
  const path = repo.path;
  const head = repo.head_branch ?? "HEAD";
  const isHead = c.sha === repo.head_sha;
  const canRename = !!repo.head_branch && headLine(repo).includes(c.sha);
  return [
    ...(repo.head_branch && !isHead ? [{ label: `Rebase ${head} onto this commit`, onSelect: () => act(path, { type: "rebase", onto: c.sha }) }] : []),
    { label: "Checkout", children: [
      ...repo.refs.filter((r) => r.sha === c.sha && r.kind !== "Tag").map((r) => ({
        label: r.name, disabled: r.kind === "Local" && r.name === repo.head_branch,
        onSelect: () => r.kind === "Remote" ? act(path, { type: "checkoutRemote", remoteBranch: r.name }) : act(path, { type: "checkout", target: r.name }),
      })),
      { label: "this commit (detached HEAD)", onSelect: () => act(path, { type: "checkout", target: c.sha }) },
    ] },
    { label: "Create worktree from", children: [
      { label: "this commit", onSelect: () => addWorktree(repo, undefined, c.sha) },
      ...repo.refs.filter((r) => r.sha === c.sha && r.kind !== "Tag").map((r) => ({
        label: r.name, onSelect: () => addWorktree(repo, undefined, r.sha),
      })),
    ] },
    { label: "Create branch here", shortcut: "", onSelect: () => promptBranch(path, c.sha) },
    { separator: true },
    commitActionMenu(repo, c, "cherryPick", isHead),
    resetMenu(repo, c.sha),
    { label: "Rename commit", disabled: !canRename, onSelect: () => rewordCommit(repo, c.sha) },
    commitActionMenu(repo, c, "revert"),
    { label: "Create patch from commit", onSelect: () => savePatch(path, [c.sha], patchName(c.summary)) },
    ...(repo.head_branch && repo.remotes.length ? [{ label: `Push ${head} and start a pull request`, onSelect: () => startPullRequest(path, head) }] : []),
    ...(repo.refs.find((r) => r.sha === c.sha && r.kind !== "Tag") ? [
      { label: "Explain Branch Changes (Preview)", onSelect: () => explainBranch(path, repo.refs.find((r) => r.sha === c.sha && r.kind !== "Tag")!.name) },
    ] : []),
    { separator: true },
    {
      label: "Compare commit against working directory",
      onSelect: () => {
        const st = useStore.getState();
        st.setUi(st.activeId, { compareWorkdir: c.sha, multi: [], selection: { type: "commit", sha: c.sha }, diff: null, history: null });
      },
    },
    { separator: true },
    { label: "Edit commit history", children: historyItems(repo, c) },
    { separator: true },
    ...repo.refs.filter((r) => r.sha === c.sha && r.kind !== "Tag").flatMap((r): MenuEntry[] => [
      { label: `Copy branch name: ${r.name}`, onSelect: () => copy(r.name) },
      { label: useStore.getState().settings.pinned?.[path] === r.full ? `Unpin ${r.name} from Left` : `Pin to Left: ${r.name}`, onSelect: () => togglePinned(path, r.full) },
      { label: `Solo: ${r.name}`, onSelect: () => toggleSolo(path, r.full) },
    ]),
    { label: "Copy commit sha", onSelect: () => copy(c.sha) },
    { label: "Copy commit message", onSelect: () => copy(c.body ? `${c.summary}\n\n${c.body}` : c.summary) },
    ...linkItems(repo, "Copy link to this commit", { kind: "commit", sha: c.sha }),
    { separator: true },
    { label: "Create tag here", onSelect: () => promptTag(path, c.sha) },
    { label: "Create annotated tag here", onSelect: () => promptTag(path, c.sha, true) },
  ];
}

/** Reword / squash / drop / move / interactive rebase, for commits on the checked-out branch's line. */
function historyItems(repo: RepoView, c: Commit): MenuEntry[] {
  const line = headLine(repo);
  const pos = line.indexOf(c.sha);
  if (pos < 0 || !repo.head_branch) return [{ label: "Select a commit on the checked-out branch", disabled: true }];
  const children = pos; // commits newer than this one on the line
  const head = repo.head_branch;
  const items: MenuEntry[] = [];
  if (children > 0)
    items.push({
      label: `Interactive Rebase ${children} children of ${c.sha.slice(0, useStore.getState().metrics.sha_len)}`,
      onSelect: () => openRebase(activeTab(), repo.path, c.sha, c.sha.slice(0, useStore.getState().metrics.sha_len)),
    });
  items.push({ label: `Interactive Rebase ${head} from here`, onSelect: () => openRebase(activeTab(), repo.path, baseFor(repo, c.sha), head) });
  if (pos + 1 < line.length) items.push({ label: "Squash into the commit below", onSelect: () => squashCommits(repo, [c.sha]) });
  items.push({ label: "Drop commit", danger: true, onSelect: () => dropCommits(repo, [c.sha]) });
  if (pos > 0) items.push({ label: "Move commit up", onSelect: () => moveCommit(repo, c.sha, "up") });
  if (pos + 1 < line.length) items.push({ label: "Move commit down", onSelect: () => moveCommit(repo, c.sha, "down") });
  return items;
}

export function stashMenu(repo: RepoView, st: Stash): MenuEntry[] {
  const path = repo.path;
  return [
    { label: "Apply Stash", onSelect: () => act(path, { type: "stashApply", index: st.index }) },
    { label: "Pop Stash", onSelect: () => act(path, { type: "stashPop", index: st.index }) },
    {
      label: "Edit stash message",
      onSelect: () =>
        useStore.getState().setDialog({
          kind: "prompt",
          title: "Edit stash message",
          label: "Stash message",
          value: st.message,
          confirm: "Save",
          onSubmit: (v) => act(path, { type: "stashRename", index: st.index, message: v }),
        }),
    },
    {
      label: "Delete Stash",
      danger: true,
      onSelect: () => confirmThen("Delete this stash?", st.message, "Delete", () => act(path, { type: "stashDrop", index: st.index })),
    },
    { separator: true },
    { label: "Copy stash sha", onSelect: () => copy(st.sha) },
  ];
}

export function fileMenu(repo: RepoView, file: string, staged: boolean, inCommit: boolean, commitSha?: string): MenuEntry[] {
  const path = repo.path;
  const items: MenuEntry[] = [];
  const stash = commitSha ? repo.stashes.find((st) => st.sha === commitSha) : undefined;
  if (stash) {
    items.push({ label: "Apply stashed file", onSelect: () => act(path, { type: "stashTakeFile", index: stash.index, path: file }) });
    items.push({ separator: true });
  }
  if (!inCommit) {
    if (staged) items.push({ label: "Unstage", onSelect: () => act(path, { type: "unstage", paths: [file] }, true) });
    else {
      items.push({ label: "Stage", onSelect: () => act(path, { type: "stage", paths: [file] }, true) });
      items.push({ label: "Stash file", onSelect: () => act(path, { type: "stashFiles", paths: [file], message: "" }) });
      items.push({
        label: "Discard changes",
        danger: true,
        onSelect: () => confirmThen("Discard changes?", `Discard all changes to ${file}? You can undo this.`, "Discard", () => act(path, { type: "discard", paths: [file] })),
      });
    }
    items.push({ separator: true });
  }
  const dir = file.includes("/") ? file.slice(0, file.lastIndexOf("/") + 1) : "";
  const ext = /\.[^./]+$/.exec(file)?.[0];
  const tracked = staged || inCommit || repo.status.unstaged.some((f) => f.path === file && f.status !== "?");
  const ign = (pattern: string, untrack: string[]) => act(path, { type: "ignore", pattern, untrack: tracked ? untrack : [] });
  items.push({
    label: tracked ? "Ignore and stop tracking" : "Ignore",
    children: [
      { label: "Ignore this file", onSelect: () => ign("/" + file, [file]) },
      ...(dir ? [{ label: `Ignore all files in "${dir}"`, onSelect: () => ign("/" + dir, [dir]) }] : []),
      ...(ext ? [{ label: `Ignore all files with extension ${ext}`, onSelect: () => ign("*" + ext, [file]) }] : []),
    ],
  });
  if (ext) items.push({ label: `Track ${ext} files with Git LFS`, onSelect: () => act(path, { type: "lfsTrack", pattern: "*" + ext }) });
  if (!inCommit)
    items.push({
      label: "Open in External Diff Tool",
      shortcut: "Ctrl+D",
      onSelect: () => act(path, { type: "externalDiff", file, staged, tool: useStore.getState().settings.diffTool }),
    });
  if (repo.status.unstaged.some((f) => f.path === file && f.status === "U"))
    items.push({ label: "Open in External Merge Tool", onSelect: () => act(path, { type: "externalMerge", file, tool: useStore.getState().settings.mergeTool }) });
  items.push({ separator: true });
  items.push({ label: "Copy file path", onSelect: () => copy(file) });
  // File menu: "Copy file contents to clipboard" (the version shown: working copy, or the commit's).
  items.push({
    label: "Copy file contents to clipboard",
    onSelect: () =>
      api.fileText(path, file, inCommit && commitSha ? commitSha : staged ? ":" : undefined).then(copy, (e) => useStore.getState().toast(false, String(e))),
  });
  const fail = (e: unknown) => useStore.getState().toast(false, String(e));
  items.push({ label: `Open in ${editorName(useStore.getState().settings.externalEditor)}`, onSelect: () => api.openInEditor(path, file).catch(fail) });
  items.push({ label: "Open file in default program", onSelect: () => api.openDefault(path, file).catch(fail) });
  items.push({ label: "Open in File Manager", onSelect: () => api.revealFile(path, file).catch(fail) });
  items.push({ separator: true });
  items.push({ label: "File History", onSelect: () => openHistory(file, "history") });
  items.push({ label: "Blame", onSelect: () => openHistory(file, "blame") });
  return items;
}
