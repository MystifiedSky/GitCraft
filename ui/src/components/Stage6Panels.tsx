import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertTriangle, Bot, CheckCircle2, ChevronDown, FileDown, FileUp, Folder, GitBranch, House, MoreVertical, Plug, Search, Plus } from "lucide-react";
import { api, type RepoView } from "@/lib/api";
import { ai, applyPatch, patchName, savePatch, type AgentSession } from "@/lib/ai";
import { act, copy, openRepoPath } from "@/lib/repo";
import { conflictTarget, conflictRefsKey } from "@/lib/conflict";
import { runInTerminal } from "@/lib/terminal-launch";
import { useStore } from "@/lib/store";
import { cn, timeSince } from "@/lib/utils";
import { CtxMenu, DropMenu } from "./ui/menu";
import { AGENT_LABEL, AGENT_COMMAND, agentConnection, agentLaunch, sameFolder, folderKey } from "@/lib/agents";
import { workingChanges } from "@/lib/working-changes";

export function useConflictCheck(repo: RepoView | undefined) {
  const global = useStore((s) => s.settings.conflictDetection !== false);
  const prefs = useStore((s) => (repo ? s.settings.repoPrefs[repo.path] : undefined));
  const target = repo ? conflictTarget(repo, prefs?.baseBranches ?? "", prefs?.excludedBranches ?? "", global && prefs?.conflictDetection !== false) : null;
  return useQuery({
    queryKey: ["conflictCheck", repo?.path, repo ? conflictRefsKey(repo) : "", target],
    queryFn: () => ai.conflictCheck(repo!.path, target ?? undefined),
    enabled: !!repo && !!repo.head_branch && target !== null,
    staleTime: Infinity,
  });
}

/** Inside TEAMS: early warning that your branch would conflict with the target branch (computed locally). */
export function ConflictCheckRows({ repo }: { repo: RepoView }) {
  const q = useConflictCheck(repo);
  const c = q.data;
  const s = useStore();
  const prefs = s.settings.repoPrefs[repo.path];
  if (repo.head_branch && conflictTarget(repo, prefs?.baseBranches ?? "", prefs?.excludedBranches ?? "", s.settings.conflictDetection !== false && prefs?.conflictDetection !== false) === null)
    return (
      <Note>
        Conflict prevention is off here.{" "}
        <button className="text-link hover:underline" onClick={() => s.setDialog({ kind: "prefs", section: "conflict" })}>
          Preferences
        </button>
      </Note>
    );
  if (!repo.head_branch) return <Note>Check out a branch to see whether it would conflict with the main branch.</Note>;
  if (q.isLoading) return <Note>Checking for conflicts…</Note>;
  if (!c || c.note) return <Note>{c?.note ?? "Could not check for conflicts."}</Note>;
  if (c.files.length === 0)
    return (
      <div className="flex items-center gap-1.5 h-[24px] pl-[30px] pr-2 text-[12.5px] text-fg2">
        <CheckCircle2 size={13} className="text-good shrink-0" />
        <span className="truncate">
          No conflicts with <span className="text-white/85">{c.target}</span>
        </span>
      </div>
    );
  return (
    <>
      <div className="flex items-center gap-1.5 h-[24px] pl-[30px] pr-2 text-[12.5px] text-warn" title="Merging your branch with this one would stop on these files">
        <AlertTriangle size={13} className="shrink-0" />
        <span className="truncate">
          {c.files.length} file{c.files.length === 1 ? "" : "s"} would conflict with {c.target}
        </span>
      </div>
      {c.files.map((f) => (
        <div key={f} className="h-[22px] pl-[50px] pr-2 text-[12px] text-fg2 truncate" title={f}>
          {f}
        </div>
      ))}
    </>
  );
}

