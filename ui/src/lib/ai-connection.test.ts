import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { ai } from "./ai";
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
  it.each(requests)("%s waits for the latest local connection settings", async (command, run) => {
    const config = { ...DEFAULT_AI, provider: "custom" as const, endpoint: "http://localhost:1234/v1", defaultModel: "local-model", responseFormat: "text" as const };
    useStore.getState().setSettings({ ai: config });
    let release!: () => void;
    vi.mocked(invoke).mockImplementationOnce(() => new Promise<void>((resolve) => { release = resolve; }));
    vi.mocked(invoke).mockResolvedValueOnce("result");
    const request = run();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenNthCalledWith(1, "set_ai_config", { config });
    release();
    await expect(request).resolves.toBe("result");
    expect(vi.mocked(invoke).mock.calls[1][0]).toBe(command);
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
    vi.mocked(invoke).mockResolvedValueOnce(undefined).mockResolvedValueOnce(["Summarize changes", ""]);
    await expect(ai.commitMessage("/repo")).resolves.toEqual(["Summarize changes", ""]);
    expect(invoke).toHaveBeenNthCalledWith(1, "set_ai_config", { config: expect.objectContaining({ provider, cliPath, cliConfigDir }) });
    expect(invoke).toHaveBeenNthCalledWith(2, "ai_commit_message", { path: "/repo" });
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
