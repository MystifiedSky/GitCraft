import { readRepositorySummary } from "@/lib/read-pool";
import { sameFolder } from "@/lib/agents";
import { useQueries } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { ChevronDown, ChevronRight, CloudDownload, Download, FolderOpen, Search, Star, Trash2, X } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import { act, openRepoPath, refresh, setBulkProgress, queryClient } from "@/lib/repo";
import { useStore } from "@/lib/store";
import { cn } from "@/lib/utils";
import { Dialog } from "./ui/dialog";

interface Summary {
  exists: boolean;
  branch: string | null;
  changes: number;
  ahead: number;
  behind: number;
}

const name = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;
const same = sameFolder;

export function toggleFavorite(path: string) {
  const s = useStore.getState();
  const fav = s.settings.favorites;
  s.setSettings({ favorites: fav.some((f) => same(f, path)) ? fav.filter((f) => !same(f, path)) : [...fav, path] });
}

/** "Open Favorite Repo N" (Favorites menu). */
export function openFavorite(n: number) {
  const p = useStore.getState().settings.favorites[n - 1];
  if (p) void openRepoPath(p);
  else useStore.getState().toast(false, `There is no favorite repository ${n}. Star one in Repository Management.`);
}

/** Repository Management: open, favorite and recent repositories with their branch and WIP summary,
 *  and fetch / pull / open / remove for one or all of them. */
