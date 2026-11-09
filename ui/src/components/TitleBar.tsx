import { checkForUpdates, download, installAndRestart, RELEASES_URL, updateMenuLabel, useUpdates } from "@/lib/updates";
import { hosting } from "@/lib/hosting";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
import { DropMenu, type MenuEntry } from "./ui/menu";
import { useStore } from "@/lib/store";
import { api } from "@/lib/api";
import { act } from "@/lib/repo";
import { MOD } from "@/lib/utils";
import { resetZoom, stepZoom } from "@/lib/zoom";
import { AppMark } from "./Icons";

const win = () => getCurrentWindow();

/** Custom title bar and the File / Edit / View / Help menu bar. */
export function TitleBar() {
  return (
    <div className="h-[30px] bg-titlebar flex items-center shrink-0" data-tauri-drag-region>
      <div className="pl-2.5 pr-2 pointer-events-none">
        <AppMark size={16} />
      </div>
      <span className="text-[12px] text-white/90 pointer-events-none" data-tauri-drag-region>
        GitCraft
      </span>
      <div className="flex-1 h-full" data-tauri-drag-region />
      <WinButton onClick={() => win().minimize()} label="Minimize">
        <Minus size={14} strokeWidth={1.4} />
      </WinButton>
      <WinButton onClick={() => win().toggleMaximize()} label="Maximize">
        <Square size={11} strokeWidth={1.4} />
      </WinButton>
      <WinButton onClick={() => win().close()} label="Close" danger>
        <X size={16} strokeWidth={1.3} />
      </WinButton>
    </div>
  );
}

function WinButton({ children, onClick, label, danger }: { children: React.ReactNode; onClick: () => void; label: string; danger?: boolean }) {
  return (
    <button
      aria-label={label}
      onClick={onClick}
      className={`w-[46px] h-full flex items-center justify-center text-white/80 ${danger ? "hover:bg-[#c42b1c]" : "hover:bg-white/10"} hover:text-white`}
    >
      {children}
    </button>
  );
}

