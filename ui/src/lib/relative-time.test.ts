import { describe, expect, it } from "vitest";
import { fromNow } from "./relative-time";

const now = 1_800_000_000_000;
const ago = (ms: number) => fromNow(now - ms, now);
const S = 1000;
const M = 60 * S;
const H = 60 * M;
const D = 24 * H;

describe("relative time wording", () => {
  it("uses the usual thresholds from seconds to hours", () => {
    expect(ago(0)).toBe("a few seconds ago");
    expect(ago(44 * S)).toBe("a few seconds ago");
    expect(ago(45 * S)).toBe("a minute ago");
    expect(ago(89 * S)).toBe("a minute ago");
    expect(ago(90 * S)).toBe("2 minutes ago");
    expect(ago(44 * M)).toBe("44 minutes ago");
    expect(ago(45 * M)).toBe("an hour ago");
    expect(ago(89 * M)).toBe("an hour ago");
    expect(ago(90 * M)).toBe("2 hours ago");
    expect(ago(21 * H)).toBe("21 hours ago");
  });

  it("continues through days, months and years", () => {
    expect(ago(22 * H)).toBe("a day ago");
    expect(ago(35 * H)).toBe("a day ago");
    expect(ago(36 * H)).toBe("2 days ago");
    expect(ago(25 * D)).toBe("25 days ago");
    expect(ago(26 * D)).toBe("a month ago");
    expect(ago(45 * D)).toBe("a month ago");
    expect(ago(46 * D)).toBe("2 months ago");
    expect(ago(319 * D)).toBe("10 months ago");
    expect(ago(320 * D)).toBe("a year ago");
    expect(ago(548 * D)).toBe("2 years ago");
  });

  it("words the future as 'in …'", () => {
    expect(fromNow(now + 5 * M + 1, now)).toBe("in 5 minutes");
  });
});
