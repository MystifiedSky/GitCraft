import { useQuery } from "@tanstack/react-query";
import { History, ScanLine, X } from "lucide-react";
import { useMemo, useState } from "react";
import type { BlameLine, RepoView } from "@/lib/api";
import { api } from "@/lib/api";
import { useStore, usePanelStore } from "@/lib/store";
import { Avatar } from "./Icons";
import { MonacoDiff } from "./LazyEditors";
import { cn, fullDate, GRAPH_COLORS, timeSince } from "@/lib/utils";

function Since({ time }: { time: number }) {
  const units = useStore((s) => s.metrics.time_units);
  const t = timeSince(time, units);
  return (
    <span style={{ color: t.color }} title={fullDate(time)}>
      {t.text}
    </span>
  );
}

/** File History and Blame for one file (opened from a file's menu or Ctrl+Shift+H). */
export function HistoryView({ repo, tabId, file, mode, sha }: { repo: RepoView; tabId: string; file: string; mode: "history" | "blame"; sha: string | null }) {
  const s = usePanelStore();
  const m = s.metrics;
  const hist = useQuery({ queryKey: ["history", repo.path, file], queryFn: () => api.fileHistory(repo.path, file) });
  const entries = hist.data ?? [];
  const selected = entries.find((entry) => entry.sha === sha);
  const current = selected ?? entries[0];
  const blameFile = sha ? selected?.path : file;
  const setHist = (patch: Partial<{ mode: "history" | "blame"; sha: string | null }>) => s.setUi(tabId, { history: { file, mode, sha, ...patch } });
  const close = () => s.setUi(tabId, { history: null });
  const onGraph = (c: string) => {
    if (!repo.commits.some((x) => x.sha === c)) return s.toast(false, "This commit is not visible on the graph");
    s.setUi(tabId, { history: null, selection: { type: "commit", sha: c }, diff: null });
  };

  return (
    <div className="h-full flex flex-col bg-app min-w-0">
      <div className="flex items-center gap-2 h-[30px] pl-3 pr-2 bg-toolbar/60 border-b border-black/30 shrink-0">
        <History size={14} className="text-link" />
        <span className="text-[13px] text-white font-semibold whitespace-nowrap">File History</span>
        <span className="text-[12.5px] text-fg2 truncate flex-1" title={file}>
          {file}
        </span>
        <div className="flex rounded-[3px] overflow-hidden border border-white/10 text-[12px]">
          <button className={cn("px-2.5 py-[2px] flex items-center gap-1", mode === "history" ? "bg-white/15 text-white" : "text-fg2 hover:text-white")} onClick={() => setHist({ mode: "history" })}>
            <History size={12} /> History
          </button>
          <button className={cn("px-2.5 py-[2px] flex items-center gap-1", mode === "blame" ? "bg-white/15 text-white" : "text-fg2 hover:text-white")} onClick={() => setHist({ mode: "blame" })}>
            <ScanLine size={12} /> Blame
          </button>
        </div>
        <button onClick={close} className="text-fg2 hover:text-white p-1" title="Close (Esc)">
          <X size={16} />
        </button>
      </div>
      <div className="flex-1 flex min-h-0">
        <div className="w-[250px] shrink-0 border-r border-black/30 bg-panel overflow-y-auto">
          {hist.isLoading && <div className="p-3 text-fg3 text-[12px]">Loading history…</div>}
          {!hist.isLoading && entries.length === 0 && <div className="p-3 text-fg3 text-[12px]">No commits touch this file.</div>}
          {mode === "blame" && (
            <button onClick={() => setHist({ sha: null })} className={cn("w-full text-left px-3 py-2 border-b border-white/5 text-[12.5px]", !sha ? "bg-selrow text-white" : "text-fg hover:bg-hoverrow")}>
              Working folder (current)
            </button>
          )}
          {entries.map((e) => (
            <button
              key={e.sha}
              onClick={() => setHist({ sha: e.sha })}
              onDoubleClick={() => onGraph(e.sha)}
              title="Double-click to view on the graph"
              className={cn("w-full text-left flex gap-2 px-3 py-2 border-b border-white/5", current?.sha === e.sha && (mode === "history" || sha) ? "bg-selrow" : "hover:bg-hoverrow")}
            >
              <Avatar name={e.author} email={e.email} size={24} />
              <span className="flex flex-col min-w-0">
                <span className="text-[12.5px] text-white truncate">{e.summary}</span>
                <span className="text-[11px] text-fg3 truncate">
                  {e.author} · <Since time={e.time} /> · <span className="mono">{e.sha.slice(0, m.sha_len)}</span>
                </span>
                {e.path !== file && <span className="text-[10.5px] text-fg3 truncate">was {e.path}</span>}
              </span>
            </button>
          ))}
          {entries.length >= 500 && <div className="p-3 text-[11px] text-fg3">Showing the latest 500 commits.</div>}
        </div>
        <div className="flex-1 min-w-0">
          {mode === "history" ? current ? <HistoryDiff repo={repo} file={current.path} sha={current.sha} parent={current.parents[0] ?? null} onGraph={() => onGraph(current.sha)} /> : null : blameFile ? <Blame repo={repo} file={blameFile} rev={sha} onPick={(c) => onGraph(c)} /> : <div className="p-4 text-fg3">{hist.isLoading ? "Loading file history�" : "Select a revision from this file's loaded history to view its blame."}</div>}
        </div>
      </div>
    </div>
  );
}

