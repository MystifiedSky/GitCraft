import {
  Archive,
  Bot,
  ChevronLeft,
  ChevronRight,
  Check,
  Cloud,
  CloudCog,
  FlaskConical,
  Folder,
  GitPullRequest,
  Laptop,
  List,
  Package,
  Search,
  Tag,
  Users,
  CircleDot,
  Network,
  Eye,
  EyeOff,
  ChevronsRight,
} from "lucide-react";
import { useMemo, useRef, useState, type ReactNode } from "react";
import type { GitRef, RepoView } from "@/lib/api";
import { useStore, usePanelStore } from "@/lib/store";
import { act, openRepoPath, toggleHidden } from "@/lib/repo";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import {
  addRemote,
  addSubmodule,
  addWorktree,
  editRemote,
  finishGitflow,
  initGitflow,
  lockWorktree,
  openCleanup,
  removeRemote,
  removeWorktree,
  startGitflow,
} from "@/lib/manage";
import { Plus, Lock, Brush, GitFork } from "lucide-react";
import { branchFromIssue, hosting, startPullRequest } from "@/lib/hosting";
import { branchMenu, stashMenu } from "@/lib/menus";
import { CtxMenu } from "./ui/menu";
import { cn } from "@/lib/utils";
import { AgentsView, ConflictCheckRows, PatchRows, useConflictCheck } from "./Stage6Panels";

type SectionKey = "lfs" | "gitflow" | "local" | "remote" | "worktrees" | "stashes" | "tags" | "submodules" | "cloud" | "prs" | "issues" | "teams";