export function PatchRows({ repo }: { repo: RepoView }) {
  const head = repo.commits.find((c) => c.sha === repo.head_sha);
  return (
    <>
      <button className="w-full flex items-center gap-1.5 h-[24px] pl-[30px] pr-2 text-[12.5px] text-fg hover:bg-hoverrow" onClick={() => savePatch(repo.path, [], patchName(repo.head_branch ?? "changes"))}>
        <FileDown size={13} className="opacity-70" /> Save uncommitted changes as a patch…
      </button>
      {head && (
        <button className="w-full flex items-center gap-1.5 h-[24px] pl-[30px] pr-2 text-[12.5px] text-fg hover:bg-hoverrow" title={head.summary} onClick={() => savePatch(repo.path, [head.sha], patchName(head.summary))}>
          <FileDown size={13} className="opacity-70" /> <span className="truncate">Save the last commit as a patch…</span>
        </button>
      )}
      <button className="w-full flex items-center gap-1.5 h-[24px] pl-[30px] pr-2 text-[12.5px] text-fg hover:bg-hoverrow" onClick={() => applyPatch(repo.path)}>
        <FileUp size={13} className="opacity-70" /> Apply a patch file…
      </button>
    </>
  );
}

/** Start in the requested folder, including linked worktrees and resumed sessions. */
function launchAgent(path: string, agent: string, resume?: string, setup: string[] = []) {
  const st = useStore.getState();
  try {
    const { command, env } = agentLaunch(st.settings, agent, resume);
    const sep = st.settings.termShell === "cmd" ? " && " : "; ";
    runInTerminal([...setup, command].join(sep), AGENT_LABEL[agent], path, env);
  } catch (e) { st.toast(false, String(e)); }
}

async function startAgent(repo: RepoView, worktree: boolean) {
  const st = useStore.getState();
  const set = st.settings;
  if (!AGENT_COMMAND[set.codingAgent]) return st.setDialog({ kind: "prefs", section: "tools" });
  if (!worktree) return launchAgent(repo.path, set.codingAgent);
  st.setDialog({
    kind: "prompt",
    title: "Start an agent session in a new worktree",
    label: "New branch name",
    value: `agent/${new Date().toISOString().replace(/[-:]/g, "").slice(0, 15)}`,
    confirm: "Start",
    onSubmit: async (branch) => {
      const parent = repo.path.replace(/[\\/][^\\/]+$/, "");
      const folder = `${parent}${repo.path.includes("\\") ? "\\" : "/"}${repo.name}-${branch.replace(/[^\w.-]+/g, "-")}`;
      const ok = await act(repo.path, { type: "worktreeAdd", path: folder, branch, newBranch: true, start: null });
      if (ok) launchAgent(folder, set.codingAgent, undefined, set.agentSetup.split("\n").map((l) => l.trim()).filter(Boolean));
    },
  });
}

const sessionStatus = (a?: AgentSession) => !a ? "No saved sessions" : a.status === "recent" ? "Recent activity" : a.status === "finished" ? "Last turn finished" : "No recent activity";

