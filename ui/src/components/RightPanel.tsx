import { useQuery } from "@tanstack/react-query";
import {
  ArrowDownAZ,
  ChevronDown,
  ChevronRight,
  FileDown,
  FileUp,
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
import { useEffect, useMemo, useState, type ReactNode } from "react";
import type { Commit, FileChange, RepoView } from "@/lib/api";
import { api, WORKDIR } from "@/lib/api";
import { useStore, useTabUi, type DiffTarget, usePanelStore } from "@/lib/store";
import { act, confirmThen, copy, WIP_COMMIT_EVENT } from "@/lib/repo";
import { fileMenu } from "@/lib/menus";
import { baseFor, dropCommits, headLine, openCherryPick, openRebase, squashCommits } from "@/lib/rewrite";
import { CtxMenu } from "./ui/menu";
import { Button } from "./ui/button";
import { Collapsible } from "./ui/collapsible";
import { Avatar, StatusIcon } from "./Icons";
import { cn, fullDate, mix, MOD, withCoauthors } from "@/lib/utils";
import { applyPatch, composeCommits, explainCommit, generateCommitMessage, generateStashTitle, patchName, savePatch } from "@/lib/ai";

import { workingChanges } from "@/lib/working-changes";

/** Right panel: staging and committing (WIP) or the details of a selected commit. */
export function RightPanel({ repo, tabId }: { repo: RepoView; tabId: string }) {
  const ui = useTabUi(tabId);
  const setUi = useStore((s) => s.setUi);
  const { total } = workingChanges(repo.status);
  const inspecting = ui.selection.type === "commit" || ui.multi.length > 1 || !!ui.compareWorkdir;
  return (
    <div className="h-full flex flex-col bg-panel min-w-0">
      {inspecting && total > 0 && (
        <div className="flex items-center justify-center flex-wrap gap-x-2 gap-y-1 px-2 py-2 shrink-0 bg-accent/40 text-white text-[12px]" role="status">
          <span>{total} file change{total === 1 ? "" : "s"} in working directory</span>
          <button
            className="border border-white/40 rounded-[2px] px-1.5 py-0.5 hover:bg-white/15 focus-visible:outline focus-visible:outline-1 focus-visible:outline-white"
            onClick={() => setUi(tabId, { selection: { type: "wip" }, multi: [], compareWorkdir: null, diff: null, history: null, conflict: null, pr: null, issue: null })}
          >View Changes</button>
        </div>
      )}
      <div className="flex-1 min-h-0"><SelectionPanel repo={repo} tabId={tabId} /></div>
    </div>
  );
}

function SelectionPanel({ repo, tabId }: { repo: RepoView; tabId: string }) {
  const ui = useTabUi(tabId);
  if (ui.compareWorkdir) return <ComparePanel repo={repo} tabId={tabId} from={ui.compareWorkdir} to={WORKDIR} />;
  if (ui.multi.length === 2) {
    // Older commit first, so the diff reads "from -> to".
    const pos = (x: string) => repo.commits.findIndex((c) => c.sha === x);
    const [a, b] = [...ui.multi].sort((x, y) => pos(y) - pos(x));
    return <ComparePanel repo={repo} tabId={tabId} from={a} to={b} />;
  }
  if (ui.multi.length > 2) return <MultiPanel repo={repo} tabId={tabId} />;
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

function AiButton({ title, onClick }: { title: string; onClick: () => void }) {
  return (
    <button title={title} onClick={onClick} className="w-[26px] h-[24px] rounded-[3px] border border-ai/80 bg-ai/15 text-white/80 flex items-center justify-center hover:bg-ai/25 shrink-0">
      <Sparkles size={13} />
    </button>
  );
}

function ViewSwitch() {
  const s = usePanelStore();
  const v = s.settings.fileView;
  return (
    <div className="flex items-center h-[38px] px-[8px] shrink-0">
      <button
        className="text-white/75 hover:text-white"
        title={s.settings.fileSort === "status" ? "Sorted by status (click to sort by path)" : "Sorted by path (click to sort by status)"}
        onClick={() => s.setSettings({ fileSort: s.settings.fileSort === "status" ? "path" : "status" })}
      >
        <ArrowDownAZ size={16} />
      </button>
      <div className="flex-1 flex justify-center">
        <div className="flex h-[24px] rounded-[3px] overflow-hidden border border-white/10 text-[13px]">
          <button onClick={() => s.setSettings({ fileView: "path" })} className={cn("px-3 flex items-center gap-1.5", v === "path" ? "bg-white/15 text-white" : "bg-white/5 text-fg2 hover:text-white")}>
            <List size={14} /> Path
          </button>
          <button onClick={() => s.setSettings({ fileView: "tree" })} className={cn("px-3 flex items-center gap-1.5", v === "tree" ? "bg-white/15 text-white" : "bg-white/5 text-fg2 hover:text-white")}>
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
  const sortBy = useStore((s) => s.settings.fileSort);
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const sorted = useMemo(
    () => [...files].sort((a, b) => (sortBy === "status" ? a.status.localeCompare(b.status) || a.path.localeCompare(b.path) : a.path.localeCompare(b.path))),
    [files, sortBy],
  );
  const row = (f: FileChange, indent: number, label: string) => (
    <CtxMenu key={f.path} items={() => menu(f)}>
      <div
        className={cn("group flex items-center gap-[6px] h-[24px] pr-2 text-[12.5px] cursor-default hover:bg-hoverrow", activeFile === f.path && "bg-selrow")}
        style={{ paddingLeft: 12 + indent * 14 }}
        title={f.path}
      >
        <button type="button" onClick={() => onOpen(f)} className="flex flex-1 min-w-0 self-stretch items-center gap-[6px] text-left cursor-default">
          <StatusIcon status={f.status} />
          <span className="truncate flex-1 text-white/85">{label}</span>
        </button>
        {action && <span className="opacity-0 group-hover:opacity-100 group-focus-within:opacity-100">{action(f)}</span>}
      </div>
    </CtxMenu>
  );
  if (view === "path") return <div>{sorted.map((f) => row(f, 0, f.path))}</div>;

  // Tree view: nested folders; a closed folder shows how many files changed inside it.
  const root = buildTree(sorted);
  const allDirs: string[] = [];
  const collect = (n: TreeNode) => n.dirs.forEach((d) => (allDirs.push(d.path), collect(d)));
  collect(root);
  const anyOpen = allDirs.some((d) => !collapsed[d]);
  const render = (n: TreeNode, depth: number): ReactNode[] => {
    const out: ReactNode[] = [];
    for (const d of [...n.dirs.values()].sort((a, b) => a.name.localeCompare(b.name))) {
      const closed = !!collapsed[d.path];
      const counts = countFiles(d);
      out.push(
        <button
          type="button"
          key={"d:" + d.path}
          aria-expanded={!closed}
          className="flex w-full items-center gap-[6px] h-[24px] pr-2 text-left text-[12.5px] text-fg2 hover:bg-hoverrow cursor-default"
          style={{ paddingLeft: 10 + depth * 14 }}
          onClick={() => setCollapsed((c) => ({ ...c, [d.path]: !c[d.path] }))}
        >
          <ChevronRight size={12} className={cn("transition-transform shrink-0", !closed && "rotate-90")} />
          <Folder size={13} className="shrink-0" />
          <span className="truncate">{d.name}</span>
          {closed && (
            <span className="flex gap-1.5 text-[11.5px] font-semibold">
              {counts.M ? <span className="text-[#de9b43]">✎ {counts.M}</span> : null}
              {counts.A ? <span className="text-good">+ {counts.A}</span> : null}
              {counts.D ? <span className="text-[#ef7a76]">− {counts.D}</span> : null}
            </span>
          )}
        </button>,
      );
      if (!closed) out.push(...render(d, depth + 1));
    }
    for (const f of n.files) out.push(row(f, depth, f.path.split("/").pop()!));
    return out;
  };
  return (
    <div>
      {allDirs.length > 0 && (
        <button
          className="block pl-[12px] h-[20px] text-[11.5px] text-fg2 hover:text-white"
          onClick={() => setCollapsed(anyOpen ? Object.fromEntries(allDirs.map((d) => [d, true])) : {})}
        >
          {anyOpen ? "Collapse All" : "Expand All"}
        </button>
      )}
      {render(root, 0)}
    </div>
  );
}

interface TreeNode {
  name: string;
  path: string;
  dirs: Map<string, TreeNode>;
  files: FileChange[];
}

function buildTree(files: FileChange[]): TreeNode {
  const root: TreeNode = { name: "", path: "", dirs: new Map(), files: [] };
  for (const f of files) {
    let node = root;
    for (const part of f.path.split("/").slice(0, -1)) {
      const p = node.path ? `${node.path}/${part}` : part;
      if (!node.dirs.has(part)) node.dirs.set(part, { name: part, path: p, dirs: new Map(), files: [] });
      node = node.dirs.get(part)!;
    }
    node.files.push(f);
  }
  return root;
}

function countFiles(n: TreeNode): Record<string, number> {
  const out: Record<string, number> = {};
  const walk = (x: TreeNode) => {
    for (const f of x.files) {
      const k = f.status === "?" ? "A" : f.status === "R" || f.status === "U" ? "M" : f.status;
      out[k] = (out[k] ?? 0) + 1;
    }
    x.dirs.forEach(walk);
  };
  walk(n);
  return out;
}

function WorkPanel({ repo, tabId }: { repo: RepoView; tabId: string }) {
  const s = usePanelStore();
  const ui = useTabUi(tabId);
  const draft = s.drafts[repo.path] ?? { summary: "", description: "", amend: false };
  const [tab, setTab] = useState<"commit" | "stash" | "patch">("commit");
  const [optionsOpen, setOptionsOpen] = useState(false);
  const [unstagedOpen, setUnstagedOpen] = useState(true);
  const [stagedOpen, setStagedOpen] = useState(true);
  const [stashMsg, setStashMsg] = useState("");
  const conflicted = repo.status.unstaged.filter((f) => f.status === "U");
  const unstaged = repo.status.unstaged.filter((f) => f.status !== "U");
  const staged = repo.status.staged;
  const { total } = workingChanges(repo.status);
  const merging = repo.state_note === "merging";
  const [cur, inc] = repo.conflict_labels ?? [repo.head_branch ?? "HEAD", "incoming"];
  // Commit template (Preferences > Commit): fill an empty, untouched commit box.
  useEffect(() => {
    const t = s.settings;
    if (t.useCommitTemplate !== false && !merging && !draft.summary && !draft.description && !draft.amend && (t.templateSummary || t.templateDescription))
      s.setDraft(repo.path, { summary: t.templateSummary, description: t.templateDescription });
  }, [repo.path, merging, draft.summary, draft.description]);
  // Prefill the commit box with the message git prepared for the merge (MERGE_MSG).
  useEffect(() => {
    if (merging && repo.merge_msg && !draft.summary && !draft.description) {
      const [first, ...rest] = repo.merge_msg.split("\n");
      s.setDraft(repo.path, { summary: first, description: rest.join("\n").trim() });
    }
  }, [merging, repo.merge_msg]);
  const branch = repo.head_branch ?? repo.head_sha.slice(0, s.metrics.sha_len);
  const left = s.metrics.summary_counter - draft.summary.length; // the original's 72-character summary counter
  const push = s.settings.pushAfterCommit;
  const [skipHooks, setSkipHooks] = useState(s.settings.skipHooks);
  const [coauthors, setCoauthors] = useState("");
  const knownAuthors = useMemo(() => [...new Set(repo.commits.map((c) => `${c.author} <${c.email}>`))].slice(0, 200), [repo.commits]);

  const openDiff = (f: FileChange, isStaged: boolean) => {
    const target: DiffTarget = { source: { kind: isStaged ? "Staged" : "Unstaged" }, file: f.path, untracked: f.status === "?", parent: null };
    s.setUi(tabId, { diff: target });
  };

  const doCommit = async (stageAll: boolean) => {
    const description = withCoauthors(draft.description, coauthors);
    const ok = await act(repo.path, { type: "commit", summary: draft.summary, description, amend: draft.amend, stageAll, skipHooks });
    if (ok) {
      setCoauthors("");
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
  if (tab === "patch") {
    commitLabel = total === 0 ? "No Changes for a Patch" : `Save ${total} File Change${total === 1 ? "" : "s"} as a Patch`;
    commitEnabled = total > 0;
  } else if (tab === "stash") {
    commitLabel = total === 0 ? "No Changes to Stash" : `Stash Changes to ${total} File${total === 1 ? "" : "s"}`;
    commitEnabled = total > 0;
  } else if (conflicted.length > 0) {
    commitLabel = `Resolve ${conflicted.length} Conflict${conflicted.length === 1 ? "" : "s"} to Commit`;
    commitEnabled = false;
  } else if (merging && draft.summary.trim()) {
    commitLabel = "Commit and Merge";
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

  // Ctrl+Enter / Ctrl+Shift+Enter in the graph's "// WIP" box commit through this panel, with the same checks.
  const onWipCommit = (e: Event) => {
    const d = (e as CustomEvent<{ path: string; all: boolean }>).detail;
    if (d?.path !== repo.path || tab !== "commit") return;
    if (d.all) void doCommit(true);
    else if (commitEnabled) void doCommit(false);
  };
  useEffect(() => {
    window.addEventListener(WIP_COMMIT_EVENT, onWipCommit);
    return () => window.removeEventListener(WIP_COMMIT_EVENT, onWipCommit);
  });

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
          <span className="px-1.5 rounded-[3px] text-white truncate" style={{ background: mix("#15a0bf", 0.45) }}>{branch}</span>
        </div>
        <AiButton title="Compose commits with AI" onClick={() => composeCommits(repo.path)} />
      </PanelHeader>
      <ViewSwitch />
      <div className="flex-1 overflow-y-auto min-h-[60px]">
        {(conflicted.length > 0 || repo.state_note) && (
          <div className="mx-2 mt-1 mb-2 rounded-[3px] border border-bad/50 bg-bad/10">
            <div className="px-2 py-1.5 text-[12px] text-white/90 border-b border-bad/30">
              {repo.state_note === "rebasing" ? `Rebasing ${inc} onto ${cur}` : merging ? `Merging ${inc} into ${cur}` : `${repo.state_note ?? "Conflicts"} in progress`}
              {conflicted.length > 0 && (
                <div className="text-[11.5px] text-[#ef7a76]">
                  {conflicted.length === 1 ? "A file conflict was found" : `${conflicted.length} file conflicts were found`}
                </div>
              )}
            </div>
            {conflicted.length > 0 && (
              <>
                <div className="flex items-center gap-1 px-2 py-1">
                  <span className="text-[12.5px] text-white/90 flex-1">Conflicted Files ({conflicted.length})</span>
                  <Button size="sm" className="h-[20px] text-[11px]" onClick={() => act(repo.path, { type: "markResolved", paths: conflicted.map((f) => f.path) })}>
                    Mark All Resolved
                  </Button>
                </div>
                {conflicted.map((f) => (
                  <CtxMenu
                    key={f.path}
                    items={[
                      { label: "Open in the merge tool", onSelect: () => s.setUi(tabId, { conflict: f.path, diff: null }) },
                      { label: `Take current (${cur})`, onSelect: () => act(repo.path, { type: "takeSide", paths: [f.path], side: "ours" }) },
                      { label: `Take incoming (${inc})`, onSelect: () => act(repo.path, { type: "takeSide", paths: [f.path], side: "theirs" }) },
                      { separator: true },
                      { label: "Mark resolved", onSelect: () => act(repo.path, { type: "markResolved", paths: [f.path] }) },
                    ]}
                  >
                    <div
                      className={cn("group flex items-center gap-[6px] h-[24px] px-2 text-[12.5px] cursor-default hover:bg-hoverrow", ui.conflict === f.path && "bg-selrow")}
                    >
                      <button type="button" onClick={() => s.setUi(tabId, { conflict: f.path, diff: null })} className="flex flex-1 min-w-0 self-stretch items-center gap-[6px] text-left cursor-default" title="Open in the merge tool">
                        <StatusIcon status="U" />
                        <span className="truncate flex-1 text-white/85">{f.path}</span>
                      </button>
                      <span className="opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 flex gap-1">
                        <Button size="sm" className="h-5 px-1.5 text-[11px]" title={`Take current (${cur})`} onClick={() => act(repo.path, { type: "takeSide", paths: [f.path], side: "ours" })}>
                          Current
                        </Button>
                        <Button size="sm" className="h-5 px-1.5 text-[11px]" title={`Take incoming (${inc})`} onClick={() => act(repo.path, { type: "takeSide", paths: [f.path], side: "theirs" })}>
                          Incoming
                        </Button>
                      </span>
                    </div>
                  </CtxMenu>
                ))}
              </>
            )}
          </div>
        )}
        <SectionHead open={unstagedOpen} onToggle={() => setUnstagedOpen(!unstagedOpen)} title={`Unstaged Files (${unstaged.length})`}>
          {unstaged.length > 0 && (
            <Button size="sm" variant="success" onClick={() => act(repo.path, { type: "stageAll" }, true)} title={`${MOD}+Shift+S`}>
              Stage All Changes
            </Button>
          )}
        </SectionHead>
        <Collapsible open={unstagedOpen}>
          {unstaged.length === 0 && <div className="px-3 py-2 text-[12px] text-fg3">{total === 0 ? "Working directory clean" : "No unstaged changes"}</div>}
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
        </Collapsible>
        <SectionHead open={stagedOpen} onToggle={() => setStagedOpen(!stagedOpen)} title={`Staged Files (${staged.length})`}>
          {staged.length > 0 && (
            <Button size="sm" variant="danger" onClick={() => act(repo.path, { type: "unstageAll" }, true)} title={`${MOD}+Shift+U`}>
              Unstage All Changes
            </Button>
          )}
        </SectionHead>
        <Collapsible open={stagedOpen}>
          {staged.length === 0 && <div className="px-3 py-2 text-[12px] text-fg3">Stage files above to prepare a commit.</div>}
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
        </Collapsible>
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
        <TabBtn active={tab === "patch"} onClick={() => setTab("patch")} title="Patch">
          <FileDown size={15} />
          {tab === "patch" && " Patch"}
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
                  spellCheck={s.settings.spellcheck}
                  value={draft.summary}
                  data-commit-summary
                  onChange={(e) => s.setDraft(repo.path, { summary: e.target.value })}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                      e.preventDefault();
                      if (e.shiftKey || commitEnabled) void doCommit(e.shiftKey);
                    }
                  }}
                />
                <span className={cn("text-[14px] mx-2 tabular-nums", left < 0 ? "text-[#ef7a76]" : "text-white/60")}>{left}</span>
                <AiButton title="Generate a commit message with AI" onClick={() => generateCommitMessage(repo.path)} />
              </div>
              <textarea
                className="commit-message-input bg-transparent outline-none resize-none text-[13.5px] text-white/90 placeholder:text-white/70 px-[10px] py-[6px] h-[84px]"
                placeholder="Description"
                spellCheck={s.settings.spellcheck}
                value={draft.description}
                onChange={(e) => s.setDraft(repo.path, { description: e.target.value })}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                    e.preventDefault();
                    if (e.shiftKey || commitEnabled) void doCommit(e.shiftKey);
                  }
                }}
              />
            </div>
            <div className="flex items-center">
              <button aria-expanded={optionsOpen} onClick={() => setOptionsOpen(!optionsOpen)} className="flex items-center gap-1.5 text-[13.5px] text-white/70 hover:text-white">
                {optionsOpen ? <ChevronDown size={14} /> : <ChevronRight size={14} />} Commit options
              </button>
              <div className="flex-1" />
              <Button size="sm" variant="ai" className="h-[24px] font-semibold" title="Split your changes into focused commits with AI" onClick={() => composeCommits(repo.path)}>
                <Sparkles size={12} /> Compose with AI
              </Button>
            </div>
            <Collapsible open={optionsOpen}>
              <div className="flex flex-col gap-2">
              <label className="flex items-center gap-2 text-[12.5px] text-white/80 pl-5 cursor-pointer">
                <input type="checkbox" checked={push} onChange={(e) => s.setSettings({ pushAfterCommit: e.target.checked })} className="accent-[#80B1EA]" />
                Push after committing
              </label>
              <label className="flex items-center gap-2 text-[12.5px] text-white/80 pl-5" title="Adds a Co-authored-by line for each person (separate several with commas)">
                Co-authors:
                <input
                  list="gitcraft-authors"
                  value={coauthors}
                  onChange={(e) => setCoauthors(e.target.value)}
                  placeholder="Name <email>, …"
                  className="flex-1 min-w-0 h-[22px] px-1.5 bg-field rounded-[2px] border border-white/10 outline-none focus:border-accent text-[12px]"
                />
                <datalist id="gitcraft-authors">
                  {knownAuthors.map((a) => (
                    <option key={a} value={a} />
                  ))}
                </datalist>
              </label>
              <label className="flex items-center gap-2 text-[12.5px] text-white/80 pl-5 cursor-pointer">
                <input type="checkbox" checked={skipHooks} onChange={(e) => setSkipHooks(e.target.checked)} className="accent-[#80B1EA]" />
                Skip Git hooks
              </label>
              </div>
            </Collapsible>
          </>
        ) : tab === "patch" ? (
          <div className="bg-field rounded-[3px] border border-white/10 flex flex-col px-[10px] py-[8px] gap-2">
            <div className="text-[12.5px] text-fg2 leading-snug">
              Share your uncommitted changes as a .patch file, without pushing a branch. Untracked files must be staged first. Anyone can apply it with
              GitCraft or <span className="mono">git apply</span>.
            </div>
            <button className="self-start flex items-center gap-1.5 text-[12.5px] text-link hover:underline" onClick={() => applyPatch(repo.path)}>
              <FileUp size={13} /> Apply a patch file…
            </button>
          </div>
        ) : (
          <div className="bg-field rounded-[3px] border border-white/10 flex flex-col">
            <div className="flex items-center pr-[6px]">
              <input
                className="flex-1 min-w-0 bg-transparent outline-none text-[15px] text-white placeholder:text-white/70 px-[10px] pt-[8px] pb-[6px]"
                placeholder="Stash title"
                value={stashMsg}
                onChange={(e) => setStashMsg(e.target.value)}
              />
              <AiButton
                title="Name the stash with AI"
                onClick={async () => {
                  const t = await generateStashTitle(repo.path);
                  if (t) setStashMsg(t);
                }}
              />
            </div>
            <div className="text-[12px] text-fg3 px-[10px] pb-[8px]">Staged and unstaged changes, including new files, are stashed.</div>
          </div>
        )}
        <button
          disabled={!commitEnabled}
          onClick={() =>
            tab === "patch"
              ? savePatch(repo.path, [], patchName(draft.summary || branch))
              : tab === "stash"
                ? act(repo.path, { type: "stash", message: stashMsg }).then((ok) => { if (ok) setStashMsg(""); })
                : doCommit(false)
          }
          className={cn(
            "h-[40px] rounded-[3px] border text-[13.5px] flex items-center justify-center gap-2 font-semibold",
            commitEnabled ? "bg-good/25 border-good text-white hover:bg-good/35" : "bg-good/10 border-good/25 text-white/45",
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
      <button aria-expanded={open} onClick={onToggle} className="flex items-center gap-1 text-[13px] text-white/90 flex-1 min-w-0 text-left">
        <ChevronRight size={15} className={cn("shrink-0 transition-transform", open && "rotate-90")} />
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
  const s = usePanelStore();
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
        <AiButton title="Explain this commit with AI" onClick={() => explainCommit(repo.path, c.sha, c.summary)} />
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
        {files.isLoading && <div className="px-3 text-fg3 text-[12px]" role="status">Loading files…</div>}
        {files.isError && <div className="px-3 py-2 text-[12px] text-[#ef7a76]" role="alert">
          {String(files.error)} <button className="text-link hover:underline" onClick={() => files.refetch()}>Retry</button>
        </div>}
        {files.isSuccess && files.data.length === 0 && <div className="px-3 py-2 text-[12px] text-fg3">No file changes to show.</div>}
        <FileList
          files={files.data ?? []}
          activeFile={ui.diff?.source.kind === "Commit" ? ui.diff.file : undefined}
          onOpen={(f) => s.setUi(tabId, { diff: { source: { kind: "Commit", sha: c.sha }, file: f.path, untracked: false, parent: c.parents[0] ?? null } })}
          menu={(f) => fileMenu(repo, f.path, false, true, f.status === "D" ? c.parents[0] : c.sha)}
        />
      </div>
    </div>
  );
}