/** Left panel: branches, remotes, worktrees, stashes, tags and the service sections. */
export function LeftPanel({ repo, tabId, onSelectSha }: { repo: RepoView | undefined; tabId: string; onSelectSha: (sha: string) => void }) {
  const s = usePanelStore();
  const [open, setOpen] = useState<Record<SectionKey, boolean>>({} as Record<SectionKey, boolean>);
  const [filter, setFilter] = useState("");
  const filterRef = useRef<HTMLInputElement>(null);
  (window as unknown as { focusLeftFilter?: () => void }).focusLeftFilter = () => filterRef.current?.focus();
  const f = filter.trim().toLowerCase();
  const match = (n: string) => !f || n.toLowerCase().includes(f);

  const locals = useMemo(() => (repo?.refs ?? []).filter((r) => r.kind === "Local" && match(r.name)), [repo, f]);
  const remotes = useMemo(() => (repo?.refs ?? []).filter((r) => r.kind === "Remote" && match(r.name)), [repo, f]);
  const tags = useMemo(() => (repo?.refs ?? []).filter((r) => r.kind === "Tag" && match(r.name)), [repo, f]);
  const stashes = (repo?.stashes ?? []).filter((x) => match(x.message));
  const worktrees = (repo?.worktrees ?? []).filter((w) => match(w.path) || match(w.branch));
  const hiddenList = (repo && s.settings.hidden[repo.path]) || [];
  const soloList = (repo && s.settings.solo[repo.path]) || [];
  // "Viewing N": branches shown in the graph (soloed ones only while soloing).
  const viewing = soloList.length || (repo?.refs ?? []).filter((r) => r.kind !== "Tag" && !hiddenList.includes(r.full)).length;
  const isOpen = (k: SectionKey) => (f ? true : !!open[k]);
  const toggle = (k: SectionKey) => setOpen((o) => ({ ...o, [k]: !o[k] }));
  const conflicts = useConflictCheck(repo).data;

  if (s.settings.leftView === "agents" && s.settings.showAgentsView !== false) {
    return (
      <Shell tabId={tabId}>
        <AgentsView repo={repo} />
      </Shell>
    );
  }

  const branchRow = (r: GitRef, indent: number, label: string) => {
    const isHead = r.kind === "Local" && r.name === repo?.head_branch;
    return (
      <CtxMenu key={r.full} items={() => (repo ? branchMenu(repo, r) : [])}>
        <Row
          indent={indent}
          onClick={() => onSelectSha(r.sha)}
          onDoubleClick={() =>
            repo && !isHead && act(repo.path, r.kind === "Remote" ? { type: "checkoutRemote", remoteBranch: r.name } : { type: "checkout", target: r.name })
          }
          className={isHead ? "bg-good/20 text-white" : undefined}
          title={isHead ? "Checked out" : "Double-click to checkout"}
        >
          {isHead ? <Check size={13} className="shrink-0 text-good" /> : r.kind === "Local" ? <Laptop size={13} className="shrink-0 opacity-60" /> : <Cloud size={13} className="shrink-0 opacity-60" />}
          <span className={cn("truncate flex-1", isHead && "font-semibold")}>{label}</span>
          {soloList.includes(r.full) && <Badge title="Soloing">solo</Badge>}
          {r.ahead > 0 && <Badge title={`${r.ahead} ahead`}>{r.ahead}↑</Badge>}
          {r.behind > 0 && <Badge title={`${r.behind} behind`}>{r.behind}↓</Badge>}
          {repo && (
            <button
              title={hiddenList.includes(r.full) ? "Show in the graph" : "Hide in the graph"}
              onClick={(e) => (e.stopPropagation(), toggleHidden(repo.path, r.full))}
              className={cn("shrink-0 text-fg2 hover:text-white", hiddenList.includes(r.full) ? "opacity-100 text-warn" : "opacity-0 group-hover:opacity-100")}
            >
              {hiddenList.includes(r.full) ? <EyeOff size={13} /> : <Eye size={13} />}
            </button>
          )}
        </Row>
      </CtxMenu>
    );
  };

  // Group local branches by folder ("feature/x" -> folder "feature").
  const localTree = groupByFolder(locals);
  const byRemote = new Map<string, GitRef[]>();
  for (const r of remotes) {
    const [rem] = r.name.split("/");
    byRemote.set(rem, [...(byRemote.get(rem) ?? []), r]);
  }
  for (const rem of repo?.remotes ?? []) if (!byRemote.has(rem) && !f) byRemote.set(rem, []);

  return (
    <Shell tabId={tabId}>
      <div className="px-[14px] pt-[17px] pb-[6px] text-[13px] text-fg">
        Viewing <span className="text-link font-semibold">{viewing}</span>
      </div>
      <div className="px-[9px] pb-[12px]">
        <div className="flex items-center h-[22px] bg-field rounded-[3px] border border-white/10 focus-within:border-accent">
          <input
            ref={filterRef}
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && (setFilter(""), filterRef.current?.blur())}
            placeholder="Filter (Ctrl + Alt + f)"
            className="flex-1 bg-transparent outline-none px-[5px] text-[12.5px] placeholder:text-fg3 min-w-0"
          />
          <Search size={14} className="mr-[5px] text-white/70" />
        </div>
      </div>
      <div className="flex-1 overflow-y-auto overflow-x-hidden pb-4">
        <Section
          title="LOCAL"
          icon={<Laptop size={14} />}
          count={locals.length}
          open={isOpen("local")}
          onToggle={() => toggle("local")}
          actions={<SectionAction title="Clean up local branches" onClick={openCleanup} icon={<Brush size={12} />} />}
        >
          {localTree.map((g) =>
            g.folder ? (
              <div key={g.folder}>
                <Row indent={1}>
                  <Folder size={13} className="opacity-60" />
                  <span className="truncate">{g.folder}</span>
                </Row>
                {g.items.map((r) => branchRow(r, 2, r.name.slice(g.folder!.length + 1)))}
              </div>
            ) : (
              g.items.map((r) => branchRow(r, 1, r.name))
            ),
          )}
        </Section>
        <Section
          title="REMOTE"
          icon={<Cloud size={14} />}
          count={byRemote.size}
          open={isOpen("remote")}
          onToggle={() => toggle("remote")}
          actions={repo && <SectionAction title="Add a remote" onClick={() => addRemote(repo.path)} icon={<Plus size={13} />} />}
        >
          {[...byRemote.entries()].map(([rem, refs]) => (
            <div key={rem}>
              <CtxMenu
                items={() =>
                  repo
                    ? [
                        { label: `Fetch ${rem}`, onSelect: () => act(repo.path, { type: "fetchRemote", remote: rem }) },
                        { label: `Edit ${rem}`, onSelect: () => editRemote(repo.path, rem) },
                        { separator: true },
                        { label: `Remove ${rem}`, danger: true, onSelect: () => removeRemote(repo.path, rem) },
                      ]
                    : []
                }
              >
                <Row indent={1}>
                  <Cloud size={13} className="opacity-60" />
                  <span className="truncate font-semibold">{rem}</span>
                </Row>
              </CtxMenu>
              {refs.map((r) => branchRow(r, 2, r.name.slice(rem.length + 1)))}
            </div>
          ))}
        </Section>
        <Section
          title="WORKTREES"
          icon={<Network size={14} />}
          count={worktrees.length}
          open={isOpen("worktrees")}
          onToggle={() => toggle("worktrees")}
          actions={repo && <SectionAction title="Add a new worktree" onClick={() => addWorktree(repo)} icon={<Plus size={13} />} />}
        >
          {worktrees.map((w, i) => (
            <CtxMenu
              key={w.path}
              items={() =>
                !repo
                  ? []
                  : [
                      { label: "Open worktree", onSelect: () => openRepoPath(w.path) },
                      ...(i === 0
                        ? []
                        : [
                            w.locked
                              ? { label: "Unlock worktree", onSelect: () => act(repo.path, { type: "worktreeUnlock", path: w.path }) }
                              : { label: "Lock worktree", onSelect: () => lockWorktree(repo.path, w.path) },
                            { separator: true },
                            { label: "Remove worktree", danger: true, disabled: w.locked, onSelect: () => removeWorktree(repo.path, w.path) },
                          ]),
                    ]
              }
            >
              <Row indent={1} title={w.path} onDoubleClick={() => i > 0 && openRepoPath(w.path)}>
                {i === 0 ? <Check size={13} className="text-good" /> : <Folder size={13} className="opacity-60" />}
                <span className="truncate">{w.branch || w.path}</span>
                {w.locked && <Lock size={11} className="text-warn shrink-0" />}
                <span className="text-fg3 text-[11px] truncate">{i === 0 ? "main worktree" : w.path}</span>
              </Row>
            </CtxMenu>
          ))}
        </Section>
        {repo && <GitflowSection repo={repo} open={isOpen("gitflow")} onToggle={() => toggle("gitflow")} />}
        {repo && <LfsSection repo={repo} open={isOpen("lfs")} onToggle={() => toggle("lfs")} />}
        {(repo?.stashes.length ?? 0) > 0 && (
          <Section title="STASHES" icon={<Archive size={14} />} count={stashes.length} open={isOpen("stashes")} onToggle={() => toggle("stashes")}>
            {stashes.map((st) => (
              <CtxMenu key={st.sha} items={() => (repo ? stashMenu(repo, st) : [])}>
                <Row indent={1} title={st.message} onDoubleClick={() => repo && act(repo.path, { type: "stashApply", index: st.index })}>
                  <Archive size={13} className="opacity-60" />
                  <span className="truncate">{st.message}</span>
                </Row>
              </CtxMenu>
            ))}
          </Section>
        )}
        <Section title="PATCHES" icon={<CloudCog size={14} />} open={isOpen("cloud")} onToggle={() => toggle("cloud")}>
          {repo ? <PatchRows repo={repo} /> : <Note>Open a repository to save or apply patch files.</Note>}
        </Section>
        {repo ? (
          <HostedLists repo={repo} tabId={tabId} prsOpen={isOpen("prs")} issuesOpen={isOpen("issues")} toggle={toggle} />
        ) : (
          <>
            <Section title="PULL REQUESTS" icon={<GitPullRequest size={14} />} count={0} open={false} onToggle={() => toggle("prs")} />
            <Section title="ISSUES" icon={<CircleDot size={14} />} open={false} onToggle={() => toggle("issues")} />
          </>
        )}
        <Section
          title="TEAMS"
          icon={<Users size={14} />}
          count={conflicts && conflicts.files.length > 0 ? conflicts.files.length : undefined}
          open={isOpen("teams") || (!!conflicts && conflicts.files.length > 0 && open.teams === undefined)}
          onToggle={() => toggle("teams")}
        >
          {repo ? <ConflictCheckRows repo={repo} /> : <Note>Open a repository to check it for conflicts with its main branch.</Note>}
        </Section>
        {tags.length > 0 && (
          <Section title="TAGS" icon={<Tag size={14} />} count={tags.length} open={isOpen("tags")} onToggle={() => toggle("tags")}>
            {tags.map((t) => (
              <CtxMenu key={t.full} items={() => (repo ? branchMenu(repo, t) : [])}>
                <Row indent={1} onClick={() => onSelectSha(t.sha)}>
                  <Tag size={13} className="opacity-60" />
                  <span className="truncate">{t.name}</span>
                </Row>
              </CtxMenu>
            ))}
          </Section>
        )}
        {(repo?.submodules.length ?? 0) > 0 && repo && (
          <Section
            title="SUBMODULES"
            icon={<Package size={14} />}
            count={repo.submodules.length}
            open={isOpen("submodules")}
            onToggle={() => toggle("submodules")}
            actions={<SectionAction title="Add Submodule" onClick={() => addSubmodule(repo.path)} icon={<Plus size={13} />} />}
          >
            <Submodules repo={repo} />
          </Section>
        )}
        {false && <FlaskConical />}
      </div>
    </Shell>
  );
}

