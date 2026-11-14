import { describe, expect, it, vi } from "vitest";
import { DESKTOP_NOTIFICATION_IGNORED_TIMEOUT, desktopNotificationBody, shouldPostDesktopNotification } from "./desktop-notify";

describe("desktop notification rules", () => {
  it("shows one message as it is and several as a count", () => {
    expect(desktopNotificationBody(["Push failed"])).toBe("Push failed");
    expect(desktopNotificationBody(["a", "b", "c"])).toBe("You have 3 new notifications");
  });

  it("posts nothing without messages, with the setting off, or while a window has focus", () => {
    expect(shouldPostDesktopNotification([], true, false)).toBe(false);
    expect(shouldPostDesktopNotification(["x"], false, false)).toBe(false);
    expect(shouldPostDesktopNotification(["x"], true, true)).toBe(false);
    expect(shouldPostDesktopNotification(["x"], true, false)).toBe(true);
  });

  it("treats a toast as ignored after 20 seconds", () => {
    expect(DESKTOP_NOTIFICATION_IGNORED_TIMEOUT).toBe(20000);
  });
});


it("cancels read batches, closes combined toasts, and skips unrelated updates", async () => {
  const { useStore, DEFAULT_SETTINGS } = await import("./store");
  const { startDesktopNotifications } = await import("./desktop-notify");
  const original=useStore.getState(); vi.useFakeTimers();
  const close=vi.fn(); const created=vi.fn();
  class FakeNotification { static permission="granted"; close=close; constructor(){created();} }
  vi.stubGlobal("Notification",FakeNotification);vi.stubGlobal("window",{Notification:FakeNotification});vi.stubGlobal("document",{hasFocus:()=>false});
  useStore.setState({notices:[],settings:{...DEFAULT_SETTINGS,desktopNotifications:true}});
  const stop=startDesktopNotifications();
  try {
    useStore.getState().toast(false,"first");useStore.getState().markAllNotices(true);
    await vi.advanceTimersByTimeAsync(300);expect(created).not.toHaveBeenCalled();
    useStore.getState().toast(false,"second");useStore.getState().toast(false,"third");
    await vi.advanceTimersByTimeAsync(300);expect(created).toHaveBeenCalledTimes(1);
    const notices=useStore.getState().notices;const scan=vi.spyOn(notices,"map");
    for(let i=0;i<1000;i++) useStore.getState().setBusy(`progress ${i}`);
    expect(scan).not.toHaveBeenCalled();
    useStore.getState().markAllNotices(true);expect(close).toHaveBeenCalled();
  } finally {stop();vi.restoreAllMocks();useStore.setState(original,true);vi.unstubAllGlobals();vi.useRealTimers();}
});
