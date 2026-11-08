import {
  Archive,
  Bot,
  ChevronLeft,
  ChevronRight,
  Check,
  Cloud,
  CloudCog,
  FlaskConical,
  Folder,
  GitPullRequest,
  Laptop,
  List,
  Package,
  Search,
  Tag,
  Users,
  CircleDot,
  Network,
} from "lucide-react";
import { useMemo, useRef, useState, type ReactNode } from "react";
import type { GitRef, RepoView } from "@/lib/api";
import { useStore } from "@/lib/store";
import { act } from "@/lib/repo";
import { branchMenu, stashMenu } from "@/lib/menus";
import { CtxMenu } from "./ui/menu";
import { cn } from "@/lib/utils";

type SectionKey = "local" | "remote" | "worktrees" | "stashes" | "tags" | "submodules" | "cloud" | "prs" | "issues" | "teams";

/** Left panel: branches, remotes, worktrees, stashes, tags and the service sections. */
export function LeftPanel({ repo, tabId, onSelectSha }: { repo: RepoView | undefined; tabId: string; onSelectSha: (sha: string) => void }) {
  const s = useStore();
  const [open, setOpen] = useState<Record<SectionKey, boolean>>({} as Record<SectionKey, boolean>);
  const [filter, setFilter] = useState("");
  const filterRef = useRef<HTMLInputElement>(null);
  (window as unknown as { focusLeftFilter?: () => void }).focusLeftFilter = () => filterRef.current?.focus();
  const f = filter.trim().toLowerCase();
  const match = (n: string) => !f || n.toLowerCase().includes(f);

  const locals = useMemo(() => (repo?.refs ?? []).filter((r) => r.kind === "Local" && match(r.name)), [repo, f]);
  const remotes = useMemo(() => (repo?.refs ?? []).filter((r) => r.kind === "Remote" && match(r.name)), [repo, f]);
  const tags = useMemo(() => (repo?.refs ?? []).filter((r) => r.kind === "Tag" && match(r.name)), [repo, f]);
  const stashes = (repo?.stashes ?? []).filter((x) => match(x.message));
  const worktrees = (repo?.worktrees ?? []).filter((w) => match(w.path) || match(w.branch));
  const viewing = (repo?.refs ?? []).filter((r) => r.kind !== "Tag").length;
  const isOpen = (k: SectionKey) => (f ? true : !!open[k]);
  const toggle = (k: SectionKey) => setOpen((o) => ({ ...o, [k]: !o[k] }));

  if (s.settings.leftView === "agents") {
    return (
      <Shell tabId={tabId}>
        <div className="px-4 py-6 text-fg2 text-[13px] leading-relaxed">
          <Bot size={28} className="mb-2 opacity-70" />
          The Agents view watches AI coding sessions running in your repositories. It is planned for Stage 6.
        </div>
      </Shell>
    );
  }

  const branchRow = (r: GitRef, indent: number, label: string) => {
    const isHead = r.kind === "Local" && r.name === repo?.head_branch;
    return (
      <CtxMenu key={r.full} items={() => (repo ? branchMenu(repo, r) : [])}>
        <Row
          indent={indent}
          onClick={() => onSelectSha(r.sha)}
          onDoubleClick={() =>
            repo && !isHead && act(repo.path, r.kind === "Remote" ? { type: "checkoutRemote", remoteBranch: r.name } : { type: "checkout", target: r.name })
          }
          className={isHead ? "bg-good/20 text-white" : undefined}
          title={isHead ? "Checked out" : "Double-click to checkout"}
        >
          {isHead ? <Check size={13} className="shrink-0 text-good" /> : r.kind === "Local" ? <Laptop size={13} className="shrink-0 opacity-60" /> : <Cloud size={13} className="shrink-0 opacity-60" />}
          <span className={cn("truncate flex-1", isHead && "font-semibold")}>{label}</span>
          {r.ahead > 0 && <Badge title={`${r.ahead} ahead`}>{r.ahead}↑</Badge>}
          {r.behind > 0 && <Badge title={`${r.behind} behind`}>{r.behind}↓</Badge>}
        </Row>
      </CtxMenu>
    );
  };

  // Group local branches by folder ("feature/x" -> folder "feature").
  const localTree = groupByFolder(locals);
  const byRemote = new Map<string, GitRef[]>();
  for (const r of remotes) {
    const [rem] = r.name.split("/");
    byRemote.set(rem, [...(byRemote.get(rem) ?? []), r]);
  }
  for (const rem of repo?.remotes ?? []) if (!byRemote.has(rem) && !f) byRemote.set(rem, []);

  return (
    <Shell tabId={tabId}>
      <div className="px-[14px] pt-[17px] pb-[6px] text-[13px] text-fg">
        Viewing <span className="text-link font-semibold">{viewing}</span>
      </div>
      <div className="px-[9px] pb-[12px]">
        <div className="flex items-center h-[22px] bg-field rounded-[3px] border border-white/10 focus-within:border-accent">
          <input
            ref={filterRef}
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && (setFilter(""), filterRef.current?.blur())}
            placeholder="Filter (Ctrl + Alt + f)"
            className="flex-1 bg-transparent outline-none px-[5px] text-[12.5px] placeholder:text-fg3 min-w-0"
          />
          <Search size={14} className="mr-[5px] text-white/70" />
        </div>
      </div>
      <div className="flex-1 overflow-y-auto overflow-x-hidden pb-4">
        <Section title="LOCAL" icon={<Laptop size={14} />} count={locals.length} open={isOpen("local")} onToggle={() => toggle("local")}>
          {localTree.map((g) =>
            g.folder ? (
              <div key={g.folder}>
                <Row indent={1}>
                  <Folder size={13} className="opacity-60" />
                  <span className="truncate">{g.folder}</span>
                </Row>
                {g.items.map((r) => branchRow(r, 2, r.name.slice(g.folder!.length + 1)))}
              </div>
            ) : (
              g.items.map((r) => branchRow(r, 1, r.name))
            ),
          )}
        </Section>
        <Section title="REMOTE" icon={<Cloud size={14} />} count={byRemote.size} open={isOpen("remote")} onToggle={() => toggle("remote")}>
          {[...byRemote.entries()].map(([rem, refs]) => (
            <div key={rem}>
              <Row indent={1}>
                <Cloud size={13} className="opacity-60" />
                <span className="truncate font-semibold">{rem}</span>
              </Row>
              {refs.map((r) => branchRow(r, 2, r.name.slice(rem.length + 1)))}
            </div>
          ))}
        </Section>
        <Section title="WORKTREES" icon={<Network size={14} />} count={worktrees.length} open={isOpen("worktrees")} onToggle={() => toggle("worktrees")}>
          {worktrees.map((w, i) => (
            <Row key={w.path} indent={1} title={w.path}>
              {i === 0 ? <Check size={13} className="text-good" /> : <Folder size={13} className="opacity-60" />}
              <span className="truncate">{w.branch || w.path}</span>
              <span className="text-fg3 text-[11px] truncate">{i === 0 ? "main worktree" : w.path}</span>
            </Row>
          ))}
        </Section>
        {(repo?.stashes.length ?? 0) > 0 && (
          <Section title="STASHES" icon={<Archive size={14} />} count={stashes.length} open={isOpen("stashes")} onToggle={() => toggle("stashes")}>
            {stashes.map((st) => (
              <CtxMenu key={st.sha} items={() => (repo ? stashMenu(repo, st) : [])}>
                <Row indent={1} title={st.message} onDoubleClick={() => repo && act(repo.path, { type: "stashApply", index: st.index })}>
                  <Archive size={13} className="opacity-60" />
                  <span className="truncate">{st.message}</span>
                </Row>
              </CtxMenu>
            ))}
          </Section>
        )}
        <Section title="CLOUD PATCHES" icon={<CloudCog size={14} />} count={0} open={isOpen("cloud")} onToggle={() => toggle("cloud")}>
          <Note>Share changes as a patch without pushing a branch. Planned for Stage 6.</Note>
        </Section>
        <Section title="PULL REQUESTS" icon={<GitPullRequest size={14} />} count={0} open={isOpen("prs")} onToggle={() => toggle("prs")}>
          <Note>Connect GitHub, GitLab, Bitbucket or Azure DevOps to see pull requests. Planned for Stage 6.</Note>
        </Section>
        <Section title="ISSUES" icon={<CircleDot size={14} />} open={isOpen("issues")} onToggle={() => toggle("issues")}>
          <Note>Connect an issue tracker (GitHub, GitLab, Jira, Trello). Planned for Stage 6.</Note>
        </Section>
        <Section title="TEAMS" icon={<Users size={14} />} open={isOpen("teams")} onToggle={() => toggle("teams")}>
          <Note>See what teammates are working on and avoid conflicts. Planned for Stage 6.</Note>
        </Section>
        {tags.length > 0 && (
          <Section title="TAGS" icon={<Tag size={14} />} count={tags.length} open={isOpen("tags")} onToggle={() => toggle("tags")}>
            {tags.map((t) => (
              <CtxMenu key={t.full} items={() => (repo ? branchMenu(repo, t) : [])}>
                <Row indent={1} onClick={() => onSelectSha(t.sha)}>
                  <Tag size={13} className="opacity-60" />
                  <span className="truncate">{t.name}</span>
                </Row>
              </CtxMenu>
            ))}
          </Section>
        )}
        {(repo?.submodules.length ?? 0) > 0 && (
          <Section title="SUBMODULES" icon={<Package size={14} />} count={repo!.submodules.length} open={isOpen("submodules")} onToggle={() => toggle("submodules")}>
            {repo!.submodules.map((m) => (
              <Row key={m} indent={1}>
                <Package size={13} className="opacity-60" />
                <span className="truncate">{m}</span>
              </Row>
            ))}
          </Section>
        )}
        {false && <FlaskConical />}
      </div>
    </Shell>
  );
}