function Shell({ children, tabId }: { children: ReactNode; tabId: string }) {
  const s = usePanelStore();
  void tabId;
  const view = s.settings.leftView;
  return (
    <div className="h-full flex flex-col bg-panel min-w-0">
      <div className="flex items-center h-[34px] mt-[11px] px-[11px] gap-[9px] shrink-0">
        <button
          onClick={() => s.setSettings({ showLeft: false })}
          title="Collapse Panel (Ctrl+J)"
          className="w-[17px] h-[17px] rounded-full bg-white/85 text-panel flex items-center justify-center hover:bg-white shrink-0"
        >
          <ChevronLeft size={13} strokeWidth={3} />
        </button>
        <div className="flex flex-1 h-[24px] rounded-[3px] overflow-hidden border border-white/10">
          <button
            onClick={() => s.setSettings({ leftView: "list" })}
            className={cn("flex-1 flex items-center justify-center gap-1.5 text-[13px]", view === "list" ? "bg-accent/25 text-white border border-accent/70" : "bg-white/5 text-fg2 hover:text-white")}
          >
            <List size={14} /> List
          </button>
          {s.settings.showAgentsView !== false && (
            <button
              onClick={() => s.setSettings({ leftView: "agents" })}
              className={cn("flex-1 flex items-center justify-center gap-1.5 text-[13px]", view === "agents" ? "bg-accent/25 text-white border border-accent/70" : "bg-white/5 text-fg2 hover:text-white")}
            >
              <Bot size={14} /> Agents
            </button>
          )}
        </div>
      </div>
      {children}
    </div>
  );
}

