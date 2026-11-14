import { describe, expect, it } from "vitest";
import { assignTimeline, buildTimeline, showsTimeline } from "./timeline";

const now = new Date(2026, 10, 12, 15, 0, 0);
const hoursAgo = (h: number) => now.getTime() - h * 36e5 - 1000;

describe("graph timeline", () => {
  it("builds 23 hours, 6 days, 4 weeks, 11 months and 6 years of marks in that order", () => {
    const marks = buildTimeline(now);
    expect(marks).toHaveLength(23 + 6 + 4 + 11 + 6);
    expect(marks[0].text).toBe("1 hour ago");
    expect(marks[1].text).toBe("2 hours ago");
    expect(marks[23].text).toBe("yesterday");
    expect(marks[24].text).toBe("2 days ago");
    expect(marks.find((m) => m.unit === "week" && m.value === 1)?.text).toBe("a week ago");
    expect(marks.find((m) => m.unit === "month" && m.value === 1)?.text).toBe("a month ago");
    expect(marks.find((m) => m.unit === "year" && m.value === 1)?.text).toBe("a year ago");
    expect(marks[marks.length - 1].text).toBe("6+ years ago");
    expect(marks.find((m) => m.unit === "month" && m.value === 1)?.date).toBe(new Date(2026, 9, 12).getTime());
  });

  it("puts each mark on the first row older than it, a later mark replacing an earlier one", () => {
    // rows newest first: 30 min ago, 2.5 h ago, 20.5 h ago
    const times = [now.getTime() - 18e5, hoursAgo(2.5) + 1000, hoursAgo(20.5) + 1000];
    const out = assignTimeline(times, now);
    expect(out.get(0)).toBeUndefined();
    expect(out.get(1)).toBe("2 hours ago"); // marks 1h and 2h both fall before row 1; 2h wins
    expect(out.get(2)).toBe("20 hours ago");
  });

  it("shows only while the date column is hidden or the list is filtered, and never on the first row", () => {
    expect(showsTimeline(0, false, false)).toBe(false);
    expect(showsTimeline(3, false, false)).toBe(true);
    expect(showsTimeline(3, true, false)).toBe(false);
    expect(showsTimeline(3, true, true)).toBe(true);
  });
});
