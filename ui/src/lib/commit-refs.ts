import type { GitRef } from "./api";

export interface RefGroup { ref: GitRef; remote?: GitRef }

/** Combine a local branch with its tracking ref only while both point at the same commit. */
export function groupCommitRefs(refs: GitRef[]): RefGroup[] {
  const paired = new Map<GitRef, GitRef>();
  const used = new Set<GitRef>();
  const full = new Map<string, Set<GitRef>>();
  const short = new Map<string, Set<GitRef>>();
  const key = (sha: string, name: string) => `${sha}\0${name}`;
  const basename = (ref: GitRef) => ref.name.slice(ref.name.indexOf("/") + 1);
  const insert = (index: Map<string, Set<GitRef>>, name: string, ref: GitRef) => {
    const bucket = index.get(name) ?? new Set<GitRef>(); bucket.add(ref); index.set(name, bucket);
  };
  for (const ref of refs) if (ref.kind === "Remote") {
    insert(full, key(ref.sha, ref.name), ref); insert(short, key(ref.sha, basename(ref)), ref);
  }
  for (const local of refs) if (local.kind === "Local") {
    const upstream = local.upstream?.replace(/^refs\/remotes\//, "");
    const candidates = upstream ? full.get(key(local.sha, upstream)) : short.get(key(local.sha, local.name));
    // A missing upstream must never guess between several remotes.
    if (candidates?.size === 1) {
      const remote = candidates.values().next().value!;
      paired.set(local, remote); used.add(remote);
      full.get(key(remote.sha, remote.name))?.delete(remote);
      short.get(key(remote.sha, basename(remote)))?.delete(remote);
    }
  }
  return refs.filter((r) => !used.has(r)).map((ref) => ({ ref, remote: paired.get(ref) }));
}