export function AgentsView({ repo, tabId }: { repo: RepoView | undefined; tabId: string }) {
  const settings = useStore((s) => s.settings);
  const setSettings = useStore((s) => s.setSettings);
  const [filter, setFilter] = useState("");
  const [searchOpen, setSearchOpen] = useState(false);
  const searchInput = useRef<HTMLInputElement>(null);
  useEffect(() => { if (searchOpen) searchInput.current?.focus(); }, [searchOpen]);
  const worktrees = [...(repo?.worktrees ?? [])];
  if (repo && !worktrees.some((w) => sameFolder(w.path, repo.path))) worktrees.unshift({ path: repo.path, branch: repo.head_branch ?? "", locked: false });
  const roots = worktrees.map((w) => w.path);
  const claudeDir = agentConnection(settings, "claude")?.cliConfigDir ?? "";
  const codexDir = agentConnection(settings, "codex")?.cliConfigDir ?? "";
  const q = useQuery({
    queryKey: ["agents", repo?.path, roots.join("\n"), claudeDir, codexDir],
    queryFn: () => ai.agents(repo!.path, roots, claudeDir, codexDir),
    enabled: !!repo,
    refetchInterval: 10000,
  });
  const grouped = useMemo(() => {
    const groups = new Map<string, AgentSession[]>();
    for (const session of q.data ?? []) {
      const key = folderKey(session.path);
      const group = groups.get(key) ?? [];
      group.push(session); groups.set(key, group);
    }
    return groups;
  }, [q.data]);
  if (!repo) return <Note>Open a repository to see its worktrees and saved coding sessions.</Note>;
  const agentName = AGENT_LABEL[settings.codingAgent];
  const visible = worktrees.filter((w) => [w.path, w.branch, ...(grouped.get(folderKey(w.path)) ?? []).map((a) => a.title)].join(" ").toLowerCase().includes(filter.toLowerCase()));
  const agentItems = Object.entries(AGENT_LABEL).map(([key, name]) => ({
    label: (settings.codingAgent === key ? "✓ " : "") + name,
    onSelect: () => setSettings({ codingAgent: key }),
  }));
  return (
    <div className="flex-1 overflow-y-auto px-3 pt-2 pb-3">
      <div className="flex items-center gap-2 h-[32px] text-[14px] mb-1">
        <span className="font-semibold">Worktrees</span><span className="text-link">{worktrees.length}</span>
        <div className="flex-1" />
        <DropMenu align="end" items={[{ heading: "Coding agent" }, ...agentItems, { separator: true }, { label: "Agent preferences", onSelect: () => useStore.getState().setDialog({ kind: "prefs", section: "agents" }) }]}
          trigger={<button className="p-1 rounded border border-accent/70 bg-accent/20 text-white" title="Choose coding agent"><Plug size={15} /></button>} />
        <button className="p-1 text-fg2 hover:text-white" title="Filter worktrees and sessions" aria-expanded={searchOpen} onClick={() => { setSearchOpen(!searchOpen); setFilter(""); }}><Search size={15} /></button>
      </div>
      {searchOpen && <input ref={searchInput} aria-label="Filter worktrees and sessions" placeholder="Filter worktrees and sessions" value={filter} onChange={(e) => setFilter(e.target.value)} className="w-full h-[26px] px-2 mb-2 bg-field border border-white/15 rounded text-[12px] outline-none focus:border-accent" />}
      <div className="flex h-[36px] mb-2 rounded-[3px] border border-white/35 overflow-hidden">
        <button className="flex-1 min-w-0 flex items-center justify-center gap-2 text-[13px] hover:bg-white/10" onClick={() => startAgent(repo, false)} title="Start in this repository's terminal">
          <Plus size={14} /><span className="truncate">{agentName ? `Start ${agentName} Session` : "Choose a coding agent"}</span>
        </button>
        <DropMenu align="end" trigger={<button className="px-2 border-l border-white/25 hover:bg-white/10" title="More agent options"><ChevronDown size={15} /></button>}
          items={[{ label: "Start in new worktree…", onSelect: () => startAgent(repo, true) }, { heading: "Coding agent" }, ...agentItems]} />
      </div>
      {q.isLoading && <div className="py-2 text-[12px] text-fg2">Finding saved sessions…</div>}
      {q.isError && <div className="py-2 text-[12px] text-bad">Could not read saved sessions. <button className="underline" onClick={() => q.refetch()}>Retry</button></div>}
      {visible.map((w) => <AgentWorktree key={w.path} worktree={w} repo={repo} tabId={tabId} sessions={(grouped.get(folderKey(w.path)) ?? [])} />)}
      {visible.length === 0 && <div className="py-3 text-[12px] text-fg2">No matching worktrees or sessions.</div>}
      <div className="pt-2 text-[11px] text-fg3 leading-relaxed">
        Codex CLI and Claude Code session history
        {agentName && !["claude", "codex"].includes(settings.codingAgent) && ` · ${agentName} runs in the terminal.`}
      </div>
    </div>
  );
}

