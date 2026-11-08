import { FolderOpen, FolderPlus, GitBranch, Download, Search, Sparkles, Rocket } from "lucide-react";
import { useState } from "react";
import { api } from "@/lib/api";
import { useStore } from "@/lib/store";
import { openRepoPath } from "@/lib/repo";
import { MOD } from "@/lib/utils";
import { AppMark } from "./Icons";

/** "New Tab" screen: open, clone, init, the sample project and recent repositories. */
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
      <div className="max-w-[760px] mx-auto px-8 py-8">
        <div className="flex items-center gap-3 mb-6">
          <AppMark size={34} />
          <div>
            <div className="text-[20px] font-bold text-white">GitCraft</div>
            <div className="text-[12.5px] text-fg2">Open a repository to get started.</div>
          </div>
        </div>
        <div className="grid grid-cols-4 gap-3 mb-8">
          <Card icon={<FolderOpen size={22} />} title="Open a repository" sub={`${MOD}+O`} onClick={() => s.setDialog({ kind: "open" })} />
          <Card icon={<Download size={22} />} title="Clone a repository" sub={`${MOD}+N`} onClick={() => s.setDialog({ kind: "clone" })} />
          <Card icon={<FolderPlus size={22} />} title="Start a new repository" sub={`${MOD}+I`} onClick={() => s.setDialog({ kind: "init" })} />
          <Card icon={<Sparkles size={22} />} title={busy ? "Preparing…" : "Try the sample project"} sub="A small repository to explore" onClick={openSample} />
        </div>
        <div className="flex items-center justify-between mb-2">
          <div className="text-[12px] font-semibold tracking-wider text-fg2">REPOSITORIES</div>
          <div className="flex items-center h-[26px] w-[280px] bg-field rounded-[3px] border border-white/10 focus-within:border-accent">
            <Search size={13} className="mx-2 text-fg3" />
            <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search repositories" className="flex-1 bg-transparent outline-none text-[12.5px] placeholder:text-fg3" />
          </div>
        </div>
        <div className="rounded-md border border-white/8 bg-panel overflow-hidden">
          {recents.length === 0 && <div className="px-4 py-6 text-fg3 text-[13px]">No recent repositories yet.</div>}
          {recents.map((r) => (
            <button key={r} onClick={() => openRepoPath(r)} className="w-full flex items-center gap-3 px-4 h-[40px] text-left hover:bg-hoverrow border-b border-white/5 last:border-0">
              <GitBranch size={15} className="text-[#15a0bf] shrink-0" />
              <span className="text-[13.5px] text-white font-semibold">{r.split(/[\\/]/).pop()}</span>
              <span className="text-[12px] text-fg3 truncate">{r}</span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

function Card({ icon, title, sub, onClick }: { icon: React.ReactNode; title: string; sub: string; onClick: () => void }) {
  return (
    <button onClick={onClick} className="flex flex-col items-start gap-2 p-4 rounded-md bg-panel border border-white/8 hover:border-accent/60 hover:bg-panel2 text-left">
      <span className="text-link">{icon}</span>
      <span className="text-[13.5px] text-white font-semibold leading-tight">{title}</span>
      <span className="text-[11.5px] text-fg3">{sub}</span>
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
