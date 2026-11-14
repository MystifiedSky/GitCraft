import { Bell, Check, EllipsisVertical, Trash2, TriangleAlert, Undo2 } from "lucide-react";
import { useEffect, useRef } from "react";
import { useStore, noticeVisible, type NoticeFilter } from "@/lib/store";
import { checkForUpdates, download, installAndRestart, updateMenuLabel, useUpdates } from "@/lib/updates";
import { formatNoticeDate, unreadBadge } from "@/lib/notices";
import { DropMenu } from "./ui/menu";

const FILTERS: NoticeFilter[] = ["All", "Read", "Unread"];
/** Empty-state texts, one per filter. */
const EMPTY: Record<NoticeFilter, string> = {
  All: "You have no notifications",
  Read: "You have no read notifications",
  Unread: "You have no unread notifications",
};
/**
 * The list is capped at the window height minus the bars around it: title bar + tabs bar + 40 + status bar + 16.
 * GitCraft's bars are 30 (title) + 26 (menu) + 30 (tabs) + 26 (status), so it reserves 30 + 26 + 30 + 40 + 26 + 16.
 */
const MENU_RESERVED = 168;

/** The bell: unread badge ("50+" from 50) and the notification menu. */
export function NotificationMenu() {
  const root = useRef<HTMLDivElement>(null);
  const s = useStore();
  const open = s.noticeMenuOpen;
  const setOpen = (v: boolean | ((x: boolean) => boolean)) => s.setNoticeMenuOpen(typeof v === "function" ? v(useStore.getState().noticeMenuOpen) : v);
  const up = useUpdates();
  const updateRow = up.phase === "available" || up.phase === "ready" || up.phase === "downloading";

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as HTMLElement | null;
      // The filter and "more" menus render in a portal; they belong to this menu.
      if (root.current?.contains(t) || t?.closest(".menu-content")) return;
      setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const badge = unreadBadge(s.notices.filter((n) => !n.readAt).length + (updateRow ? 1 : 0));
  // Newest first.
  const shown = s.notices.filter((n) => noticeVisible(n, s.noticeFilter)).slice().reverse();
  const showUpdate = updateRow && s.noticeFilter !== "Read";
  const anyUnread = shown.some((n) => !n.readAt);
  const anyRead = shown.some((n) => n.readAt);

  return (
    <div ref={root} className="relative shrink-0 flex">
      <button
        className="relative w-[33px] flex items-center justify-center text-white/80 hover:text-white outline-none"
        title={badge ? `Notifications (${badge} unread)` : "Notifications"}
        aria-label={badge ? `Notifications, ${badge} unread` : "Notifications"}
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
      >
        <Bell size={15} />
        {badge && (
          <span className="absolute top-[3px] right-[1px] min-w-[14px] h-[14px] px-[3px] rounded-full bg-[#2f81f7] text-white text-[9px] leading-[14px] text-center font-semibold">{badge}</span>
        )}
      </button>
      {open && (
        <div
          className="menu-content absolute right-0 top-full mt-1 z-50 w-[340px] !p-0"
          role="dialog"
          aria-label="Notifications"
          style={{ maxHeight: `calc(100vh - ${MENU_RESERVED}px)`, overflow: "hidden", display: "flex", flexDirection: "column" }}
        >
          <div className="flex items-center justify-between py-1.5 pl-3 pr-2 border-b border-white/10 shrink-0">
            <div className="flex items-center gap-3">
              <span className="font-semibold text-[13px]">Notifications</span>
              <DropMenu
                items={FILTERS.map((f) => ({ label: `${f === s.noticeFilter ? "✓ " : "   "}${f}`, onSelect: () => s.setNoticeFilter(f) }))}
                trigger={
                  <button className="text-[12.5px] text-white/80 hover:text-white px-1.5 h-[22px] rounded border border-white/15" aria-label="Filter notifications">
                    {s.noticeFilter} ▾
                  </button>
                }
              />
            </div>
            <DropMenu
              align="end"
              items={[
                { label: "Mark all as read", disabled: !anyUnread, onSelect: () => s.markAllNotices(true) },
                { label: "Mark all as unread", disabled: !anyRead, onSelect: () => s.markAllNotices(false) },
                {
                  label: "Notification Settings",
                  onSelect: () => {
                    setOpen(false);
                    s.setDialog({ kind: "prefs" });
                  },
                },
              ]}
              trigger={
                <button className="w-[22px] h-[22px] flex items-center justify-center text-white/70 hover:text-white" aria-label="Notification options">
                  <EllipsisVertical size={15} />
                </button>
              }
            />
          </div>
          <div className="overflow-auto min-h-0">
            {showUpdate && (
              <div className="px-3 py-2 border-b border-white/5 text-[12.5px] flex items-center gap-2">
                <TriangleAlert size={14} className="text-warn shrink-0" />
                <span className="flex-1">{updateMenuLabel(up)}</span>
                <button
                  className="text-link hover:underline disabled:opacity-50"
                  disabled={up.phase === "downloading"}
                  onClick={() => (up.phase === "ready" ? installAndRestart() : up.phase === "available" ? download() : checkForUpdates(true))}
                >
                  {up.phase === "ready" ? "Restart" : "Update now"}
                </button>
              </div>
            )}
            {shown.map((n) => (
              <div key={n.id} className={`px-3 py-2 border-b border-white/5 text-[12.5px] ${n.readAt ? "text-white/60" : "text-white"}`}>
                <div className="flex items-start gap-2">
                  {!n.readAt && <span className="mt-[5px] w-[7px] h-[7px] rounded-full bg-[#2f81f7] shrink-0" aria-label="Unread" />}
                  <div className="flex-1 min-w-0 break-words">{n.text.length > 240 ? `${n.text.slice(0, 239)}…` : n.text}</div>
                </div>
                <div className="flex items-center justify-between mt-1 text-[11.5px] text-white/50">
                  <span>{formatNoticeDate(new Date(n.at))}</span>
                  <span className="flex items-center gap-1">
                    <button
                      className="w-[18px] h-[18px] flex items-center justify-center hover:text-white"
                      title={n.readAt ? "Mark as unread" : "Mark as read"}
                      aria-label={n.readAt ? "Mark as unread" : "Mark as read"}
                      onClick={() => s.markNotice(n.id, !n.readAt)}
                    >
                      {n.readAt ? <Undo2 size={13} /> : <Check size={13} />}
                    </button>
                    <button className="w-[18px] h-[18px] flex items-center justify-center hover:text-white" title="Remove notification" aria-label="Remove notification" onClick={() => s.removeNotice(n.id)}>
                      <Trash2 size={13} />
                    </button>
                  </span>
                </div>
              </div>
            ))}
            {!shown.length && !showUpdate && <div className="p-2 text-center text-[12.5px] text-white/70">{EMPTY[s.noticeFilter]}</div>}
          </div>
        </div>
      )}
    </div>
  );
}
