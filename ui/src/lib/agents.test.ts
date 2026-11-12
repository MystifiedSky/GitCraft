import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS } from "./store";
import { agentLaunch, sameFolder } from "./agents";
import { workingChanges } from "./working-changes";
import type { FileChange } from "./api";

describe("working directory counts", () => {
  it("counts a partly staged file once and preserves a newly added file's status", () => {
    const f = (path: string, status: string) => ({ path, status }) as FileChange;
    expect(workingChanges({ staged: [f("new.ts", "A"), f("edit.ts", "M")], unstaged: [f("new.ts", "M"), f("edit.ts", "M"), f("gone.ts", "D"), f("untracked.ts", "?")], conflicted: false }))
      .toEqual({ total: 4, added: 2, modified: 1, deleted: 1 });
  });
});

describe("agent launch and matching", () => {
  it("uses each session's own resume command", () => {
    expect(agentLaunch(DEFAULT_SETTINGS, "codex", "codex-id").command).toBe("codex resume codex-id");
    expect(agentLaunch(DEFAULT_SETTINGS, "claude", "claude-id").command).toBe("claude --resume claude-id");
    expect(() => agentLaunch(DEFAULT_SETTINGS, "codex", "id;bad")).toThrow();
  });

  it("quotes configured executables and passes config paths outside the shell command", () => {
    const settings = { ...DEFAULT_SETTINGS, termShell: "powershell", ai: { ...DEFAULT_SETTINGS.ai, provider: "codex" as const, cliPath: "C:\\Program Files\\O'Brien\\codex.exe", cliConfigDir: "C:\\Agent Config" } };
    expect(agentLaunch(settings, "codex", "id")).toEqual({ command: "& 'C:\\Program Files\\O''Brien\\codex.exe' resume id", env: { CODEX_HOME: "C:\\Agent Config" } });
  });

  it("normalizes Windows separators, extended paths and case, preserving POSIX case", () => {
    expect(sameFolder("C:\\Projects\\App", "c:/projects/app/")).toBe(true);
    expect(sameFolder("\\\\?\\C:\\Projects\\App", "C:/Projects/App")).toBe(true);
    expect(sameFolder("/Projects/App", "/projects/app")).toBe(false);
    expect(sameFolder("C:/repo", "C:/repo-other")).toBe(false);
  });
});