function Section({
  title,
  icon,
  count,
  open,
  onToggle,
  children,
  actions,
}: {
  title: string;
  icon: ReactNode;
  count?: number;
  open: boolean;
  onToggle: () => void;
  children?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <div>
      <div className="group w-full flex items-center h-[30px] pl-[8px] pr-[14px] text-[12.5px] font-semibold text-fg hover:bg-white/5 tracking-[0.01em] cursor-default" onClick={onToggle}>
        <ChevronRight size={14} className={cn("mr-[7px] transition-transform text-white/70", open && "rotate-90")} />
        <span className="mr-[6px] text-white/75">{icon}</span>
        <span className="flex-1 text-left">{title}</span>
        {actions && <span className="opacity-0 group-hover:opacity-100 mr-2 flex items-center" onClick={(e) => e.stopPropagation()}>{actions}</span>}
        {count !== undefined && <span className="font-normal text-fg">{count}</span>}
      </div>
      {open && <div className="pb-1">{children}</div>}
    </div>
  );
}

function Row({
  children,
  indent,
  onClick,
  onDoubleClick,
  className,
  title,
}: {
  children: ReactNode;
  indent: number;
  onClick?: () => void;
  onDoubleClick?: () => void;
  className?: string;
  title?: string;
}) {
  return (
    <div
      title={title}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      className={cn("group flex items-center gap-[7px] h-[24px] pr-3 text-[13px] text-fg hover:bg-hoverrow cursor-default", className)}
      style={{ paddingLeft: 12 + indent * 16 }}
    >
      {children}
    </div>
  );
}

function Badge({ children, title }: { children: ReactNode; title: string }) {
  return (
    <span title={title} className="text-[10.5px] px-1 rounded bg-white/10 text-fg2">
      {children}
    </span>
  );
}