/** Two commits (or a commit and the working folder): the files that differ and their diffs. */
function ComparePanel({ repo, tabId, from, to }: { repo: RepoView; tabId: string; from: string; to: string }) {
  const s = usePanelStore();
  const ui = useTabUi(tabId);
  const n = s.metrics.sha_len;
  const files = useQuery({
    queryKey: ["range", repo.path, from, to, to === WORKDIR ? JSON.stringify(repo.status) : ""],
    queryFn: () => api.rangeFiles(repo.path, from, to),
  });
  const short = (x: string) => (x === WORKDIR ? "working folder" : x.slice(0, n));
  const close = () => s.setUi(tabId, { multi: [], compareWorkdir: null, diff: null });
  return (
    <div className="h-full flex flex-col bg-panel min-w-0">
      <PanelHeader>
        <span className="text-[12.5px] text-white flex-1 truncate">{to === WORKDIR ? "Viewing diff between a commit and the WIP" : "Viewing diff between 2 commits"}</span>
        <button className="text-fg2 hover:text-white" title="Close (Esc)" onClick={close}>
          <X size={15} />
        </button>
      </PanelHeader>
      <div className="px-3 py-2 text-[12px] text-fg2 border-b border-white/5 shrink-0">
        <span className="mono text-link">{short(from)}</span> → <span className="mono text-link">{short(to)}</span>
        <span className="ml-2 text-fg3">{files.data ? `${files.data.length} file${files.data.length === 1 ? "" : "s"} changed` : ""}</span>
      </div>
      <ViewSwitch />
      <div className="flex-1 overflow-y-auto">
        {files.isLoading && <div className="px-3 text-fg3 text-[12px]" role="status">Loading files…</div>}
        {files.isError && <div className="px-3 py-2 text-[12px] text-[#ef7a76]" role="alert">
          {String(files.error)} <button className="text-link hover:underline" onClick={() => files.refetch()}>Retry</button>
        </div>}
        {files.isSuccess && files.data.length === 0 && <div className="px-3 py-2 text-[12px] text-fg3">No file changes to show.</div>}
        <FileList
          files={files.data ?? []}
          activeFile={ui.diff?.source.kind === "Range" ? ui.diff.file : undefined}
          onOpen={(f) => s.setUi(tabId, { diff: { source: { kind: "Range", sha: [from, to] }, file: f.path, untracked: false, parent: null } })}
          menu={(f) => fileMenu(repo, f.path, false, true, f.status === "D" ? from : to === WORKDIR ? undefined : to)}
        />
      </div>
    </div>
  );
}

