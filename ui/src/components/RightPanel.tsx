import { useQuery } from "@tanstack/react-query";
import {
  ArrowDownAZ,
  ChevronDown,
  ChevronRight,
  CloudUpload,
  Copy,
  Download,
  FolderTree,
  GitCommitHorizontal,
  List,
  Sparkles,
  Trash2,
  X,
  Folder,
} from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";
import type { Commit, FileChange, RepoView } from "@/lib/api";
import { api } from "@/lib/api";
import { useStore, useTabUi, type DiffTarget } from "@/lib/store";
import { act, confirmThen, copy } from "@/lib/repo";
import { fileMenu } from "@/lib/menus";
import { CtxMenu } from "./ui/menu";
import { Button } from "./ui/button";
import { Avatar, StatusIcon } from "./Icons";
import { cn, fullDate, MOD } from "@/lib/utils";

/** Right panel: staging and committing (WIP) or the details of a selected commit. */
export function RightPanel({ repo, tabId }: { repo: RepoView; tabId: string }) {
  const ui = useTabUi(tabId);
  if (ui.selection.type === "commit") {
    const sha = ui.selection.sha;
    const c = repo.commits.find((x) => x.sha === sha);
    if (c) return <CommitDetails repo={repo} tabId={tabId} commit={c} />;
  }
  return <WorkPanel repo={repo} tabId={tabId} />;
}

function PanelHeader({ children }: { children: ReactNode }) {
  return <div className="flex items-center h-[30px] px-[8px] gap-2 shrink-0 bg-panel border-b border-black/20">{children}</div>;
}

function AiButton({ title }: { title: string }) {
  return (
    <button title={title} className="w-[26px] h-[24px] rounded-[3px] border border-ai/80 bg-ai/15 text-white/80 flex items-center justify-center hover:bg-ai/25 shrink-0">
      <Sparkles size={13} />
    </button>
  );
}

function ViewSwitch() {
  const s = useStore();
  const v = s.settings.fileView;
  return (
    <div className="flex items-center h-[38px] px-[8px] shrink-0">
      <button className="text-white/75 hover:text-white" title="Sort files">
        <ArrowDownAZ size={16} />
      </button>
      <div className="flex-1 flex justify-center">
        <div className="flex h-[24px] rounded-[3px] overflow-hidden border border-white/10 text-[13px]">
          <button onClick={() => s.setSettings({ fileView: "path" })} className={cn("px-3 flex items-center gap-1.5", v === "path" ? "bg-[#444] text-white" : "bg-[#313131] text-fg2 hover:text-white")}>
            <List size={14} /> Path
          </button>
          <button onClick={() => s.setSettings({ fileView: "tree" })} className={cn("px-3 flex items-center gap-1.5", v === "tree" ? "bg-[#444] text-white" : "bg-[#313131] text-fg2 hover:text-white")}>
            <FolderTree size={14} /> Tree
          </button>
        </div>
      </div>
      <div className="w-4" />
    </div>
  );
}

