import { useShallow } from "zustand/react/shallow";
import {
  ArrowDownToLine,
  ArrowUpFromLine,
  ChevronDown,
  ChevronRight,
  Download,
  GitBranch,
  LoaderCircle,
  Redo2,
  Search,
  SquareTerminal,
  Undo2,
  Upload,
  Zap,
} from "lucide-react";
import type { ReactNode } from "react";
import type { RepoView } from "@/lib/api";
import { api } from "@/lib/api";
import { useStore } from "@/lib/store";
import { act, openRepoPath, promptBranch } from "@/lib/repo";
import { DropMenu, type MenuEntry } from "./ui/menu";
import { addSubmodule, addWorktree, initGitflow, openCleanup } from "@/lib/manage";
import { cn, MOD } from "@/lib/utils";
import { SPIN_TURN_MS, useSpinning } from "@/lib/spin";

const PULL_LABELS: Record<string, string> = {
  fetch: "Fetch All",
  ff: "Pull (fast-forward if possible)",
  ffonly: "Pull (fast-forward only)",
  rebase: "Pull (rebase)",
};

/** The toolbar under the tabs: repository and branch pickers plus the main Git buttons. */
export function Toolbar({ repo, tabId }: { repo: RepoView | undefined; tabId: string }) {
  const s = useStore(useShallow((s) => ({ settings: s.settings, metrics: s.metrics, tabs: s.tabs, setActive: s.setActive, setDialog: s.setDialog, setSettings: s.setSettings, setUi: s.setUi })));
  const path = repo?.path;
  const labels = s.settings.toolbarLabels;
  const locals = repo?.refs.filter((r) => r.kind === "Local") ?? [];
  const detached = !!repo && !repo.head_branch;
  // Commits to pull / push for the checked-out branch (shown as small counts).
  const head = repo?.refs.find((r) => r.kind === "Local" && r.name === repo.head_branch);
  const hasChanges = !!repo && repo.status.unstaged.length + repo.status.staged.length > 0;
  const pulling = useSpinning(path, "pull");
  const pushing = useSpinning(path, "push");
  const stashing = useSpinning(path, "stash");
  const popping = useSpinning(path, "pop");

  const repoMenu: MenuEntry[] = [
    { heading: "Open Repositories" },
    ...s.tabs.filter((t) => t.kind === "repo").map((t) => ({ label: t.title, onSelect: () => s.setActive(t.id) })),
    { separator: true },
    { heading: "Recent" },
    ...s.settings.recents.slice(0, 8).map((r) => ({ label: r, onSelect: () => openRepoPath(r) })),
    { separator: true },
    { label: "Open Repo...", onSelect: () => s.setDialog({ kind: "open" }) },
    { label: "Clone Repo...", onSelect: () => s.setDialog({ kind: "clone" }) },
    { label: "Init Repo...", onSelect: () => s.setDialog({ kind: "init" }) },
  ];
  const branchMenu: MenuEntry[] = [
    { heading: "Checkout a local branch" },
    ...locals.map((b) => ({
      label: (b.name === repo?.head_branch ? "✓ " : "   ") + b.name,
      onSelect: () => path && b.name !== repo?.head_branch && act(path, { type: "checkout", target: b.name }),
    })),
    { separator: true },
    { label: "Create branch...", shortcut: `${MOD}+B`, onSelect: () => path && promptBranch(path, null) },
  ];
  const pullMenu: MenuEntry[] = (["fetch", "ff", "ffonly", "rebase"] as const).map((m) => ({
    label: (s.settings.pullMode === m ? "● " : "○ ") + PULL_LABELS[m],
    onSelect: () => s.setSettings({ pullMode: m }),
  }));
  const actionsMenu: MenuEntry[] = [
    { label: "Fetch all remotes", shortcut: `${MOD}+L`, onSelect: () => path && act(path, { type: "fetch" }) },
    { label: "Open in File Manager", onSelect: () => path && api.openFileManager(path) },
    { label: "Open External Terminal", onSelect: () => path && api.openTerminal(path) },
    { separator: true },
    { label: "Add remote...", onSelect: () => path && promptRemote(path) },
    { label: "Activity log", onSelect: () => s.setDialog({ kind: "log" }) },
    { separator: true },
    { label: "Initialize / edit Gitflow...", onSelect: () => path && initGitflow(path) },
    { label: "Add Submodule...", onSelect: () => path && addSubmodule(path) },
    { label: "Add a new worktree...", onSelect: () => repo && addWorktree(repo) },
    { label: "Clean Up Local Branches...", onSelect: openCleanup },
    { label: "Sparse Checkout...", onSelect: () => s.setDialog({ kind: "sparse" }) },
    { separator: true },
    { label: "Git LFS: enable for this repository", onSelect: () => path && act(path, { type: "lfsInstall" }) },
    { label: "Git LFS: track a pattern...", onSelect: () => path && promptLfsTrack(path) },
    { label: "Git LFS: pull LFS files", onSelect: () => path && act(path, { type: "lfsPull" }) },
  ];

  return (
    // The Undo ... Terminal group is centred in the window, Actions / Search sit at the right
    // edge. In a narrow window the pickers keep their space and the group slides right until it meets Actions.
    // Left column minimum: clamp(300px, 100vw - (centre group + right group), 389px) - see TOOLBAR_* below.
    <div
      className="h-[50px] bg-toolbar grid items-center shrink-0 border-b border-black/30 text-white/90"
      style={{ gridTemplateColumns: `minmax(clamp(${TOOLBAR_LEFT_MIN}px, calc(100vw - ${s.metrics.toolbar_center + TOOLBAR_RIGHT}px), ${s.metrics.toolbar_left_max}px), 1fr) auto minmax(max-content, 1fr)` }}
    >
      {/* repository / branch pickers */}
      {/* .toolbar .upper .bread-crumb{max-width:45vw} */}
      <div className="flex items-center h-full min-w-0 max-w-[45vw]">
      <DropMenu
        items={repoMenu}
        trigger={
          <button className="flex flex-col items-start pl-[17px] pr-2 h-full justify-center min-w-0 max-w-[150px] hover:bg-white/5 outline-none">
            <span className="text-[11px] text-white/60 leading-none mb-1">repository</span>
            <span className="text-[15px] font-bold leading-none truncate max-w-full">{repo?.name ?? "—"}</span>
          </button>
        }
      />
      <ChevronRight size={16} className="text-white/50 mx-0.5 shrink-0" />
      <DropMenu
        items={branchMenu}
        trigger={
          <button className="flex flex-col items-start pl-2 pr-2 h-full justify-center min-w-0 w-[130px] hover:bg-white/5 outline-none">
            <span className="text-[11px] text-white/60 leading-none mb-1">branch</span>
            <span className="flex items-center gap-1 text-[15px] leading-none w-full">
              <span className="truncate">{repo ? (repo.head_branch ?? `${repo.head_sha.slice(0, s.metrics.sha_len)} (detached)`) : "—"}</span>
              <ChevronDown size={14} className="shrink-0 ml-1" />
            </span>
          </button>
        }
      />
      </div>
      {/* .toolbar .upper .btn-toolbar{justify-content:center;padding-left:20px} */}
      <div className="flex items-center justify-center h-full pl-[20px]">
      <TB label="Undo" labels={labels} title={repo?.undo.undo ? `Undo ${repo.undo.undo}` : "Nothing to undo"} disabled={!repo?.undo.undo} onClick={() => path && act(path, { type: "undo" })}>
        <Undo2 size={19} />
      </TB>
      <TB label="Redo" labels={labels} title={repo?.undo.redo ? `Redo ${repo.undo.redo}` : "Nothing to redo"} disabled={!repo?.undo.redo} onClick={() => path && act(path, { type: "redo" })}>
        <Redo2 size={19} />
      </TB>
      <div className="flex items-stretch h-full">
        <TB
          label="Pull"
          labels={labels}
          title={detached ? "Pull is not available when in a detached HEAD state" : PULL_LABELS[s.settings.pullMode]}
          disabled={!repo || (detached && s.settings.pullMode !== "fetch")}
          onClick={() => path && act(path, { type: "pull", mode: s.settings.pullMode })}
          badge={head?.behind}
          spinning={pulling}
        >
          <ArrowDownToLine size={19} />
        </TB>
        <DropMenu
          items={pullMenu}
          trigger={
            // .toolbar-pull-button button:nth-child(2){margin-left:0;width:14px} (+2 px right margin)
            // While pulling the options are disabled too.
            <button disabled={pulling} className="w-[14px] mr-[2px] flex items-start pt-[13px] justify-center hover:bg-white/5 disabled:opacity-35 outline-none" title="Pull options">
              <ChevronDown size={13} />
            </button>
          }
        />
      </div>
      <TB label="Push" labels={labels} title={detached ? "Push is not available when in a detached HEAD state" : "Push"} disabled={!repo || detached} onClick={() => path && act(path, { type: "push", force: false })} badge={head?.ahead} spinning={pushing}>
        <ArrowUpFromLine size={19} />
      </TB>
      <TB label="Branch" labels={labels} title={`Create branch (${MOD}+B)`} disabled={!repo} onClick={() => path && promptBranch(path, null)}>
        <GitBranch size={19} />
      </TB>
      <TB label="Stash" labels={labels} title="Stash changes" disabled={!hasChanges && !stashing} spinning={stashing} onClick={() => path && act(path, { type: "stash", message: "" })}>
        <Download size={19} />
      </TB>
      <TB label="Pop" labels={labels} title="Pop the latest stash" disabled={!repo?.stashes.length && !popping} spinning={popping} onClick={() => path && act(path, { type: "stashPop", index: 0 })}>
        <Upload size={19} />
      </TB>
      <div className="w-px h-[34px] bg-white/20 mx-1" />
      <TB label="Terminal" labels={labels} title="Toggle the terminal panel (Ctrl+`)" disabled={!repo} onClick={() => s.setSettings({ showTerminal: !s.settings.showTerminal })}>
        <SquareTerminal size={19} />
      </TB>
      </div>
      <div className="flex items-center h-full justify-end">
      <DropMenu
        align="end"
        items={actionsMenu}
        trigger={
          <button disabled={!repo} className="flex flex-col items-center justify-center w-[56px] h-full hover:bg-white/5 disabled:opacity-40 outline-none">
            {labels && <span className="text-[11px] text-white/75 leading-none mb-[5px]">Actions</span>}
            <Zap size={19} />
          </button>
        }
      />
      <TB label="Search" labels={labels} title={`Search commits (${MOD}+F)`} disabled={!repo} onClick={() => s.setUi(tabId, { searchOpen: true })}>
        <Search size={19} />
      </TB>
      </div>
    </div>
  );
}

