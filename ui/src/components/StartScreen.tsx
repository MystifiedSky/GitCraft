import { CloudDownload, ExternalLink, FolderOpen, Rocket, Search, SquarePlus } from "lucide-react";
import { useState } from "react";
import { api } from "@/lib/api";
import { useStore } from "@/lib/store";
import { openRepoPath } from "@/lib/repo";
import { MOD } from "@/lib/utils";

/** "New Tab" screen, laid out like the original: Repositories (Open / Clone / Create, search, Recent)
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
    <div className="h-full overflow-auto bg-app">
      <div className="flex min-h-full pl-[15%] pr-[8%] pt-[30px]">
        <div className="flex-[2.6] min-w-0 pr-6">
          <div className="text-[15px] text-white/90 mb-[12px]">Repositories</div>
          <div className="flex gap-2 mb-[12px]">
            <BigButton icon={<FolderOpen size={14} />} label="Open" title={`Open Repo (${MOD}+O)`} onClick={() => s.setDialog({ kind: "open" })} />
            <BigButton icon={<CloudDownload size={14} />} label="Clone" title={`Clone Repo (${MOD}+N)`} onClick={() => s.setDialog({ kind: "clone" })} />
            <BigButton icon={<SquarePlus size={14} />} label="Create" title={`Init Repo (${MOD}+I)`} onClick={() => s.setDialog({ kind: "init" })} />
          </div>
          <div className="flex items-center h-[24px] bg-field rounded-[2px] border border-white/10 focus-within:border-accent mb-[14px]">
            <Search size={12} className="ml-1.5 mr-1 text-fg3" />
            <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search repositories (Ctrl + Alt + F)" className="flex-1 bg-transparent outline-none text-[11.5px] placeholder:text-fg3" />
          </div>
          <div className="text-[12px] text-fg mb-1">Recent</div>
          {recents.length === 0 && (
            <div className="text-[12.5px] text-fg3 py-1">
              No recent repositories yet.{" "}
              <button className="text-link hover:underline" onClick={openSample}>
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
        <div className="w-px bg-white/8 mx-3 shrink-0" />
        <div className="flex-[1.15] min-w-[200px] pl-3">
          <div className="text-[15px] text-white/90 mb-[12px]">Connect More Integrations</div>
          <p className="text-[12px] text-fg leading-[16px] mb-[10px]">
            Pull requests, issues and CI/CD from GitHub, GitLab, Bitbucket and Azure DevOps come to GitCraft in Stage 6 of the plan.
          </p>
          <button
            onClick={() => s.setDialog({ kind: "plan" })}
            className="inline-flex items-center gap-2 h-[34px] px-2.5 rounded-[3px] bg-white/[0.06] border border-white/10 text-[13px] text-white/90 hover:bg-white/10 mb-[20px]"
          >
            See the Feature Plan <ExternalLink size={13} />
          </button>
          <div className="text-[12px] text-fg mb-1">Resources</div>
          <div className="flex flex-col items-start text-[12px] leading-[17px]">
            <button className="text-link hover:underline" onClick={openSample}>
              {busy ? "Preparing…" : "Try the sample project"}
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

export function LaunchpadScreen() {
  return (
    <div className="h-full flex items-center justify-center bg-app">
      <div className="text-center max-w-[420px]">
        <Rocket size={40} className="mx-auto mb-3 text-link" />
        <div className="text-[18px] text-white font-semibold mb-2">Launchpad</div>
        <div className="text-[13px] text-fg2 leading-relaxed">
          One place for your pull requests, issues and work in progress across every repository. It needs connections to GitHub, GitLab and other services, which arrive in Stage 6 of the plan.
        </div>
      </div>
    </div>
  );
}
