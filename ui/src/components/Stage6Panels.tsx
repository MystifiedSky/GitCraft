import { useQuery } from "@tanstack/react-query";
import { AlertTriangle, Bot, CheckCircle2, FileDown, FileUp } from "lucide-react";
import type { RepoView } from "@/lib/api";
import { ai, applyPatch, patchName, savePatch } from "@/lib/ai";
import { act, copy } from "@/lib/repo";
import { conflictTarget } from "@/lib/conflict";
import { runInTerminal } from "./TerminalPanel";
import { useStore } from "@/lib/store";
import { cn, timeSince } from "@/lib/utils";
import { CtxMenu } from "./ui/menu";

/** Key that changes whenever HEAD or any remote branch moves, so the check re-runs after commits and fetches. */
const refsKey = (repo: RepoView) => repo.head_sha + repo.refs.filter((r) => r.kind === "Remote").map((r) => r.sha.slice(0, 8)).join("");

export function useConflictCheck(repo: RepoView | undefined) {
  const global = useStore((s) => s.settings.conflictDetection !== false);
  const prefs = useStore((s) => (repo ? s.settings.repoPrefs[repo.path] : undefined));
  const target = repo ? conflictTarget(repo, prefs?.baseBranches ?? "", prefs?.excludedBranches ?? "", global && prefs?.conflictDetection !== false) : null;
  return useQuery({
    queryKey: ["conflictCheck", repo?.path, repo ? refsKey(repo) : "", target],
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

/** Agents view: Claude Code sessions that worked in this repository (read from ~/.claude/projects). */
const AGENT_COMMAND: Record<string, string> = { claude: "claude", codex: "codex", copilot: "copilot", opencode: "opencode", gemini: "gemini" };

/** Start the coding agent chosen in Preferences > External Tools in the in-app terminal; in a new worktree the
 *  Agents > Setup commands run first. */
async function startAgent(repo: RepoView, worktree: boolean) {
  const st = useStore.getState();
  const set = st.settings;
  const cmd = AGENT_COMMAND[set.codingAgent];
  if (!cmd) return st.setDialog({ kind: "prefs", section: "tools" });
  const agent = [cmd, set.agentArgs.trim()].filter(Boolean).join(" ");
  const sep = set.termShell === "cmd" ? " && " : "; ";
  if (!worktree) return runInTerminal(agent, AGENT_LABEL[set.codingAgent] ?? cmd);
  st.setDialog({
    kind: "prompt",
    title: "Start an agent session in a new worktree",
    label: "New branch name",
    value: `agent/${new Date().toISOString().slice(0, 10)}`,
    confirm: "Start",
    onSubmit: async (branch) => {
      const parent = repo.path.replace(/[\\/][^\\/]+$/, "");
      const folder = `${parent}${repo.path.includes("\\") ? "\\" : "/"}${repo.name}-${branch.replace(/[^\w.-]+/g, "-")}`;
      const ok = await act(repo.path, { type: "worktreeAdd", path: folder, branch, newBranch: true, start: null });
      if (!ok) return;
      const setup = set.agentSetup.split("\n").map((l) => l.trim()).filter(Boolean);
      runInTerminal([`cd "${folder}"`, ...setup, agent].join(sep), `${AGENT_LABEL[set.codingAgent] ?? cmd} (${branch})`);
    },
  });
}

const AGENT_LABEL: Record<string, string> = { claude: "Claude Code", codex: "Codex", copilot: "Copilot CLI", opencode: "OpenCode", gemini: "Gemini CLI" };

export function AgentsView({ repo }: { repo: RepoView | undefined }) {
  const units = useStore((s) => s.metrics.time_units);
  const agentName = useStore((s) => AGENT_LABEL[s.settings.codingAgent]);
  const q = useQuery({ queryKey: ["agents", repo?.path], queryFn: () => ai.agents(repo!.path), enabled: !!repo, refetchInterval: 10000 });
  if (!repo) return <Note>Open a repository to see the AI coding sessions that worked in it.</Note>;
  const list = q.data ?? [];
  return (
    <div className="flex-1 overflow-y-auto pt-2">
      <div className="flex gap-2 px-3 pb-2">
        <button className="flex-1 h-[26px] rounded-[3px] border border-accent/70 bg-accent/20 text-[12.5px] text-white hover:bg-accent/30" onClick={() => startAgent(repo, false)} title="In this repository, in the in-app terminal">
          {agentName ? `Start ${agentName}` : "Choose a coding agent"}
        </button>
        <button className="h-[26px] px-2 rounded-[3px] border border-white/10 bg-white/5 text-[12.5px] text-fg hover:bg-white/10" onClick={() => startAgent(repo, true)} title="Create a worktree, run the setup commands, then start the agent">
          In new worktree
        </button>
      </div>
      <div className="px-3 pb-1 text-[12px] font-semibold tracking-wider text-fg2">SESSIONS ({list.length})</div>
      {list.length === 0 && !q.isLoading && (
        <div className="px-4 py-4 text-fg2 text-[12.5px] leading-relaxed">
          <Bot size={26} className="mb-2 opacity-70" />
          No Claude Code sessions have worked in this folder yet. Start one with <span className="mono">claude</span> in the terminal ({"Ctrl+`"}) and it shows up here.
        </div>
      )}
      {list.map((a) => {
        const t = timeSince(a.modified, units);
        return (
          <CtxMenu
            key={a.id}
            items={[
              { label: "Copy resume command", onSelect: () => copy(`claude --resume ${a.id}`) },
              { label: "Copy session id", onSelect: () => copy(a.id) },
            ]}
          >
            <div className="flex items-start gap-2 px-3 py-1.5 hover:bg-hoverrow cursor-default" title={`${a.title}\n${a.messages} messages · right-click to copy the resume command`}>
              <span className={cn("mt-[5px] w-[7px] h-[7px] rounded-full shrink-0", a.status === "active" ? "bg-good" : "bg-white/25")} />
              <div className="min-w-0 flex-1">
                <div className="text-[12.5px] text-white/90 truncate">{a.title}</div>
                <div className="text-[11px] text-fg3">
                  {a.status === "active" ? "working" : "idle"} · {a.messages} messages · <span style={{ color: t.color }}>{t.text}</span>
                </div>
              </div>
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