/** Widths (px) of the toolbar groups (.toolbar-btn width 48px + margin 0 2px = 52 each):
 *  centre = 20 padding + Undo, Redo, Pull, Push, Branch, Stash, Pop, Terminal (8 x 52) + Pull options (14 + 2)
 *  + divider (9) = metrics.toolbar_center (src/metrics.rs); right = Actions (56) + Search (52). The left column keeps at
 *  least 300 px for the pickers and at most metrics.toolbar_left_max (src/metrics.rs). */
const TOOLBAR_RIGHT = 108;
const TOOLBAR_LEFT_MIN = 300;

function TB({
  children,
  label,
  labels,
  title,
  onClick,
  disabled,
  className,
  badge,
  spinning,
}: {
  children: ReactNode;
  label: string;
  labels: boolean;
  title: string;
  onClick: () => void;
  disabled?: boolean;
  className?: string;
  badge?: number;
  /** The button's action is running: a spinning circle replaces the icon. */
  spinning?: boolean;
}) {
  return (
    <button
      title={title}
      // While it spins, another press does nothing.
      onClick={spinning ? undefined : onClick}
      disabled={disabled}
      aria-busy={spinning || undefined}
      className={cn("flex flex-col items-center justify-center w-[52px] h-full hover:bg-white/5 disabled:opacity-35 outline-none", spinning && "cursor-default", className)}
    >
      {/* The label shows while spinning even when toolbar labels are off. */}
      {(labels || spinning) && <span className="text-[11px] text-white/75 leading-none mb-[5px]">{label}</span>}
      <span className="relative">
        {spinning ? <LoaderCircle size={19} style={{ animation: `spin ${SPIN_TURN_MS}ms linear infinite` }} /> : children}
        {!!badge && badge > 0 && (
          <span className="absolute -top-[5px] -right-[11px] min-w-[15px] h-[14px] px-[3px] rounded-full bg-accent text-white text-[9.5px] font-bold leading-[14px] text-center">
            {badge}
          </span>
        )}
      </span>
    </button>
  );
}

export function promptRemote(path: string) {
  useStore.getState().setDialog({
    kind: "prompt",
    title: "Add a remote",
    label: "Remote name",
    value: "origin",
    confirm: "Add Remote",
    extraLabel: "Remote URL",
    onSubmit: (name, url) => act(path, { type: "addRemote", name, url: url ?? "" }),
  });
}

function promptLfsTrack(path: string) {
  useStore.getState().setDialog({
    kind: "prompt",
    title: "Track with Git LFS",
    label: "Pattern (for example *.psd or assets/**)",
    value: "*.psd",
    confirm: "Track",
    onSubmit: (pattern) => act(path, { type: "lfsTrack", pattern }),
  });
}
