import type { MenuEntry } from "@/components/ui/menu";
import type { Commit, GitRef, RepoView, Stash } from "./api";
import { api } from "./api";
import { act, clearSolo, confirmThen, copy, openHistory, promptBranch, promptTag, toggleHidden, toggleSolo } from "./repo";
import { useStore } from "./store";

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
    items.push({ label: `Rebase ${head} onto ${ref.name}`, onSelect: () => act(path, { type: "rebase", onto: ref.name }) });
  }
  items.push({ separator: true });
  if (isHead) items.push({ label: `Push ${ref.name}`, onSelect: () => act(path, { type: "push", force: false }) });
  items.push({ label: "Create branch here", onSelect: () => promptBranch(path, ref.sha) });
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
  if ((st.solo[path] ?? []).length > 0) items.push({ label: "Stop soloing all branches", onSelect: () => clearSolo(path) });
  items.push({ separator: true });
  items.push({ label: "Copy branch name", onSelect: () => copy(ref.name) });
  items.push({ label: "Copy commit sha", onSelect: () => copy(ref.sha) });
  return items;
}

export function tagMenu(repo: RepoView, ref: GitRef): MenuEntry[] {
  const path = repo.path;
  return [
    { label: `Checkout the commit at ${ref.name}`, onSelect: () => act(path, { type: "checkout", target: ref.name }) },
    { label: "Create branch here", onSelect: () => promptBranch(path, ref.sha) },
    { separator: true },
    { label: `Delete ${ref.name} locally`, danger: true, onSelect: () => act(path, { type: "deleteTag", name: ref.name }) },
    { label: (useStore.getState().settings.hidden[path] ?? []).includes(ref.full) ? "Show in the graph" : "Hide in the graph", onSelect: () => toggleHidden(path, ref.full) },
    { separator: true },
    { label: "Copy tag name", onSelect: () => copy(ref.name) },
  ];
}

export function commitMenu(repo: RepoView, c: Commit): MenuEntry[] {
  const path = repo.path;
  const head = repo.head_branch ?? "HEAD";
  const isHead = c.sha === repo.head_sha;
  const reset = (mode: string, label: string) => () =>
    mode === "hard"
      ? confirmThen(`Reset ${head} to this commit?`, "Hard reset throws away all uncommitted changes. Undo can move the branch back, but not bring back uncommitted work.", "Reset Hard", () =>
          act(path, { type: "reset", sha: c.sha, mode }),
        )
      : act(path, { type: "reset", sha: c.sha, mode }).then(() => void label);
  return [
    { label: "Checkout this commit", onSelect: () => act(path, { type: "checkout", target: c.sha }) },
    { label: "Create branch here", shortcut: "", onSelect: () => promptBranch(path, c.sha) },
    { label: "Create tag here", onSelect: () => promptTag(path, c.sha) },
    { separator: true },
    { label: "Cherry pick commit", disabled: isHead, onSelect: () => act(path, { type: "cherryPick", sha: c.sha }) },
    {
      label: `Reset ${head} to this commit`,
      disabled: isHead,
      children: [
        { label: "Soft - keep all changes", onSelect: reset("soft", "soft") },
        { label: "Mixed - keep working copy but reset index", onSelect: reset("mixed", "mixed") },
        { label: "Hard - discard all changes", danger: true, onSelect: reset("hard", "hard") },
      ],
    },
    { label: "Revert commit", onSelect: () => act(path, { type: "revert", sha: c.sha }) },
    { separator: true },
    {
      label: "Compare commit against working directory",
      onSelect: () => {
        const st = useStore.getState();
        st.setUi(st.activeId, { compareWorkdir: c.sha, multi: [], selection: { type: "commit", sha: c.sha }, diff: null, history: null });
      },
    },
    { separator: true },
    { label: "Interactive rebase, reword and squash come in Stage 3", disabled: true },
    { separator: true },
    { label: "Copy commit sha", onSelect: () => copy(c.sha) },
    { label: "Copy commit message", onSelect: () => copy(c.body ? `${c.summary}\n\n${c.body}` : c.summary) },
  ];
}

export function stashMenu(repo: RepoView, st: Stash): MenuEntry[] {
  const path = repo.path;
  return [
    { label: "Apply Stash", onSelect: () => act(path, { type: "stashApply", index: st.index }) },
    { label: "Pop Stash", onSelect: () => act(path, { type: "stashPop", index: st.index }) },
    {
      label: "Delete Stash",
      danger: true,
      onSelect: () => confirmThen("Delete this stash?", st.message, "Delete", () => act(path, { type: "stashDrop", index: st.index })),
    },
    { separator: true },
    { label: "Copy stash sha", onSelect: () => copy(st.sha) },
  ];
}

export function fileMenu(repo: RepoView, file: string, staged: boolean, inCommit: boolean): MenuEntry[] {
  const path = repo.path;
  const items: MenuEntry[] = [];
  if (!inCommit) {
    if (staged) items.push({ label: "Unstage", onSelect: () => act(path, { type: "unstage", paths: [file] }, true) });
    else {
      items.push({ label: "Stage", onSelect: () => act(path, { type: "stage", paths: [file] }, true) });
      items.push({
        label: "Discard changes",
        danger: true,
        onSelect: () => confirmThen("Discard changes?", `Discard all changes to ${file}? You can undo this.`, "Discard", () => act(path, { type: "discard", paths: [file] })),
      });
    }
    items.push({ separator: true });
  }
  items.push({ label: "Copy file path", onSelect: () => copy(file) });
  items.push({ label: "Open in File Manager", onSelect: () => api.openFileManager(`${path}\\${file.replace(/\/[^/]*$/, "").replace(/\//g, "\\")}`) });
  items.push({ separator: true });
  items.push({ label: "File History", onSelect: () => openHistory(file, "history") });
  items.push({ label: "Blame", onSelect: () => openHistory(file, "blame") });
  return items;
}