function Note({ children }: { children: ReactNode }) {
  return <div className="pl-[44px] pr-4 py-1 text-[12px] text-fg3 leading-snug">{children}</div>;
}

function groupByFolder(refs: GitRef[]) {
  const out: { folder: string | null; items: GitRef[] }[] = [];
  const folders = new Map<string, GitRef[]>();
  const plain: GitRef[] = [];
  for (const r of refs) {
    const i = r.name.indexOf("/");
    if (i > 0) {
      const f = r.name.slice(0, i);
      folders.set(f, [...(folders.get(f) ?? []), r]);
    } else plain.push(r);
  }
  for (const [folder, items] of folders) out.push({ folder, items });
  out.push({ folder: null, items: plain });
  return out;
}

/** The left panel shrunk to a strip of section icons with counts (shown while a file is open). */
export function LeftStrip({ repo, onExpand }: { repo: RepoView | undefined; onExpand: () => void }) {
  const locals = (repo?.refs ?? []).filter((r) => r.kind === "Local").length;
  const items: [ReactNode, string, number | null][] = [
    [<Laptop size={15} />, "Local", locals],
    [<Cloud size={15} />, "Remote", repo?.remotes.length ?? 0],
    [<Network size={15} />, "Worktrees", repo?.worktrees.length ?? 0],
    [<CloudCog size={15} />, "Cloud Patches", 0],
    [<GitPullRequest size={15} />, "Pull Requests", 0],
    [<CircleDot size={15} />, "Issues", null],
    [<Users size={15} />, "Teams", null],
  ];
  return (
    <div className="w-[28px] shrink-0 bg-panel border-r border-black/30 flex flex-col items-center pt-[9px] gap-[3px]">
      <button onClick={onExpand} title="Expand Panel (Ctrl+J)" className="w-[17px] h-[17px] rounded-full bg-white/85 text-panel flex items-center justify-center hover:bg-white mb-2">
        <ChevronsRight size={12} strokeWidth={3} />
      </button>
      {items.map(([icon, label, n]) => (
        <button key={label} onClick={onExpand} title={label} className="flex flex-col items-center text-white/70 hover:text-white py-1">
          {icon}
          {n !== null && <span className="text-[9.5px] leading-none mt-0.5 text-fg2">{n}</span>}
        </button>
      ))}
    </div>
  );
}

function SectionAction({ title, onClick, icon }: { title: string; onClick: () => void; icon: ReactNode }) {
  return (
    <button title={title} onClick={onClick} className="w-[18px] h-[18px] rounded flex items-center justify-center text-white/70 hover:text-white hover:bg-white/10">
      {icon}
    </button>
  );
}

const SUB_STATE: Record<string, [string, string]> = {
  "in-sync": ["#5cb85c", "This submodule is in-sync with your repository."],
  "out-of-sync": ["#de9b43", "This submodule is out-of-sync with your repository."],
  uninitialized: ["#888888", "Your submodule directory has not been initialized."],
  conflict: ["#d9413d", "This submodule has a conflict."],
};

function Submodules({ repo }: { repo: RepoView }) {
  const q = useQuery({ queryKey: ["submodules", repo.path, repo.head_sha, JSON.stringify(repo.status)], queryFn: () => api.submodulesList(repo.path) });
  const list = q.data ?? repo.submodules.map((p) => ({ path: p, sha: "", state: "in-sync" as const, url: "" }));
  return (
    <>
      {list.map((m) => (
        <CtxMenu
          key={m.path}
          items={[
            { label: `Open submodule: ${m.path}`, disabled: m.state === "uninitialized", onSelect: () => openRepoPath(`${repo.path}\\${m.path.replace(/\//g, "\\")}`) },
            { label: m.state === "uninitialized" ? `Initialize ${m.path}` : `Update ${m.path}`, onSelect: () => act(repo.path, { type: "submoduleUpdate", paths: [m.path] }) },
            { separator: true },
            { label: "Delete this submodule", danger: true, onSelect: () => act(repo.path, { type: "submoduleRemove", path: m.path }) },
          ]}
        >
          <Row indent={1} title={`${m.url}\n${SUB_STATE[m.state]?.[1] ?? ""}`} onDoubleClick={() => m.state !== "uninitialized" && openRepoPath(`${repo.path}\\${m.path.replace(/\//g, "\\")}`)}>
            <Package size={13} style={{ color: SUB_STATE[m.state]?.[0] }} />
            <span className="truncate">{m.path}</span>
            {m.sha && <span className="mono text-[10.5px] text-fg3">{m.sha.slice(0, 7)}</span>}
          </Row>
        </CtxMenu>
      ))}
    </>
  );
}

