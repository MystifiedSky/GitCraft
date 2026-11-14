import { afterEach, expect, it, vi } from "vitest";
import { api } from "./api";
import { useStore } from "./store";
import { startPersistence, flushPersistence } from "./persistence";
const original = useStore.getState();
afterEach(() => { useStore.setState(original, true); vi.restoreAllMocks(); vi.useRealTimers(); });
it("flushes the latest settings and tabs before the debounce expires", async () => {
  vi.useFakeTimers();
  const save = vi.spyOn(api,"saveSettings").mockResolvedValue(undefined);
  const stop = startPersistence();
  useStore.getState().setSettings({ rememberTabs:true });
  useStore.getState().openRepoTab("/repos/new", "new");
  expect(save).not.toHaveBeenCalled();
  await flushPersistence();
  expect(save).toHaveBeenCalledWith(expect.objectContaining({tabs:["/repos/new"]}));
  await vi.advanceTimersByTimeAsync(500);
  expect(save).toHaveBeenCalledTimes(1); stop();
});
it("reports a failed shutdown save and allows retry", async () => {
  const save=vi.spyOn(api,"saveSettings").mockRejectedValueOnce(new Error("disk full")).mockResolvedValue(undefined);
  const stop=startPersistence(); useStore.getState().setSettings({ rememberTabs:false });
  await expect(flushPersistence()).rejects.toThrow("disk full");
  await flushPersistence(); expect(save).toHaveBeenCalledTimes(2); stop();
});


it("includes edits made while an earlier save is in flight", async () => {
  let finish!:()=>void;
  const save=vi.spyOn(api,"saveSettings").mockImplementationOnce(()=>new Promise<void>(resolve=>{finish=resolve;})).mockResolvedValue(undefined);
  const stop=startPersistence();useStore.getState().setSettings({rememberTabs:false});const flush=flushPersistence();
  await Promise.resolve();await Promise.resolve();
  useStore.getState().setSettings({rememberTabs:true});finish();await flush;
  expect(save).toHaveBeenCalledTimes(2);expect(save).toHaveBeenLastCalledWith(expect.objectContaining({settings:expect.objectContaining({rememberTabs:true})}));stop();
});
