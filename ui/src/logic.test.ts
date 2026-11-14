// Front-end logic tests (run with `npm test`): the merge tool's parser, the command palette's
// fuzzy matching, shortcut parsing, the rebase todo order and the relative time format.
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "./lib/api";
import { parseConflicts } from "./components/ConflictView";
import { comboOf, dispatchKey, fuzzy } from "./lib/commands";
import { todo } from "./lib/rewrite";
import { timeSince, withCoauthors } from "./lib/utils";
import { indexOnly, predictStatus, queryClient, refresh, refreshStatus, signInProblem } from "./lib/repo";
import { globMatch } from "./lib/conflict";
import { patchName } from "./lib/ai";
import { formatDate } from "./lib/integrations";
import { DEFAULT_METRICS, useStore, type RebasePlan } from "./lib/store";

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

describe("time format (timeSince)", () => {
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

describe("custom date formats and patch names", () => {
  const date = new Date(2026, 10, 11, 17, 5, 6);

  it("keeps bracketed literals and expands tokens outside them", () => {
    expect(formatDate(date, "[YYYY] YYYY-MM-DD [at] h:mm A [open [DD]")).toBe("YYYY 2026-11-11 at 5:05 PM open [DD");
  });

  it("still expands date tokens after an unmatched opening bracket", () => {
    const unclosed = "[".repeat(100_000);
    expect(formatDate(date, "[ok] " + unclosed + "YYYY")).toBe("ok " + unclosed + "2026");
  });

  it("trims punctuation from patch names and falls back for empty names", () => {
    expect(patchName("...Fix / the UI!!!")).toBe("fix-the-ui.patch");
    expect(patchName("?!")).toBe("changes.patch");
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
  it("matches the base branch patterns", () => {
    expect(globMatch("origin/main", "origin/main")).toBe(true);
    expect(globMatch("origin/**", "origin/feature/x")).toBe(true);
    expect(globMatch("origin/feature/**", "origin/bugfix/x")).toBe(false);
    expect(globMatch("origin/*", "origin/feature/x")).toBe(false);
  });
});

describe("repository cache refreshes", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    queryClient.clear();
  });

  it("refreshes open diffs, history and blame after a save or HEAD change", async () => {
    const kinds = ["repo", "diff", "versions", "history", "blame", "range", "conflict"];
    for (const kind of kinds) queryClient.setQueryData([kind, "/refresh-test"], { value: "old" });
    await refresh("/refresh-test");
    for (const kind of kinds) expect(queryClient.getQueryState([kind, "/refresh-test"])?.isInvalidated).toBe(true);
  });

  it("refreshes file contents even when their status still says modified", async () => {
    const status = { unstaged: [{ path: "a.txt", status: "M" }], staged: [], conflicted: false };
    vi.spyOn(api, "repoStatus").mockResolvedValue(status);
    queryClient.setQueryData(["versions", "/status-test", "a.txt"], { new: "old contents" });
    await refreshStatus("/status-test");
    expect(queryClient.getQueryState(["versions", "/status-test", "a.txt"])?.isInvalidated).toBe(true);
  });

  it("coalesces watcher bursts and reads again after an in-flight change", async () => {
    const status = { unstaged: [], staged: [], conflicted: false };
    let finish!: (value: typeof status) => void;
    const pending = new Promise<typeof status>((resolve) => { finish = resolve; });
    const read = vi.spyOn(api, "repoStatus").mockReturnValueOnce(pending).mockResolvedValue(status);
    const first = refreshStatus("/burst-test");
    const second = refreshStatus("/burst-test");
    const third = refreshStatus("/burst-test");
    expect(read).toHaveBeenCalledTimes(1);
    finish(status);
    await Promise.all([first, second, third]);
    expect(read).toHaveBeenCalledTimes(2);
  });
});

describe("shortcut ownership", () => {
  const key = (value: string, target: unknown = null) => ({
    key: value, code: "", ctrlKey: true, metaKey: false, altKey: false, shiftKey: false,
    target, preventDefault: vi.fn(),
  }) as unknown as KeyboardEvent;

  it("leaves editor shortcuts alone while typing", () => {
    const event = key("b");
    expect(dispatchKey(event, true)).toBe(false);
    expect(event.preventDefault).not.toHaveBeenCalled();
  });

  it("leaves terminal Ctrl+W with the shell instead of closing a repository", () => {
    const event = key("w", { closest: () => ({}) });
    expect(dispatchKey(event, true)).toBe(false);
    expect(event.preventDefault).not.toHaveBeenCalled();
  });

  it("blocks repository commands behind a modal", () => {
    const before = useStore.getState().dialog;
    useStore.setState({ dialog: { kind: "info", title: "Test", text: "" } });
    try {
      const event = key("b");
      expect(dispatchKey(event, false)).toBe(false);
      expect(event.preventDefault).not.toHaveBeenCalled();
    } finally {
      useStore.setState({ dialog: before });
    }
  });
});

describe("toolbar busy spinner", () => {
  it("spins Pull, Push, Stash and Pop for their own actions only", async () => {
    const { spinButtonFor } = await import("./lib/spin");
    expect(spinButtonFor({ type: "pull", mode: "ff" }, false)).toBe("pull");
    expect(spinButtonFor({ type: "fetch" }, false)).toBe("pull");
    expect(spinButtonFor({ type: "fetch" }, true)).toBe(null);
    expect(spinButtonFor({ type: "push", force: false }, false)).toBe("push");
    expect(spinButtonFor({ type: "stash", message: "" }, false)).toBe("stash");
    expect(spinButtonFor({ type: "stashPop", index: 0 }, false)).toBe("pop");
    expect(spinButtonFor({ type: "stageAll" }, false)).toBe(null);
  });

  it("keeps the circle for at least one turn without holding back the answer", async () => {
    vi.useFakeTimers();
    try {
      const { withSpin, isSpinning, SPIN_MIN_MS } = await import("./lib/spin");
      const result = await withSpin("C:/r", "stash", async () => {
        expect(isSpinning("C:/r", "stash")).toBe(true);
        return 7;
      });
      expect(result).toBe(7); // answered at once
      expect(isSpinning("C:/r", "stash")).toBe(true); // circle still showing
      expect(isSpinning("C:/r", "pull")).toBe(false);
      vi.advanceTimersByTime(SPIN_MIN_MS + 1);
      expect(isSpinning("C:/r", "stash")).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("web links to commits, branches and tags", () => {
  it("turns ssh and https remotes into web pages", async () => {
    const { webBase, webLink } = await import("./lib/weblinks");
    expect(webBase("git@github.com:me/app.git")).toBe("https://github.com/me/app");
    expect(webBase("https://user@github.com/me/app.git")).toBe("https://github.com/me/app");
    expect(webBase("ssh://git@gitlab.example.com:2222/group/sub/app.git")).toBe("https://gitlab.example.com/group/sub/app");
    expect(webBase("https://git.example.com:8443/team/app")).toBe("https://git.example.com:8443/team/app");
    expect(webBase("git@ssh.dev.azure.com:v3/org/proj/app")).toBe("https://dev.azure.com/org/proj/_git/app");
    expect(webBase("C:/repos/app")).toBe(null);
    expect(webLink("git@github.com:me/app.git", { kind: "commit", sha: "abc" })).toBe("https://github.com/me/app/commit/abc");
    expect(webLink("git@github.com:me/app.git", { kind: "branch", name: "feature/x y" })).toBe("https://github.com/me/app/tree/feature/x%20y");
    expect(webLink("git@gitlab.com:me/app.git", { kind: "tag", name: "v1" })).toBe("https://gitlab.com/me/app/-/tags/v1");
    expect(webLink("git@bitbucket.org:me/app.git", { kind: "commit", sha: "abc" })).toBe("https://bitbucket.org/me/app/commits/abc");
    expect(webLink("https://dev.azure.com/org/proj/_git/app", { kind: "branch", name: "main" })).toBe("https://dev.azure.com/org/proj/_git/app?version=GBmain");
  });
});

describe("tab order", () => {
  it("moves a tab before another or to the end", () => {
    const tabs = ["a", "b", "c", "d"].map((id) => ({ id, kind: "repo" as const, title: id, path: id }));
    useStore.setState({ tabs });
    const order = () => useStore.getState().tabs.map((t) => t.id).join("");
    useStore.getState().moveTab("d", "b");
    expect(order()).toBe("adbc");
    useStore.getState().moveTab("a", null);
    expect(order()).toBe("dbca");
    useStore.getState().moveTab("b", "b");
    expect(order()).toBe("dbca");
  });
});

describe("start-up tab restore", () => {
  it("brings every saved tab back at once with the saved one active", async () => {
    const { restoreTabs } = await import("./lib/store");
    useStore.setState({ tabs: [{ id: "s", kind: "start", title: "New Tab" }], activeId: "s" });
    restoreTabs(["C:/r/alpha", String.raw`C:\r\beta`, "C:/r/gamma"], 1);
    const st = useStore.getState();
    expect(st.tabs.map((t) => t.title)).toEqual(["alpha", "beta", "gamma"]);
    expect(st.tabs.find((t) => t.id === st.activeId)?.title).toBe("beta");
  });
});
