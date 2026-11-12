import { afterEach, describe, expect, it, vi } from "vitest";
import { api, type RepoView } from "./api";
import { branchMenu, commitMenu, fileMenu, tagMenu, isLoadedAncestor } from "./menus";
import { addWorktree } from "./manage";
import { hosting, startPullRequest } from "./hosting";
import { queryClient } from "./repo";
import { useStore } from "./store";

vi.mock("@/components/TitleBar", () => ({ editorName: () => "Editor" }));

const commit = { sha: "abc123", parents: [], author: "A", email: "a@b.c", time: 1, summary: "Selected", body: "" };
const ref = { name: "main", full: "refs/heads/main", sha: commit.sha, kind: "Local" as const, upstream: "", ahead: 0, behind: 0 };
const repo: RepoView = {
  path: "/repos/a", name: "a", commits: [commit], refs: [ref], head_branch: "main", head_sha: commit.sha,
  remotes: [], worktrees: [], submodules: [], stashes: [], status: { staged: [], unstaged: [], conflicted: false },
  state_note: null, has_more: false, merge_msg: null, conflict_labels: null,
  graph: { rows: [], width: 0 }, undo: { undo: null, redo: null },
};

afterEach(() => {
  vi.restoreAllMocks();
  queryClient.clear();
  useStore.setState(useStore.getInitialState(), true);
});

describe("reference commit and branch actions", () => {
  it("offers all reset modes on the current commit and its branch label", () => {
    for (const items of [commitMenu(repo, commit), branchMenu(repo, ref)]) {
      const reset = items.find((i) => i.label === "Reset main to this commit")!;
      expect(reset.disabled).not.toBe(true);
      expect(reset.children?.map((i) => i.label)).toEqual([
        "Soft - keep all changes", "Mixed - keep working copy but reset index", "Hard - discard all changes",
      ]);
    }
  });

  it("requires confirmation before hard reset and uses the selected commit", async () => {
    const run = vi.spyOn(api, "run").mockResolvedValue({ ok: true, message: "Reset branch" });
    const reset = commitMenu(repo, commit).find((i) => i.label === "Reset main to this commit")!;
    reset.children![2].onSelect!();
    expect(run).not.toHaveBeenCalled();
    const dialog = useStore.getState().dialog;
    expect(dialog?.kind).toBe("confirm");
    if (dialog?.kind !== "confirm") throw new Error("Missing reset confirmation");
    await dialog.onConfirm();
    expect(run).toHaveBeenCalledWith(repo.path, { type: "reset", sha: commit.sha, mode: "hard" });
  });

  it("offers detached checkout, worktrees, and a separate annotated tag entry", () => {
    const items = commitMenu(repo, commit);
    expect(items.find((i) => i.label === "Checkout")?.children?.map((i) => i.label)).toEqual(["main", "this commit (detached HEAD)"]);
    expect(items.find((i) => i.label === "Create worktree from")?.children?.[0].label).toBe("this commit");
    items.find((i) => i.label === "Create annotated tag here")!.onSelect!();
    const dialog = useStore.getState().dialog;
    if (dialog?.kind !== "prompt") throw new Error("Missing annotated tag dialog");
    expect(dialog.title).toBe("Create annotated tag here");
  });

  it("creates a new worktree from the chosen commit and uses the native path separator", async () => {
    const run = vi.spyOn(api, "run").mockResolvedValue({ ok: false, message: "Test response" });
    addWorktree(repo, undefined, commit.sha);
    const dialog = useStore.getState().dialog;
    if (dialog?.kind !== "form") throw new Error("Missing worktree dialog");
    expect(dialog.fields[0].value).toBe("/repos/a-worktree");
    await dialog.onSubmit({ path: "/repos/selected", branch: "selected", new: true });
    expect(run).toHaveBeenCalledWith(repo.path, { type: "worktreeAdd", path: "/repos/selected", branch: "selected", newBranch: true, start: commit.sha });
  });

  it("pushes the selected branch when creating a pull request", async () => {
    const run = vi.spyOn(api, "run").mockResolvedValue({ ok: false, message: "Test stops before remote creation" });
    vi.spyOn(hosting, "template").mockResolvedValue(null);
    await startPullRequest(repo.path, "feature/selected");
    const dialog = useStore.getState().dialog;
    if (dialog?.kind !== "form") throw new Error("Missing pull request dialog");
    await dialog.onSubmit({ target: "main", title: "Feature", body: "", draft: false });
    expect(run).toHaveBeenCalledWith(repo.path, { type: "pushBranch", branch: "feature/selected" });
  });
});