function Shell({ children, tabId }: { children: ReactNode; tabId: string }) {
  const s = useStore();
  void tabId;
  const view = s.settings.leftView;
  return (
    <div className="h-full flex flex-col bg-panel min-w-0">
      <div className="flex items-center h-[34px] mt-[11px] px-[11px] gap-[9px] shrink-0">
        <button
          onClick={() => s.setSettings({ showLeft: false })}
          title="Collapse Panel (Ctrl+J)"
          className="w-[17px] h-[17px] rounded-full bg-white/85 text-panel flex items-center justify-center hover:bg-white shrink-0"
        >
          <ChevronLeft size={13} strokeWidth={3} />
        </button>
        <div className="flex flex-1 h-[24px] rounded-[3px] overflow-hidden border border-white/10">
          <button
            onClick={() => s.setSettings({ leftView: "list" })}
            className={cn("flex-1 flex items-center justify-center gap-1.5 text-[13px]", view === "list" ? "bg-[#2e3d5a] text-white border border-accent/70" : "bg-white/5 text-fg2 hover:text-white")}
          >
            <List size={14} /> List
          </button>
          <button
            onClick={() => s.setSettings({ leftView: "agents" })}
            className={cn("flex-1 flex items-center justify-center gap-1.5 text-[13px]", view === "agents" ? "bg-[#2e3d5a] text-white border border-accent/70" : "bg-white/5 text-fg2 hover:text-white")}
          >
            <Bot size={14} /> Agents
          </button>
        </div>
      </div>
      {children}
    </div>
  );
}