/** Three or more commits picked: what can be done with them together. */
function MultiPanel({ repo, tabId }: { repo: RepoView; tabId: string }) {
  const s = usePanelStore();
  const ui = useTabUi(tabId);
  const picked = repo.commits.filter((c) => ui.multi.includes(c.sha));
  const oldestFirst = [...picked].reverse().map((c) => c.sha);
  const line = headLine(repo);
  const onLine = !!repo.head_branch && ui.multi.every((x) => line.includes(x));
  return (
    <div className="h-full flex flex-col bg-panel min-w-0">
      <PanelHeader>
        <span className="text-[12.5px] text-white flex-1">{picked.length} commits selected</span>
        <button className="text-fg2 hover:text-white" title="Clear selection (Esc)" onClick={() => s.setUi(tabId, { multi: [] })}>
          <X size={15} />
        </button>
      </PanelHeader>
      <div className="p-3 text-[12px] text-fg3 leading-relaxed">A combined diff is not shown for this selection. Select exactly 2 commits to compare them.</div>
      <div className="px-3 pb-3 flex flex-wrap gap-2">
        <Button size="sm" variant="primary" onClick={() => act(repo.path, { type: "cherryPickMany", shas: oldestFirst })}>
          Cherry pick {picked.length} commits
        </Button>
        <Button size="sm" onClick={() => openCherryPick(tabId, repo, ui.multi)}>
          Interactive Cherry Pick
        </Button>
        {onLine && (
          <>
            <Button size="sm" onClick={() => squashCommits(repo, ui.multi)}>
              Squash {picked.length} commits
            </Button>
            <Button size="sm" variant="danger" onClick={() => dropCommits(repo, ui.multi)}>
              Drop {picked.length} commits
            </Button>
            <Button size="sm" onClick={() => openRebase(tabId, repo.path, baseFor(repo, oldestFirst[0]), repo.head_branch ?? "HEAD")}>
              Interactive Rebase
            </Button>
          </>
        )}
      </div>
      {!onLine && <div className="px-3 pb-3 text-[11.5px] text-fg3">Squash, drop and interactive rebase work on commits of the checked-out branch.</div>}
      <div className="flex-1 overflow-y-auto border-t border-white/5">
        {picked.map((c) => (
          <div key={c.sha} className="flex items-center gap-2 px-3 h-[30px] text-[12.5px] border-b border-white/5">
            <Avatar name={c.author} email={c.email} size={18} />
            <span className="truncate flex-1 text-white/90">{c.summary}</span>
            <span className="mono text-[11px] text-fg3">{c.sha.slice(0, s.metrics.sha_len)}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
