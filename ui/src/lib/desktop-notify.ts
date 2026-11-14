// Desktop (system) notifications: posted only while the app is in the background and the setting is on,
// several at once become one toast, an unclicked toast is dismissed after 20 s, clicking one opens the bell menu,
// and removing or reading a notification closes its toast.
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useStore } from "./store";

/** A toast nobody clicked or closed is treated as ignored after 20 s and dismissed. */
export const DESKTOP_NOTIFICATION_IGNORED_TIMEOUT = 20_000;
/** Notifications that arrive within this window are sent as one toast (the length is an estimate). */
export const DESKTOP_NOTIFICATION_BATCH_MS = 300;
export const DESKTOP_NOTIFICATION_TITLE = "GitCraft";

/** NotificationDesktop-XNewMessages: "You have {0} new notifications". One message is shown as it is. */
export function desktopNotificationBody(messages: string[]): string {
  return messages.length > 1 ? `You have ${messages.length} new notifications` : (messages[0] ?? "");
}

/**
 * maybePostDesktopNotifications: nothing is posted when there are no messages, when the setting is off (default off), or when a
 * window of the app has focus.
 */
export function shouldPostDesktopNotification(messages: string[], settingOn: boolean, windowFocused: boolean): boolean {
  return messages.length > 0 && settingOn && !windowFocused;
}

const open = new Map<number, Notification>();
let pending: { id: number; text: string }[] = [];
let timer: ReturnType<typeof setTimeout> | undefined;

function show(body: string, id?: number): Notification | null {
  if (!("Notification" in window) || Notification.permission !== "granted") return null;
  try {
    const n = new Notification(DESKTOP_NOTIFICATION_TITLE, { body, tag: id === undefined ? undefined : `gitcraft-${id}` });
    // CLICKED: restore and focus the window, then open the notification menu.
    n.onclick = () => {
      void getCurrentWindow()
        .unminimize()
        .then(() => getCurrentWindow().setFocus())
        .catch(() => undefined);
      window.focus();
      useStore.getState().setNoticeMenuOpen(true);
      n.close();
    };
    // IGNORED: after 20 s the toast is treated as ignored and dismissed.
    setTimeout(() => n.close(), DESKTOP_NOTIFICATION_IGNORED_TIMEOUT);
    return n;
  } catch {
    return null;
  }
}

/** Post one desktop notification for these messages if the rules allow it. */
export function postDesktopNotifications(messages: string[], ids: number[] = []) {
  const st = useStore.getState();
  if (!shouldPostDesktopNotification(messages, st.settings.desktopNotifications, document.hasFocus())) return;
  const n = show(desktopNotificationBody(messages), messages.length === 1 ? ids[0] : undefined);
  if (n && messages.length === 1 && ids[0] !== undefined) open.set(ids[0], n);
}

/** Turning the setting on shows a native "Desktop notifications enabled" toast. Returns whether one was shown. */
export async function enableDesktopNotifications(): Promise<boolean> {
  if ("Notification" in window && Notification.permission !== "granted") await Notification.requestPermission().catch(() => undefined);
  return show("Desktop notifications enabled") !== null;
}

/**
 * Watch the notification center: new entries are posted (batched) and a removed or read entry takes its toast out of the Action Center,
 * so it also leaves the Action Center.
 */
export function startDesktopNotifications() {
  let known = new Map<number, boolean>(useStore.getState().notices.map((n) => [n.id, !!n.readAt]));
  return useStore.subscribe((state) => {
    const now = new Map(state.notices.map((n) => [n.id, !!n.readAt] as const));
    for (const n of state.notices) if (!known.has(n.id)) pending.push({ id: n.id, text: n.text });
    for (const [id, toast] of open) {
      if (!now.has(id) || now.get(id)) {
        toast.close();
        open.delete(id);
      }
    }
    known = now;
    if (pending.length && !timer) {
      timer = setTimeout(() => {
        const batch = pending;
        pending = [];
        timer = undefined;
        postDesktopNotifications(
          batch.map((b) => b.text.split("\n")[0]),
          batch.map((b) => b.id),
        );
      }, DESKTOP_NOTIFICATION_BATCH_MS);
    }
  });
}