function HistoryDiff({ repo, file, sha, parent, onGraph }: { repo: RepoView; file: string; sha: string; parent: string | null; onGraph: () => void }) {
  const set = useStore((s) => s.settings);
  const [largeKey, setLargeKey] = useState<string | null>(null);
  const key = JSON.stringify([file, sha, parent]);
  const allowLarge = largeKey === key;
  const v = useQuery({
    queryKey: ["versions", repo.path, file, sha, parent, allowLarge],
    queryFn: () => api.fileVersions(repo.path, { kind: "Commit", sha }, file, false, parent, allowLarge),
    staleTime: Infinity,
  });
  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center gap-2 h-[26px] px-3 text-[11.5px] text-fg2 border-b border-white/5 shrink-0">
        <span className="truncate flex-1">{file}</span>
        <button className="text-link hover:underline" onClick={onGraph}>
          View on graph
        </button>
      </div>
      <div className="flex-1 min-h-0">
        {v.isError ? <div className="p-4 text-fg3">{String(v.error)}</div> : v.data?.too_large ? (
          <div className="p-4 text-fg3">This file exceeds the {Math.round((v.data.preview_limit ?? 0) / 1048576)} MB preview limit. {allowLarge ? "Open it in an external editor to inspect it." : <button className="text-link underline" onClick={() => setLargeKey(key)}>Load preview (up to 20 MB)</button>}</div>
        ) : v.data && !v.data.binary ? (
          <MonacoDiff
            original={v.data.old ?? ""}
            modified={v.data.new ?? ""}
            file={file + "#" + sha}
            sideBySide={set.diffMode === "split"}
            wordWrap={set.wordWrap}
            ignoreWhitespace={set.ignoreWhitespace}
            editable={false}
          />
        ) : v.data?.binary ? (
          <div className="p-4 text-fg3 text-[13px]">Binary file</div>
        ) : (
          <div className="p-4 text-fg3 text-[13px]">Loading…</div>
        )}
      </div>
    </div>
  );
}

/** Blame: each run of lines from the same commit gets a coloured bar and its author, date and sha. */
function Blame({ repo, file, rev, onPick }: { repo: RepoView; file: string; rev: string | null; onPick: (sha: string) => void }) {
  const m = useStore((s) => s.metrics);
  const wrap = useStore((s) => s.settings.wordWrap);
  const q = useQuery({ queryKey: ["blame", repo.path, file, rev], queryFn: () => api.blame(repo.path, file, rev) });
  const [hover, setHover] = useState<string | null>(null);
  const colors = useMemo(() => {
    // Same palette as the graph (code-blame-color-0..9 = graph-color-0..9 in the theme).
    const map = new Map<string, string>();
    for (const l of q.data ?? []) if (!map.has(l.sha)) map.set(l.sha, GRAPH_COLORS[map.size % GRAPH_COLORS.length]);
    return map;
  }, [q.data]);
  if (q.isLoading) return <div className="p-4 text-fg3 text-[13px]">Loading blame…</div>;
  if (q.isError) return <div className="p-4 text-[#ef7a76] text-[13px]">{String(q.error)}</div>;
  const lines = q.data ?? [];
  const uncommitted = (l: BlameLine) => /^0+$/.test(l.sha);
  return (
    <div className="h-full overflow-auto mono text-[12px] leading-[19px]">
      <table className="border-collapse min-w-full">
        <tbody>
          {lines.map((l, i) => {
            const first = i === 0 || lines[i - 1].sha !== l.sha;
            const color = uncommitted(l) ? "#888" : colors.get(l.sha)!;
            return (
              <tr key={i} className={cn(hover === l.sha && "bg-white/[0.04]", first && i > 0 && "border-t border-white/5")} onMouseEnter={() => setHover(l.sha)}>
                <td className="w-[230px] max-w-[230px] align-top font-sans pl-2 pr-2 select-none" style={{ boxShadow: `inset 3px 0 0 ${color}` }}>
                  {first && (
                    <button className="flex items-center gap-1.5 text-[11.5px] w-full min-w-0 text-left" onClick={() => !uncommitted(l) && onPick(l.sha)} title={uncommitted(l) ? "Not committed yet" : `${l.summary}\n${fullDate(l.time)}\nClick to view on the graph`}>
                      <span className="text-white/85 truncate max-w-[90px]">{uncommitted(l) ? "You" : l.author}</span>
                      <span className="text-fg3 whitespace-nowrap">{uncommitted(l) ? "uncommitted" : <Since time={l.time} />}</span>
                      {!uncommitted(l) && <span className="mono text-link ml-auto">{l.sha.slice(0, m.sha_len)}</span>}
                    </button>
                  )}
                </td>
                <td className="w-[44px] text-right pr-2 text-fg3 select-none align-top border-r border-white/5">{l.line}</td>
                <td className={cn("pl-3 pr-4 text-white/85 select-text", wrap ? "whitespace-pre-wrap break-all" : "whitespace-pre")}>{l.text}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