describe("fast-forward menu eligibility", () => {
  it("uses ancestry rather than the ordering of unrelated branches", () => {
    const tip = { ...commit, sha: "other", parents: ["missing"] };
    const divergent = { ...repo, commits: [tip, commit] };
    expect(isLoadedAncestor(divergent, commit.sha, tip.sha)).toBe(false);
    const descendant = { ...repo, commits: [{ ...tip, parents: [commit.sha] }, commit] };
    expect(isLoadedAncestor(descendant, commit.sha, tip.sha)).toBe(true);
    const remote = { ...ref, kind: "Remote" as const, name: "origin/other", full: "refs/remotes/origin/other", sha: tip.sha };
    expect(branchMenu(divergent, remote).some((i) => i.label === "Fast-forward main to origin/other")).toBe(false);
    expect(branchMenu(descendant, remote).some((i) => i.label === "Fast-forward main to origin/other")).toBe(true);
  });
});

describe("audit gaps", () => {
  it("copies staged contents from the index and unstaged contents from the working folder", async () => {
    const read = vi.spyOn(api, "fileText").mockRejectedValue("stop before clipboard");
    fileMenu(repo, "a.txt", true, false).find((i) => i.label === "Copy file contents to clipboard")!.onSelect!();
    expect(read).toHaveBeenLastCalledWith(repo.path, "a.txt", ":");
    fileMenu(repo, "a.txt", false, false).find((i) => i.label === "Copy file contents to clipboard")!.onSelect!();
    expect(read).toHaveBeenLastCalledWith(repo.path, "a.txt", undefined);
  });

  it("requires confirmation and passes the chosen parent for merge actions", async () => {
    const run = vi.spyOn(api, "run").mockResolvedValue({ ok: true, message: "Done" });
    const merge = { ...commit, parents: ["first", "second"] };
    for (const [label, type] of [["Revert commit", "revert"], ["Cherry pick commit", "cherryPick"]]) {
      const items = commitMenu({ ...repo, head_sha: "another" }, merge);
      const menu = items.find((i) => i.label === label)!;
      expect(menu.children).toHaveLength(2);
      run.mockClear();
      menu.children![1].onSelect!();
      expect(run).not.toHaveBeenCalled();
      const dialog = useStore.getState().dialog;
      if (dialog?.kind !== "confirm") throw Error("Missing merge parent confirmation");
      await dialog.onConfirm();
      expect(run).toHaveBeenCalledWith(repo.path, { type, sha: commit.sha, mainline: 2 });
    }
  });

  it("offers each remote and keeps remote tag deletion behind confirmation", async () => {
    const run = vi.spyOn(api, "run").mockResolvedValue({ ok: true, message: "Done" });
    const tag = { ...ref, kind: "Tag" as const, name: "v1", full: "refs/tags/v1" };
    expect(tagMenu(repo, tag).some((i) => i.label?.includes("to remote"))).toBe(false);
    const items = tagMenu({ ...repo, remotes: ["origin", "backup"] }, tag);
    expect(items.find((i) => i.label === "Push v1 to remote")?.children?.map((i) => i.label)).toEqual(["origin", "backup"]);
    items.find((i) => i.label === "Delete v1 from remote")!.children![1].onSelect!();
    expect(run).not.toHaveBeenCalled();
    const dialog = useStore.getState().dialog;
    if (dialog?.kind !== "confirm") throw Error("Missing remote deletion confirmation");
    await dialog.onConfirm();
    expect(run).toHaveBeenCalledWith(repo.path, { type: "deleteRemoteTag", name: "v1", remote: "backup" });
  });
});
