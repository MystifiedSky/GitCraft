import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { ai, generateCommitMessage } from "./ai";
import { aiApi } from "./integrations";
import { DEFAULT_AI, useStore } from "./store";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("./repo", () => ({ act: vi.fn() }));

const original = useStore.getState().settings;
afterEach(() => {
  useStore.setState({ settings: original });
  vi.mocked(invoke).mockReset();
});

describe("all AI features share the selected connection", () => {
  const requests: [string, () => Promise<unknown>][] = [
    ["ai_commit_message", () => ai.commitMessage("/repo")],
    ["ai_stash_message", () => ai.stashMessage("/repo")],
    ["ai_compose", () => ai.compose("/repo")],
    ["ai_explain_commit", () => ai.explainCommit("/repo", "abc")],
    ["ai_explain_branch", () => ai.explainBranch("/repo", "feature")],
    ["ai_pull_request", () => aiApi.pullRequest("/repo", "feature", "main")],
    ["ai_resolve_conflict", () => aiApi.resolveConflict("base", "ours", "theirs", "file")],
    ["ai_test_connection", () => aiApi.test()],
  ];
  it.each(requests)("%s carries the selected connection with the operation", async (command, run) => {
    const config = { ...DEFAULT_AI, provider: "custom" as const, endpoint: "http://localhost:1234/v1", defaultModel: "local-model", responseFormat: "text" as const };
    useStore.getState().setSettings({ ai: config });
    vi.mocked(invoke).mockResolvedValueOnce("result");
    await expect(run()).resolves.toBe("result");
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith(command, expect.objectContaining({ config }));
  });

  it("does not send a request if applying connection settings fails", async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error("settings unavailable"));
    await expect(aiApi.test()).rejects.toThrow("settings unavailable");
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it.each(["codex", "claude_code"] as const)("sends %s program and account folder to the same commit helper", async (provider) => {
    const cliPath = "/example/bin/agent";
    const cliConfigDir = "/example/account";
    useStore.getState().setSettings({ ai: { ...DEFAULT_AI, provider, cliPath, cliConfigDir } });
    vi.mocked(invoke).mockResolvedValueOnce(["Summarize changes", ""]);
    await expect(ai.commitMessage("/repo")).resolves.toEqual(["Summarize changes", ""]);
    expect(invoke).toHaveBeenCalledWith("ai_commit_message", { path: "/repo", sha: null, config: expect.objectContaining({ provider, cliPath, cliConfigDir }) });
  });
});

it("discovers the program and models without changing the active connection or requesting a draft", async () => {
  const config = { ...DEFAULT_AI, provider: "claude_code" as const, cliConfigDir: "/personal/claude" };
  const model = { id: "sonnet", name: "Sonnet" };
  vi.mocked(invoke).mockResolvedValueOnce({ path: "/bin/claude", version: "Claude Code" }).mockResolvedValueOnce([model]);
  await expect(aiApi.findProgram(config.provider)).resolves.toMatchObject({ path: "/bin/claude" });
  await expect(aiApi.listModels(config)).resolves.toEqual([model]);
  expect(vi.mocked(invoke).mock.calls).toEqual([
    ["ai_find_program", { provider: "claude_code" }],
    ["ai_list_models", { config }],
  ]);
});

it("preserves a commit message edited while AI generation was running", async () => {
  const path = "/repo/ai-race";
  let finish!: (message: [string, string]) => void;
  const request = vi.spyOn(ai, "commitMessage").mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  try {
    const generating = generateCommitMessage(path);
    useStore.getState().setDraft(path, { summary: "Keep my newer draft", description: "Typed while waiting" });
    finish(["Generated suggestion", "Generated body"]);
    await generating;
    expect(useStore.getState().drafts[path].summary).toBe("Keep my newer draft");
    expect(useStore.getState().dialog).toMatchObject({ kind: "choice", text: expect.stringContaining("Generated suggestion") });
  } finally { request.mockRestore(); useStore.setState({ dialog: null }); }
});


it("keeps concurrent operations bound to different connection snapshots", async () => {
  const pending: (() => void)[] = [];
  vi.mocked(invoke).mockImplementation((_command, args) => new Promise(resolve => pending.push(() => resolve((args as {config: {provider: string}}).config.provider))));
  useStore.getState().setSettings({ ai: { ...DEFAULT_AI, provider: "codex" } });
  const first = aiApi.test();
  useStore.getState().setSettings({ ai: { ...DEFAULT_AI, provider: "claude_code" } });
  const second = aiApi.test();
  pending[1](); pending[0]();
  await expect(first).resolves.toBe("codex"); await expect(second).resolves.toBe("claude_code");
});