function FileList({
  files,
  onOpen,
  menu,
  action,
  activeFile,
}: {
  files: FileChange[];
  onOpen: (f: FileChange) => void;
  menu: (f: FileChange) => ReturnType<typeof fileMenu>;
  action?: (f: FileChange) => ReactNode;
  activeFile?: string;
}) {
  const view = useStore((s) => s.settings.fileView);
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const row = (f: FileChange, indent: number, label: string) => (
    <CtxMenu key={f.path} items={() => menu(f)}>
      <div
        onClick={() => onOpen(f)}
        className={cn("group flex items-center gap-[6px] h-[24px] pr-2 text-[12.5px] cursor-default hover:bg-hoverrow", activeFile === f.path && "bg-selrow")}
        style={{ paddingLeft: 12 + indent * 14 }}
        title={f.path}
      >
        <StatusIcon status={f.status} />
        <span className="truncate flex-1 text-white/85">{label}</span>
        {action && <span className="opacity-0 group-hover:opacity-100">{action(f)}</span>}
      </div>
    </CtxMenu>
  );
  if (view === "path") return <div>{files.map((f) => row(f, 0, f.path))}</div>;
  // Tree view: group by folder.
  const groups = new Map<string, FileChange[]>();
  for (const f of files) {
    const i = f.path.lastIndexOf("/");
    const dir = i >= 0 ? f.path.slice(0, i) : "";
    groups.set(dir, [...(groups.get(dir) ?? []), f]);
  }
  const dirs = [...groups.keys()].sort();
  return (
    <div>
      {dirs.map((d) => (
        <div key={d || "."}>
          {d && (
            <div
              className="flex items-center gap-[6px] h-[24px] pl-[10px] text-[12.5px] text-fg2 hover:bg-hoverrow cursor-default"
              onClick={() => setCollapsed((c) => ({ ...c, [d]: !c[d] }))}
            >
              <ChevronRight size={12} className={cn("transition-transform", !collapsed[d] && "rotate-90")} />
              <Folder size={13} />
              <span className="truncate">{d}</span>
            </div>
          )}
          {!collapsed[d] && groups.get(d)!.map((f) => row(f, d ? 2 : 0, f.path.slice(d ? d.length + 1 : 0)))}
        </div>
      ))}
    </div>
  );
}

