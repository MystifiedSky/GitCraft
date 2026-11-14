import { ChevronDown, Folder, GitBranch, Plus, Rocket, Settings, X } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useStore, usePanelStore } from "@/lib/store";
import { DropMenu, type MenuEntry } from "./ui/menu";
import { CtxMenu } from "./ui/menu";
import { PROFILE_COLORS, ProfileMark } from "./Icons";
import { switchProfile } from "@/lib/prefs";
import { cn, MOD } from "@/lib/utils";
import { NotificationMenu } from "./NotificationMenu";

const DEFAULT_TAB_WIDTH = 97;
const MIN_TAB_WIDTH = 76;
const MAX_TAB_WIDTH = 480;
const tabWidth = (value: number = DEFAULT_TAB_WIDTH) => Number.isFinite(value) ? Math.max(MIN_TAB_WIDTH, Math.min(MAX_TAB_WIDTH, value)) : DEFAULT_TAB_WIDTH;

/** Tab dragging: the drag
 *  starts after 20 px along the row; the dragged tab is drawn at half opacity with a grabbing cursor.
 *  Tabs that make room slide over in 250 ms
 *  (the sortable library's default transition, `transform 250ms ease`). */
const TAB_DRAG_START = 20;
const TAB_DRAG_OPACITY = 0.5;
/** Within this distance of the strip's edge a drag scrolls the strip (GitCraft's choice). */
const TAB_DRAG_EDGE = 32;

