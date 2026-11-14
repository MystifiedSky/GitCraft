import { describe, expect, it } from "vitest";
import { markerRects, tickSize } from "./scrollmarkers";

describe("scroll markers", () => {
  it("clamps block ticks to 5-20 px and centres short ones in their row slice", () => {
    // 1000 rows in 400 px: slice 0.4 px, natural height 0.4 -> clamped to 5, taller than the slice so no offset
    expect(tickSize("localBranches", 1000, 400)).toEqual({ height: 5, offset: 0 });
    // 10 rows in 400 px: slice 40, natural height 40 -> clamped to 20, centred (offset 10)
    expect(tickSize("localBranches", 10, 400)).toEqual({ height: 20, offset: 10 });
    expect(tickSize("selection", 10, 400)).toEqual({ height: 4, offset: 18 });
  });

  it("puts tags in the right lane and the checked-out commit across the left two", () => {
    const rects = markerRects(["tags", "head"], { tags: [5], head: [0] }, [], 10, 400, 12);
    const tag = rects.find((r) => r.type === "tags")!;
    const head = rects.find((r) => r.type === "head")!;
    expect([tag.left, tag.width]).toEqual([8, 4]);
    expect([head.left, head.width]).toEqual([0, 8]);
    expect(tag.top).toBeCloseTo((5 * 400) / 10 + 10 - 10);
  });

  it("shows only upstream rows for remote branches when just local branches are enabled", () => {
    const rects = markerRects(["localBranches"], { localBranches: [1, 2], remoteBranches: [3, 4] }, [2], 10, 400, 12);
    expect(rects.filter((r) => r.type === "remoteBranches")).toHaveLength(1);
    expect(rects.filter((r) => r.type === "localBranches")).toHaveLength(2);
  });

  it("draws nothing for an empty list", () => {
    expect(markerRects(["tags"], { tags: [] }, [], 0, 400, 12)).toEqual([]);
  });
});
