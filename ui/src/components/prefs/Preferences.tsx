import { useQuery } from "@tanstack/react-query";
import {
  Bell,
  Bot,
  Brackets,
  Code,
  Cog,
  FileSignature,
  FileText,
  FlaskConical,
  FolderTree,
  GitCommitHorizontal,
  GitMerge,
  Layers,
  ListTodo,
  Paintbrush,
  Plug,
  ShieldCheck,
  Sparkles,
  SquareTerminal,
  Type,
  Users,
  Wrench,
  X,
  ArrowLeftRight,
} from "lucide-react";
import { useState, type ReactNode } from "react";
import { api } from "@/lib/api";
import { useStore } from "@/lib/store";
import { cn } from "@/lib/utils";
import { ProfilesPanel } from "../Dialogs";
import { AgentsSection, CommitSection, EditorSection, ExperimentalSection, GeneralSection, NotificationsSection, SshSection, TerminalSection, ToolsSection, UiSection } from "./GeneralSections";
import { AiSection, IntegrationsSection, SigningSection } from "./IntegrationSections";
import { ConflictSection, EncodingSection, GitflowSection, HooksSection, IssueTrackerSection, LfsSection, SparseSection, SubmodulesSection } from "./RepoSections";
import { H, Row, Text } from "./controls";

type Item = { id: string; label: string; icon: ReactNode; render: (path: string) => ReactNode };

/** The original's Preferences tabs (render.bundle.js labelAndIconByTabType). */
const GENERAL: Item[] = [
  { id: "general", label: "General", icon: <Cog size={15} />, render: () => <GeneralSection /> },
  { id: "profiles", label: "Profiles", icon: <Users size={15} />, render: () => <ProfilesSection /> },
  { id: "ssh", label: "SSH", icon: <ShieldCheck size={15} />, render: () => <SshSection /> },
  { id: "integrations", label: "Integrations", icon: <Plug size={15} />, render: () => <IntegrationsSection /> },
  { id: "gpg", label: "Commit Signing", icon: <FileSignature size={15} />, render: () => <SigningSection /> },
  { id: "ui", label: "UI Customization", icon: <Paintbrush size={15} />, render: () => <UiSection /> },
  { id: "notifications", label: "Notifications", icon: <Bell size={15} />, render: () => <NotificationsSection /> },
  { id: "editor", label: "Editor", icon: <Code size={15} />, render: () => <EditorSection /> },
  { id: "cli", label: "In-App Terminal", icon: <SquareTerminal size={15} />, render: () => <TerminalSection /> },
  { id: "tools", label: "External Tools", icon: <Wrench size={15} />, render: () => <ToolsSection /> },
  { id: "ai", label: "AI", icon: <Sparkles size={15} />, render: () => <AiSection /> },
  { id: "agents", label: "Agents", icon: <Bot size={15} />, render: () => <AgentsSection /> },
  { id: "commit", label: "Commit", icon: <GitCommitHorizontal size={15} />, render: () => <CommitSection /> },
  { id: "experimental", label: "Experimental", icon: <FlaskConical size={15} />, render: () => <ExperimentalSection /> },
];

const REPO: Item[] = [
  { id: "gitflow", label: "Gitflow", icon: <ArrowLeftRight size={15} />, render: (p) => <GitflowSection path={p} /> },
  { id: "githooks", label: "Git Hooks", icon: <Brackets size={15} />, render: (p) => <HooksSection path={p} /> },
  { id: "lfs", label: "LFS", icon: <FileText size={15} />, render: (p) => <LfsSection path={p} /> },
  { id: "encoding", label: "Encoding", icon: <Type size={15} />, render: (p) => <EncodingSection path={p} /> },
  { id: "conflict", label: "Conflict Prevention", icon: <GitMerge size={15} />, render: (p) => <ConflictSection path={p} /> },
  { id: "issues", label: "Issue Tracker", icon: <ListTodo size={15} />, render: (p) => <IssueTrackerSection path={p} /> },
  { id: "sparse", label: "Sparse Checkout", icon: <FolderTree size={15} />, render: (p) => <SparseSection path={p} /> },
  { id: "submodules", label: "Submodules", icon: <Layers size={15} />, render: (p) => <SubmodulesSection path={p} /> },
];

/** Full-window Preferences, like the original: section list on the left, the section on the right. */
export function PreferencesScreen({ onClose, section }: { onClose: () => void; section?: string }) {
  const s = useStore();
  const tab = s.tabs.find((t) => t.id === s.activeId);
  const path = tab?.kind === "repo" ? tab.path ?? "" : "";
  const [sel, setSel] = useState(section ?? "general");
  const item = [...GENERAL, ...(path ? REPO : [])].find((x) => x.id === sel) ?? GENERAL[0];
  const nav = (x: Item) => (
    <button key={x.id} onClick={() => setSel(x.id)} className={cn("w-full flex items-center gap-2.5 h-[32px] px-4 text-[13px] text-left", sel === x.id ? "bg-selrow text-white" : "text-fg hover:bg-hoverrow")}>
      <span className="text-white/70 w-[16px] flex justify-center">{x.icon}</span>
      {x.label}
    </button>
  );
  return (
    <div className="fixed left-0 right-0 top-[56px] bottom-0 z-[45] flex bg-app" onKeyDown={(e) => e.key === "Escape" && onClose()}>
      <div className="w-[250px] shrink-0 bg-panel border-r border-black/30 overflow-y-auto pb-4">
        <div className="flex items-center h-[48px] px-4 text-[16px] font-semibold text-white">Preferences</div>
        {GENERAL.map(nav)}
        {path && (
          <>
            <div className="px-4 pt-5 pb-1 text-[11px] font-semibold tracking-wider text-fg3">REPO-SPECIFIC PREFERENCES</div>
            <div className="px-4 pb-1 text-[12px] text-fg2 truncate" title={path}>
              Repo: {tab?.title ?? path.split(/[\\/]/).pop()}
            </div>
            {REPO.map(nav)}
          </>
        )}
      </div>
      <div className="flex-1 min-w-0 overflow-y-auto">
        <div className="max-w-[1000px] px-8 py-6">{item.render(path)}</div>
      </div>
      <button title="Exit Preferences (Esc)" onClick={onClose} className="absolute top-3 right-4 h-[30px] px-3 rounded-[3px] border border-white/10 bg-white/[0.06] text-[12.5px] text-fg hover:bg-white/10 flex items-center gap-1.5">
        <X size={14} /> Exit Preferences
      </button>
    </div>
  );
}

function ProfilesSection() {
  const q = useQuery({ queryKey: ["accounts", null], queryFn: () => api.accountInfo(null) });
  const toast = useStore((s) => s.toast);
  const save = async (key: string, value: string) => {
    await api.setGitConfig(null, "global", key, value);
    toast(true, "Saved to your global Git config");
    q.refetch();
  };
  return (
    <>
      <H>Profiles</H>
      <p className="text-[12.5px] text-fg2 leading-relaxed mb-3 max-w-[760px]">
        GitCraft uses profiles to store your application preferences as well as git config information. Switching profiles also sets your global Git author name and email.
      </p>
      <Row label="Author Name">
        <Text value={q.data?.name ?? ""} onSave={(v) => save("user.name", v.trim())} />
      </Row>
      <Row label="Author Email">
        <Text value={q.data?.email ?? ""} onSave={(v) => save("user.email", v.trim())} />
      </Row>
      <div className="mt-5">
        <ProfilesPanel />
      </div>
    </>
  );
}
