// Keyboard navigation rules: selecting the next / previous commit along the history (first parent / only child).
export type NavSelection = { type: "wip" } | { type: "commit"; sha: string };

interface NavCommit {
  sha: string;
  parents: string[];
}

/**
 * Shift+Down ("select next topological"): from the WIP row go to the checked-out commit; otherwise follow the FIRST PARENT of the
 * last selected commit (a root commit stays selected). With nothing selected, the default commit is chosen (WIP if present, else HEAD).
 */
export function nextTopological(commits: NavCommit[], headSha: string, hasWip: boolean, current: NavSelection | null): NavSelection | null {
  if (!current) return defaultSelection(commits, headSha, hasWip);
  if (current.type === "wip") return headSha ? { type: "commit", sha: headSha } : null;
  const c = commits.find((x) => x.sha === current.sha);
  const parent = c?.parents[0];
  return { type: "commit", sha: parent ?? current.sha };
}

/**
 * Shift+Up ("select previous topological"): the only CHILD of the selected commit; with none or several children nothing changes,
 * except that the checked-out commit with no children goes to the WIP row when there is one.
 */
export function previousTopological(commits: NavCommit[], headSha: string, hasWip: boolean, current: NavSelection | null): NavSelection | null {
  if (!current || current.type === "wip") return current ? null : defaultSelection(commits, headSha, hasWip);
  const children = commits.filter((x) => x.parents.includes(current.sha));
  if (current.sha === headSha && hasWip) return children.length === 0 ? { type: "wip" } : null;
  return children.length === 1 ? { type: "commit", sha: children[0].sha } : null;
}

export function defaultSelection(commits: NavCommit[], headSha: string, hasWip: boolean): NavSelection | null {
  if (hasWip) return { type: "wip" };
  if (headSha) return { type: "commit", sha: headSha };
  return commits.length ? { type: "commit", sha: commits[0].sha } : null;
}

/**
 * Rows marked while a label is hovered. A tag marks only its own commit; a branch marks its commit and the unlabelled commits
 * below it along the first parent, up to (not including) the next commit that has a label of its own.
 */
export function hoverSegment(commits: NavCommit[], labelled: Set<string>, index: number, isTag: boolean, positions?: ReadonlyMap<string, number>): Set<number> {
  const out = new Set<number>();
  if (index < 0 || index >= commits.length) return out;
  out.add(index);
  if (isTag) return out;
  const position = positions ?? new Map(commits.map((c, i) => [c.sha, i]));
  let i = index;
  for (;;) {
    const parent = commits[i].parents[0];
    const next = parent === undefined ? undefined : position.get(parent);
    if (next === undefined || next <= i || labelled.has(commits[next].sha)) break;
    out.add(next);
    i = next;
  }
  return out;
}