/** Slide a tab from `fromX` px away into its place, metrics.tab_slide_ms (src/metrics.rs TAB_SLIDE_TIME_MS) long with an ease curve. */
function slideTab(el: HTMLElement, fromX: number) {
  el.style.transition = "";
  const inFlightX = new DOMMatrixReadOnly(getComputedStyle(el).transform).m41;
  el.getAnimations().forEach((animation) => animation.cancel());
  if (!fromX || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  el.animate([{ transform: `translateX(${fromX + inFlightX}px)` }, { transform: "translateX(0)" }], { duration: useStore.getState().metrics.tab_slide_ms, easing: "ease" });
}

type TabDrag = { id: string; pointerId: number; startX: number; originLeft: number; dx: number; active: boolean; scale: number; order: string[] };

/** Repository tabs. */
export function TabsBar() {
  const s = usePanelStore();
  const strip = useRef<HTMLDivElement>(null);
  const drag = useRef<TabDrag | null>(null);
  const [dragging, setDragging] = useState<string | null>(null);
  const lefts = useRef(new Map<string, number>());
  useEffect(() => {
    strip.current?.querySelector<HTMLElement>('[data-active-tab="true"]')?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [s.activeId]);

  const tabEls = () => [...(strip.current?.querySelectorAll<HTMLElement>("[data-tab-id]") ?? [])];
  /** Keep the dragged tab under the pointer: its layout place changes when it swaps, the pointer doesn't. */
  const placeDragged = () => {
    const d = drag.current;
    const el = d && tabEls().find((x) => x.dataset.tabId === d.id);
    if (!d || !el) return;
    el.style.transition = "none";
    el.style.transform = `translateX(${d.originLeft + d.dx - el.offsetLeft}px)`;
  };
  // After the order changes, slide the other tabs from where they were to their new place (FLIP), and re-place the dragged one.
  useLayoutEffect(() => {
    const els = tabEls();
    for (const el of els) {
      const id = el.dataset.tabId!;
      const before = lefts.current.get(id);
      if (drag.current?.active && id === drag.current.id) continue;
      if (before !== undefined && before !== el.offsetLeft && drag.current?.active) slideTab(el, before - el.offsetLeft);
    }
    placeDragged();
    lefts.current = new Map(els.map((el) => [el.dataset.tabId!, el.offsetLeft]));
  });

  const onTabPointerDown = (e: React.PointerEvent<HTMLDivElement>, id: string) => {
    if (e.button !== 0 || drag.current) return;
    s.setActive(id);
    const el = e.currentTarget;
    drag.current = {
      id,
      pointerId: e.pointerId,
      startX: e.clientX,
      originLeft: el.offsetLeft,
      dx: 0,
      active: false,
      scale: el.getBoundingClientRect().width / el.offsetWidth || 1,
      order: useStore.getState().tabs.map((t) => t.id),
    };
    // Follow the pointer even if it leaves the tab before the drag starts.
    el.setPointerCapture(e.pointerId);
  };
  const onTabPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d || d.pointerId !== e.pointerId) return;
    d.dx = (e.clientX - d.startX) / d.scale;
    if (!d.active) {
      if (Math.abs(d.dx) < TAB_DRAG_START) return;
      d.active = true;
      setDragging(d.id);
    }
    const box = strip.current;
    if (box) {
      const r = box.getBoundingClientRect();
      const step = e.clientX < r.left + TAB_DRAG_EDGE ? -12 : e.clientX > r.right - TAB_DRAG_EDGE ? 12 : 0;
      if (step) {
        box.scrollLeft += step;
        d.startX -= step * d.scale; // the strip moved under the pointer
        d.dx = (e.clientX - d.startX) / d.scale;
      }
    }
    placeDragged();
    // Where the dragged tab's middle now falls among the other tabs.
    const els = tabEls();
    const me = els.find((x) => x.dataset.tabId === d.id);
    if (!me) return;
    const mid = d.originLeft + d.dx + me.offsetWidth / 2;
    const others = els.filter((x) => x !== me);
    const at = others.findIndex((x) => mid < x.offsetLeft + x.offsetWidth / 2);
    const beforeId = at < 0 ? null : others[at].dataset.tabId!;
    const curNext = els[els.indexOf(me) + 1]?.dataset.tabId ?? null;
    if (beforeId !== curNext) useStore.getState().moveTab(d.id, beforeId);
  };
  const endDrag = (cancel: boolean) => {
    const d = drag.current;
    drag.current = null;
    if (!d?.active) return;
    if (cancel) {
      // Put every tab back where it was (like the sortable list's drag cancel).
      const st = useStore.getState();
      const byId = new Map(st.tabs.map((t) => [t.id, t]));
      useStore.setState({ tabs: [...d.order.map((id) => byId.get(id)).filter((t) => !!t), ...st.tabs.filter((t) => !d.order.includes(t.id))] });
    }
    const el = tabEls().find((x) => x.dataset.tabId === d.id);
    if (el) {
      // Settle the dropped tab into its place from wherever the pointer left it.
      const from = new DOMMatrixReadOnly(getComputedStyle(el).transform).m41;
      el.style.transform = "";
      slideTab(el, from);
    }
    setDragging(null);
  };
  useEffect(() => {
    if (!dragging) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") (e.preventDefault(), e.stopPropagation(), endDrag(true));
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [dragging]);
  const shift = (id: string, by: -1 | 1) => {
    const shown = s.tabs.filter((t) => t.kind !== "launchpad");
    const i = shown.findIndex((t) => t.id === id);
    if (by < 0 && i > 0) s.moveTab(id, shown[i - 1].id);
    if (by > 0 && i >= 0 && i < shown.length - 1) s.moveTab(id, shown[i + 2]?.id ?? null);
  };
  const resizeTab = (path: string, width: number) => {
    const st = useStore.getState();
    st.setSettings({ tabWidths: { ...st.settings.tabWidths, [path]: tabWidth(width) } });
  };
  const tabList: MenuEntry[] = [
    { heading: "Open Tabs" },
    ...s.tabs.map((t) => ({ label: t.title, onSelect: () => s.setActive(t.id) })),
    ...(s.closed.length ? [{ separator: true }, { heading: "Closed Recently" }, ...s.closed.slice().reverse().map((t) => ({ label: t.title, onSelect: () => s.openRepoTab(t.path!, t.title) }))] : []),
  ];

  return (
    <div className="h-[30px] bg-tabs flex items-stretch shrink-0 border-b border-black/30">
      <IconTab title="Repositories (start screen)" onClick={() => s.newTab()}>
        <Folder size={15} />
      </IconTab>
      <IconTab title="Launchpad" onClick={s.openLaunchpad} active={s.tabs.find((t) => t.id === s.activeId)?.kind === "launchpad"}>
        <Rocket size={15} />
      </IconTab>
      <div
        ref={strip}
        // A mouse wheel scrolls the tab strip sideways when tabs don't fit.
        onWheel={(e) => {
          if (strip.current && Math.abs(e.deltaY) > Math.abs(e.deltaX)) strip.current.scrollLeft += e.deltaY;
        }}
        className="relative flex items-stretch min-w-0 overflow-x-auto overflow-y-hidden [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
      >
        {s.tabs
          .filter((t) => t.kind !== "launchpad")
          .map((t, i, shown) => {
            const active = t.id === s.activeId;
            return (
              <CtxMenu
                key={t.id}
                items={[
                  { label: "Close tab", onSelect: () => s.closeTab(t.id) },
                  { label: "Close other tabs", onSelect: () => s.tabs.filter((o) => o.id !== t.id).forEach((o) => s.closeTab(o.id)) },
                  { label: "Close tabs to the right", onSelect: () => s.tabs.slice(s.tabs.findIndex((o) => o.id === t.id) + 1).forEach((o) => s.closeTab(o.id)) },
                  { separator: true },
                  {
                    label: "Rename tab",
                    onSelect: () =>
                      s.setDialog({
                        kind: "prompt",
                        title: "Rename tab",
                        label: t.kind === "repo" ? "Repository alias (empty = clear alias)" : "Tab alias",
                        value: t.title,
                        confirm: "Save alias",
                        onSubmit: (v) => s.renameTab(t.id, v),
                      }),
                  },
                  { label: "Reopen Closed Tab", disabled: s.closed.length === 0, onSelect: s.reopenTab },
                  { separator: true },
                  { label: "Move Tab Left", shortcut: `${MOD}+Shift+PgUp`, disabled: i === 0, onSelect: () => shift(t.id, -1) },
                  { label: "Move Tab Right", shortcut: `${MOD}+Shift+PgDn`, disabled: i === shown.length - 1, onSelect: () => shift(t.id, 1) },
                ]}
              >
                <div
                  onPointerDown={(e) => onTabPointerDown(e, t.id)}
                  onPointerMove={onTabPointerMove}
                  onPointerUp={() => endDrag(false)}
                  onPointerCancel={() => endDrag(false)}
                  onLostPointerCapture={() => endDrag(false)}
                  data-tab-id={t.id}
                  data-active-tab={active ? "true" : undefined}
                  style={{
                    width: t.kind === "repo" && t.path ? tabWidth(s.settings.tabWidths?.[t.path]) : DEFAULT_TAB_WIDTH,
                    opacity: dragging === t.id ? TAB_DRAG_OPACITY : undefined,
                    cursor: dragging === t.id ? "grabbing" : undefined,
                    zIndex: dragging === t.id ? 20 : undefined,
                  }}
                  title={t.path ?? t.title}
                  className={cn(
                    "group relative flex items-center gap-1.5 shrink-0 min-w-0 pr-1.5 text-[13px] font-semibold border-r border-black/25 cursor-default touch-none select-none",
                    active ? "bg-toolbar text-white" : "text-white/75 hover:bg-white/5",
                  )}
                >
                  <button
                    type="button"
                    aria-pressed={active}
                    onClick={() => s.setActive(t.id)}
                    onAuxClick={(e) => {
                      if (e.button === 1) s.closeTab(t.id);
                    }}
                    className="flex items-center self-stretch gap-1.5 min-w-0 flex-1 pl-3 text-left cursor-default"
                  >
                    {t.kind === "repo" && <GitBranch size={12} className="shrink-0 opacity-70" />}
                    <span className="truncate">{t.title}</span>
                  </button>
                  <button
                    onMouseDown={(e) => e.stopPropagation()}
                    onPointerDown={(e) => e.stopPropagation()}
                    onClick={() => s.closeTab(t.id)}
                    className={cn("shrink-0 rounded hover:bg-white/15 p-0.5", active ? "opacity-100" : "opacity-0 group-hover:opacity-100 focus-visible:opacity-100")}
                    aria-label="Close tab"
                  >
                    <X size={13} />
                  </button>
                  {t.kind === "repo" && t.path && (
                    <TabResizeHandle title={t.title} width={tabWidth(s.settings.tabWidths?.[t.path])} onResize={(width) => resizeTab(t.path!, width)} />
                  )}
                </div>
              </CtxMenu>
            );
          })}
      </div>
      <IconTab title={`New tab (${MOD}+T)`} onClick={s.newTab}>
        <Plus size={16} />
      </IconTab>
      <div className="flex-1 min-w-0" />
      <DropMenu
        items={tabList}
        trigger={
          <button className="w-[33px] shrink-0 flex items-center justify-center text-white/80 hover:text-white outline-none" title={`Tabs list (${MOD}+Shift+A)`}>
            <ChevronDown size={16} />
          </button>
        }
      />
      <NotificationMenu />
      <IconTab title="Preferences" onClick={() => s.setDialog({ kind: "prefs" })}>
        <Settings size={15} />
      </IconTab>
      <DropMenu
        align="end"
        items={[
          { heading: "Profiles" },
          ...s.profiles.map((p) => ({ label: (p.id === s.activeProfile ? "✓ " : "   ") + p.name, onSelect: () => switchProfile(p.id) })),
          { separator: true },
          { label: "Manage profiles...", onSelect: () => s.setDialog({ kind: "profiles" }) },
          { label: "Accounts...", onSelect: () => s.setDialog({ kind: "accounts" }) },
          { label: "Preferences...", onSelect: () => s.setDialog({ kind: "prefs" }) },
        ]}
        trigger={
          <button className="shrink-0 flex items-center gap-1.5 pl-2 pr-2 text-[13px] text-white/90 hover:bg-white/5 outline-none">
            <ProfileMark color={PROFILE_COLORS[s.profiles.find((p) => p.id === s.activeProfile)?.icon ?? 0] ?? PROFILE_COLORS[0]} />
            <span className="font-semibold">{s.profiles.find((p) => p.id === s.activeProfile)?.name ?? "Default Profile"}</span>
            <ChevronDown size={13} />
          </button>
        }
      />
    </div>
  );
}

function IconTab({ children, title, onClick, active }: { children: React.ReactNode; title: string; onClick: () => void; active?: boolean }) {
  return (
    <button
      title={title}
      onClick={onClick}
      className={cn("w-[38px] shrink-0 flex items-center justify-center text-white/80 hover:text-white border-r border-black/25 outline-none", active && "bg-toolbar text-white")}
    >
      {children}
    </button>
  );
}

/** Capture the pointer so resizing continues beyond the tab edge and stops on release/cancel. */
function TabResizeHandle({ title, width, onResize }: { title: string; width: number; onResize: (width: number) => void }) {
  const drag = useRef<{ pointerId: number; x: number; width: number; scale: number } | null>(null);
  const [resizing, setResizing] = useState(false);
  const endResize = (cancel: boolean) => {
    const start = drag.current;
    drag.current = null;
    setResizing(false);
    if (cancel && start) onResize(start.width);
  };
  useEffect(() => {
    if (!resizing) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      endResize(true);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [resizing, onResize]);
  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={`Resize ${title} tab`}
      aria-valuemin={MIN_TAB_WIDTH}
      aria-valuemax={MAX_TAB_WIDTH}
      aria-valuenow={Math.round(width)}
      tabIndex={0}
      title="Drag to resize tab. Escape cancels. Double-click to reset."
      className={cn(
        "absolute right-0 inset-y-0 z-10 w-[5px] cursor-col-resize touch-none select-none outline-none hover:border-r-2 hover:border-accent focus-visible:border-r-2 focus-visible:border-accent",
        resizing && "border-r-2 border-accent",
      )}
      onMouseDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
      onDoubleClick={(e) => {
        e.stopPropagation();
        onResize(DEFAULT_TAB_WIDTH);
      }}
      onPointerDown={(e) => {
        if (e.button !== 0 || drag.current) return;
        e.preventDefault();
        e.stopPropagation();
        const tab = e.currentTarget.parentElement!;
        drag.current = {
          pointerId: e.pointerId,
          x: e.clientX,
          width: tab.offsetWidth,
          scale: tab.getBoundingClientRect().width / tab.offsetWidth || 1,
        };
        e.currentTarget.setPointerCapture(e.pointerId);
        setResizing(true);
      }}
      onPointerMove={(e) => {
        const start = drag.current;
        if (start?.pointerId === e.pointerId) onResize(start.width + (e.clientX - start.x) / start.scale);
      }}
      onPointerUp={(e) => {
        if (drag.current?.pointerId !== e.pointerId) return;
        endResize(false);
        e.currentTarget.releasePointerCapture(e.pointerId);
      }}
      onPointerCancel={() => endResize(true)}
      onLostPointerCapture={() => endResize(true)}
      onKeyDown={(e) => {
        const next = e.key === "ArrowLeft" ? width - 10 : e.key === "ArrowRight" ? width + 10 : e.key === "Home" ? MIN_TAB_WIDTH : e.key === "End" ? MAX_TAB_WIDTH : null;
        if (next === null) return;
        e.preventDefault();
        e.stopPropagation();
        onResize(next);
      }}
    />
  );
}
