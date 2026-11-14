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
  ArrowLeft,
  ArrowLeftRight,
  EllipsisVertical,
  Folder,
} from "lucide-react";
import { useState, type ReactNode } from "react";
import { api } from "@/lib/api";
import { useStore } from "@/lib/store";
import { cn } from "@/lib/utils";
import { DropMenu } from "../ui/menu";
import { PROFILE_COLORS, ProfileMark } from "../Icons";
import { removeProfile, switchProfile } from "@/lib/prefs";
import { Button } from "../ui/button";
import { AgentsSection, CommitSection, EditorSection, ExperimentalSection, GeneralSection, NotificationsSection, SshSection, TerminalSection, ToolsSection, UiSection } from "./GeneralSections";
import { AiSection, IntegrationsSection, SigningSection } from "./IntegrationSections";
import { ConflictSection, EncodingSection, GitflowSection, HooksSection, IssueTrackerSection, LfsSection, SparseSection, SubmodulesSection } from "./RepoSections";
import { H, Row, Text } from "./controls";

type Item = { id: string; label: string; icon: ReactNode; render: (path: string) => ReactNode };

/** The Preferences tabs. */
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

/** Full-window Preferences: section list on the left, the section on the right. */
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
    <section aria-label="Preferences" className="fixed left-0 right-0 top-[56px] bottom-0 z-[45] flex bg-app">
      <div className="w-[250px] shrink-0 bg-panel border-r border-black/30 overflow-y-auto pb-4">
        <button title="Exit Preferences (Esc)" onClick={onClose} className="w-full flex items-center gap-2 h-[40px] px-3 text-[15px] text-white hover:bg-hoverrow text-left">
          <ArrowLeft size={16} /> Exit Preferences
        </button>
        <div className="px-3 pt-1 pb-1 text-[12px] text-fg2">Current profile</div>
        <div className="flex items-center gap-2 h-[32px] px-4 bg-white/[0.04] text-[13px] text-white">
          <Users size={15} className="text-white/70" />
          <span className="truncate">{s.profiles.find((x) => x.id === s.activeProfile)?.name ?? "Default Profile"}</span>
        </div>
        <div className="px-3 pt-4 pb-1 text-[12px] text-fg2">Preferences</div>
        {GENERAL.map(nav)}
        {path && (
          <>
            <div className="px-3 pt-5 pb-1 text-[12px] text-fg2">Repo-Specific Preferences</div>
            <div className="px-3 pb-1 text-[12px] text-fg2 truncate flex items-center gap-1.5" title={path}>
              <Folder size={12} /> Repo:{tab?.title ?? path.split(/[\\/]/).pop()}
            </div>
            {REPO.map(nav)}
          </>
        )}
      </div>
      <div className="flex-1 min-w-0 overflow-y-auto">
        <div className="max-w-[1000px] px-8 py-6">{item.render(path)}</div>
      </div>
    </section>
  );
}

function ProfilesSection() {
  const q = useQuery({ queryKey: ["accounts", null], queryFn: () => api.accountInfo(null) });
  const toast = useStore((s) => s.toast);
  const profiles = useStore((s) => s.profiles);
  const active = useStore((s) => s.activeProfile);
  const keep = useStore((s) => s.settings.keepGitconfigUpdated);
  const setSettings = useStore((s) => s.setSettings);
  const setDialog = useStore((s) => s.setDialog);
  const save = async (key: string, value: string) => {
    await api.setGitConfig(null, "global", key, value);
    toast(true, "Saved to your global Git config");
    q.refetch();
  };
  const sorted = [...profiles].sort((a, b) => a.name.localeCompare(b.name));
  return (
    <>
      <H>Profiles</H>
      <p className="text-[12.5px] text-fg2 leading-relaxed mb-3 max-w-[880px]">
        GitCraft uses profiles to store your application preferences as well as git config information. Switching profiles also sets your global Git author name and email.
      </p>
      <div className="max-w-[750px] rounded bg-white/[0.04] px-4 py-3 mb-4">
        <div className="text-[15px] text-white mb-1">My Profiles</div>
        <label className="flex items-center gap-2 text-[12.5px] text-fg">
          <input type="checkbox" className="accent-[#80B1EA]" checked={keep} onChange={(e) => setSettings({ keepGitconfigUpdated: e.target.checked })} />
          Keep my .gitconfig updated with my GitCraft Profile preferences
        </label>
        <div className="text-[11px] text-fg3 mt-0.5">When checked, switching profiles updates your global .gitconfig to match, including commit author name and email.</div>
      </div>
      <Button size="sm" className="mb-4" onClick={() => setDialog({ kind: "addProfile" })}>
        + Add a Profile
      </Button>
      <div className="max-w-[890px]">
        <div className="grid grid-cols-[2fr_1.5fr_1.5fr_40px] gap-3 px-3 pb-2 text-[12px] text-fg2 border-b border-white/10">
          <span>Profile Name ▴</span>
          <span>Author Name</span>
          <span>Author Email</span>
          <span />
        </div>
        {sorted.map((p) => (
          <div key={p.id} className={cn("grid grid-cols-[2fr_1.5fr_1.5fr_40px] gap-3 items-center px-3 h-[40px] text-[13px] border", p.id === active ? "border-accent bg-selrow" : "border-transparent hover:bg-hoverrow")}>
            <span className="flex items-center gap-2 min-w-0">
              <ProfileMark color={PROFILE_COLORS[p.icon ?? 0] ?? PROFILE_COLORS[0]} />
              <span className="truncate">{p.name}</span>
              {p.id === active && <span className="text-[11px] text-good">current</span>}
            </span>
            <span className="truncate text-fg2">{p.id === active ? (q.data?.name ?? p.gitName) : p.gitName}</span>
            <span className="truncate text-fg2">{p.id === active ? (q.data?.email ?? p.gitEmail) : p.gitEmail}</span>
            <DropMenu
              align="end"
              items={[
                { label: "Switch to this profile", disabled: p.id === active, onSelect: () => void switchProfile(p.id) },
                { label: "Delete profile", danger: true, disabled: p.id === active || profiles.length <= 1, onSelect: () => removeProfile(p.id) },
              ]}
              trigger={
                <button aria-label={`Options for ${p.name}`} className="w-[24px] h-[24px] flex items-center justify-center text-fg2 hover:text-white">
                  <EllipsisVertical size={15} />
                </button>
              }
            />
          </div>
        ))}
      </div>
      <div className="mt-6 max-w-[620px]">
        <div className="text-[12px] text-fg2 mb-2">Current profile identity (global Git config)</div>
        <Row label="Author Name">
          <Text value={q.data?.name ?? ""} onSave={(v) => save("user.name", v.trim())} />
        </Row>
        <Row label="Author Email">
          <Text value={q.data?.email ?? ""} onSave={(v) => save("user.email", v.trim())} />
        </Row>
      </div>
    </>
  );
}
