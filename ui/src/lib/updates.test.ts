import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), check: vi.fn(), relaunch: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: mocks.check }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: mocks.relaunch }));

function offeredUpdate() {
  return { version: "0.1.99", body: "Release notes", download: vi.fn().mockResolvedValue(undefined), install: vi.fn().mockResolvedValue(undefined), close: vi.fn().mockResolvedValue(undefined) };
}

describe("automatic GitCraft updates", () => {
  let updates: typeof import("./updates");
  let useStore: typeof import("./store").useStore;
  let stop: (() => void) | undefined;

  beforeEach(async () => {
    vi.resetModules();
    vi.resetAllMocks();
    vi.useFakeTimers();
    mocks.invoke.mockResolvedValue({ configured: true });
    mocks.check.mockResolvedValue(null);
    updates = await import("./updates");
    ({ useStore } = await import("./store"));
  });

  afterEach(() => {
    stop?.();
    stop = undefined;
    vi.useRealTimers();
  });

  it("checks at launch by default, then hourly, and cancels the schedule on cleanup", async () => {
    expect(useStore.getState().settings.autoUpdate).toBe(true);
    stop = updates.startUpdateChecks();
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.check).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(updates.CHECK_EVERY_MS - 1);
    expect(mocks.check).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(mocks.check).toHaveBeenCalledTimes(2);
    stop();
    await vi.advanceTimersByTimeAsync(updates.CHECK_EVERY_MS);
    expect(mocks.check).toHaveBeenCalledTimes(2);
  });

  it("skips background checks while disabled but allows a manual download", async () => {
    useStore.getState().setSettings({ autoUpdate: false });
    const update = offeredUpdate();
    mocks.check.mockResolvedValue(update);
    stop = updates.startUpdateChecks();
    await vi.advanceTimersByTimeAsync(updates.CHECK_EVERY_MS * 2);
    await updates.checkForUpdates(false);
    expect(mocks.invoke).not.toHaveBeenCalled();
    expect(mocks.check).not.toHaveBeenCalled();
    await updates.checkForUpdates(true);
    expect(mocks.check).toHaveBeenCalledTimes(1);
    expect(update.download).toHaveBeenCalledTimes(1);
    expect(updates.useUpdates.getState().phase).toBe("ready");
  });

  it("keeps a downloaded update ready across later checks and installs only on request", async () => {
    const update = offeredUpdate();
    mocks.check.mockResolvedValue(update);
    stop = updates.startUpdateChecks();
    await vi.advanceTimersByTimeAsync(0);
    expect(updates.useUpdates.getState()).toMatchObject({ phase: "ready", version: update.version });
    await vi.advanceTimersByTimeAsync(updates.CHECK_EVERY_MS * 3);
    expect(updates.useUpdates.getState().phase).toBe("ready");
    expect(mocks.check).toHaveBeenCalledTimes(1);
    expect(update.download).toHaveBeenCalledTimes(1);
    expect(update.install).not.toHaveBeenCalled();
    await updates.installAndRestart();
    expect(update.install).toHaveBeenCalledTimes(1);
    expect(mocks.relaunch).toHaveBeenCalledTimes(1);
  });

  it("stops an automatic check before download if the preference changes while it is checking", async () => {
    const update = offeredUpdate();
    let offer!: (value: ReturnType<typeof offeredUpdate>) => void;
    const started = new Promise<void>((resolve) => {
      mocks.check.mockImplementationOnce(() => new Promise((accept) => {
        offer = accept;
        resolve();
      }));
    });
    const checking = updates.checkForUpdates(false);
    await started;
    useStore.getState().setSettings({ autoUpdate: false });
    offer(update);
    await checking;
    expect(update.download).not.toHaveBeenCalled();
    expect(updates.useUpdates.getState().phase).toBe("available");
    await updates.download();
    expect(update.download).toHaveBeenCalledTimes(1);
  });

  it("does not overlap a scheduled check and a manual check", async () => {
    await Promise.all([updates.checkForUpdates(false), updates.checkForUpdates(true)]);
    expect(mocks.invoke).toHaveBeenCalledTimes(1);
    expect(mocks.check).toHaveBeenCalledTimes(1);
  });
});
