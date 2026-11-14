import { describe, expect, it } from "vitest";
import { nextTopological, previousTopological } from "./graphnav";

// d (HEAD) -> c (merge of b and x) ; c -> b, x ; b -> a ; x -> a ; a root
const commits = [
  { sha: "d", parents: ["c"] },
  { sha: "c", parents: ["b", "x"] },
  { sha: "b", parents: ["a"] },
  { sha: "x", parents: ["a"] },
  { sha: "a", parents: [] },
];

describe("topological selection", () => {
  it("goes from the WIP row to HEAD and then down the first parents", () => {
    expect(nextTopological(commits, "d", true, { type: "wip" })).toEqual({ type: "commit", sha: "d" });
    expect(nextTopological(commits, "d", true, { type: "commit", sha: "c" })).toEqual({ type: "commit", sha: "b" });
    expect(nextTopological(commits, "d", true, { type: "commit", sha: "a" })).toEqual({ type: "commit", sha: "a" });
  });
  it("goes up to the only child, stays put with several, and reaches WIP from a childless HEAD", () => {
    expect(previousTopological(commits, "d", true, { type: "commit", sha: "b" })).toEqual({ type: "commit", sha: "c" });
    expect(previousTopological(commits, "d", true, { type: "commit", sha: "a" })).toBeNull();
    expect(previousTopological(commits, "d", true, { type: "commit", sha: "d" })).toEqual({ type: "wip" });
    expect(previousTopological(commits, "d", false, { type: "commit", sha: "d" })).toBeNull();
  });
  it("chooses the default commit when nothing is selected", () => {
    expect(nextTopological(commits, "d", true, null)).toEqual({ type: "wip" });
    expect(nextTopological(commits, "d", false, null)).toEqual({ type: "commit", sha: "d" });
  });
});
