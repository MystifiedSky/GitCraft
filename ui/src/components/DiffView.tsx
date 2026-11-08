import { useQuery } from "@tanstack/react-query";
import { X } from "lucide-react";
import type { RepoView } from "@/lib/api";
import { api } from "@/lib/api";
import type { DiffTarget } from "@/lib/store";
import { useStore } from "@/lib/store";
import { act } from "@/lib/repo";
import { Button } from "./ui/button";
import { cn } from "@/lib/utils";

/** Inline diff of one file; opens over the graph like the original. */
export function DiffView({ repo, tabId, target }: { repo: RepoView; tabId: string; target: DiffTarget }) {
  const setUi = useStore((s) => s.setUi);
  const key = ["diff", repo.path, JSON.stringify(target), target.source.kind === "Commit" ? "" : JSON.stringify(repo.status)];
  const q = useQuery({ queryKey: key, queryFn: () => api.fileDiff(repo.path, target.source, target.file, target.untracked, target.parent) });
  const close = () => setUi(tabId, { diff: null });
  const lines = q.data ?? [];
  const added = lines.filter((l) => l.kind === "Added").length;
  const removed = lines.filter((l) => l.kind === "Removed").length;
  const stillThere =
    target.source.kind === "Unstaged"
      ? repo.status.unstaged.some((f) => f.path === target.file)
      : target.source.kind === "Staged"
        ? repo.status.staged.some((f) => f.path === target.file)
        : true;

  return (
    <div className="h-full flex flex-col bg-app min-w-0">
      <div className="flex items-center gap-2 h-[40px] px-3 bg-panel border-b border-black/30 shrink-0">
        <div className="flex flex-col min-w-0 flex-1">
          <span className="text-[13px] text-white truncate font-semibold">{target.file.split("/").pop()}</span>
          <span className="text-[11px] text-fg3 truncate">{target.file}</span>
        </div>
        <span className="text-[11.5px] text-good">+{added}</span>
        <span className="text-[11.5px] text-[#ef7a76]">−{removed}</span>
        <div className="flex rounded overflow-hidden border border-white/10 text-[11.5px]">
          <span className="px-2 py-0.5 bg-white/10 text-white">Inline View</span>
          <span className="px-2 py-0.5 text-fg3" title="Hunk and Split views come in Stage 2">
            Split View
          </span>
        </div>
        {target.source.kind === "Unstaged" && stillThere && (
          <Button size="sm" variant="success" onClick={() => act(repo.path, { type: "stage", paths: [target.file] }, true).then(close)}>
            Stage File
          </Button>
        )}
        {target.source.kind === "Staged" && stillThere && (
          <Button size="sm" variant="danger" onClick={() => act(repo.path, { type: "unstage", paths: [target.file] }, true).then(close)}>
            Unstage File
          </Button>
        )}
        <button onClick={close} className="text-fg2 hover:text-white p-1" title="Close (Esc)">
          <X size={16} />
        </button>
      </div>
      <div className="flex-1 overflow-auto mono text-[12px] leading-[19px] select-text">
        {q.isLoading && <div className="p-4 text-fg3 font-sans">Loading diff…</div>}
        {q.isError && <div className="p-4 text-[#ef7a76] font-sans">{String(q.error)}</div>}
        {!q.isLoading && lines.length === 0 && <div className="p-4 text-fg3 font-sans">No changes to show (the file may be empty, or only its mode changed).</div>}
        <table className="border-collapse min-w-full">
          <tbody>
            {lines.map((l, i) =>
              l.kind === "Hunk" ? (
                <tr key={i} className="bg-accent/10 text-fg2">
                  <td colSpan={3} className="px-3 py-0.5">
                    {l.text}
                  </td>
                </tr>
              ) : (
                <tr key={i} className={cn(l.kind === "Added" && "bg-good/20", l.kind === "Removed" && "bg-bad/20", l.kind === "Meta" && "text-fg3")}>
                  <td className="w-[44px] text-right pr-2 text-fg3 select-none align-top">{l.old_no ?? ""}</td>
                  <td className="w-[44px] text-right pr-2 text-fg3 select-none align-top border-r border-white/5">{l.new_no ?? ""}</td>
                  <td className="pl-2 pr-4 whitespace-pre text-white/85">
                    <span className="select-none inline-block w-3 text-fg3">{l.kind === "Added" ? "+" : l.kind === "Removed" ? "-" : " "}</span>
                    {l.text}
                  </td>
                </tr>
              ),
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
