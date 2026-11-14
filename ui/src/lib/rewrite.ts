// Planning history edits: interactive rebase / cherry pick, and the one-click versions
// (reword, drop, squash, move) that the right-click menus offer.
import { api, type Commit, type RepoView, type TodoItem } from "./api";
import { act, confirmThen } from "./repo";
import { ai } from "./ai";
import { useStore, type RebaseItem, type RebasePlan } from "./store";

const toItem = (c: Commit): RebaseItem => ({
  action: "pick",
  sha: c.sha,
  author: c.author,
  email: c.email,
  summary: c.summary,
  message: c.body ? `${c.summary}\n\n${c.body}` : c.summary,
});

/** Commits on the checked-out branch's first-parent line, newest first, up to the first merge. */
export function headLine(repo: RepoView): string[] {
  const out: string[] = [];
  const bySha = new Map(repo.commits.map((c) => [c.sha, c]));
  let c = bySha.get(repo.head_sha);
  while (c) {
    out.push(c.sha);
    if (c.parents.length !== 1) break; // a merge (or the root) ends the line we can rewrite
    c = bySha.get(c.parents[0]);
  }
  return out;
}

/** What a rebase that rewrites `sha` and everything after it replays onto. */
export function baseFor(repo: RepoView, sha: string): string {
  const c = repo.commits.find((x) => x.sha === sha);
  return c?.parents[0] ?? "--root";
}

/** Open the Interactive Rebase panel for the commits after `base` (newest first). */
export async function openRebase(tabId: string, path: string, base: string, onto: string, preset?: (items: RebaseItem[]) => RebaseItem[]) {
  const st = useStore.getState();
  try {
    const { commits, expected } = await api.rebasePlan(path, base);
    if (!commits.length) return st.toast(false, "There are no commits to rebase.");
    let items = commits.map(toItem).reverse();
    const original = items.map((i) => ({ ...i }));
    if (preset) items = preset(items);
    const plan: RebasePlan = { mode: "rebase", base, onto, items, original, expected };
    st.setUi(tabId, { rebase: plan, diff: null, history: null, conflict: null });
  } catch (e) {
    st.toast(false, String(e));
  }
}

/** Open the panel in Interactive Cherry Pick mode for the picked commits. */
export function openCherryPick(tabId: string, repo: RepoView, shas: string[]) {
  const st = useStore.getState();
  const order = new Map(repo.commits.map((c, i) => [c.sha, i]));
  const commits = repo.commits.filter((c) => shas.includes(c.sha)).sort((a, b) => order.get(a.sha)! - order.get(b.sha)!);
  const items = commits.map(toItem);
  st.setUi(tabId, {
    rebase: { mode: "cherry", base: "", onto: repo.head_branch ?? repo.head_sha.slice(0, st.metrics.sha_len), items, original: items.map((i) => ({ ...i })) },
    diff: null,
    history: null,
    conflict: null,
  });
}

/** Turn the shown plan (newest first) into git's order (oldest first). */
export function todo(plan: RebasePlan): TodoItem[] {
  return [...plan.items].reverse().map((i) => ({ action: i.action, sha: i.sha, message: i.action === "reword" ? i.message : null }));
}

export async function runPlan(tabId: string, path: string, plan: RebasePlan) {
  const st = useStore.getState();
  const items = todo(plan);
  const ok =
    plan.mode === "rebase"
      ? await act(path, { type: "interactiveRebase", base: plan.base, items, expected: plan.expected! })
      : await act(path, { type: "interactiveCherryPick", items: items.filter((i) => i.action !== "drop") });
  st.setUi(tabId, { rebase: null, multi: [] });
  return ok;
}

/** One-click edits from the menus: run a rebase straight away with a prepared plan. */
async function quick(path: string, base: string, edit: (items: RebaseItem[]) => RebaseItem[] | null) {
  const st = useStore.getState();
  try {
    const { commits, expected } = await api.rebasePlan(path, base);
    const items = edit(commits.map(toItem).reverse());
    if (!items) return;
    const plan: RebasePlan = { mode: "rebase", base, onto: base, items, original: commits.map(toItem).reverse(), expected };
    await act(path, { type: "interactiveRebase", base, items: todo(plan), expected });
  } catch (e) {
    st.toast(false, String(e));
  }
}

function oldest(repo: RepoView, shas: string[]) {
  const line = headLine(repo);
  return [...shas].sort((a, b) => line.indexOf(b) - line.indexOf(a))[0];
}

export function rewordCommit(repo: RepoView, sha: string) {
  const c = repo.commits.find((x) => x.sha === sha)!;
  const line = headLine(repo);
  const n = line.indexOf(sha) + 1;
  useStore.getState().setDialog({
    kind: "message",
    title: "Rename commit",
    label: "Commit message (first line is the summary)",
    value: c.body ? `${c.summary}\n\n${c.body}` : c.summary,
    confirm: "Rename commit",
    help: `Renaming changes this commit's ID${n > 1 ? ` and the IDs of ${n - 1} newer commit${n === 2 ? "" : "s"}` : ""}. Review the message before saving. You can undo this with Undo.`,
    generate: async () => {
      const [summary, description] = await ai.commitMessage(repo.path, sha);
      return description ? `${summary}\n\n${description}` : summary;
    },
    onSubmit: (msg) => quick(repo.path, baseFor(repo, sha), (items) => items.map((i) => (i.sha === sha ? { ...i, action: "reword", message: msg } : i))),
  });
}

export function dropCommits(repo: RepoView, shas: string[]) {
  const n = shas.length;
  confirmThen(n === 1 ? "Drop commit" : `Drop ${n} commits`, n === 1 ? "Are you sure you want to drop 1 commit?" : `Are you sure you want to drop ${n} commits?`, n === 1 ? "Drop commit" : `Drop ${n} commits`, () =>
    quick(repo.path, baseFor(repo, oldest(repo, shas)), (items) => items.map((i) => (shas.includes(i.sha) ? { ...i, action: "drop" } : i))),
  );
}

/** Squash the picked commits into the commit just below the oldest of them. */
export function squashCommits(repo: RepoView, shas: string[]) {
  const line = headLine(repo);
  const old = oldest(repo, shas);
  const target = line[line.indexOf(old) + 1];
  if (!target) return useStore.getState().toast(false, "There is no older commit to squash into.");
  void quick(repo.path, baseFor(repo, target), (items) => items.map((i) => (shas.includes(i.sha) ? { ...i, action: "squash" } : i)));
}

/** Move a commit one place newer (up) or older (down). */
export function moveCommit(repo: RepoView, sha: string, dir: "up" | "down") {
  const line = headLine(repo);
  const i = line.indexOf(sha);
  const other = dir === "up" ? line[i - 1] : line[i + 1];
  if (!other) return;
  const lower = dir === "up" ? sha : other;
  void quick(repo.path, baseFor(repo, lower), (items) => {
    const a = items.findIndex((x) => x.sha === sha);
    const b = items.findIndex((x) => x.sha === other);
    const out = [...items];
    [out[a], out[b]] = [out[b], out[a]];
    return out;
  });
}
