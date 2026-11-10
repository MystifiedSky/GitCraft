import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import { act, refresh } from "@/lib/repo";
import { addSubmodule, initGitflow } from "@/lib/manage";
import { useStore, type RepoPrefs } from "@/lib/store";
import { tools } from "@/lib/integrations";
import { Area, Btn, H, Note, PathField, Row, Select, Text, Toggle } from "./controls";

/** Per-repository preferences live in settings.repoPrefs[path]. */
export function useRepoPrefs(path: string) {
  const s = useStore();
  const p: RepoPrefs = s.settings.repoPrefs[path] ?? {};
  const put = (patch: Partial<RepoPrefs>) => s.setSettings({ repoPrefs: { ...s.settings.repoPrefs, [path]: { ...p, ...patch } } });
  return { p, put, s };
}

export function GitflowSection({ path }: { path: string }) {
  const q = useQuery({ queryKey: ["gitflowConfig", path], queryFn: () => api.gitflowConfig(path) });
  const c = q.data;
  return (
    <>
      <H>Gitflow</H>
      <Note>Gitflow keeps feature, release and hotfix work on separate branches. GitCraft runs it itself, no extra tools needed.</Note>
      {c && (
        <>
          <Row label="Status">{c.initialized ? <span className="text-good text-[13px]">Initialized</span> : <span className="text-fg3 text-[13px]">Not initialized</span>}</Row>
          {c.initialized &&
            (
              [
                ["Production branch", c.master],
                ["Development branch", c.develop],
                ["Feature prefix", c.feature],
                ["Release prefix", c.release],
                ["Hotfix prefix", c.hotfix],
                ["Version tag prefix", c.versiontag || "(none)"],
              ] as const
            ).map(([l, v]) => (
              <Row key={l} label={l}>
                <span className="mono text-[13px] text-fg">{v}</span>
              </Row>
            ))}
        </>
      )}
      <Row label={c?.initialized ? "Change the settings" : "Initialize Gitflow"}>
        <Btn primary onClick={() => initGitflow(path).then(() => q.refetch())}>
          {c?.initialized ? "Edit Gitflow" : "Initialize Gitflow"}
        </Btn>
      </Row>
    </>
  );
}

export function HooksSection({ path }: { path: string }) {
  const q = useQuery({ queryKey: ["hooksPath", path], queryFn: () => tools.configGet(path, "local", ["core.hooksPath"]) });
  const toast = useStore((s) => s.toast);
  const value = q.data?.["core.hooksPath"] ?? "";
  return (
    <>
      <H>Git Hooks</H>
      <Note>Git hooks are shell scripts that execute after an event such as a commit or push. Git for Windows runs them with its own sh, so no extra setup is needed.</Note>
      <Row label="Git Hooks directory" help="The directory that Git hooks will use for this repository and its linked worktrees. Empty = .git/hooks.">
        <PathField
          folder
          title="Select a directory to use"
          value={value}
          placeholder=".git/hooks"
          onSave={async (v) => {
            try {
              await api.setGitConfig(path, "local", "core.hooksPath", v.trim());
              q.refetch();
            } catch {
              toast(false, "Invalid Git hooks directory.");
            }
          }}
        />
      </Row>
    </>
  );
}

export function LfsSection({ path }: { path: string }) {
  const q = useQuery({ queryKey: ["lfsInfo", path], queryFn: () => api.lfsInfo(path) });
  const i = q.data;
  const after = () => setTimeout(() => q.refetch(), 300);
  return (
    <>
      <H>LFS</H>
      {!i ? (
        <Note>Loading…</Note>
      ) : !i.available ? (
        <Note>Git LFS isn't installed. Git for Windows includes it; elsewhere install git-lfs.</Note>
      ) : (
        <>
          <Row label="Git LFS" help={i.version}>
            {i.enabled ? <span className="text-good text-[13px]">Initialized for this repository</span> : <Btn primary onClick={() => act(path, { type: "lfsInstall" }).then(after)}>Initialize LFS</Btn>}
          </Row>
          <Row label="Tracked patterns" wide>
            <div className="flex flex-col gap-1 w-full">
              {i.patterns.length === 0 && <span className="text-[12.5px] text-fg3">None</span>}
              {i.patterns.map((p) => (
                <div key={p} className="flex items-center gap-2">
                  <span className="mono text-[12.5px] text-fg flex-1">{p}</span>
                  <Btn onClick={() => act(path, { type: "lfsUntrack", pattern: p }).then(after)}>Untrack</Btn>
                </div>
              ))}
            </div>
          </Row>
          <Row label="Track a pattern">
            <Text value="" placeholder="*.psd" onSave={(v) => v.trim() && act(path, { type: "lfsTrack", pattern: v.trim() }).then(after)} mono />
          </Row>
          <Row label="LFS files">
            <Btn onClick={() => act(path, { type: "lfsPull" })}>Pull LFS files</Btn>
          </Row>
        </>
      )}
    </>
  );
}

const ENCODINGS: [string, string][] = [
  ["", "Default (UTF-8)"],
  ["utf-8", "UTF-8"],
  ["utf-16le", "UTF-16 LE"],
  ["utf-16be", "UTF-16 BE"],
  ["windows-1252", "Western (Windows 1252)"],
  ["iso-8859-1", "Western (ISO 8859-1)"],
  ["iso-8859-2", "Central European (ISO 8859-2)"],
  ["windows-1251", "Cyrillic (Windows 1251)"],
  ["koi8-r", "Cyrillic (KOI8-R)"],
  ["shift_jis", "Japanese (Shift JIS)"],
  ["euc-jp", "Japanese (EUC-JP)"],
  ["gbk", "Chinese Simplified (GBK)"],
  ["big5", "Chinese Traditional (Big5)"],
  ["euc-kr", "Korean (EUC-KR)"],
];

