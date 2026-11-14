import { describe, expect, it } from "vitest";
import { formatNoticeDate, integrationHint, unreadBadge } from "./notices";
import { noticeVisible, type Notice } from "./store";

describe("notification rules", () => {
  it("shows no badge for zero, the count below 50 and 50+ from 50", () => {
    expect(unreadBadge(0)).toBe("");
    expect(unreadBadge(7)).toBe("7");
    expect(unreadBadge(49)).toBe("49");
    expect(unreadBadge(50)).toBe("50+");
    expect(unreadBadge(120)).toBe("50+");
  });

  it("filters All / Read / Unread by readAt", () => {
    const unread: Notice = { id: 1, text: "a", at: 1, readAt: null };
    const read: Notice = { id: 2, text: "b", at: 2, readAt: 5 };
    expect([unread, read].filter((n) => noticeVisible(n, "All"))).toHaveLength(2);
    expect([unread, read].filter((n) => noticeVisible(n, "Unread"))).toEqual([unread]);
    expect([unread, read].filter((n) => noticeVisible(n, "Read"))).toEqual([read]);
  });

  it("dates rows as Today / Yesterday / month and ordinal, then @ time", () => {
    const now = new Date(2026, 10, 12, 15, 30);
    expect(formatNoticeDate(new Date(2026, 10, 12, 0, 0), now)).toMatch(/^Today @ 12:00am$|^Today @ 00:00$/);
    expect(formatNoticeDate(new Date(2026, 10, 11, 23, 59), now)).toMatch(/^Yesterday @ /);
    expect(formatNoticeDate(new Date(2026, 10, 1, 9, 5), now)).toMatch(/^November 1st @ /);
    expect(formatNoticeDate(new Date(2026, 9, 22, 9, 5), now)).toMatch(/^October 22nd @ /);
    expect(formatNoticeDate(new Date(2026, 9, 13, 9, 5), now)).toMatch(/^October 13th @ /);
    expect(formatNoticeDate(new Date(2026, 9, 3, 9, 5), now)).toMatch(/^October 3rd @ /);
  });

  it("picks the status-bar hint", () => {
    expect(integrationHint([], "github", [])).toBe("Connect an integration");
    expect(integrationHint(["gitlab"], "github", [])).toBe("Select an integration");
    expect(integrationHint(["gitlab"], null, [])).toBe("Select an integration");
    expect(integrationHint(["github"], "github", [])).toBe("No Pull Requests");
    expect(integrationHint(["github"], "github", null)).toBeNull();
    // Open non-draft requests are in the hidden "other" bucket; drafts show their sentence.
    expect(integrationHint(["github"], "github", [{ number: 4, repo: "o/r", draft: false }])).toBe("No Pull Requests");
    expect(integrationHint(["github"], "github", [{ number: 4, repo: "o/r", draft: false }, { number: 9, repo: "o/r", draft: true }])).toBe("o/r#9 is in draft");
  });
});
