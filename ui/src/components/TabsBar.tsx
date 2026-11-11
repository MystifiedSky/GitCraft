import { Bell, ChevronDown, Folder, GitBranch, Plus, Rocket, Settings, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useStore, usePanelStore } from "@/lib/store";
import { DropMenu, type MenuEntry } from "./ui/menu";
import { CtxMenu } from "./ui/menu";
import { ProfileMark } from "./Icons";
import { switchProfile } from "@/lib/prefs";
import { cn, MOD } from "@/lib/utils";

const DEFAULT_TAB_WIDTH = 97;
const MIN_TAB_WIDTH = 76;
const MAX_TAB_WIDTH = 480;
const tabWidth = (value: number = DEFAULT_TAB_WIDTH) => Number.isFinite(value) ? Math.max(MIN_TAB_WIDTH, Math.min(MAX_TAB_WIDTH, value)) : DEFAULT_TAB_WIDTH;

/** Repository tabs, like the original's tab strip. */
export function TabsBar() {
  const s = usePanelStore();
  const strip = useRef<HTMLDivElement>(null);
  useEffect(() => {
    strip.current?.querySelector<HTMLElement>('[data-active-tab="true"]')?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [s.activeId]);
  const resizeTab = (path: string, width: number) => {
    const st = useStore.getState();
    st.setSettings({ tabWidths: { ...st.settings.tabWidths, [path]: tabWidth(width) } });
  };
  const tabList: MenuEntry[] = [
    { heading: "Open Tabs" },
    ...s.tabs.map((t) => ({ label: t.title, onSelect: () => s.setActive(t.id) })),
    ...(s.closed.length ? [{ separator: true }, { heading: "Closed Recently" }, ...s.closed.slice().reverse().map((t) => ({ label: t.title, onSelect: () => s.openRepoTab(t.path!, t.title) }))] : []),
  ];
  const notifications: MenuEntry[] = [{ heading: "Notifications" }, { label: "You're all caught up", disabled: true }];

  return (
    <div className="h-[30px] bg-tabs flex items-stretch shrink-0 border-b border-black/30">
      <IconTab title="Repositories (start screen)" onClick={() => s.newTab()}>
        <Folder size={15} />
      </IconTab>
      <IconTab title="Launchpad" onClick={s.openLaunchpad} active={s.tabs.find((t) => t.id === s.activeId)?.kind === "launchpad"}>
        <Rocket size={15} />
      </IconTab>
      <div ref={strip} className="flex items-stretch min-w-0 overflow-x-auto overflow-y-hidden [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
        {s.tabs
          .filter((t) => t.kind !== "launchpad")
          .map((t) => {
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
                ]}
              >
                <div
                  onMouseDown={(e) => {
                    if (e.button === 1) s.closeTab(t.id);
                    else if (e.button === 0) s.setActive(t.id);
                  }}
                  data-active-tab={active ? "true" : undefined}
                  style={{ width: t.kind === "repo" && t.path ? tabWidth(s.settings.tabWidths?.[t.path]) : DEFAULT_TAB_WIDTH }}
                  title={t.path ?? t.title}
                  className={cn(
                    "group relative flex items-center gap-1.5 shrink-0 min-w-0 pl-3 pr-1.5 text-[13px] font-semibold border-r border-black/25 cursor-default",
                    active ? "bg-toolbar text-white" : "text-white/75 hover:bg-white/5",
                  )}
                >
                  {t.kind === "repo" ? <GitBranch size={12} className="shrink-0 opacity-70" /> : null}
                  <span className="truncate flex-1">{t.title}</span>
                  <button
                    onMouseDown={(e) => e.stopPropagation()}
                    onClick={() => s.closeTab(t.id)}
                    className={cn("shrink-0 rounded hover:bg-white/15 p-0.5", active ? "opacity-100" : "opacity-0 group-hover:opacity-100")}
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
      <DropMenu
        items={tabList}
        trigger={
          <button className="w-[33px] shrink-0 flex items-center justify-center text-white/80 hover:text-white outline-none" title={`Tabs list (${MOD}+Shift+A)`}>
            <ChevronDown size={16} />
          </button>
        }
      />
      <div className="flex-1 min-w-0" />
      <DropMenu
        align="end"
        items={notifications}
        trigger={
          <button className="w-[33px] shrink-0 flex items-center justify-center text-white/80 hover:text-white outline-none" title="Notifications">
            <Bell size={15} />
          </button>
        }
      />
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
            <ProfileMark />
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
  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={`Resize ${title} tab`}
      aria-valuemin={MIN_TAB_WIDTH}
      aria-valuemax={MAX_TAB_WIDTH}
      aria-valuenow={Math.round(width)}
      tabIndex={0}
      title="Drag to resize tab. Double-click to reset."
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
        drag.current = null;
        setResizing(false);
        e.currentTarget.releasePointerCapture(e.pointerId);
      }}
      onPointerCancel={() => {
        drag.current = null;
        setResizing(false);
      }}
      onLostPointerCapture={() => {
        drag.current = null;
        setResizing(false);
      }}
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