export function EncodingSection({ path }: { path: string }) {
  const { p, put } = useRepoPrefs(path);
  return (
    <>
      <H>Encoding</H>
      <Row label="Default encoding" help="Default encoding when reading or writing files in this repo.">
        <Select value={p.encoding ?? ""} onChange={(v) => put({ encoding: v })} options={ENCODINGS} />
      </Row>
    </>
  );
}

export function ConflictSection({ path }: { path: string }) {
  const { p, put, s } = useRepoPrefs(path);
  const global = s.settings.conflictDetection;
  return (
    <>
      <H>Conflict Prevention</H>
      <Note>GitCraft can automatically compare your branch against its "Base" branch to see if any conflicts arise while you're working.</Note>
      <Row
        label="Enable proactive conflict detection with target branch"
        help="Unchecking this will disable proactive conflict detection only for this repository."
        warning={!global ? "Globally disabled for all repositories. To re-enable, go to Preferences > General." : undefined}
      >
        <Toggle value={p.conflictDetection ?? true} disabled={!global} onChange={(v) => put({ conflictDetection: v })} />
      </Row>
      <Row
        label="Base branches"
        wide
        help={
          'One pattern per line. GitCraft compares your branch with the first branch that matches (the remote\'s default branch when empty).\norigin/main - include a single branch (main) on origin\norigin/** - include all branches on origin\norigin/feature/** - include branches on origin which begin with feature/\n!origin/abc - exclude a single branch (abc) on origin'
        }
      >
        <Area value={p.baseBranches ?? ""} placeholder={"origin/main\norigin/develop"} onSave={(v) => put({ baseBranches: v })} mono />
      </Row>
      <Row label="Branches Excluded from Conflict Prevention" help="No conflict warnings while one of these branches is checked out (one per line, * allowed)." wide>
        <Area value={p.excludedBranches ?? ""} rows={3} onSave={(v) => put({ excludedBranches: v })} mono />
      </Row>
    </>
  );
}

export function IssueTrackerSection({ path }: { path: string }) {
  const { p, put } = useRepoPrefs(path);
  const t = p.tracker ?? { kind: "", project: "" };
  const kinds: [string, string][] = [
    ["", "The repository's hosting service (GitHub / GitLab / Bitbucket / Azure DevOps)"],
    ["jira", "Jira Cloud"],
    ["jira-server", "Jira Data Center"],
    ["trello", "Trello"],
    ["azure", "Azure Boards (Azure DevOps)"],
  ];
  return (
    <>
      <H>Issue Tracker</H>
      <Note>Which issues the ISSUES section shows for this repository. Connect the tracker first in Preferences &gt; Integrations.</Note>
      <Row label="Issue tracker">
        <Select value={t.kind} onChange={(v) => put({ tracker: { kind: v, project: t.project } })} options={kinds} />
      </Row>
      {t.kind && (
        <Row label={t.kind === "trello" ? "Board id" : t.kind === "azure" ? "Project" : "Project key"} help={t.kind === "trello" ? "The id in the board's address (trello.com/b/<id>/...). Empty shows the cards assigned to you." : "Empty shows the issues assigned to you."}>
          <Text value={t.project} placeholder={t.kind === "trello" ? "abc123XY" : t.kind === "azure" ? "MyProject" : "ABC"} onSave={(v) => put({ tracker: { kind: t.kind, project: v.trim() } })} />
        </Row>
      )}
    </>
  );
}

export function SparseSection({ path }: { path: string }) {
  const q = useQuery({ queryKey: ["sparseInfo", path], queryFn: () => api.sparseInfo(path) });
  const s = useStore();
  return (
    <>
      <H>Sparse Checkout</H>
      <Note>Keep only some folders of a big repository in your working folder.</Note>
      <Row label="Status">{q.data?.enabled ? <span className="text-good text-[13px]">On: {q.data.patterns.join(", ")}</span> : <span className="text-fg3 text-[13px]">Off</span>}</Row>
      <Row label="Folders">
        <Btn primary onClick={() => s.setDialog({ kind: "sparse" })}>
          Choose folders…
        </Btn>
        {q.data?.enabled && (
          <Btn danger onClick={() => act(path, { type: "sparseDisable" }).then(() => q.refetch())}>
            Turn off
          </Btn>
        )}
      </Row>
    </>
  );
}

export function SubmodulesSection({ path }: { path: string }) {
  const q = useQuery({ queryKey: ["submodulesList", path], queryFn: () => api.submodulesList(path) });
  const after = () => {
    q.refetch();
    refresh(path);
  };
  const label = { "in-sync": "in sync", "out-of-sync": "out of sync", uninitialized: "not initialized", conflict: "conflict" } as const;
  return (
    <>
      <H>Submodules</H>
      {(q.data?.length ?? 0) === 0 && <Note>This repository has no submodules.</Note>}
      {q.data?.map((m) => (
        <Row key={m.path} label={<span className="mono">{m.path}</span>} help={m.url}>
          <span className="text-[12.5px] text-fg2 flex-1">{label[m.state]}</span>
          <Btn onClick={() => act(path, { type: "submoduleUpdate", paths: [m.path] }).then(after)}>Update</Btn>
          <Btn danger onClick={() => act(path, { type: "submoduleRemove", path: m.path }).then(after)}>
            Remove
          </Btn>
        </Row>
      ))}
      <Row label="Submodules">
        <Btn onClick={() => addSubmodule(path)}>Add Submodule</Btn>
        {(q.data?.length ?? 0) > 0 && <Btn onClick={() => act(path, { type: "submoduleUpdate", paths: [] }).then(after)}>Update all</Btn>}
      </Row>
    </>
  );
}