/** GITFLOW: start features / releases / hotfixes and finish the ones in progress. */
function GitflowSection({ repo, open, onToggle }: { repo: RepoView; open: boolean; onToggle: () => void }) {
  const q = useQuery({ queryKey: ["gitflow", repo.path, repo.refs.length], queryFn: () => api.gitflowConfig(repo.path) });
  const c = q.data;
  if (!c?.initialized) return null;
  const kinds = [
    ["feature", c.feature],
    ["release", c.release],
    ["hotfix", c.hotfix],
  ] as const;
  return (
    <Section
      title="GITFLOW"
      icon={<GitFork size={14} />}
      open={open}
      onToggle={onToggle}
      actions={<SectionAction title="Gitflow settings" onClick={() => initGitflow(repo.path)} icon={<Plus size={13} />} />}
    >
      <div className="flex gap-1.5 pl-[28px] pr-3 py-1">
        {kinds.map(([k]) => (
          <button key={k} onClick={() => startGitflow(repo.path, k)} className="px-2 h-[22px] rounded-[3px] bg-white/5 border border-white/10 text-[11.5px] text-fg hover:text-white hover:bg-white/10">
            Start {k[0].toUpperCase() + k.slice(1)}
          </button>
        ))}
      </div>
      {kinds.map(([k, prefix]) =>
        repo.refs
          .filter((r) => r.kind === "Local" && prefix && r.name.startsWith(prefix))
          .map((r) => (
            <CtxMenu key={r.full} items={[{ label: `Finish ${r.name}`, onSelect: () => finishGitflow(repo.path, k, r.name.slice(prefix.length)) }]}>
              <Row indent={1} title={`Right-click to finish this ${k}`}>
                <GitFork size={13} className="opacity-60" />
                <span className="truncate flex-1">{r.name}</span>
                <button
                  className="text-[11px] text-link hover:underline opacity-0 group-hover:opacity-100"
                  onClick={(e) => (e.stopPropagation(), finishGitflow(repo.path, k, r.name.slice(prefix.length)))}
                >
                  Finish
                </button>
              </Row>
            </CtxMenu>
          )),
      )}
    </Section>
  );
}

/** LFS: tracked patterns and locked files. */
function LfsSection({ repo, open, onToggle }: { repo: RepoView; open: boolean; onToggle: () => void }) {
  const q = useQuery({ queryKey: ["lfs", repo.path, JSON.stringify(repo.status)], queryFn: () => api.lfsInfo(repo.path), staleTime: 30_000 });
  const lfs = q.data;
  if (!lfs?.available || (!lfs.enabled && lfs.patterns.length === 0)) return null;
  return (
    <Section title="LFS" icon={<Package size={14} />} count={lfs.patterns.length} open={open} onToggle={onToggle}>
      {lfs.patterns.map((p) => (
        <CtxMenu key={p} items={[{ label: `Stop tracking ${p}`, onSelect: () => act(repo.path, { type: "lfsUntrack", pattern: p }) }]}>
          <Row indent={1}>
            <Package size={13} className="opacity-60" />
            <span className="truncate mono text-[12px]">{p}</span>
          </Row>
        </CtxMenu>
      ))}
      {lfs.locks.map((l) => (
        <CtxMenu
          key={l.id}
          items={[
            { label: `Unlock ${l.path}`, onSelect: () => act(repo.path, { type: "lfsUnlock", path: l.path, force: false }) },
            { label: "Force unlock", danger: true, onSelect: () => act(repo.path, { type: "lfsUnlock", path: l.path, force: true }) },
          ]}
        >
          <Row indent={1} title={`Locked by ${l.owner}`}>
            <Lock size={12} className="text-warn" />
            <span className="truncate">{l.path}</span>
            <span className="text-[11px] text-fg3">{l.owner}</span>
          </Row>
        </CtxMenu>
      ))}
      {lfs.patterns.length === 0 && <Note>No patterns tracked yet. Right-click a file to track its type with LFS.</Note>}
    </Section>
  );
}

