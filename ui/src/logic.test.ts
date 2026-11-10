// Front-end logic tests (run with `npm test`): the merge tool's parser, the command palette's
// fuzzy matching, shortcut parsing, the rebase todo order and the original's time format.
import { describe, expect, it } from "vitest";
import { parseConflicts } from "./components/ConflictView";
import { comboOf, fuzzy } from "./lib/commands";
import { todo } from "./lib/rewrite";
import { timeSince, withCoauthors } from "./lib/utils";
import { indexOnly, predictStatus, signInProblem } from "./lib/repo";
import { globMatch } from "./lib/conflict";
import { DEFAULT_METRICS, type RebasePlan } from "./lib/store";

describe("merge tool", () => {
  it("splits a file into shared text and conflict chunks", () => {
    const text = "a\n<<<<<<< HEAD\nours 1\nours 2\n=======\ntheirs\n>>>>>>> feature\nb\n";
    const segs = parseConflicts(text);
    expect(segs).toHaveLength(3);
    expect(segs[0]).toEqual({ kind: "common", lines: ["a"] });
    expect(segs[1]).toMatchObject({ kind: "conflict", ours: ["ours 1", "ours 2"], theirs: ["theirs"], theirsMark: ">>>>>>> feature" });
    expect(segs[2]).toEqual({ kind: "common", lines: ["b"] });
  });

  it("understands diff3 style (with the base section) and CRLF files", () => {
    const segs = parseConflicts("<<<<<<< ours\r\nx\r\n||||||| base\r\nb\r\n=======\r\ny\r\n>>>>>>> theirs\r\n");
    expect(segs[0]).toMatchObject({ kind: "conflict", ours: ["x"], base: ["b"], theirs: ["y"] });
  });
});

describe("command palette", () => {
  it("ranks direct matches first and accepts letters in order", () => {
    expect(fuzzy("fetch", "Fetch All")).toBeGreaterThan(fuzzy("fa", "Fetch All"));
    expect(fuzzy("stg all", "Stage all changes")).toBeGreaterThan(0);
    expect(fuzzy("zzz", "Fetch All")).toBe(0);
  });

  it("turns key presses into shortcut names", () => {
    const ev = (o: Partial<KeyboardEvent>) => ({ ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, code: "", ...o }) as KeyboardEvent;
    expect(comboOf(ev({ key: "s", ctrlKey: true, shiftKey: true }))).toBe("Ctrl+Shift+S");
    expect(comboOf(ev({ key: "!", code: "Digit1", ctrlKey: true, shiftKey: true }))).toBe("Ctrl+Shift+1");
    expect(comboOf(ev({ key: "F11" }))).toBe("F11");
    expect(comboOf(ev({ key: "Control", ctrlKey: true }))).toBeNull();
  });
});

describe("interactive rebase", () => {
  it("sends git the oldest commit first, with messages only for rewords", () => {
    const item = (sha: string, action: "pick" | "reword" | "squash" | "drop") => ({ action, sha, author: "", email: "", summary: sha, message: `${sha} message` });
    const plan: RebasePlan = { mode: "rebase", base: "b", onto: "b", items: [item("new", "drop"), item("mid", "reword"), item("old", "pick")], original: [] };
    expect(todo(plan)).toEqual([
      { action: "pick", sha: "old", message: null },
      { action: "reword", sha: "mid", message: "mid message" },
      { action: "drop", sha: "new", message: null },
    ]);
  });
});

describe("time format (the original's timeSince)", () => {
  const units = DEFAULT_METRICS.time_units;
  const now = 2_000_000_000;
  it("uses the first unit that fits, coloured by age", () => {
    expect(timeSince(now - 90, units, now).text).toBe("1m");
    expect(timeSince(now - 3 * 86400, units, now)).toMatchObject({ text: "3d", color: "#de9b43" });
    expect(timeSince(now - 2 * 604800, units, now).text).toBe("2w");
    expect(timeSince(now - 31556736, units, now).text).toBe("1y");
    expect(timeSince(now - 5, units, now).text).toBe("5s");
  });
});

describe("co-authors", () => {
  it("adds one Co-authored-by line per valid person after a blank line", () => {
    expect(withCoauthors("Body", "Ann <ann@x.org>, bad entry, Bo <bo@y.io>")).toBe("Body\n\nCo-authored-by: Ann <ann@x.org>\nCo-authored-by: Bo <bo@y.io>");
    expect(withCoauthors("", "Ann <ann@x.org>")).toBe("Co-authored-by: Ann <ann@x.org>");
    expect(withCoauthors("Body", "")).toBe("Body");
  });
});

describe("staging feedback", () => {
  const s = { unstaged: [{ path: "a.txt", status: "M" }, { path: "new.txt", status: "?" }, { path: "c.txt", status: "U" }], staged: [{ path: "b.txt", status: "A" }], conflicted: true };
  it("moves files to Staged straight away (new files become added; conflicts stay put)", () => {
    const r = predictStatus(s, { type: "stageAll" })!;
    expect(r.unstaged.map((f) => f.path)).toEqual(["c.txt"]);
    expect(r.staged).toEqual([{ path: "a.txt", status: "M" }, { path: "b.txt", status: "A" }, { path: "new.txt", status: "A" }]);
  });
  it("moves one file back to Unstaged, and only staging actions count as index-only", () => {
    const r = predictStatus(s, { type: "unstage", paths: ["b.txt"] })!;
    expect(r.staged).toEqual([]);
    expect(r.unstaged.find((f) => f.path === "b.txt")?.status).toBe("?");
    expect(indexOnly({ type: "stageAll" })).toBe(true);
    expect(indexOnly({ type: "commit", summary: "x", description: "", amend: false, stageAll: false, skipHooks: false })).toBe(false);
  });
});

describe("sign-in problems", () => {
  it("tells SSH key refusals from HTTPS sign-in failures", () => {
    expect(signInProblem("git@github.com: Permission denied (publickey).\nfatal: Could not read from remote repository.")).toBe("ssh");
    expect(signInProblem("fatal: could not read Username for 'https://github.com': terminal prompts disabled")).toBe("https");
    expect(signInProblem("remote: Permission to a/b.git denied to someone.\nfatal: unable to access ...: The requested URL returned error: 403")).toBe("https");
    expect(signInProblem("! [rejected] main -> main (fetch first)")).toBe(null);
  });
});

describe("conflict prevention rules", () => {
  it("matches the original's base branch patterns", () => {
    expect(globMatch("origin/main", "origin/main")).toBe(true);
    expect(globMatch("origin/**", "origin/feature/x")).toBe(true);
    expect(globMatch("origin/feature/**", "origin/bugfix/x")).toBe(false);
    expect(globMatch("origin/*", "origin/feature/x")).toBe(false);
  });
});
