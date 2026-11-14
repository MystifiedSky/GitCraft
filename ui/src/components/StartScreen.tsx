import { CircleDot, CloudDownload, ExternalLink, FolderOpen, GitPullRequest, Rocket, Search, SquarePlus } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { hosting } from "@/lib/hosting";
import { timeSince } from "@/lib/utils";
import { useState } from "react";
import { api } from "@/lib/api";
import { useStore } from "@/lib/store";
import { openRepoPath } from "@/lib/repo";
import { openWorkspace } from "@/lib/prefs";
import { MOD } from "@/lib/utils";
import { RELEASES_URL } from "@/lib/updates";

/** "New Tab" screen: Repositories (Open / Clone / Create, search, Recent)
 *  on the left and integrations / resources on the right. */
export function StartScreen() {
  const s = useStore();
  const [q, setQ] = useState("");
  const [busy, setBusy] = useState(false);
  const recents = s.settings.recents.filter((r) => r.toLowerCase().includes(q.toLowerCase()));

  const openSample = async () => {
    setBusy(true);
    try {
      const p = await api.sampleRepo();
      await openRepoPath(p);
    } catch (e) {
      s.toast(false, String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex-1 min-h-0 overflow-auto bg-app">
      <div className="start-layout flex min-h-full pl-[15%] pr-[8%] pt-[30px]">
        <div className="start-repositories flex-[2.6] min-w-0 pr-6">
          <div className="text-[15px] text-white/90 mb-[12px]">Repositories</div>
          <div className="flex gap-2 mb-[12px]">
            <BigButton icon={<FolderOpen size={14} />} label="Open" title={`Open Repo (${MOD}+O)`} onClick={() => s.setDialog({ kind: "open" })} />
            <BigButton icon={<CloudDownload size={14} />} label="Clone" title={`Clone Repo (${MOD}+N)`} onClick={() => s.setDialog({ kind: "clone" })} />
            <BigButton icon={<SquarePlus size={14} />} label="Create" title={`Init Repo (${MOD}+I)`} onClick={() => s.setDialog({ kind: "init" })} />
          </div>
          <div className="flex items-center h-[24px] bg-field rounded-[2px] border border-white/10 focus-within:border-accent mb-[14px]">
            <Search size={12} className="ml-1.5 mr-1 text-fg3" />
            <input aria-label="Search repositories" data-start-filter value={q} onChange={(e) => setQ(e.target.value)} placeholder={`Search repositories (${MOD} + Alt + F)`} className="flex-1 min-w-0 bg-transparent outline-none text-[11.5px] placeholder:text-fg3" />
          </div>
          {s.settings.favorites.length > 0 && (
            <>
              <div className="text-[12px] text-fg mb-1">Favorites</div>
              <div className="mb-[12px]">
                {s.settings.favorites.filter((r) => r.toLowerCase().includes(q.toLowerCase())).map((r) => (
                  <div key={r} className="flex items-baseline gap-2.5 h-[17px] text-[12px] whitespace-nowrap">
                    <button onClick={() => openRepoPath(r)} className="text-link font-semibold hover:underline shrink-0">
                      {r.split(/[\\/]/).pop()}
                    </button>
                    <span className="text-fg2 truncate">{r}</span>
                  </div>
                ))}
              </div>
            </>
          )}
          <div className="flex items-baseline text-[12px] text-fg mb-1">
            <span className="flex-1">Recent</span>
            <button className="text-link hover:underline" onClick={() => s.setDialog({ kind: "repoManagement" })}>
              Repository Management
            </button>
          </div>
          {recents.length === 0 && (
            <div className="text-[12.5px] text-fg3 py-1">
              {q.trim() ? `No repositories match “${q}”.` : "No recent repositories yet."}{" "}
              <button disabled={busy} className="text-link hover:underline disabled:opacity-50" onClick={openSample}>
                {busy ? "Preparing the sample project…" : "Try the sample project"}
              </button>
            </div>
          )}
          {recents.map((r) => (
            <div key={r} className="flex items-baseline gap-2.5 h-[17px] text-[12px] whitespace-nowrap">
              <button onClick={() => openRepoPath(r)} className="text-link font-semibold hover:underline shrink-0">
                {r.split(/[\\/]/).pop()}
              </button>
              <span className="text-fg2 truncate">{r}</span>
            </div>
          ))}
        </div>
        <div className="start-divider w-px bg-white/8 mx-3 shrink-0" />
        <div className="start-integrations flex-[1.15] min-w-[200px] pl-3">
          <div className="text-[15px] text-white/90 mb-[12px]">Connect More Integrations</div>
          <p className="text-[12px] text-fg leading-[16px] mb-[10px]">
            Connect GitHub or GitLab to see pull requests and issues in the left panel and on the Launchpad.
          </p>
          <button
            onClick={() => s.setDialog({ kind: "accounts" })}
            className="inline-flex items-center gap-2 h-[34px] px-2.5 rounded-[3px] bg-white/[0.06] border border-white/10 text-[13px] text-white/90 hover:bg-white/10 mb-[20px]"
          >
            Connect Integrations <ExternalLink size={13} />
          </button>
          <div className="text-[12px] text-fg mb-1">Workspaces</div>
          <div className="flex flex-col items-start text-[12px] leading-[17px] mb-[20px]">
            {s.settings.workspaces.map((w) => (
              <button key={w.id} className="text-link hover:underline" onClick={() => openWorkspace(w.id)} title={w.repos.join("\n")}>
                {w.name} ({w.repos.length})
              </button>
            ))}
            <button className="text-link hover:underline" onClick={() => s.setDialog({ kind: "workspaces" })}>
              {s.settings.workspaces.length ? "Manage workspaces" : "Create a workspace"}
            </button>
          </div>
          <div className="text-[12px] text-fg mb-1">Resources</div>
          <div className="flex flex-col items-start text-[12px] leading-[17px]">
            <button className="text-link hover:underline" onClick={openSample}>
              {busy ? "Preparing…" : "Try the sample project"}
            </button>
            <button className="text-link hover:underline" onClick={() => hosting.openUrl("https://git-scm.com/book/en/v2/Getting-Started-What-is-Git%3F")}>
              Intro Tutorials
            </button>
            <button className="text-link hover:underline" onClick={() => hosting.openUrl(RELEASES_URL)}>
              Release Notes
            </button>
            <button className="text-link hover:underline inline-flex items-center gap-1" onClick={() => hosting.openUrl("https://git-scm.com/docs")}>
              Documentation <ExternalLink size={12} />
            </button>
            <button className="text-link hover:underline" onClick={() => s.setDialog({ kind: "shortcuts" })}>
              Keyboard Shortcuts
            </button>
            <button className="text-link hover:underline" onClick={() => s.setDialog({ kind: "plan" })}>
              Feature Plan
            </button>
            <button className="text-link hover:underline" onClick={() => s.setDialog({ kind: "about" })}>
              About GitCraft
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

function BigButton({ icon, label, title, onClick }: { icon: React.ReactNode; label: string; title: string; onClick: () => void }) {
  return (
    <button title={title} onClick={onClick} className="flex items-center gap-2 h-[34px] px-2.5 rounded-[3px] bg-white/[0.06] border border-white/10 text-[13.5px] text-white/90 hover:bg-white/10">
      <span className="text-white/85">{icon}</span>
      {label}
    </button>
  );
}

/** Launchpad: open pull requests and issues that involve you, across connected services. */
export function LaunchpadScreen() {
  const s = useStore();
  const units = s.metrics.time_units;
  const q = useQuery({ queryKey: ["launchpad", s.settings.gitlabHost], queryFn: hosting.launchpad, refetchInterval: 5 * 60_000 });
  const status = useQuery({ queryKey: ["hostingStatus", s.settings.gitlabHost], queryFn: hosting.status });
  const connected = !!(status.data?.github || status.data?.gitlab);
  const since = (iso: string) => {
    const t = timeSince(Date.parse(iso) / 1000, units);
    return <span style={{ color: t.color }}>{t.text}</span>;
  };
  return (
    <div className="flex-1 min-h-0 overflow-auto bg-app">
      <div className="max-w-[900px] mx-auto px-8 py-6">
        <div className="flex items-center gap-3 mb-4">
          <Rocket size={22} className="text-link" />
          <div className="text-[18px] text-white font-semibold flex-1">Launchpad</div>
          <button className="text-[12.5px] text-link hover:underline" onClick={() => q.refetch()}>
            {q.isFetching ? "Refreshing…" : "Refresh"}
          </button>
        </div>
        {!connected && !status.isLoading && (
          <div className="rounded border border-white/10 bg-panel p-4 text-[13px] text-fg2 mb-4">
            Connect GitHub or GitLab to see your pull requests and issues here.{" "}
            <button className="text-link hover:underline" onClick={() => s.setDialog({ kind: "accounts" })}>
              Open Accounts
            </button>
          </div>
        )}
        {q.data?.errors.map((e) => (
          <div key={e} className="text-[12px] text-[#ef7a76] mb-1">
            {e}
          </div>
        ))}
        <div className="text-[12px] font-semibold tracking-wider text-fg2 mt-3 mb-1">PULL REQUESTS ({q.data?.pull_requests.length ?? 0})</div>
        <div className="rounded border border-white/8 bg-panel mb-5">
          {q.data?.pull_requests.length === 0 && <div className="px-3 py-3 text-fg3 text-[12.5px]">Nothing needs you right now.</div>}
          {q.data?.pull_requests.map((p) => (
            <button key={p.url} onClick={() => hosting.openUrl(p.url)} className="w-full flex items-center gap-2 px-3 h-[34px] text-left text-[13px] border-b border-white/5 last:border-0 hover:bg-hoverrow">
              <GitPullRequest size={14} className={p.draft ? "text-fg3" : "text-good"} />
              <span className="text-fg3 text-[12px] w-[160px] truncate shrink-0">{p.repo}</span>
              <span className="text-fg3 text-[12px]">#{p.number}</span>
              <span className="truncate flex-1 text-white/90">{p.title}</span>
              <span className="text-[12px] text-fg2">{p.author}</span>
              <span className="text-[12px] w-[34px] text-right">{since(p.updated)}</span>
            </button>
          ))}
        </div>
        <div className="text-[12px] font-semibold tracking-wider text-fg2 mb-1">ISSUES ASSIGNED TO YOU ({q.data?.issues.length ?? 0})</div>
        <div className="rounded border border-white/8 bg-panel">
          {q.data?.issues.length === 0 && <div className="px-3 py-3 text-fg3 text-[12.5px]">No open issues assigned to you.</div>}
          {q.data?.issues.map((i) => (
            <button key={i.url} onClick={() => hosting.openUrl(i.url)} className="w-full flex items-center gap-2 px-3 h-[34px] text-left text-[13px] border-b border-white/5 last:border-0 hover:bg-hoverrow">
              <CircleDot size={14} className="text-good" />
              <span className="text-fg3 text-[12px] w-[160px] truncate shrink-0">{i.repo}</span>
              <span className="text-fg3 text-[12px]">{i.key || `#${i.number}`}</span>
              <span className="truncate flex-1 text-white/90">{i.title}</span>
              <span className="text-[12px] w-[34px] text-right">{since(i.updated)}</span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
