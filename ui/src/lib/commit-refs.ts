import type { GitRef } from "./api";

export interface RefGroup { ref: GitRef; remote?: GitRef }

/** Combine a local branch with its tracking ref only while both point at the same commit. */
export function groupCommitRefs(refs: GitRef[]): RefGroup[] {
  const paired = new Map<GitRef, GitRef>();
  const used = new Set<GitRef>();
  for (const local of refs.filter((r) => r.kind === "Local")) {
    const candidates = refs.filter((r) => r.kind === "Remote" && r.sha === local.sha && !used.has(r));
    const upstream = local.upstream?.replace(/^refs\/remotes\//, "");
    const matching = upstream ? candidates.filter((r) => r.name === upstream) : candidates.filter((r) => r.name.slice(r.name.indexOf("/") + 1) === local.name);
    // Without a configured upstream, do not guess between identically named branches on several remotes.
    if (matching.length === 1) { paired.set(local, matching[0]); used.add(matching[0]); }
  }
  return refs.filter((r) => !used.has(r)).map((ref) => ({ ref, remote: paired.get(ref) }));
}