function Section({ title, icon, count, open, onToggle, children }: { title: string; icon: ReactNode; count?: number; open: boolean; onToggle: () => void; children?: ReactNode }) {
  return (
    <div>
      <button onClick={onToggle} className="w-full flex items-center h-[30px] pl-[8px] pr-[14px] text-[12.5px] font-semibold text-fg hover:bg-white/5 tracking-[0.01em]">
        <ChevronRight size={14} className={cn("mr-[7px] transition-transform text-white/70", open && "rotate-90")} />
        <span className="mr-[6px] text-white/75">{icon}</span>
        <span className="flex-1 text-left">{title}</span>
        {count !== undefined && <span className="font-normal text-fg">{count}</span>}
      </button>
      {open && <div className="pb-1">{children}</div>}
    </div>
  );
}

function Row({
  children,
  indent,
  onClick,
  onDoubleClick,
  className,
  title,
}: {
  children: ReactNode;
  indent: number;
  onClick?: () => void;
  onDoubleClick?: () => void;
  className?: string;
  title?: string;
}) {
  return (
    <div
      title={title}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      className={cn("flex items-center gap-[7px] h-[24px] pr-3 text-[13px] text-fg hover:bg-hoverrow cursor-default", className)}
      style={{ paddingLeft: 12 + indent * 16 }}
    >
      {children}
    </div>
  );
}

function Badge({ children, title }: { children: ReactNode; title: string }) {
  return (
    <span title={title} className="text-[10.5px] px-1 rounded bg-white/10 text-fg2">
      {children}
    </span>
  );
}

function Note({ children }: { children: ReactNode }) {
  return <div className="pl-[44px] pr-4 py-1 text-[12px] text-fg3 leading-snug">{children}</div>;
}

function groupByFolder(refs: GitRef[]) {
  const out: { folder: string | null; items: GitRef[] }[] = [];
  const folders = new Map<string, GitRef[]>();
  const plain: GitRef[] = [];
  for (const r of refs) {
    const i = r.name.indexOf("/");
    if (i > 0) {
      const f = r.name.slice(0, i);
      folders.set(f, [...(folders.get(f) ?? []), r]);
    } else plain.push(r);
  }
  for (const [folder, items] of folders) out.push({ folder, items });
  out.push({ folder: null, items: plain });
  return out;
}