export function RepoManagementDialog({ onClose }: { onClose: () => void }) {
  const s = useStore();
  const [q, setQ] = useState("");
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const open = [...new Set(s.tabs.filter((t) => t.kind === "repo" && t.path).map((t) => t.path!))];
  const groups: [string, string[]][] = [
    ["Open", open],
    ["Favorites", s.settings.favorites],
    ["Recent", s.settings.recents],
  ];
  const all = [...new Set(groups.flatMap(([, ps]) => ps))];
  const match = (p: string) => !q || p.toLowerCase().includes(q.toLowerCase());
  const visible = new Set(groups.filter(([label]) => !collapsed[label]).flatMap(([, paths]) => paths.filter(match)));
  const visibleKey = [...visible].join("\0");
  const allKey = all.join("\0");
  useEffect(() => {
    for (const path of all) if (!visible.has(path)) void queryClient.cancelQueries({queryKey:["repoSummary",path], exact:true});
  }, [visibleKey, allKey]);
  const summaries = useQueries({
    queries: all.map((p) => ({ queryKey: ["repoSummary", p], queryFn: ({ signal }: {signal: AbortSignal}) => readRepositorySummary(() => invoke<Summary>("repo_summary", { path: p }), signal), enabled: visible.has(p), staleTime: 15000 })),
  });
  const sum = (p: string) => summaries[all.indexOf(p)]?.data;


  const runAll = async (paths: string[], what: "fetch" | "pull") => {
    const ok = paths.filter((p) => sum(p)?.exists);
    if (!ok.length) return s.toast(false, what === "fetch" ? "No repositories to fetch" : "No repositories to pull");
    let failed = 0;
    let next = 0;
    let completed = 0;
    const worker = async () => {
      while (next < ok.length) {
        const p = ok[next++];
        setBulkProgress(`${what === "fetch" ? "Fetching" : "Pulling"} repositories (${completed} of ${ok.length} complete)…`);
        try {
          if (!(await act(p, what === "fetch" ? { type: "fetch" } : { type: "pull", mode: s.settings.pullMode === "fetch" ? "ff" : s.settings.pullMode }, true))) failed++;
        } catch (error) { failed++; s.toast(false, `${name(p)}: ${error}`); }
        completed++;
        setBulkProgress(`${what === "fetch" ? "Fetching" : "Pulling"} repositories (${completed} of ${ok.length} complete)…`);
      }
    };
    await Promise.all(Array.from({ length: Math.min(3, ok.length) }, worker));
    setBulkProgress(null);
    summaries.forEach((summary, index) => { if (visible.has(all[index])) void summary.refetch(); });
    if (!failed) s.toast(true, `${what === "fetch" ? "Fetched" : "Pulled"} ${ok.length} repositor${ok.length === 1 ? "y" : "ies"}`);
  };

  const removeRecent = (paths: string[]) => s.setSettings({ recents: s.settings.recents.filter((r) => !paths.some((p) => same(p, r))) });

  return (
    <Dialog open onClose={onClose} title="Repository Management" width={760}>
      <div className="flex items-center gap-2 mb-3">
        <div className="flex-1 flex items-center h-[26px] bg-field rounded-[2px] border border-white/10 focus-within:border-accent">
          <Search size={12} className="ml-1.5 mr-1 text-fg3" />
          <input data-dialog-initial-focus aria-label="Search repositories" value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search repositories" className="flex-1 bg-transparent outline-none text-[12.5px] placeholder:text-fg3" />
        </div>
        <ToolBtn title="Fetch all repositories" onClick={() => runAll(all, "fetch")} icon={<CloudDownload size={14} />} label="Fetch all" />
        <ToolBtn title="Pull all repositories" onClick={() => runAll(all, "pull")} icon={<Download size={14} />} label="Pull all" />
      </div>
      <div className="max-h-[58vh] overflow-y-auto pr-1">
        {groups.map(([title, paths]) => {
          const list = paths.filter(match);
          const isCollapsed = !!collapsed[title];
          return (
            <div key={title} className="mb-2">
              <div className="flex items-center h-[28px] text-[12px] font-semibold tracking-wider text-fg2">
                <button className="flex items-center gap-1 flex-1 text-left" onClick={() => setCollapsed((c) => ({ ...c, [title]: !isCollapsed }))} title={isCollapsed ? "Expand" : "Collapse"}>
                  {isCollapsed ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
                  {title.toUpperCase()} ({list.length})
                </button>
                {list.length > 1 && (
                  <span className="flex gap-3 font-normal tracking-normal">
                    {title !== "Open" && (
                      <button className="text-link hover:underline" onClick={() => list.forEach((p) => openRepoPath(p))}>
                        Open {list.length} repositories
                      </button>
                    )}
                    <button className="text-link hover:underline" onClick={() => runAll(list, "fetch")}>
                      Fetch {list.length}
                    </button>
                    {title === "Recent" && (
                      <button className="text-link hover:underline" onClick={() => removeRecent(list)}>
                        Remove all
                      </button>
                    )}
                  </span>
                )}
              </div>
              {!isCollapsed && list.length === 0 && <div className="pl-6 py-1 text-[12px] text-fg3">{q ? "No results found" : "None yet"}</div>}
              {!isCollapsed &&
                list.map((p) => {
                  const m = sum(p);
                  const fav = s.settings.favorites.some((f) => same(f, p));
                  return (
                    <div key={title + p} className="group flex items-center gap-2 h-[34px] pl-5 pr-2 rounded hover:bg-hoverrow text-[12.5px]">
                      <button title={fav ? "Remove repository from favorites" : "Add repository to favorites"} onClick={() => toggleFavorite(p)}>
                        <Star size={14} className={fav ? "fill-[#e5c07b] text-[#e5c07b]" : "text-fg3 hover:text-white"} />
                      </button>
                      <button className="text-link font-semibold hover:underline shrink-0" onClick={() => (openRepoPath(p), onClose())} title="Open">
                        {name(p)}
                      </button>
                      <span className="text-fg3 truncate flex-1" title={p}>
                        {p}
                      </span>
                      {m && !m.exists && <span className="text-[#ef7a76] text-[11.5px]">Repo is unreachable and may have been deleted</span>}
                      {m?.exists && (
                        <span className="flex items-center gap-2 text-[11.5px] text-fg2 shrink-0">
                          {m.branch && <span className="px-1.5 rounded-[3px] bg-white/8 text-white/85">{m.branch}</span>}
                          {(m.ahead > 0 || m.behind > 0) && (
                            <span title="ahead / behind its upstream">
                              ↑{m.ahead} ↓{m.behind}
                            </span>
                          )}
                          <span className={cn(m.changes ? "text-warn" : "text-fg3")} title="WIP summary">
                            {m.changes ? `${m.changes} change${m.changes === 1 ? "" : "s"}` : "clean"}
                          </span>
                        </span>
                      )}
                      <span className="flex gap-1 opacity-0 group-hover:opacity-100">
                        <IconBtn title="Fetch repository" onClick={() => runAll([p], "fetch")} icon={<CloudDownload size={13} />} />
                        <IconBtn title="Pull repository" onClick={() => runAll([p], "pull")} icon={<Download size={13} />} />
                        <IconBtn title="Open in File Manager" onClick={() => api.openFileManager(p)} icon={<FolderOpen size={13} />} />
                        {title === "Recent" && <IconBtn title="Remove repository" onClick={() => removeRecent([p])} icon={<Trash2 size={13} />} />}
                        {title === "Open" && (
                          <IconBtn
                            title="Close repository"
                            onClick={() => s.tabs.filter((t) => t.path && same(t.path, p)).forEach((t) => s.closeTab(t.id))}
                            icon={<X size={13} />}
                          />
                        )}
                      </span>
                    </div>
                  );
                })}
            </div>
          );
        })}
      </div>
    </Dialog>
  );
}

function ToolBtn({ icon, label, title, onClick }: { icon: React.ReactNode; label: string; title: string; onClick: () => void }) {
  return (
    <button title={title} onClick={onClick} className="flex items-center gap-1.5 h-[26px] px-2.5 rounded-[3px] bg-white/[0.06] border border-white/10 text-[12.5px] text-white/90 hover:bg-white/10">
      {icon}
      {label}
    </button>
  );
}

function IconBtn({ icon, title, onClick }: { icon: React.ReactNode; title: string; onClick: () => void }) {
  return (
    <button title={title} onClick={onClick} className="w-[22px] h-[22px] rounded flex items-center justify-center text-fg2 hover:text-white hover:bg-white/10">
      {icon}
    </button>
  );
}
