import { expect, it } from "vitest";
import { RevisionGuard } from "./revision";

it("keeps newer conflict edits when a delayed AI answer arrives", async () => {
  const guard = new RevisionGuard();
  guard.observe("repo/file:original");
  let finish!: (value: string) => void;
  let output = "original";
  const current = guard.capture();
  const pending = new Promise<string>((resolve) => { finish = resolve; }).then((answer) => {
    if (current()) output = answer;
  });
  guard.invalidate();
  output = "my newer edit";
  finish("old AI answer");
  await pending;
  expect(output).toBe("my newer edit");
});

it("rejects results for another file, changed inputs, and closed editors", () => {
  const guard = new RevisionGuard();
  guard.observe("file-a:version-1");
  const first = guard.capture();
  guard.observe("file-b:version-1");
  expect(first()).toBe(false);
  const second = guard.capture();
  guard.observe("file-b:version-2");
  expect(second()).toBe(false);
  const third = guard.capture();
  guard.observe("file-b:version-2");
  expect(third()).toBe(true);
  guard.invalidate();
  expect(third()).toBe(false);
});
