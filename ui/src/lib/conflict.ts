import type { RepoView } from "./api";

/** Include full local and remote target identities in conflict cache invalidation. */
export const conflictRefsKey = (repo: Pick<RepoView, "head_sha" | "refs">) => JSON.stringify([repo.head_sha, repo.refs.map((r) => [r.full, r.sha]).sort((a, b) => a[0].localeCompare(b[0]))]);

/** "origin/**", "origin/feature/**", "!origin/abc" (Conflict Prevention > Base branches). */
export function globMatch(pattern: string, name: string) {
  const re = pattern
    .trim()
    .replace(/[.+?^${}()|[\]\\]/g, "\\$&")
    .replace(/\*\*?/g, (wildcard) => (wildcard === "**" ? ".*" : "[^/]*"));
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


export type Segment =
  | { kind: "common"; lines: string[] }
  | { kind: "conflict"; ours: string[]; base: string[]; theirs: string[]; oursMark: string; theirsMark: string; raw: string[] };

export interface ConflictPick {
  a: boolean[];
  b: boolean[];
  touched: boolean;
}

/** Recognize complete Git conflict blocks; incomplete or ordinary marker-like text stays intact. */
export function parseConflicts(text: string): Segment[] {
  const lines = text.replace(/\r\n/g, "\n").split("\n");
  if (lines[lines.length - 1] === "") lines.pop();
  const out: Segment[] = [];
  let common: string[] = [];
  let i = 0;
  while (i < lines.length) {
    const opening = /^(<{7,})(?: .*)?$/.exec(lines[i]);
    if (!opening) {
      common.push(lines[i++]);
      continue;
    }
    const size = opening[1].length;
    const marker = (line: string, char: string) => line === char.repeat(size) || line.startsWith(char.repeat(size) + " ");
    const ours: string[] = [];
    const base: string[] = [];
    const theirs: string[] = [];
    let part = ours;
    let separator = false;
    let baseSeen = false;
    let end = i + 1;
    for (; end < lines.length; end++) {
      const line = lines[end];
      if (marker(line, "<")) break; // malformed/nested block: never discard its contents
      if (marker(line, ">")) break;
      if (!separator && !baseSeen && marker(line, "|")) {
        baseSeen = true;
        part = base;
      } else if (!separator && line === "=".repeat(size)) {
        separator = true;
        part = theirs;
      } else part.push(line);
    }
    if (end >= lines.length || !separator || !marker(lines[end], ">")) {
      common.push(lines[i++]);
      continue;
    }
    if (common.length) out.push({ kind: "common", lines: common });
    common = [];
    out.push({ kind: "conflict", ours, base, theirs, oursMark: lines[i], theirsMark: lines[end], raw: lines.slice(i, end + 1) });
    i = end + 1;
  }
  if (common.length) out.push({ kind: "common", lines: common });
  return out;
}

/** Untouched chunks retain their exact marker labels and diff3 base; don't invent a final newline. */
export function conflictOutput(segments: Segment[], picks: Record<number, ConflictPick>, working: string): string {
  const out: string[] = [];
  segments.forEach((seg, i) => {
    if (seg.kind === "common") {
      out.push(...seg.lines);
    } else {
      const p = picks[i];
      if (!p?.touched) out.push(...seg.raw);
      else {
        out.push(...seg.ours.filter((_, k) => p.a[k]), ...seg.theirs.filter((_, k) => p.b[k]));
      }
    }
  });
  return out.join("\n") + (out.length && working.endsWith("\n") ? "\n" : "");
}

/** Even an incomplete marker block must not be automatically staged as resolved. */
export function hasConflictMarkers(text: string): boolean {
  return /^(?:<{7,}(?: .*)?|\|{7,}(?: .*)?|={7,}|>{7,}(?: .*)?)\r?$/m.test(text);
}
