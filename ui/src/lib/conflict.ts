import type { RepoView } from "./api";

/** "origin/**", "origin/feature/**", "!origin/abc" (Conflict Prevention > Base branches). */
export function globMatch(pattern: string, name: string) {
  const re = pattern
    .trim()
    .replace(/[.+?^${}()|[\]\\]/g, "\\$&")
    .replace(/\*\*/g, "\u0000")
    .replace(/\*/g, "[^/]*")
    .replace(/\u0000/g, ".*");
  return new RegExp(`^${re}$`).test(name);
}

/** Which branch to compare with, from the repository's Conflict Prevention settings.
 *  null = turned off; undefined = the remote's default branch. */
export function conflictTarget(repo: RepoView, base: string, excluded: string, enabled: boolean): string | null | undefined {
  if (!enabled) return null;
  const lines = (t: string) => t.split("\n").map((l) => l.trim()).filter(Boolean);
  if (repo.head_branch && lines(excluded).some((p) => globMatch(p, repo.head_branch!))) return null;
  const rules = lines(base);
  if (!rules.length) return undefined;
  const names = repo.refs.filter((r) => r.kind !== "Tag").map((r) => r.name).filter((n) => n !== repo.head_branch);
  const exclude = rules.filter((r) => r.startsWith("!")).map((r) => r.slice(1));
  for (const inc of rules.filter((r) => !r.startsWith("!"))) {
    const hit = names.find((n) => globMatch(inc, n) && !exclude.some((x) => globMatch(x, n)));
    if (hit) return hit;
  }
  return undefined;
}

