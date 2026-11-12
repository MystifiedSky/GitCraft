import type { RepoView } from "./api";

/** A file changed in both the index and the working folder is counted once. */
export function workingChanges(status: RepoView["status"]) {
  const files = new Map(status.staged.map((f) => [f.path, f]));
  for (const f of status.unstaged) {
    const staged = files.get(f.path);
    files.set(f.path, staged?.status === "A" || staged?.status === "R" ? staged : f);
  }
  const list = [...files.values()];
  return {
    total: list.length,
    added: list.filter((f) => f.status === "A" || f.status === "?").length,
    deleted: list.filter((f) => f.status === "D").length,
    modified: list.filter((f) => !["A", "?", "D"].includes(f.status)).length,
  };
}