export function MenuBar() {
  const s = useStore();
  const tab = s.tabs.find((t) => t.id === s.activeId);
  const repoPath = tab?.kind === "repo" ? tab.path : undefined;
  const set = s.settings;
  const up = useUpdates();

  const file: MenuEntry[] = [
    // Same order as the original's File menu (reference picture 1).
    { label: "New Tab", shortcut: `${MOD}+T`, onSelect: s.newTab },
    { label: "Close Tab", shortcut: `${MOD}+W`, onSelect: () => s.closeTab(s.activeId) },
    { label: "Reopen Closed Tab", shortcut: `${MOD}+Shift+T`, disabled: s.closed.length === 0, onSelect: s.reopenTab },
    { separator: true },
    { label: "Clone Repo...", shortcut: `${MOD}+N`, onSelect: () => s.setDialog({ kind: "clone" }) },
    { label: "Init Repo...", shortcut: `${MOD}+I`, onSelect: () => s.setDialog({ kind: "init" }) },
    { label: "Open Repo...", shortcut: `${MOD}+O`, onSelect: () => s.setDialog({ kind: "open" }) },
    { label: "Open Repo Management", shortcut: `Alt+${MOD}+O`, onSelect: () => s.setDialog({ kind: "repoManagement" }) },
    { label: "Open Repo in VS Code", shortcut: `${MOD}+Shift+E`, disabled: !repoPath, onSelect: () => repoPath && api.openInEditor(repoPath) },
    { separator: true },
    { label: "Open External Terminal", shortcut: "Alt+T", disabled: !repoPath, onSelect: () => repoPath && api.openTerminal(repoPath) },
    { label: "Open in File Manager", shortcut: "Alt+O", disabled: !repoPath, onSelect: () => repoPath && api.openFileManager(repoPath) },
    { separator: true },
    { label: "File History / Blame...", shortcut: `${MOD}+Shift+H`, disabled: !repoPath, onSelect: () => s.setDialog({ kind: "fileFinder" }) },
    { separator: true },
    { label: "Preferences...", shortcut: `${MOD}+,`, onSelect: () => s.setDialog({ kind: "prefs" }) },
    { separator: true },
    {
      label: updateMenuLabel(up),
      disabled: up.phase === "checking" || up.phase === "downloading",
      onSelect: () => (up.phase === "ready" ? installAndRestart() : up.phase === "available" ? download() : checkForUpdates(true)),
    },
    { label: "Exit", onSelect: () => win().close() },
  ];
  const edit: MenuEntry[] = [
    { label: "Undo", shortcut: `${MOD}+Z`, disabled: !repoPath, onSelect: () => repoPath && act(repoPath, { type: "undo" }) },
    { label: "Redo", shortcut: `${MOD}+Y`, disabled: !repoPath, onSelect: () => repoPath && act(repoPath, { type: "redo" }) },
    { separator: true },
    { label: "Cut", shortcut: `${MOD}+X`, onSelect: () => document.execCommand("cut") },
    { label: "Copy", shortcut: `${MOD}+C`, onSelect: () => document.execCommand("copy") },
    { label: "Paste", shortcut: `${MOD}+V`, onSelect: () => navigator.clipboard.readText().then((t) => document.execCommand("insertText", false, t)) },
    { label: "Select All", shortcut: `${MOD}+A`, onSelect: () => document.execCommand("selectAll") },
  ];
  const view: MenuEntry[] = [
    { label: `${set.showLeft ? "✓ " : "   "}Show Left Panel`, shortcut: `${MOD}+J`, onSelect: () => s.setSettings({ showLeft: !set.showLeft }) },
    { label: `${set.showRight ? "✓ " : "   "}Show Commit Details Panel`, shortcut: `${MOD}+K`, onSelect: () => s.setSettings({ showRight: !set.showRight }) },
    { label: `${set.toolbarLabels ? "✓ " : "   "}Show Toolbar Labels`, onSelect: () => s.setSettings({ toolbarLabels: !set.toolbarLabels }) },
    { label: `${set.showTerminal ? "✓ " : "   "}Show Terminal Panel`, shortcut: `${MOD}+\``, onSelect: () => s.setSettings({ showTerminal: !set.showTerminal }) },
    { separator: true },
    { label: "Increase Zoom", shortcut: `${MOD}+=`, onSelect: () => stepZoom(1) },
    { label: "Decrease Zoom", shortcut: `${MOD}+-`, onSelect: () => stepZoom(-1) },
    { label: "Reset Zoom", shortcut: `${MOD}+0`, onSelect: resetZoom },
    { separator: true },
    { label: "Toggle Full Screen", shortcut: "F11", onSelect: async () => win().setFullscreen(!(await win().isFullscreen())) },
    { label: "Activity Log", onSelect: () => s.setDialog({ kind: "log" }) },
    { label: "Reload Window", onSelect: () => location.reload() },
  ];
  const help: MenuEntry[] = [
    { label: "Command Palette", shortcut: `${MOD}+Shift+P`, onSelect: () => s.setPalette("root") },
    { label: "Keyboard Shortcuts", shortcut: `${MOD}+/`, onSelect: () => s.setDialog({ kind: "shortcuts" }) },
    { label: "Feature Plan", onSelect: () => s.setDialog({ kind: "plan" }) },
    { label: "View Activity Logs", onSelect: () => s.setDialog({ kind: "log" }) },
    { separator: true },
    { label: "View Release Notes", onSelect: () => hosting.openUrl(RELEASES_URL) },
    { label: "About GitCraft", onSelect: () => s.setDialog({ kind: "about" }) },
  ];

  return (
    <div className="h-[26px] bg-menubar flex items-center px-0.5 shrink-0 text-[13px] text-white/90">
      {(
        [
          ["File", file],
          ["Edit", edit],
          ["View", view],
          ["Help", help],
        ] as const
      ).map(([name, items]) => (
        <DropMenu
          key={name}
          items={items as MenuEntry[]}
          trigger={<button className="px-[7px] h-[22px] rounded-[3px] hover:bg-white/10 data-[state=open]:bg-white/10 outline-none">{name}</button>}
        />
      ))}
    </div>
  );
}
