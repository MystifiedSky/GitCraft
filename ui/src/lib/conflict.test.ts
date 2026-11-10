import { describe, expect, it } from "vitest";
import { conflictOutput, hasConflictMarkers, parseConflicts } from "./conflict";

describe("conflict output safety", () => {
  const block = "<<<<<<< ours\ncurrent\n||||||| base\nancestor\n=======\nincoming\n>>>>>>> theirs";

  it("retains the base section, labels and final-newline state of untouched diff3 chunks", () => {
    for (const suffix of ["", "\n"]) {
      const text = "before\n" + block + "\nafter" + suffix;
      expect(conflictOutput(parseConflicts(text), {}, text)).toBe(text);
    }
  });

  it("preserves untouched chunks when another chunk is resolved", () => {
    const text = block + "\nbetween\n" + block + "\n";
    const segments = parseConflicts(text);
    const output = conflictOutput(segments, { 0: { a: [true], b: [false], touched: true } }, text);
    expect(output).toBe("current\nbetween\n" + block + "\n");
    expect(hasConflictMarkers(output)).toBe(true);
  });

  it("recognizes custom marker sizes without consuming marker-like source text", () => {
    const text = "<<<<<<<<< ours\n<<<<<<<< source text\n=========\nincoming\n>>>>>>>>> theirs";
    const segments = parseConflicts(text);
    expect(segments[0]).toMatchObject({ kind: "conflict", ours: ["<<<<<<<< source text"], theirs: ["incoming"] });
    expect(conflictOutput(segments, {}, text)).toBe(text);
    expect(parseConflicts("<<<<<<<ordinary\n=======ordinary\n>>>>>>>ordinary")).toEqual([
      { kind: "common", lines: ["<<<<<<<ordinary", "=======ordinary", ">>>>>>>ordinary"] },
    ]);
  });

  it("preserves incomplete blocks instead of inventing a closing marker", () => {
    for (const text of ["before\n<<<<<<< ours\ncontent\n", "<<<<<<< ours\none\n=======\ntwo", "<<<<<<< ours\none\n>>>>>>> theirs"]) {
      expect(conflictOutput(parseConflicts(text), {}, text)).toBe(text);
      expect(hasConflictMarkers(text)).toBe(true);
    }
  });

  it("keeps manually edited or AI output unresolved while any marker remains", () => {
    expect(hasConflictMarkers(block + "\nmanual addition")).toBe(true);
    expect(hasConflictMarkers("resolved\n=======\n")).toBe(true);
    expect(hasConflictMarkers("<<<<<<< ours\r\nedited\r\n")).toBe(true);
    expect(hasConflictMarkers("resolved\n")).toBe(false);
    expect(hasConflictMarkers("const comparison = a << b;")).toBe(false);
  });

  it("normalizes CRLF for the editor and permits a truly empty resolved result", () => {
    const text = block.replace(/\n/g, "\r\n") + "\r\n";
    expect(conflictOutput(parseConflicts(text), {}, text)).toBe(block + "\n");
    expect(conflictOutput(parseConflicts(block), { 0: { a: [false], b: [false], touched: true } }, block)).toBe("");
  });
});