/** PULL REQUESTS and ISSUES from GitHub / GitLab for this repository's remote. */
function HostedLists({ repo, tabId, prsOpen, issuesOpen, toggle }: { repo: RepoView; tabId: string; prsOpen: boolean; issuesOpen: boolean; toggle: (k: SectionKey) => void }) {
  const s = usePanelStore();
  const remote = useQuery({ queryKey: ["remote", repo.path, repo.remotes.join()], queryFn: () => hosting.remote(repo.path), staleTime: Infinity });
  const on = !!remote.data;
  const prs = useQuery({ queryKey: ["prs", repo.path], queryFn: () => hosting.prs(repo.path), enabled: on && prsOpen, staleTime: 60_000 });
  const issues = useQuery({ queryKey: ["issues", repo.path], queryFn: () => hosting.issues(repo.path), enabled: on && issuesOpen, staleTime: 60_000 });
  const service = remote.data?.provider === "gitlab" ? "GitLab" : "GitHub";
  const note = (q: { isLoading: boolean; isError: boolean; error: unknown }) =>
    !on ? (
      <Note>This repository's remote isn't on GitHub or GitLab.</Note>
    ) : q.isLoading ? (
      <Note>Loading…</Note>
    ) : q.isError ? (
      <Note>
        {String(q.error)}{" "}
        <button className="text-link hover:underline" onClick={() => s.setDialog({ kind: "accounts" })}>
          Accounts
        </button>
      </Note>
    ) : null;
  return (
    <>
      <Section
        title={remote.data?.provider === "gitlab" ? "MERGE REQUESTS" : "PULL REQUESTS"}
        icon={<GitPullRequest size={14} />}
        count={prs.data?.length ?? 0}
        open={prsOpen}
        onToggle={() => toggle("prs")}
        actions={
          on && repo.head_branch ? (
            <SectionAction title={`Start a pull request from ${repo.head_branch}`} onClick={() => startPullRequest(repo.path, repo.head_branch!)} icon={<Plus size={13} />} />
          ) : undefined
        }
      >
        {note(prs)}
        {prs.data?.map((p) => (
          <CtxMenu
            key={p.number}
            items={[
              { label: "View pull request", onSelect: () => s.setUi(tabId, { pr: p.number, issue: null }) },
              { label: `View on ${service}`, onSelect: () => hosting.openUrl(p.url) },
            ]}
          >
            <Row indent={1} title={`${p.source} → ${p.target}\nby ${p.author}`} onClick={() => s.setUi(tabId, { pr: p.number, issue: null, diff: null, history: null })}>
              <GitPullRequest size={13} className={p.draft ? "text-fg3" : "text-good"} />
              <span className="text-fg3 text-[11.5px]">#{p.number}</span>
              <span className="truncate flex-1">{p.title}</span>
            </Row>
          </CtxMenu>
        ))}
        {on && prs.data?.length === 0 && <Note>No open pull requests.</Note>}
      </Section>
      <Section title="ISSUES" icon={<CircleDot size={14} />} count={issues.data?.length} open={issuesOpen} onToggle={() => toggle("issues")}>
        {note(issues)}
        {issues.data?.map((i) => (
          <CtxMenu
            key={i.number}
            items={[
              { label: "View issue", onSelect: () => s.setUi(tabId, { issue: i.number, pr: null }) },
              { label: "Create branch for this issue", onSelect: () => branchFromIssue(repo.path, i) },
              { label: `View issue in ${service}`, onSelect: () => hosting.openUrl(i.url) },
            ]}
          >
            <Row indent={1} title={i.labels.join(", ")} onClick={() => s.setUi(tabId, { issue: i.number, pr: null, diff: null, history: null })}>
              <CircleDot size={13} className="text-good" />
              <span className="text-fg3 text-[11.5px]">{i.key || `#${i.number}`}</span>
              <span className="truncate flex-1">{i.title}</span>
            </Row>
          </CtxMenu>
        ))}
        {on && issues.data?.length === 0 && <Note>No open issues.</Note>}
      </Section>
    </>
  );
}