function AgentWorktree({ worktree: w, repo, tabId, sessions }: { worktree: RepoView["worktrees"][number]; repo: RepoView; tabId: string; sessions: AgentSession[] }) {
  const current = sameFolder(w.path, repo.path);
  const q = useQuery({ queryKey: ["agentWorktreeStatus", w.path], queryFn: () => api.repoStatus(w.path), enabled: !current, refetchInterval: 10000 });
  const status = current ? repo.status : q.data;
  const counts = status ? workingChanges(status) : null;
  const units = useStore((s) => s.metrics.time_units);
  const agent = useStore((s) => s.settings.codingAgent);
  const open = () => {
    if (!current) return openRepoPath(w.path);
    useStore.getState().setUi(tabId, { selection: { type: "wip" }, multi: [], compareWorkdir: null, diff: null, history: null, conflict: null, pr: null, issue: null });
  };
  return (
    <div className={cn("rounded-[3px] border overflow-hidden mb-2", current ? "border-accent/80" : "border-white/20")}>
      <div className={cn("px-2.5 py-2", current ? "bg-white/10" : "bg-white/5")}>
        <div className="flex items-center gap-1.5">
          {current && <House size={15} className="text-fg2 shrink-0" />}
          <GitBranch size={13} className="text-fg2 shrink-0" />
          <button onClick={open} className="font-semibold text-[14px] text-white/90 truncate hover:text-white" title={current ? "View working changes" : "Open worktree"}>{w.branch || "(detached HEAD)"}</button>
          <div className="flex-1" />
          <DropMenu align="end" trigger={<button className="p-1 text-fg2 hover:text-white" title="Worktree actions"><MoreVertical size={14} /></button>}
            items={[{ label: current ? "View Changes" : "Open worktree", onSelect: open }, { label: `Start ${AGENT_LABEL[agent] ?? "agent"} here`, onSelect: () => launchAgent(w.path, agent) }, { label: "Copy worktree path", onSelect: () => copy(w.path) }]} />
        </div>
        <div className="flex items-center gap-1.5 text-[12px] text-fg2 mt-1" title={w.path}><Folder size={13} className="shrink-0" /><span className="truncate">{w.path.split(/[\\/]/).pop()}</span>{w.locked && <span className="text-[10px]">Locked</span>}</div>
        <button onClick={open} className="flex gap-2 text-[12px] mt-1" title="View working changes">
          {counts ? <>
            {counts.added > 0 && <span className="text-good">+{counts.added}</span>}
            {counts.modified > 0 && <span className="text-warn">✎{counts.modified}</span>}
            {counts.deleted > 0 && <span className="text-bad">−{counts.deleted}</span>}
            {counts.total === 0 && <span className="text-fg3">No file changes</span>}
          </> : <span className="text-fg3">{q.isError ? "Changes unavailable" : "Loading changes…"}</span>}
        </button>
      </div>
      <div className="px-2.5 py-1.5 flex items-center gap-1.5 text-[12px] text-fg2 border-t border-white/5" title="Status reflects saved activity, not a live process check.">
        <span className={cn("w-[7px] h-[7px] rounded-full shrink-0", sessions[0]?.status === "recent" ? "bg-good" : "bg-white/40")} />
        <span className="flex-1 min-w-0 truncate">{sessionStatus(sessions[0])}</span>
        <span className="text-[11px] text-fg3 whitespace-nowrap">{sessions.length} session{sessions.length === 1 ? "" : "s"}</span>
      </div>
      {sessions.map((a) => {
        const time = timeSince(a.modified, units);
        const resume = () => launchAgent(w.path, a.agent, a.id);
        return (
          <CtxMenu key={a.agent + a.id} items={[
            { label: "Resume session", onSelect: resume },
            { label: "Copy resume command", onSelect: () => copy((a.agent === "claude" ? "claude --resume " : "codex resume ") + a.id) },
            { label: "Copy session id", onSelect: () => copy(a.id) },
          ]}>
            <div className="px-2.5 py-2 border-t border-white/5 hover:bg-hoverrow" title={a.title}>
              <div className="flex items-center gap-2 text-[12px]"><Bot size={13} className="text-fg2 shrink-0" /><span className="truncate text-white/90 flex-1">{a.title}</span><button onClick={resume} className="text-link hover:underline text-[11px]">Resume</button></div>
              <div className="mt-1 text-[10.5px] text-fg3">{AGENT_LABEL[a.agent]} · {sessionStatus(a)} · <span style={{ color: time.color }}>{time.text}</span></div>
            </div>
          </CtxMenu>
        );
      })}
    </div>
  );
}

function Note({ children }: { children: React.ReactNode }) {
  return <div className="pl-[30px] pr-3 py-1 text-[12px] text-fg3 leading-snug">{children}</div>;
}
