import { QueryObserver } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api, type RepoView, type HistoryPage } from "./api";
import { act, loadMoreRepo, onWatcherChange, openRepoPath, queryClient, refresh, refreshStatus, readRepoSnapshot, releaseRepo, repoData } from "./repo";
import { useStore } from "./store";

const commit = (sha: string) => ({ sha, parents: [], author: "A", email: "a@b.c", time: 1, summary: sha, body: "" });
const cursor = { revs: ["a".repeat(40)], offset: 1, layout: { lanes: [null], pinned_next: null, pinned: false, width: 1 } };
const row = { col: 0, color: 0, up: [], down: [], merge: false };
const repo: RepoView = {
  path: "/repos/paging", name: "paging", commits: [commit("first")], refs: [], stashes: [], worktrees: [], submodules: [], remotes: [],
  head_branch: "main", head_sha: "first", status: { unstaged: [], staged: [], conflicted: false }, state_note: null,
  has_more: true, history_cursor: cursor, merge_msg: null, conflict_labels: null,
  graph: { rows: [row], width: 1 }, undo: { undo: null, redo: null },
};
const page: HistoryPage = { commits: [commit("second")], graph: { rows: [row], width: 2 }, has_more: false, history_cursor: null };
const cache = () => queryClient.getQueryCache().findAll({ queryKey: ["repo", repo.path] })[0];
async function open() {
  vi.spyOn(api, "openRepo").mockResolvedValue(repo);
  await openRepoPath(repo.path);
}
afterEach(() => {
  releaseRepo(repo.path);
  queryClient.clear();
  useStore.setState(useStore.getInitialState(), true);
  vi.restoreAllMocks();
});
describe("repository loading and refresh", () => {
  it("seeds a fresh query on open instead of reading the repository twice", async () => {
    await open();
    const read = vi.fn(() => api.openRepo(repo.path));
    expect(await queryClient.fetchQuery({ queryKey: cache().queryKey, queryFn: read, staleTime: Infinity })).toEqual(repo);
    expect(read).not.toHaveBeenCalled();
    expect(api.openRepo).toHaveBeenCalledTimes(1);
  });
  it("coalesces page requests and appends rows without replacing the first page", async () => {
    await open();
    let finish!: (p: HistoryPage) => void;
    const read = vi.spyOn(api, "historyPage").mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    const first = loadMoreRepo(repo.path);
    const second = loadMoreRepo(repo.path);
    expect(first).toBe(second);
    expect(read).toHaveBeenCalledTimes(1);
    finish(page);
    await first;
    const result = cache().state.data as RepoView;
    expect(result.commits.map((c) => c.sha)).toEqual(["first", "second"]);
    expect(result.graph.rows).toHaveLength(2);
    expect(result.graph.width).toBe(2);
    expect(result.has_more).toBe(false);
  });
  it("discards a page from before a full refresh", async () => {
    await open();
    let finish!: (p: HistoryPage) => void;
    vi.spyOn(api, "historyPage").mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    const loading = loadMoreRepo(repo.path);
    await refresh(repo.path);
    finish(page);
    await loading;
    expect((cache().state.data as RepoView).commits).toHaveLength(1);
  });
  it("releases closed snapshots and refuses late page results", async () => {
    await open();
    let finish!: (p: HistoryPage) => void;
    vi.spyOn(api, "historyPage").mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    const loading = loadMoreRepo(repo.path);
    releaseRepo(repo.path);
    finish(page);
    await loading;
    expect(repoData(repo.path)).toBeUndefined();
    expect(cache()).toBeUndefined();
  });
  it("defers hidden repository events and combines checkout watcher events into its final refresh", async () => {
    await open();
    const invalidate = vi.spyOn(queryClient, "invalidateQueries").mockResolvedValue();
    let finish!: (r: { ok: boolean; message: string }) => void;
    vi.spyOn(api, "run").mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    const action = act(repo.path, { type: "checkout", target: "main" }, true);
    onWatcherChange(repo.path, true);
    onWatcherChange(repo.path, false);
    expect(invalidate).not.toHaveBeenCalled();
    finish({ ok: true, message: "Checked out" });
    expect(await action).toBe(true);
    expect(invalidate.mock.calls.filter(([arg]) => arg?.queryKey?.[0] === "repo")).toHaveLength(1);
    expect(useStore.getState().busy).toBeNull();
    invalidate.mockClear();
    useStore.getState().newTab();
    onWatcherChange(repo.path, true);
    expect(invalidate).not.toHaveBeenCalled();
  });
});

describe("repository refresh safety", () => {
  it("rejects a delayed status response from before a full refresh", async () => {
    await open();
    let finish!: (status: RepoView["status"]) => void;
    vi.spyOn(api, "repoStatus").mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    const reading = refreshStatus(repo.path);
    const fresh = { ...repo, head_sha: "new-head", status: { staged: [], unstaged: [{ path: "new-branch.txt", status: "M" }], conflicted: false } };
    const observer = new QueryObserver(queryClient, { queryKey: cache().queryKey, queryFn: async () => fresh, staleTime: Infinity });
    const stop = observer.subscribe(() => {});
    try {
      await refresh(repo.path);
      finish({ staged: [], unstaged: [{ path: "old-branch.txt", status: "M" }], conflicted: false });
      await reading;
      expect((cache().state.data as RepoView).status).toEqual(fresh.status);
    } finally { stop(); }
  });
  it("keeps loaded history and selection when only metadata changed", async () => {
    await open();
    const pages = vi.spyOn(api, "historyPage").mockResolvedValue(page);
    await loadMoreRepo(repo.path);
    const tab = useStore.getState().activeId;
    useStore.getState().setUi(tab, { selection: { type: "commit", sha: "second" } });
    const observer = new QueryObserver(queryClient, { queryKey: cache().queryKey, staleTime: Infinity,
      queryFn: () => readRepoSnapshot(repo.path, 500, { hidden: [], solo: [] }, cache().state.data as RepoView) });
    const stop = observer.subscribe(() => {});
    try {
      await refresh(repo.path);
      expect((cache().state.data as RepoView).commits.map((c) => c.sha)).toEqual(["first", "second"]);
      expect(pages).toHaveBeenCalledTimes(1);
      expect(useStore.getState().ui[tab].selection).toEqual({ type: "commit", sha: "second" });
    } finally { stop(); }
  });
  it("restores the selected older commit when new commits shift it onto later pages", async () => {
    await open();
    const previous = { ...repo, commits: [commit("first"), commit("selected")] };
    useStore.getState().setUi(useStore.getState().activeId, { selection: { type: "commit", sha: "selected" } });
    vi.mocked(api.openRepo).mockResolvedValue({ ...repo, head_sha: "new", commits: [commit("new")] });
    vi.spyOn(api, "historyPage").mockResolvedValue({ ...page, commits: [commit("first"), commit("selected")] });
    const result = await readRepoSnapshot(repo.path, 500, { hidden: [], solo: [] }, previous);
    expect(result.commits.map((c) => c.sha)).toEqual(["new", "first", "selected"]);
  });
});