function WorkPanel({ repo, tabId }: { repo: RepoView; tabId: string }) {
  const s = useStore();
  const ui = useTabUi(tabId);
  const draft = s.drafts[repo.path] ?? { summary: "", description: "", amend: false };
  const [tab, setTab] = useState<"commit" | "stash">("commit");
  const [optionsOpen, setOptionsOpen] = useState(false);
  const [unstagedOpen, setUnstagedOpen] = useState(true);
  const [stagedOpen, setStagedOpen] = useState(true);
  const [stashMsg, setStashMsg] = useState("");
  const { unstaged, staged } = repo.status;
  const total = new Set([...unstaged.map((f) => f.path), ...staged.map((f) => f.path)]).size;
  const branch = repo.head_branch ?? repo.head_sha.slice(0, s.metrics.sha_len);
  const left = s.metrics.summary_counter - draft.summary.length; // the original's 72-character summary counter
  const push = s.settings.pushAfterCommit;

  const openDiff = (f: FileChange, isStaged: boolean) => {
    const target: DiffTarget = { source: { kind: isStaged ? "Staged" : "Unstaged" }, file: f.path, untracked: f.status === "?", parent: null };
    s.setUi(tabId, { diff: target });
  };

  const doCommit = async (stageAll: boolean) => {
    const ok = await act(repo.path, { type: "commit", summary: draft.summary, description: draft.description, amend: draft.amend, stageAll });
    if (ok) {
      s.setDraft(repo.path, { summary: "", description: "", amend: false });
      s.setUi(tabId, { diff: null });
      if (push) await act(repo.path, { type: "push", force: false });
    }
  };

  const toggleAmend = async (on: boolean) => {
    if (on) {
      const [summary, description] = await api.lastCommitMessage(repo.path);
      s.setDraft(repo.path, { amend: true, summary, description });
    } else s.setDraft(repo.path, { amend: false, summary: "", description: "" });
  };

  let commitLabel: string;
  let commitEnabled = true;
  if (tab === "stash") {
    commitLabel = total === 0 ? "No Changes to Stash" : `Stash Changes to ${total} File${total === 1 ? "" : "s"}`;
    commitEnabled = total > 0;
  } else if (staged.length === 0 && !draft.amend) {
    commitLabel = "Stage Changes to Commit";
    commitEnabled = false;
  } else if (!draft.summary.trim()) {
    commitLabel = "Type a Message to Commit";
    commitEnabled = false;
  } else if (draft.amend) {
    commitLabel = "Amend Previous Commit" + (push ? " and Push" : "");
  } else {
    commitLabel = `Commit Changes to ${staged.length} File${staged.length === 1 ? "" : "s"}` + (push ? " and Push" : "");
  }

  return (
    <div className="h-full flex flex-col bg-panel min-w-0">
      <PanelHeader>
        <button
          title="Discard all changes"
          disabled={total === 0}
          onClick={() => confirmThen("Discard all changes?", "All uncommitted changes will be discarded. You can bring them back with Undo.", "Discard All", () => act(repo.path, { type: "discardAll" }))}
          className="w-[22px] h-[20px] rounded-[3px] border border-bad text-[#ef7a76] flex items-center justify-center hover:bg-bad/15 disabled:opacity-30"
        >
          <Trash2 size={12} />
        </button>
        <div className="flex-1 flex items-center justify-center gap-1.5 text-[13px] text-white/90 min-w-0">
          <span className="whitespace-nowrap">
            {total} file change{total === 1 ? "" : "s"} on
          </span>
          <span className="px-1.5 rounded-[3px] bg-[#195f71] text-white truncate">{branch}</span>
        </div>
        <AiButton title="AI features come in Stage 6" />
      </PanelHeader>
      <ViewSwitch />
      <div className="flex-1 overflow-y-auto min-h-[60px]">
        <SectionHead open={unstagedOpen} onToggle={() => setUnstagedOpen(!unstagedOpen)} title={`Unstaged Files (${unstaged.length})`}>
          {unstaged.length > 0 && (
            <Button size="sm" variant="success" onClick={() => act(repo.path, { type: "stageAll" }, true)} title={`${MOD}+Shift+S`}>
              Stage All Changes
            </Button>
          )}
        </SectionHead>
        {unstagedOpen && (
          <FileList
            files={unstaged}
            activeFile={ui.diff?.source.kind === "Unstaged" ? ui.diff.file : undefined}
            onOpen={(f) => openDiff(f, false)}
            menu={(f) => fileMenu(repo, f.path, false, false)}
            action={(f) => (
              <Button size="sm" variant="success" className="h-5 px-1.5 text-[11px]" onClick={(e) => (e.stopPropagation(), act(repo.path, { type: "stage", paths: [f.path] }, true))}>
                Stage File
              </Button>
            )}
          />
        )}
        <SectionHead open={stagedOpen} onToggle={() => setStagedOpen(!stagedOpen)} title={`Staged Files (${staged.length})`}>
          {staged.length > 0 && (
            <Button size="sm" variant="danger" onClick={() => act(repo.path, { type: "unstageAll" }, true)} title={`${MOD}+Shift+U`}>
              Unstage All Changes
            </Button>
          )}
        </SectionHead>
        {stagedOpen && (
          <FileList
            files={staged}
            activeFile={ui.diff?.source.kind === "Staged" ? ui.diff.file : undefined}
            onOpen={(f) => openDiff(f, true)}
            menu={(f) => fileMenu(repo, f.path, true, false)}
            action={(f) => (
              <Button size="sm" variant="danger" className="h-5 px-1.5 text-[11px]" onClick={(e) => (e.stopPropagation(), act(repo.path, { type: "unstage", paths: [f.path] }, true))}>
                Unstage File
              </Button>
            )}
          />
        )}
      </div>
      <div className="flex justify-center py-[8px] shrink-0">
        <div className="w-[100px] h-[4px] rounded bg-white/10" />
      </div>
      <div className="flex items-end px-[8px] gap-0.5 shrink-0 border-b border-white/10">
        <TabBtn active={tab === "commit"} onClick={() => setTab("commit")} title="Commit">
          <GitCommitHorizontal size={15} /> Commit
        </TabBtn>
        <TabBtn active={tab === "stash"} onClick={() => setTab("stash")} title="Stash">
          <Download size={15} />
          {tab === "stash" && " Stash"}
        </TabBtn>
        <TabBtn active={false} onClick={() => s.toast(true, "Cloud Patches come in Stage 6")} title="Cloud Patch (Stage 6)">
          <CloudUpload size={15} />
        </TabBtn>
      </div>
      <div className="px-[8px] pt-[10px] pb-[8px] shrink-0 flex flex-col gap-[8px]">
        {tab === "commit" ? (
          <>
            <label className="flex items-center gap-2 text-[13.5px] text-white/85 cursor-pointer">
              <input type="checkbox" checked={draft.amend} onChange={(e) => toggleAmend(e.target.checked)} className="w-[13px] h-[13px] accent-[#80B1EA]" />
              Amend previous commit
            </label>
            <div className="bg-field rounded-[3px] border border-white/10 focus-within:border-accent/70 flex flex-col">
              <div className="flex items-center pl-[10px] pr-[6px] pt-[6px]">
                <input
                  className="commit-message-input flex-1 bg-transparent outline-none text-[15px] text-white placeholder:text-white/70 min-w-0"
                  placeholder="Commit summary"
                  value={draft.summary}
                  data-commit-summary
                  onChange={(e) => s.setDraft(repo.path, { summary: e.target.value })}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                      e.preventDefault();
                      if (e.shiftKey) doCommit(true);
                      else if (commitEnabled) doCommit(false);
                    }
                  }}
                />
                <span className={cn("text-[14px] mx-2 tabular-nums", left < 0 ? "text-[#ef7a76]" : "text-white/60")}>{left}</span>
                <AiButton title="Generate a commit message with AI (Stage 6)" />
              </div>
              <textarea
                className="commit-message-input bg-transparent outline-none resize-none text-[13.5px] text-white/90 placeholder:text-white/70 px-[10px] py-[6px] h-[84px]"
                placeholder="Description"
                value={draft.description}
                onChange={(e) => s.setDraft(repo.path, { description: e.target.value })}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                    e.preventDefault();
                    if (e.shiftKey) doCommit(true);
                    else if (commitEnabled) doCommit(false);
                  }
                }}
              />
            </div>
            <div className="flex items-center">
              <button onClick={() => setOptionsOpen(!optionsOpen)} className="flex items-center gap-1.5 text-[13.5px] text-white/70 hover:text-white">
                {optionsOpen ? <ChevronDown size={14} /> : <ChevronRight size={14} />} Commit options
              </button>
              <div className="flex-1" />
              <Button size="sm" variant="ai" className="h-[24px] font-semibold" title="AI features come in Stage 6" onClick={() => s.toast(true, "Composing commits with AI comes in Stage 6")}>
                <Sparkles size={12} /> Compose with AI
              </Button>
            </div>
            {optionsOpen && (
              <label className="flex items-center gap-2 text-[12.5px] text-white/80 pl-5 cursor-pointer">
                <input type="checkbox" checked={push} onChange={(e) => s.setSettings({ pushAfterCommit: e.target.checked })} className="accent-[#80B1EA]" />
                Push after committing
              </label>
            )}
          </>
        ) : (
          <div className="bg-field rounded-[3px] border border-white/10 flex flex-col">
            <input
              className="bg-transparent outline-none text-[15px] text-white placeholder:text-white/70 px-[10px] pt-[8px] pb-[6px]"
              placeholder="Stash title"
              value={stashMsg}
              onChange={(e) => setStashMsg(e.target.value)}
            />
            <div className="text-[12px] text-fg3 px-[10px] pb-[8px]">Staged and unstaged changes, including new files, are stashed.</div>
          </div>
        )}
        <button
          disabled={!commitEnabled}
          onClick={() => (tab === "stash" ? act(repo.path, { type: "stash", message: stashMsg }).then(() => setStashMsg("")) : doCommit(false))}
          className={cn(
            "h-[40px] rounded-[3px] border text-[13.5px] flex items-center justify-center gap-2 font-semibold",
            commitEnabled ? "bg-good/25 border-good text-white hover:bg-good/35" : "bg-[#2c3935] border-[#3b4a44] text-white/45",
          )}
        >
          <GitCommitHorizontal size={15} />
          {commitLabel}
        </button>
      </div>
    </div>
  );
}

