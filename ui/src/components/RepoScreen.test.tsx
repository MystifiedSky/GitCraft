import { afterEach, expect, it, vi } from "vitest";
import { renderToString } from "react-dom/server";
import { QueryClientProvider } from "@tanstack/react-query";
import { RepoScreen } from "../App";
import { api, type RepoView } from "@/lib/api";
import { openRepoPath, queryClient, releaseRepo } from "@/lib/repo";
import { useStore } from "@/lib/store";
// Client rendering reads the live store; SSR normally uses Zustand's boot snapshot.
vi.mock("@/lib/store", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/store")>();
  const useStore = Object.assign((select: (s: ReturnType<typeof actual.useStore.getState>) => unknown) => select(actual.useStore.getState()), actual.useStore);
  return { ...actual, useStore, useTabUi: (id: string) => actual.useStore.getState().ui[id] };
});
vi.mock("./DiffView", () => ({ DiffView: () => <div>retained-file-editor</div> }));
vi.mock("./ConflictView", () => ({ ConflictView: () => <div>retained-conflict-editor</div> }));
vi.mock("./RightPanel", () => ({ RightPanel: () => <div>retained-commit-panel</div> }));
vi.mock("./LeftPanel", () => ({ LeftPanel: () => null, LeftStrip: () => null }));
vi.mock("./Toolbar", () => ({ Toolbar: () => null }));
const repo: RepoView = { path: "/retained", name: "retained", commits: [], refs: [], stashes: [], worktrees: [], submodules: [], remotes: [], head_branch: "main", head_sha: "a", status: { staged: [], unstaged: [], conflicted: false }, state_note: null, has_more: false, merge_msg: null, conflict_labels: null, graph: { rows: [], width: 1 }, undo: { undo: null, redo: null } };
afterEach(() => { releaseRepo(repo.path); queryClient.clear(); vi.restoreAllMocks(); useStore.setState(useStore.getInitialState(), true); });
it.each(["diff", "conflict"] as const)("keeps the %s editor mounted when its repository tab becomes inactive", async (view) => {
  vi.spyOn(api, "openRepo").mockResolvedValue(repo);
  await openRepoPath(repo.path);
  const state = useStore.getState();
  const tab = state.tabs.find((t) => t.id === state.activeId)!;
  state.setUi(tab.id, view === "conflict" ? { conflict: "file.txt" } : { diff: { source: { kind: "Unstaged" }, file: "file.txt", parent: null, untracked: false } });
  const rendered = renderToString(<QueryClientProvider client={queryClient}><RepoScreen tab={tab} active={false} /></QueryClientProvider>);
  expect(rendered).toContain(view === "diff" ? "retained-file-editor" : "retained-conflict-editor");
  expect(rendered).toContain("retained-commit-panel");
});
