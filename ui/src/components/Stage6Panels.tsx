import { useQuery } from "@tanstack/react-query";
import { AlertTriangle, Bot, CheckCircle2, FileDown, FileUp } from "lucide-react";
import type { RepoView } from "@/lib/api";
import { ai, applyPatch, patchName, savePatch } from "@/lib/ai";
import { copy } from "@/lib/repo";
import { useStore } from "@/lib/store";
import { cn, timeSince } from "@/lib/utils";
import { CtxMenu } from "./ui/menu";

/** Key that changes whenever HEAD or any remote branch moves, so the check re-runs after commits and fetches. */
const refsKey = (repo: RepoView) => repo.head_sha + repo.refs.filter((r) => r.kind === "Remote").map((r) => r.sha.slice(0, 8)).join("");

export function useConflictCheck(repo: RepoView | undefined) {
  return useQuery({
    queryKey: ["conflictCheck", repo?.path, repo ? refsKey(repo) : ""],
    queryFn: () => ai.conflictCheck(repo!.path),
    enabled: !!repo && !!repo.head_branch,
    staleTime: Infinity,
  });
}

/** Inside TEAMS: early warning that your branch would conflict with the target branch (computed locally). */
export function ConflictCheckRows({ repo }: { repo: RepoView }) {
  const q = useConflictCheck(repo);
  const c = q.data;
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
export function AgentsView({ repo }: { repo: RepoView | undefined }) {
  const units = useStore((s) => s.metrics.time_units);
  const q = useQuery({ queryKey: ["agents", repo?.path], queryFn: () => ai.agents(repo!.path), enabled: !!repo, refetchInterval: 10000 });
  if (!repo) return <Note>Open a repository to see the AI coding sessions that worked in it.</Note>;
  const list = q.data ?? [];
  return (
    <div className="flex-1 overflow-y-auto pt-2">
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
