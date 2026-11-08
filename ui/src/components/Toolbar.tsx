import {
  ArrowDownToLine,
  ArrowUpFromLine,
  ChevronDown,
  ChevronRight,
  Download,
  GitBranch,
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
import { cn, MOD } from "@/lib/utils";

const PULL_LABELS: Record<string, string> = {
  fetch: "Fetch All",
  ff: "Pull (fast-forward if possible)",
  ffonly: "Pull (fast-forward only)",
  rebase: "Pull (rebase)",
};

/** The toolbar under the tabs: repository and branch pickers plus the main Git buttons. */
export function Toolbar({ repo, tabId }: { repo: RepoView | undefined; tabId: string }) {
  const s = useStore();
  const path = repo?.path;
  const labels = s.settings.toolbarLabels;
  const locals = repo?.refs.filter((r) => r.kind === "Local") ?? [];
  const detached = !!repo && !repo.head_branch;
  const hasChanges = !!repo && repo.status.unstaged.length + repo.status.staged.length > 0;

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
    { label: "Gitflow, LFS and submodules come in Stage 4", disabled: true },
  ];

  return (
    <div className="h-[50px] bg-toolbar flex items-center shrink-0 border-b border-black/30 text-white/90">
      {/* repository / branch pickers */}
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
      <div className="flex-1 min-w-2" />
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
          className="pr-0 w-[44px]"
        >
          <ArrowDownToLine size={19} />
        </TB>
        <DropMenu
          items={pullMenu}
          trigger={
            <button className="w-[16px] flex items-start pt-[13px] justify-center hover:bg-white/5 outline-none" title="Pull options">
              <ChevronDown size={13} />
            </button>
          }
        />
      </div>
      <TB label="Push" labels={labels} title={detached ? "Push is not available when in a detached HEAD state" : "Push"} disabled={!repo || detached} onClick={() => path && act(path, { type: "push", force: false })}>
        <ArrowUpFromLine size={19} />
      </TB>
      <TB label="Branch" labels={labels} title={`Create branch (${MOD}+B)`} disabled={!repo} onClick={() => path && promptBranch(path, null)}>
        <GitBranch size={19} />
      </TB>
      <TB label="Stash" labels={labels} title="Stash changes" disabled={!hasChanges} onClick={() => path && act(path, { type: "stash", message: "" })}>
        <Download size={19} />
      </TB>
      <TB label="Pop" labels={labels} title="Pop the latest stash" disabled={!repo?.stashes.length} onClick={() => path && act(path, { type: "stashPop", index: 0 })}>
        <Upload size={19} />
      </TB>
      <div className="w-px h-[34px] bg-white/20 mx-1" />
      <TB label="Terminal" labels={labels} title="Open a terminal in this repository" disabled={!repo} onClick={() => path && api.openTerminal(path)}>
        <SquareTerminal size={19} />
      </TB>
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
  );
}

function TB({
  children,
  label,
  labels,
  title,
  onClick,
  disabled,
  className,
}: {
  children: ReactNode;
  label: string;
  labels: boolean;
  title: string;
  onClick: () => void;
  disabled?: boolean;
  className?: string;
}) {
  return (
    <button
      title={title}
      onClick={onClick}
      disabled={disabled}
      className={cn("flex flex-col items-center justify-center w-[52px] h-full hover:bg-white/5 disabled:opacity-35 outline-none", className)}
    >
      {labels && <span className="text-[11px] text-white/75 leading-none mb-[5px]">{label}</span>}
      {children}
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
