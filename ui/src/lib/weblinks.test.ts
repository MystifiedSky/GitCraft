import { expect, it } from "vitest";
import { webBase } from "./weblinks";
it("normalizes long trailing slash runs without changing remote paths", () => {
  expect(webBase("git@example.com:owner/repo" + "/".repeat(100000))).toBe("https://example.com/owner/repo");
  expect(webBase("https://example.com/owner/repo.git/")).toBe("https://example.com/owner/repo");
});