function SectionHead({ open, onToggle, title, children }: { open: boolean; onToggle: () => void; title: string; children?: ReactNode }) {
  return (
    <div className="flex items-center h-[34px] pl-[6px] pr-[8px] gap-1 border-b border-white/5">
      <button onClick={onToggle} className="flex items-center gap-1 text-[14px] text-white/90 flex-1 text-left">
        {open ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
        {title}
      </button>
      {children}
    </div>
  );
}

function TabBtn({ active, onClick, children, title }: { active: boolean; onClick: () => void; children: ReactNode; title: string }) {
  return (
    <button
      title={title}
      onClick={onClick}
      className={cn("flex items-center gap-1.5 h-[30px] px-[10px] text-[14px] rounded-t-[3px]", active ? "bg-field text-white border border-white/10 border-b-transparent" : "text-white/70 hover:text-white")}
    >
      {children}
    </button>
  );
}

function CommitDetails({ repo, tabId, commit: c }: { repo: RepoView; tabId: string; commit: Commit }) {
  const s = useStore();
  const ui = useTabUi(tabId);
  const files = useQuery({ queryKey: ["commitFiles", repo.path, c.sha], queryFn: () => api.commitFiles(repo.path, c.sha), staleTime: Infinity });
  const counts = useMemo(() => {
    const f = files.data ?? [];
    return { m: f.filter((x) => x.status === "M" || x.status === "R").length, a: f.filter((x) => x.status === "A").length, d: f.filter((x) => x.status === "D").length };
  }, [files.data]);

  return (
    <div className="h-full flex flex-col bg-panel min-w-0">
      <PanelHeader>
        <span className="text-[12.5px] text-fg2">commit:</span>
        <button className="mono text-[12.5px] text-link hover:underline" title="Copy commit sha" onClick={() => copy(c.sha)}>
          {c.sha.slice(0, s.metrics.sha_len)}
        </button>
        <button className="text-fg3 hover:text-white" onClick={() => copy(c.sha)} title="Copy commit sha">
          <Copy size={12} />
        </button>
        <div className="flex-1" />
        <AiButton title="Explain this commit with AI (Stage 6)" />
        <button className="text-fg2 hover:text-white" title="Close (Esc)" onClick={() => s.setUi(tabId, { selection: { type: "wip" }, diff: null })}>
          <X size={15} />
        </button>
      </PanelHeader>
      <div className="px-3 pt-3 pb-2 shrink-0 select-text">
        <div className="text-[15px] text-white font-semibold leading-snug break-words">{c.summary}</div>
        {c.body && <div className="text-[13px] text-white/75 mt-2 whitespace-pre-wrap break-words max-h-[120px] overflow-auto">{c.body}</div>}
      </div>
      <div className="flex items-center gap-2 px-3 py-2 border-y border-white/5 shrink-0">
        <Avatar name={c.author} email={c.email} size={28} />
        <div className="flex flex-col min-w-0 flex-1">
          <span className="text-[13px] text-white truncate">{c.author}</span>
          <span className="text-[11.5px] text-fg3 truncate">authored {fullDate(c.time)}</span>
        </div>
        <div className="flex flex-col items-end text-[11.5px] text-fg3">
          {c.parents.map((p) => (
            <button key={p} className="hover:text-white" onClick={() => s.setUi(tabId, { selection: { type: "commit", sha: p }, diff: null })} title="Jump to parent commit">
              parent: <span className="mono text-link">{p.slice(0, s.metrics.sha_len)}</span>
            </button>
          ))}
        </div>
      </div>
      <div className="flex items-center gap-3 px-3 h-[32px] text-[12px] shrink-0">
        {counts.m > 0 && <span className="text-[#de9b43]">✎ {counts.m} modified</span>}
        {counts.a > 0 && <span className="text-good">+ {counts.a} added</span>}
        {counts.d > 0 && <span className="text-[#ef7a76]">− {counts.d} deleted</span>}
        <div className="flex-1" />
      </div>
      <ViewSwitch />
      <div className="flex-1 overflow-y-auto">
        {files.isLoading && <div className="px-3 text-fg3 text-[12px]">Loading files…</div>}
        <FileList
          files={files.data ?? []}
          activeFile={ui.diff?.source.kind === "Commit" ? ui.diff.file : undefined}
          onOpen={(f) => s.setUi(tabId, { diff: { source: { kind: "Commit", sha: c.sha }, file: f.path, untracked: false, parent: c.parents[0] ?? null } })}
          menu={(f) => fileMenu(repo, f.path, false, true)}
        />
      </div>
    </div>
  );
}
