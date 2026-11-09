import { Bell, ChevronDown, Folder, GitBranch, Plus, Rocket, Settings, X } from "lucide-react";
import { useStore, usePanelStore } from "@/lib/store";
import { DropMenu, type MenuEntry } from "./ui/menu";
import { CtxMenu } from "./ui/menu";
import { ProfileMark } from "./Icons";
import { switchProfile } from "@/lib/prefs";
import { cn, MOD } from "@/lib/utils";

/** Repository tabs, like the original's tab strip. */
export function TabsBar() {
  const s = usePanelStore();
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
      <div className="flex items-stretch min-w-0 overflow-hidden">
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
                  title={t.path ?? t.title}
                  className={cn(
                    "group relative flex items-center gap-1.5 w-[97px] pl-3 pr-1.5 text-[13px] font-semibold border-r border-black/25 cursor-default",
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
          <button className="w-[33px] flex items-center justify-center text-white/80 hover:text-white outline-none" title={`Tabs list (${MOD}+Shift+A)`}>
            <ChevronDown size={16} />
          </button>
        }
      />
      <div className="flex-1" />
      <DropMenu
        align="end"
        items={notifications}
        trigger={
          <button className="w-[33px] flex items-center justify-center text-white/80 hover:text-white outline-none" title="Notifications">
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
          <button className="flex items-center gap-1.5 pl-2 pr-2 text-[13px] text-white/90 hover:bg-white/5 outline-none">
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
      className={cn("w-[38px] flex items-center justify-center text-white/80 hover:text-white border-r border-black/25 outline-none", active && "bg-toolbar text-white")}
    >
      {children}
    </button>
  );
}
