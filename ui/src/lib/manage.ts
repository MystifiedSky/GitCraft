// Stage 4 dialogs: remotes, worktrees, submodules, Gitflow, branch clean-up.
import { api, type RepoView } from "./api";
import { act, confirmThen, openRepoPath } from "./repo";
import { useStore, type FormField } from "./store";

const form = (title: string, fields: FormField[], confirm: string, onSubmit: (v: Record<string, string | boolean>) => void, help?: string) =>
  useStore.getState().setDialog({ kind: "form", title, fields, confirm, onSubmit, help });

const str = (v: string | boolean | undefined) => (typeof v === "string" ? v.trim() : "");

export function addRemote(path: string) {
  form(
    "Add Remote",
    [
      { key: "name", label: "Name", value: "origin" },
      { key: "url", label: "Pull URL", value: "", placeholder: "https://github.com/owner/project.git" },
      { key: "push", label: "Push URL (optional)", value: "" },
    ],
    "Add Remote",
    async (v) => {
      const ok = await act(path, { type: "addRemote", name: str(v.name), url: str(v.url) });
      if (ok && str(v.push)) await act(path, { type: "remoteEdit", name: str(v.name), newName: "", url: str(v.url), pushUrl: str(v.push) }, true);
      if (ok) await act(path, { type: "fetchRemote", remote: str(v.name) }, true);
    },
  );
}

export async function editRemote(path: string, name: string) {
  const r = (await api.remotesList(path)).find((x) => x.name === name);
  form(
    "Edit Remote",
    [
      { key: "name", label: "Name", value: name },
      { key: "url", label: "Pull URL", value: r?.fetch_url ?? "" },
      { key: "push", label: "Push URL", value: r?.push_url ?? "" },
    ],
    "Save",
    (v) => act(path, { type: "remoteEdit", name, newName: str(v.name), url: str(v.url), pushUrl: str(v.push) }),
  );
}

export function removeRemote(path: string, name: string) {
  confirmThen(`Remove ${name}?`, "The remote and its remote-tracking branches are removed from this repository. Nothing on the server changes.", "Remove", () =>
    act(path, { type: "remoteRemove", name }),
  );
}

export function addWorktree(repo: RepoView, branch?: string, start?: string) {
  const parent = repo.path.replace(/[\\/][^\\/]+$/, "");
  const name = (branch ?? "worktree").replace(/[\\/]/g, "-");
  form(
    "Add a new worktree",
    [
      { key: "path", label: "Folder for the worktree", value: `${parent}${repo.path.includes("\\") ? "\\" : "/"}${repo.name}-${name}`, browse: true },
      { key: "branch", label: "Branch", value: branch ?? "", placeholder: "existing branch, or a new branch name" },
      { key: "new", label: start ? `Create a new branch from ${start.slice(0, 8)}` : "Create this as a new branch from the checked-out commit", value: !branch, type: "checkbox" },
    ],
    "Create Worktree",
    async (v) => {
      const ok = await act(repo.path, { type: "worktreeAdd", path: str(v.path), branch: str(v.branch), newBranch: !!v.new, start: start ?? null });
      if (ok) await openRepoPath(str(v.path));
    },
    "A worktree is a second working folder for the same repository, with its own branch checked out.",
  );
}

export function lockWorktree(path: string, wt: string) {
  form("Lock worktree", [{ key: "msg", label: "Message (optional):", value: "", placeholder: "Lock message" }], "Lock", (v) => act(path, { type: "worktreeLock", path: wt, message: str(v.msg) }));
}

export function removeWorktree(path: string, wt: string) {
  useStore.getState().setDialog({
    kind: "choice",
    title: "Remove worktree",
    text: `Remove the worktree at '${wt}'? Its folder is deleted; the branch stays.`,
    choices: [
      { label: "Remove", variant: "danger", onSelect: () => act(path, { type: "worktreeRemove", path: wt, force: false }) },
      { label: "Remove (force)", variant: "danger", onSelect: () => act(path, { type: "worktreeRemove", path: wt, force: true }) },
    ],
  });
}

export function addSubmodule(path: string) {
  form(
    "Add Submodule",
    [
      { key: "url", label: "Remote URL", value: "", placeholder: "https://github.com/owner/library.git" },
      { key: "path", label: "Path", value: "", placeholder: "libs/library" },
    ],
    "Add Submodule",
    (v) => act(path, { type: "submoduleAdd", url: str(v.url), path: str(v.path) }),
  );
}

export async function initGitflow(path: string) {
  const c = await api.gitflowConfig(path);
  form(
    c.initialized ? "Update Gitflow Config" : "Initialize Gitflow",
    [
      { key: "master", label: "Main/Master", value: c.master },
      { key: "develop", label: "Develop", value: c.develop },
      { key: "feature", label: "Feature prefix", value: c.feature },
      { key: "release", label: "Release prefix", value: c.release },
      { key: "hotfix", label: "Hotfix prefix", value: c.hotfix },
      { key: "versiontag", label: "Version Tag prefix", value: c.versiontag },
    ],
    c.initialized ? "Update Gitflow Config" : "Initialize Gitflow",
    (v) =>
      act(path, {
        type: "gitflowInit",
        config: { master: str(v.master), develop: str(v.develop), feature: str(v.feature), release: str(v.release), hotfix: str(v.hotfix), versiontag: str(v.versiontag), initialized: true },
      }),
    c.initialized ? "Changing these prefixes will cause any existing branches with different prefixes to be ignored by Gitflow." : undefined,
  );
}

export function startGitflow(path: string, kind: "feature" | "release" | "hotfix") {
  const label = { feature: "Feature", release: "Release", hotfix: "Hotfix" }[kind];
  form(`Start ${label}`, [{ key: "name", label: kind === "feature" ? "Feature name" : `${label} version`, value: "" }], `Start ${label}`, (v) =>
    act(path, { type: "gitflowStart", kind, name: str(v.name) }),
  );
}

export function finishGitflow(path: string, kind: "feature" | "release" | "hotfix", name: string) {
  const label = { feature: "Feature", release: "Release", hotfix: "Hotfix" }[kind];
  const fields: FormField[] = [{ key: "delete", label: "Delete branch", value: true, type: "checkbox" }];
  if (kind !== "feature") fields.push({ key: "tag", label: "Tag message", value: "", placeholder: "tag message" });
  form(`Finish ${label}: ${name}`, fields, `Finish ${label}`, (v) => act(path, { type: "gitflowFinish", kind, name, keep: !v.delete, tagMessage: str(v.tag) }));
}

export function openCleanup() {
  useStore.getState().setDialog({ kind: "cleanup" });
}
