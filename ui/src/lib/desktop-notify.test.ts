import { describe, expect, it } from "vitest";
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
