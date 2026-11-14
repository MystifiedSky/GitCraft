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
