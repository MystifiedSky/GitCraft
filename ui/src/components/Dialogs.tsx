import { SERVICES, integrations } from "@/lib/integrations";
import { PreferencesScreen } from "./prefs/Preferences";
import { RepoManagementDialog } from "./RepoManagement";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import { useStore } from "@/lib/store";
import { act, openHistory, openRepoPath, refresh } from "@/lib/repo";
import type { FormField } from "@/lib/store";
import { fullDate } from "@/lib/utils";
import { Dialog, Field, inputClass } from "./ui/dialog";
import { Button } from "./ui/button";
import { AppMark } from "./Icons";
import { cn, MOD } from "@/lib/utils";
import { COMMANDS, comboOf, keysOf, type Group } from "@/lib/commands";
import { hosting as hostingApi } from "@/lib/hosting";
import { THEMES, addProfile, deleteWorkspace, openWorkspace, removeProfile, saveWorkspace, switchProfile } from "@/lib/prefs";

export function Dialogs() {
  const d = useStore((s) => s.dialog);
  const close = () => useStore.getState().setDialog(null);
  if (!d) return null;
  switch (d.kind) {
    case "prompt":
      return <PromptDialog key={d.title} {...d} onClose={close} />;
    case "confirm":
      return (
        <Dialog
          open
          onClose={close}
          title={d.title}
          footer={
            <>
              <Button onClick={close}>Cancel</Button>
              <Button
                autoFocus
                variant={d.danger ? "danger" : "primary"}
                onClick={() => {
                  close();
                  d.onConfirm();
                }}
              >
                {d.confirm}
              </Button>
            </>
          }
        >
          <p className="text-[13px] text-fg leading-relaxed">{d.text}</p>
        </Dialog>
      );
    case "open":
      return <OpenDialog onClose={close} />;
    case "clone":
      return <CloneDialog onClose={close} />;
    case "init":
      return <InitDialog onClose={close} />;
    case "log":
      return <LogDialog onClose={close} />;
    case "shortcuts":
      return <ShortcutsDialog onClose={close} />;
    case "about":
      return (
        <Dialog open onClose={close} title="About GitCraft" width={380}>
          <div className="flex flex-col items-center gap-2 py-2">
            <AppMark size={48} />
            <div className="text-[18px] font-bold text-white">GitCraft</div>
            <div className="text-fg2 text-[12.5px]">Version {useStore.getState().version} · Stage 6</div>
            <div className="text-fg3 text-[12px] text-center mt-2">A graphical Git client built with Tauri, Rust and React. Every action runs the real Git command line.</div>
          </div>
        </Dialog>
      );
    case "plan":
      return <PlanDialog onClose={close} />;
    case "prefs":
      return <PreferencesScreen onClose={close} section={d.section} />;
    case "choice":
      return (
        <Dialog
          open
          onClose={close}
          title={d.title}
          footer={
            <>
              <Button onClick={close}>Cancel</Button>
              {d.choices.map((c) => (
                <Button
                  key={c.label}
                  variant={c.variant ?? "default"}
                  onClick={() => {
                    close();
                    c.onSelect();
                  }}
                >
                  {c.label}
                </Button>
              ))}
            </>
          }
        >
          <p className="text-[13px] text-fg leading-relaxed whitespace-pre-wrap">{d.text}</p>
        </Dialog>
      );
    case "repoManagement":
      return <RepoManagementDialog onClose={close} />;
    case "info":
      return (
        <Dialog open onClose={close} title={d.title} width={560} footer={<Button variant="primary" onClick={close}>Close</Button>}>
          <div className="text-[13px] text-fg leading-relaxed whitespace-pre-wrap select-text max-h-[60vh] overflow-auto">{d.text}</div>
          <div className="text-[11.5px] text-fg3 mt-3">Generated using your selected AI connection. Review the result before using it.</div>
        </Dialog>
      );
    case "message":
      return <MessageDialog key={d.title} {...d} onClose={close} />;
    case "form":
      return <FormDialog key={d.title} {...d} onClose={close} />;
    case "cleanup":
      return <CleanupDialog onClose={close} />;
    case "sparse":
      return <SparseDialog onClose={close} />;
    case "accounts":
      return <PreferencesScreen onClose={close} section="integrations" />;
    case "profiles":
      return <ProfilesDialog onClose={close} />;
    case "workspaces":
      return <WorkspacesDialog onClose={close} />;
    case "fileFinder":
      return <FileFinder onClose={close} />;
  }
}

function PromptDialog({
  title,
  label,
  value,
  confirm,
  onSubmit,
  extraLabel,
  onClose,
}: {
  title: string;
  label: string;
  value: string;
  confirm: string;
  onSubmit: (v: string, extra?: string) => void;
  extraLabel?: string;
  onClose: () => void;
}) {
  const [v, setV] = useState(value);
  const [x, setX] = useState("");
  const submit = () => {
    if (!v.trim() && !label.includes("optional")) return;
    onClose();
    onSubmit(v.trim(), extraLabel ? x : undefined);
  };
  return (
    <Dialog
      open
      onClose={onClose}
      title={title}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={submit}>
            {confirm}
          </Button>
        </>
      }
    >
      <Field label={label}>
        <input autoFocus className={inputClass} value={v} onChange={(e) => setV(e.target.value)} onKeyDown={(e) => e.key === "Enter" && submit()} />
      </Field>
      {extraLabel && (
        <Field label={extraLabel}>
          <input className={inputClass} value={x} onChange={(e) => setX(e.target.value)} onKeyDown={(e) => e.key === "Enter" && submit()} />
        </Field>
      )}
    </Dialog>
  );
}

function FolderField({ label, value, onChange, title }: { label: string; value: string; onChange: (v: string) => void; title: string }) {
  return (
    <Field label={label}>
      <div className="flex gap-2">
        <input className={cn(inputClass, "flex-1")} value={value} onChange={(e) => onChange(e.target.value)} placeholder="C:\\Users\\you\\Projects" />
        <Button
          onClick={async (e) => {
            e.preventDefault();
            const p = await api.pickFolder(title);
            if (p) onChange(p);
          }}
        >
          Browse
        </Button>
      </div>
    </Field>
  );
}

function OpenDialog({ onClose }: { onClose: () => void }) {
  const [p, setP] = useState("");
  const go = async () => {
    if (!p.trim()) return;
    onClose();
    await openRepoPath(p.trim());
  };
  useEffect(() => {
    // Go straight to the system folder picker, like the original's Open Repo.
    api.pickFolder("Open a Git repository").then((r) => {
      if (r) {
        onClose();
        openRepoPath(r);
      }
    });
  }, []);
  return (
    <Dialog
      open
      onClose={onClose}
      title="Open a repository"
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={go}>
            Open
          </Button>
        </>
      }
    >
      <FolderField label="Repository folder" value={p} onChange={setP} title="Open a Git repository" />
    </Dialog>
  );
}

function CloneDialog({ onClose }: { onClose: () => void }) {
  const s = useStore();
  const [url, setUrl] = useState("");
  const [dir, setDir] = useState(s.settings.defaultCloneFolder);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [shallow, setShallow] = useState(false);
  const [depth, setDepth] = useState("1");
  const go = async () => {
    if (!url.trim() || !dir.trim()) return;
    setBusy(true);
    setError("");
    s.setBusy("Cloning…");
    try {
      const path = await api.cloneRepo(url, dir, shallow ? Math.max(1, Number(depth) || 1) : undefined);
      s.setSettings({ defaultCloneFolder: dir });
      onClose();
      await openRepoPath(path);
      s.toast(true, "Clone finished");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      s.setBusy(null);
    }
  };
  return (
    <Dialog
      open
      onClose={onClose}
      title="Clone a repository"
      width={480}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={busy} onClick={go}>
            {busy ? "Cloning…" : "Clone the repo!"}
          </Button>
        </>
      }
    >
      <HostedRepoPicker onPick={setUrl} />
      <Field label="URL">
        <input autoFocus className={inputClass} value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://github.com/owner/project.git" />
      </Field>
      <FolderField label="Where to clone to" value={dir} onChange={setDir} title="Choose where to clone to" />
      {url && dir && <div className="text-[12px] text-fg3 -mt-1 mb-2">Full path: {dir.replace(/[\\/]$/, "")}\{url.replace(/\/$/, "").split(/[/\\:]/).pop()?.replace(/\.git$/, "")}</div>}
      <label className="flex items-center gap-2 text-[12.5px] text-fg mb-2">
        <input type="checkbox" className="accent-[#80B1EA]" checked={shallow} onChange={(e) => setShallow(e.target.checked)} />
        Shallow clone: only the latest
        <input value={depth} disabled={!shallow} onChange={(e) => setDepth(e.target.value.replace(/\D/g, ""))} className={cn(inputClass, "w-14 h-6")} />
        commits
      </label>
      {error && <div className="text-[12px] text-[#ef7a76] whitespace-pre-wrap">{error}</div>}
    </Dialog>
  );
}

/** "GitCraft will help you find stuff on {service} when cloning": your repositories on connected services. */
function HostedRepoPicker({ onPick }: { onPick: (url: string) => void }) {
  const s = useStore();
  const st = useQuery({ queryKey: ["integrationStatuses", s.settings.integrations], queryFn: () => integrations.statuses(SERVICES.filter((x) => x.kind === "hosting").map((x) => x.id)) });
  const connected = (st.data ?? []).filter((x) => x.connected && !x.error);
  const [svc, setSvc] = useState("");
  const [q, setQ] = useState("");
  const id = svc || connected[0]?.id || "";
  const repos = useQuery({ queryKey: ["hostedRepos", id], queryFn: () => integrations.repos(id), enabled: !!id });
  if (!connected.length)
    return (
      <div className="text-[12px] text-fg3 mb-2">
        Connect GitHub, GitLab, Bitbucket or Azure DevOps to pick from your repositories.{" "}
        <button className="text-link hover:underline" onClick={() => s.setDialog({ kind: "prefs", section: "integrations" })}>
          Integrations
        </button>
      </div>
    );
  const list = (repos.data ?? []).filter((r) => r.full_name.toLowerCase().includes(q.toLowerCase()));
  return (
    <div className="mb-3">
      <div className="flex gap-2 mb-1.5">
        <select className={cn(inputClass, "w-[180px]")} value={id} onChange={(e) => setSvc(e.target.value)}>
          {connected.map((c) => (
            <option key={c.id} value={c.id}>
              {c.label}
            </option>
          ))}
        </select>
        <input className={cn(inputClass, "flex-1")} placeholder="Search your repositories" value={q} onChange={(e) => setQ(e.target.value)} />
      </div>
      <div className="h-[150px] overflow-auto rounded border border-white/8 bg-white/[0.02]">
        {repos.isLoading && <div className="p-2 text-[12px] text-fg3">Loading…</div>}
        {repos.error && <div className="p-2 text-[12px] text-[#ef7a76]">{String(repos.error)}</div>}
        {list.map((r) => (
          <button key={r.full_name} className="w-full flex items-center gap-2 px-2 h-[26px] text-left text-[12.5px] hover:bg-hoverrow" title={r.description} onClick={() => onPick(r.https_url || r.ssh_url)}>
            <span className="truncate text-fg flex-1">{r.full_name}</span>
            {r.private && <span className="text-[11px] text-fg3">private</span>}
          </button>
        ))}
      </div>
    </div>
  );
}

function InitDialog({ onClose }: { onClose: () => void }) {
  const s = useStore();
  const [name, setName] = useState("");
  const [dir, setDir] = useState(s.settings.defaultCloneFolder);
  const [error, setError] = useState("");
  const [branch, setBranch] = useState(s.settings.defaultBranch);
  const [ignores, setIgnores] = useState<string[]>([]);
  const [license, setLicense] = useState("");
  const [readme, setReadme] = useState(true);
  const templates = useQuery({ queryKey: ["gitignoreTemplates"], queryFn: api.gitignoreTemplates, staleTime: Infinity });
  const go = async () => {
    if (!name.trim() || !dir.trim()) return;
    try {
      const path = await api.initRepo(dir, name, { branch: branch || s.settings.defaultBranch, gitignore: ignores, license, readme });
      onClose();
      await openRepoPath(path);
      s.toast(true, "Repository created");
    } catch (e) {
      setError(String(e));
    }
  };
  return (
    <Dialog
      open
      onClose={onClose}
      title="Start a new repository"
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={go}>
            Create Repository
          </Button>
        </>
      }
    >
      <Field label="Name">
        <input autoFocus className={inputClass} value={name} onChange={(e) => setName(e.target.value)} placeholder="my-project" />
      </Field>
      <FolderField label="Initialize in" value={dir} onChange={setDir} title="Choose a parent folder" />
      <Field label="Default branch name">
        <input className={inputClass} value={branch} onChange={(e) => setBranch(e.target.value)} />
      </Field>
      <div className="text-[12px] text-fg2 mb-1">.gitignore template</div>
      <div className="flex flex-wrap gap-x-4 gap-y-1 mb-3">
        {(templates.data ?? []).map((t) => (
          <label key={t} className="flex items-center gap-1.5 text-[12.5px] text-fg cursor-pointer">
            <input type="checkbox" className="accent-[#80B1EA]" checked={ignores.includes(t)} onChange={() => setIgnores(ignores.includes(t) ? ignores.filter((x) => x !== t) : [...ignores, t])} />
            {t}
          </label>
        ))}
      </div>
      <div className="flex gap-4 items-center mb-2">
        <label className="flex items-center gap-1.5 text-[12.5px] text-fg">
          License
          <select value={license} onChange={(e) => setLicense(e.target.value)} className="h-7 px-1 rounded-[4px] bg-field border border-white/10 text-[12.5px] text-fg outline-none">
            <option value="">None</option>
            <option value="MIT">MIT</option>
          </select>
        </label>
        <label className="flex items-center gap-1.5 text-[12.5px] text-fg cursor-pointer">
          <input type="checkbox" className="accent-[#80B1EA]" checked={readme} onChange={(e) => setReadme(e.target.checked)} /> Add a README
        </label>
      </div>
      <div className="text-[12px] text-fg3">Chosen files are added in a first commit.</div>
      {error && <div className="text-[12px] text-[#ef7a76] mt-2">{error}</div>}
    </Dialog>
  );
}

function LogDialog({ onClose }: { onClose: () => void }) {
  const q = useQuery({ queryKey: ["log"], queryFn: api.activityLog, refetchInterval: 1000 });
  const entries = (q.data ?? []).slice().reverse();
  return (
    <Dialog open onClose={onClose} title="Activity Log" width={640}>
      {entries.length === 0 && <div className="text-fg3 text-[13px]">No Git commands have run yet in this session.</div>}
      <div className="flex flex-col gap-1.5">
        {entries.map((e, i) => (
          <div key={i} className="rounded bg-field border border-white/5 px-2.5 py-1.5">
            <div className="flex items-center gap-2 text-[12px]">
              <span className={e.ok ? "text-good" : "text-[#ef7a76]"}>{e.ok ? "✓" : "✗"}</span>
              <span className="text-fg3">{e.time}</span>
              <span className="mono text-white/85 truncate">{e.command}</span>
            </div>
            {e.output && <pre className="mono text-[11px] text-fg2 whitespace-pre-wrap mt-1 max-h-[90px] overflow-auto select-text">{e.output}</pre>}
          </div>
        ))}
      </div>
    </Dialog>
  );
}

const SHORTCUTS: [string, string][] = [
  [`${MOD}+T`, "Open a new tab"],
  [`${MOD}+W`, "Close tab"],
  [`${MOD}+Tab / ${MOD}+Shift+Tab`, "Jump to the next / previous tab"],
  [`${MOD}+Shift+T`, "Reopen closed tab"],
  [`${MOD}+O`, "Open repository"],
  [`${MOD}+N`, "Clone repository"],
  [`${MOD}+I`, "Init repository"],
  [`${MOD}+B`, "Create branch"],
  [`${MOD}+F`, "Search commits"],
  [`${MOD}+L`, "Fetch all"],
  [`${MOD}+Z / ${MOD}+Y`, "Undo / Redo"],
  [`${MOD}+Shift+S`, "Stage all files"],
  [`${MOD}+Shift+U`, "Unstage all files"],
  [`${MOD}+Shift+M`, "Focus commit message"],
  [`${MOD}+Shift+H`, "Search for a file to view its history / blame"],
  [`${MOD}+S`, "Save the file being edited"],
  [`${MOD}+Click / Shift+Click`, "Select several commits (2 = compare)"],
  ["S / U", "Stage / unstage the open file"],
  [`${MOD}+Enter`, "Commit (in message box)"],
  [`${MOD}+Shift+Enter`, "Stage all + commit (in message box)"],
  [`${MOD}+J`, "Toggle left panel"],
  [`${MOD}+K`, "Toggle commit details panel"],
  [`${MOD}+Alt+F`, "Filter the left panel"],
  [`${MOD}+= / ${MOD}+- / ${MOD}+0`, "Zoom in / out / reset"],
  ["↑ ↓ or K J", "Select previous / next commit"],
  ["Esc", "Close view"],
  ["F11", "Toggle full screen"],
];

function ShortcutsDialog({ onClose }: { onClose: () => void }) {
  const s = useStore();
  const [filter, setFilter] = useState("");
  const [recording, setRecording] = useState<string | null>(null);
  const groups: Group[] = ["Repo Actions", "Navigation", "Command Palette", "UI"];
  const f = filter.toLowerCase();
  const setKeys = (id: string, keys: string | undefined) => {
    const kb = { ...s.settings.keybindings };
    if (keys === undefined) delete kb[id];
    else kb[id] = keys;
    s.setSettings({ keybindings: kb });
  };
  const usedBy = (keys: string, id: string) => COMMANDS.find((c) => c.id !== id && keys && keysOf(c).toUpperCase() === keys.toUpperCase());
  return (
    <Dialog open onClose={onClose} title="Keyboard Shortcuts" width={600}>
      <input autoFocus className={cn(inputClass, "w-full mb-3")} placeholder={`Filter shortcuts (${MOD} + F)`} value={filter} onChange={(e) => setFilter(e.target.value)} />
      {groups.map((g) => {
        const list = COMMANDS.filter((c) => c.group === g && (!f || c.label.toLowerCase().includes(f) || keysOf(c).toLowerCase().includes(f)));
        if (!list.length) return null;
        return (
          <div key={g} className="mb-3">
            <div className="text-[12px] font-semibold tracking-wider text-fg2 mb-1">{g.toUpperCase()}</div>
            {list.map((c) => {
              const keys = keysOf(c);
              const custom = s.settings.keybindings[c.id] !== undefined;
              const clash = keys ? usedBy(keys, c.id) : undefined;
              return (
                <div key={c.id} className="flex items-center gap-2 h-[30px] border-b border-white/5 text-[13px]">
                  <span className="flex-1 text-fg truncate">{c.label}</span>
                  {clash && <span className="text-[11px] text-warn" title={`Also used by ${clash.label}`}>clash</span>}
                  <button
                    className={cn("mono text-[11.5px] px-1.5 py-0.5 rounded border min-w-[90px]", recording === c.id ? "border-accent bg-accent/20 text-white" : "border-white/10 bg-white/8 text-fg hover:border-white/30")}
                    title="Click, then press the new keys (Esc cancels, Backspace removes)"
                    onClick={() => setRecording(c.id)}
                    onKeyDown={(e) => {
                      if (recording !== c.id) return;
                      e.preventDefault();
                      e.stopPropagation();
                      if (e.key === "Escape") return setRecording(null);
                      if (e.key === "Backspace") return (setKeys(c.id, ""), setRecording(null));
                      const combo = comboOf(e.nativeEvent);
                      if (combo && (/Ctrl|Alt/.test(combo) || /^F\d+$/.test(combo))) {
                        setKeys(c.id, combo);
                        setRecording(null);
                      }
                    }}
                    onBlur={() => setRecording(null)}
                  >
                    {recording === c.id ? "Press keys…" : keys || "—"}
                  </button>
                  <button className={cn("text-[11px] w-[40px]", custom ? "text-link hover:underline" : "invisible")} onClick={() => setKeys(c.id, undefined)}>
                    Reset
                  </button>
                </div>
              );
            })}
          </div>
        );
      })}
      <div className="text-[12px] text-fg3">Shortcuts need Ctrl or Alt (or an F key). Arrow keys, J / K, S / U and Esc work as before.</div>
    </Dialog>
  );
}

const STAGES: [string, string, string[]][] = [
  ["Stage 1", "Main window and everyday Git — done", ["Tabs, toolbar, panels, commit graph", "Stage, commit, amend, discard", "Branches, tags, stashes", "Fetch, pull, push", "Undo / redo, search, activity log"]],
  ["Stage 2", "Deeper graph and diffs (Monaco) — done", ["Hunk / inline / split diffs, line staging, edit and save", "File history and blame", "More graph columns, solo / hide branches", "Compare commits, load more history, image diffs"]],
  ["Stage 3", "Rewriting history and conflicts", ["Interactive rebase", "Merge conflict tool", "Reword, squash, drop"]],
  ["Stage 4", "Repository management and accounts", ["Remotes, worktrees, submodules", "Gitflow, LFS", "Credential Manager, SSH, GPG", "Job queue with progress"]],
  ["Stage 5", "Productivity", ["Command palette", "Built-in terminal", "Workspaces, profiles, preferences", "Themes; gix and SQLite where needed"]],
  ["Stage 6", "Online services, AI and shipping", ["Pull requests, issues, Launchpad", "Cloud patches, teams", "AI helpers, Agents view", "Installers and updates"]],
];

function PlanDialog({ onClose }: { onClose: () => void }) {
  return (
    <Dialog open onClose={onClose} title="GitCraft feature plan" width={560}>
      <div className="flex flex-col gap-3">
        {STAGES.map(([n, t, items], i) => (
          <div key={n} className={cn("rounded border px-3 py-2", i <= 1 ? "border-good/60 bg-good/10" : "border-white/10 bg-field")}>
            <div className="text-[13px] text-white font-semibold">
              {n} · {t}
            </div>
            <div className="text-[12px] text-fg2 mt-1">{items.join(" · ")}</div>
          </div>
        ))}
      </div>
    </Dialog>
  );
}

export function PrefsDialog({ onClose }: { onClose: () => void }) {
  const s = useStore();
  const set = s.settings;
  const tab = s.tabs.find((t) => t.id === s.activeId);
  return (
    <Dialog open onClose={onClose} title="Preferences" width={480}>
      <div className="text-[12px] font-semibold tracking-wider text-fg2 mb-2">UI CUSTOMIZATION</div>
      <Field label="Theme">
        <select className={inputClass} value={set.theme} onChange={(e) => s.setSettings({ theme: e.target.value })}>
          {THEMES.map((t) => (
            <option key={t.id} value={t.id}>
              {t.label}
            </option>
          ))}
        </select>
      </Field>
      <Check label="Automatically download GitCraft updates (from GitHub Releases)" value={set.autoUpdate} onChange={(v) => s.setSettings({ autoUpdate: v })} />
      <Check label="Enable spell checking (commit messages)" value={set.spellcheck} onChange={(v) => s.setSettings({ spellcheck: v })} />
      <Check label="Show a notice after successful actions" value={set.notifySuccess} onChange={(v) => s.setSettings({ notifySuccess: v })} />
      <Check label="Show ghost branch/tag when hovering over or selecting a commit" value={set.ghostRefs} onChange={(v) => s.setSettings({ ghostRefs: v })} />
      <div className="text-[12px] font-semibold tracking-wider text-fg2 mt-4 mb-2">GENERAL</div>
      <Check label="Show toolbar icon labels" value={set.toolbarLabels} onChange={(v) => s.setSettings({ toolbarLabels: v })} />
      <Check label="Push after each commit" value={set.pushAfterCommit} onChange={(v) => s.setSettings({ pushAfterCommit: v })} />
      <Check label="Show left panel" value={set.showLeft} onChange={(v) => s.setSettings({ showLeft: v })} />
      <Check label="Show commit details panel" value={set.showRight} onChange={(v) => s.setSettings({ showRight: v })} />
      <Field label="Default pull behaviour" className="mt-3">
        <select className={inputClass} value={set.pullMode} onChange={(e) => s.setSettings({ pullMode: e.target.value as typeof set.pullMode })}>
          <option value="ff">Pull (fast-forward if possible)</option>
          <option value="ffonly">Pull (fast-forward only)</option>
          <option value="rebase">Pull (rebase)</option>
          <option value="fetch">Fetch All</option>
        </select>
      </Field>
      <Field label="Default folder for clone and init">
        <input className={inputClass} value={set.defaultCloneFolder} onChange={(e) => s.setSettings({ defaultCloneFolder: e.target.value })} />
      </Field>
      {tab?.path && (
        <Button size="sm" onClick={() => refresh(tab.path!)}>
          Reload repository
        </Button>
      )}
      <div className="text-[12px] font-semibold tracking-wider text-fg2 mt-4 mb-2">REMOTES</div>
      <Field label="Auto-Fetch Interval (minutes between 0 and 60; 0 turns it off)">
        <input
          className={inputClass}
          value={set.autoFetchMin}
          onChange={(e) => s.setSettings({ autoFetchMin: Math.max(0, Math.min(60, Number(e.target.value.replace(/\D/g, "") || 0))) })}
        />
      </Field>
      <Check label="Auto-Prune (remove remote branches that were deleted on the server)" value={set.autoPrune} onChange={(v) => s.setSettings({ autoPrune: v })} />
      <div className="text-[12px] font-semibold tracking-wider text-fg2 mt-4 mb-2">COMMIT</div>
      <Check label="Skip git hooks by default when committing" value={set.skipHooks} onChange={(v) => s.setSettings({ skipHooks: v })} />
      <Field label="Commit Template: Summary" className="mt-2">
        <input className={inputClass} value={set.templateSummary} placeholder="e.g. feat: " onChange={(e) => s.setSettings({ templateSummary: e.target.value })} />
      </Field>
      <Field label="Commit Template: Description">
        <textarea
          className="h-[60px] px-2 py-1 rounded-[4px] bg-field border border-white/10 text-fg text-[13px] outline-none focus:border-accent resize-none"
          value={set.templateDescription}
          onChange={(e) => s.setSettings({ templateDescription: e.target.value })}
        />
      </Field>
      <div className="text-[12px] font-semibold tracking-wider text-fg2 mt-4 mb-2">EXTERNAL TOOLS</div>
      <div className="flex gap-3">
        {(["diffTool", "mergeTool"] as const).map((k) => (
          <Field key={k} label={k === "diffTool" ? "External Diff Tool" : "External Merge Tool"} className="flex-1">
            <select className={inputClass} value={set[k]} onChange={(e) => s.setSettings({ [k]: e.target.value })}>
              <option value="">Git's configured tool</option>
              {["vscode", "meld", "kdiff3", "bc", "p4merge", "winmerge", "tortoisemerge", "vimdiff"].map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </select>
          </Field>
        ))}
      </div>
      <Field label="Default Branch Name (for new repositories)" className="mt-2">
        <input className={inputClass} value={set.defaultBranch} onChange={(e) => s.setSettings({ defaultBranch: e.target.value })} />
      </Field>
      <div className="flex gap-2 mt-3">
        <Button size="sm" onClick={() => s.setDialog({ kind: "accounts" })}>
          Accounts: identity, credentials, SSH and GPG...
        </Button>
        <Button size="sm" onClick={() => s.setDialog({ kind: "profiles" })}>
          Profiles...
        </Button>
        <Button size="sm" onClick={() => s.setDialog({ kind: "workspaces" })}>
          Workspaces...
        </Button>
      </div>
    </Dialog>
  );
}

/** A small form: text fields (optionally with a folder Browse button) and checkboxes. */
function FormDialog({
  title,
  fields,
  confirm,
  help,
  onSubmit,
  onClose,
  action,
}: {
  title: string;
  fields: FormField[];
  confirm: string;
  help?: string;
  onSubmit: (v: Record<string, string | boolean>) => void;
  onClose: () => void;
  action?: { label: string; run: (v: Record<string, string | boolean>) => Promise<Record<string, string | boolean>> };
}) {
  const [v, setV] = useState<Record<string, string | boolean>>(Object.fromEntries(fields.map((f) => [f.key, f.value])));
  const [running, setRunning] = useState(false);
  const submit = () => {
    onClose();
    onSubmit(v);
  };
  return (
    <Dialog
      open
      onClose={onClose}
      title={title}
      width={500}
      footer={
        <>
          {action && (
            <Button
              variant="ai"
              className="mr-auto"
              disabled={running}
              onClick={async () => {
                setRunning(true);
                try {
                  setV(await action.run(v));
                } catch (e) {
                  useStore.getState().toast(false, String(e));
                } finally {
                  setRunning(false);
                }
              }}
            >
              {running ? "Working…" : action.label}
            </Button>
          )}
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={submit}>
            {confirm}
          </Button>
        </>
      }
    >
      {fields.map((f, i) =>
        f.type === "checkbox" ? (
          <label key={f.key} className="flex items-center gap-2 text-[13px] text-fg mb-3 cursor-pointer">
            <input type="checkbox" checked={!!v[f.key]} onChange={(e) => setV({ ...v, [f.key]: e.target.checked })} className="accent-[#80B1EA]" />
            {f.label}
          </label>
        ) : (
          <Field key={f.key} label={f.label}>
            <div className="flex gap-2">
              <input
                autoFocus={i === 0}
                className={cn(inputClass, "flex-1")}
                value={String(v[f.key] ?? "")}
                placeholder={f.placeholder}
                onChange={(e) => setV({ ...v, [f.key]: e.target.value })}
                onKeyDown={(e) => e.key === "Enter" && submit()}
              />
              {f.browse && (
                <Button
                  onClick={async (e) => {
                    e.preventDefault();
                    const p = await api.pickFolder(f.label);
                    if (p) setV({ ...v, [f.key]: p });
                  }}
                >
                  Browse
                </Button>
              )}
            </div>
          </Field>
        ),
      )}
      {help && <div className="text-[12px] text-fg3">{help}</div>}
    </Dialog>
  );
}

/** "Clean Up Local Branches": filter by age, merge status and remote status, pick, delete. */
function CleanupDialog({ onClose }: { onClose: () => void }) {
  const s = useStore();
  const tab = s.tabs.find((t) => t.id === s.activeId);
  const path = tab?.kind === "repo" ? tab.path! : "";
  const q = useQuery({ queryKey: ["cleanup", path], queryFn: () => api.cleanupList(path), enabled: !!path });
  const [days, setDays] = useState("0");
  const [merge, setMerge] = useState<"any" | "merged" | "unmerged">("merged");
  const [up, setUp] = useState<"any" | "has" | "none">("any");
  const [picked, setPicked] = useState<string[]>([]);
  const [remotes, setRemotes] = useState<string[]>([]);
  const now = Date.now() / 1000;
  const list = (q.data ?? []).filter(
    (b) =>
      now - b.time >= Number(days || 0) * 86400 &&
      (merge === "any" || (merge === "merged") === b.merged) &&
      (up === "any" || (up === "has") === !!b.upstream),
  );
  const toggle = (arr: string[], x: string) => (arr.includes(x) ? arr.filter((y) => y !== x) : [...arr, x]);
  const sel = "h-7 px-1 rounded-[4px] bg-field border border-white/10 text-[12.5px] text-fg outline-none";
  const remoteCount = remotes.filter((r) => picked.some((p) => (q.data ?? []).find((b) => b.name === p)?.upstream === r)).length;
  return (
    <Dialog
      open
      onClose={onClose}
      title="Clean Up Local Branches"
      width={640}
      footer={
        <>
          {remoteCount > 0 && <span className="text-[12px] text-[#ef7a76] mr-auto self-center">Warning: {remoteCount} remote branch{remoteCount === 1 ? "" : "es"} will be deleted. This cannot be undone.</span>}
          <Button onClick={onClose}>Cancel</Button>
          <Button
            variant="danger"
            disabled={!picked.length}
            onClick={() => {
              onClose();
              const ups = picked.map((p) => (q.data ?? []).find((b) => b.name === p)?.upstream).filter((u): u is string => !!u && remotes.includes(u));
              act(path, { type: "deleteBranches", names: picked, remotes: ups });
            }}
          >
            Delete {picked.length} branch{picked.length === 1 ? "" : "es"}
          </Button>
        </>
      }
    >
      <div className="flex flex-wrap gap-3 items-center text-[12.5px] text-fg2 mb-3">
        <label className="flex items-center gap-1.5">
          Older than (days):
          <input value={days} onChange={(e) => setDays(e.target.value.replace(/\D/g, ""))} className={cn(inputClass, "w-14 h-7")} />
        </label>
        <label className="flex items-center gap-1.5">
          Merge status:
          <select value={merge} onChange={(e) => setMerge(e.target.value as typeof merge)} className={sel}>
            <option value="any">Any</option>
            <option value="merged">Merged</option>
            <option value="unmerged">Not merged</option>
          </select>
        </label>
        <label className="flex items-center gap-1.5">
          Remote status:
          <select value={up} onChange={(e) => setUp(e.target.value as typeof up)} className={sel}>
            <option value="any">Any</option>
            <option value="has">Has upstream</option>
            <option value="none">Doesn't have upstream</option>
          </select>
        </label>
      </div>
      <div className="text-[12px] text-fg3 mb-1.5">{list.length === 1 ? "1 branch matching filters" : `${list.length} branches matching filters`}</div>
      <div className="max-h-[300px] overflow-auto rounded border border-white/5">
        {q.isLoading && <div className="p-2 text-fg3 text-[12px]">Loading…</div>}
        {!q.isLoading && list.length === 0 && <div className="p-3 text-fg3 text-[12.5px]">No branches match the current filters</div>}
        {list.map((b) => (
          <div key={b.name} className="flex items-center gap-2 px-2 h-[30px] border-b border-white/5 text-[12.5px]">
            <input type="checkbox" checked={picked.includes(b.name)} onChange={() => setPicked(toggle(picked, b.name))} className="accent-[#80B1EA]" />
            <span className="flex-1 truncate text-white/90">{b.name}</span>
            <span className={cn("text-[11px]", b.merged ? "text-good" : "text-fg3")}>{b.merged ? "merged" : "not merged"}</span>
            <span className="w-[90px] text-[11px] text-fg3 text-right">{fullDate(b.time).split(",")[0]}</span>
            <span className="w-[150px] truncate text-[11px] text-fg2">{b.upstream || "None"}</span>
            {b.upstream && (
              <label className="flex items-center gap-1 text-[11px] text-fg3" title={`Also delete remote branch ${b.upstream}`}>
                <input type="checkbox" checked={remotes.includes(b.upstream)} onChange={() => setRemotes(toggle(remotes, b.upstream))} className="accent-[#d9413d]" />
                remote
              </label>
            )}
          </div>
        ))}
      </div>
      <button className="mt-2 text-[12px] text-link hover:underline" onClick={() => setPicked(list.map((b) => b.name))}>
        Select all branches
      </button>
    </Dialog>
  );
}

/** A longer, multi-line text (e.g. a commit message to reword). */
function MessageDialog({ title, label, value, confirm, help, onSubmit, onClose }: { title: string; label: string; value: string; confirm: string; help?: string; onSubmit: (v: string) => void; onClose: () => void }) {
  const [v, setV] = useState(value);
  const submit = () => {
    if (!v.trim()) return;
    onClose();
    onSubmit(v.trim());
  };
  return (
    <Dialog
      open
      onClose={onClose}
      title={title}
      width={520}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={submit}>
            {confirm}
          </Button>
        </>
      }
    >
      <Field label={label}>
        <textarea
          autoFocus
          value={v}
          onChange={(e) => setV(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && (e.ctrlKey || e.metaKey) && submit()}
          className="h-[140px] px-2 py-1.5 rounded-[4px] bg-field border border-white/10 text-fg text-[13px] outline-none focus:border-accent resize-none"
        />
      </Field>
      {help && <div className="text-[12px] text-fg3">{help}</div>}
    </Dialog>
  );
}

/** Ctrl+Shift+H: search for a file to view its history / blame. */
function FileFinder({ onClose }: { onClose: () => void }) {
  const s = useStore();
  const tab = s.tabs.find((t) => t.id === s.activeId);
  const path = tab?.kind === "repo" ? tab.path : undefined;
  const q = useQuery({ queryKey: ["tracked", path], queryFn: () => api.trackedFiles(path!), enabled: !!path });
  const [text, setText] = useState("");
  const [idx, setIdx] = useState(0);
  const words = text.toLowerCase().split(/\s+/).filter(Boolean);
  const list = (q.data ?? []).filter((f) => words.every((w) => f.toLowerCase().includes(w))).slice(0, 200);
  const pick = (f: string) => {
    onClose();
    openHistory(f, "history");
  };
  return (
    <Dialog open onClose={onClose} title="Search for a file to view its history / blame" width={560}>
      {!path && <div className="text-fg3 text-[13px]">Open a repository first.</div>}
      {path && (
        <>
          <input
            autoFocus
            className={cn(inputClass, "w-full mb-2")}
            placeholder="Type part of a file name"
            value={text}
            onChange={(e) => (setText(e.target.value), setIdx(0))}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") (e.preventDefault(), setIdx((i) => Math.min(list.length - 1, i + 1)));
              if (e.key === "ArrowUp") (e.preventDefault(), setIdx((i) => Math.max(0, i - 1)));
              if (e.key === "Enter" && list[idx]) pick(list[idx]);
            }}
          />
          <div className="max-h-[320px] overflow-auto rounded border border-white/5">
            {q.isLoading && <div className="p-2 text-fg3 text-[12px]">Loading files…</div>}
            {list.map((f, i) => (
              <button key={f} onClick={() => pick(f)} className={cn("w-full text-left px-2.5 py-1 text-[12.5px] truncate", i === idx ? "bg-accent/40 text-white" : "text-fg hover:bg-hoverrow")}>
                <span className="text-white">{f.split("/").pop()}</span> <span className="text-fg3">{f}</span>
              </button>
            ))}
            {!q.isLoading && list.length === 0 && <div className="p-2 text-fg3 text-[12px]">No matching files.</div>}
          </div>
        </>
      )}
    </Dialog>
  );
}

function Check({ label, value, onChange }: { label: string; value: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="flex items-center gap-2 text-[13px] text-fg py-1 cursor-pointer">
      <input type="checkbox" checked={value} onChange={(e) => onChange(e.target.checked)} className="accent-[#80B1EA]" />
      {label}
    </label>
  );
}

export function Toasts() {
  const toasts = useStore((s) => s.toasts);
  const drop = useStore((s) => s.dropToast);
  // UI Customization > Notification Location (the original's default: bottom left).
  const where = useStore((s) => s.settings.notificationLocation) ?? "bottom-left";
  const pos = { "top-left": "left-3 top-[100px]", "top-right": "right-3 top-[100px]", "bottom-left": "left-3 bottom-9", "bottom-right": "right-3 bottom-9" }[where];
  return (
    <div className={cn("fixed z-[70] flex flex-col gap-2 w-[300px] pointer-events-none", pos, where.startsWith("top") && "flex-col-reverse")}>
      {toasts.map((t) => (
        <div
          key={t.id}
          onClick={() => drop(t.id)}
          className={cn("pointer-events-auto rounded-md px-3 py-2 text-[12.5px] shadow-xl border bg-[var(--menu-bg)] whitespace-pre-wrap break-words max-h-[160px] overflow-auto", t.ok ? "border-good/60" : "border-bad/80")}
        >
          <span className={t.ok ? "text-good" : "text-[#ef7a76]"}>{t.ok ? "✓ " : "✗ "}</span>
          {t.text}
        </div>
      ))}
    </div>
  );
}

/** Sparse checkout: keep only some folders in the working folder. */
function SparseDialog({ onClose }: { onClose: () => void }) {
  const s = useStore();
  const tab = s.tabs.find((t) => t.id === s.activeId);
  const path = tab?.kind === "repo" ? tab.path! : "";
  const q = useQuery({ queryKey: ["sparse", path], queryFn: () => api.sparseInfo(path), enabled: !!path });
  const [text, setText] = useState<string | null>(null);
  const value = text ?? (q.data?.patterns.join("\n") || "/src/\n/docs/");
  return (
    <Dialog
      open
      onClose={onClose}
      title="Sparse Checkout"
      width={480}
      footer={
        <>
          {q.data?.enabled && (
            <Button
              variant="danger"
              onClick={() => {
                onClose();
                act(path, { type: "sparseDisable" });
              }}
            >
              Turn Off
            </Button>
          )}
          <Button onClick={onClose}>Cancel</Button>
          <Button
            variant="primary"
            onClick={() => {
              onClose();
              act(path, { type: "sparseSet", patterns: value.split(/\r?\n/).map((x) => x.trim()).filter(Boolean) });
            }}
          >
            {q.data?.enabled ? "Update" : "Turn On"}
          </Button>
        </>
      }
    >
      <p className="text-[12.5px] text-fg2 mb-2 leading-relaxed">Only files matching these patterns stay in the working folder (one per line, .gitignore style). Everything else is still in the repository.</p>
      <textarea
        value={value}
        onChange={(e) => setText(e.target.value)}
        className="w-full h-[140px] mono text-[12px] bg-field border border-white/10 rounded-[3px] px-2 py-1.5 text-fg outline-none focus:border-accent resize-none"
      />
    </Dialog>
  );
}

/** Identity, credentials, SSH keys and GPG signing (Git's own settings, so other tools agree). */
export function AccountsDialog({ onClose }: { onClose: () => void }) {
  const s = useStore();
  const tab = s.tabs.find((t) => t.id === s.activeId);
  const path = tab?.kind === "repo" ? tab.path! : null;
  const q = useQuery({ queryKey: ["accounts", path], queryFn: () => api.accountInfo(path) });
  const a = q.data;
  const [edits, setEdits] = useState<Record<string, string>>({});
  const val = (k: string, fallback: string) => edits[k] ?? fallback;
  const save = async (scope: "global" | "local", key: string, value: string) => {
    try {
      await api.setGitConfig(path, scope, key, value);
      s.toast(true, `Saved ${key}`);
      q.refetch();
    } catch (e) {
      s.toast(false, String(e));
    }
  };
  const row = (label: string, key: string, scope: "global" | "local", value: string, placeholder = "") => (
    <div className="flex items-center gap-2 mb-2">
      <span className="w-[150px] text-[12.5px] text-fg2 shrink-0">{label}</span>
      <input className={cn(inputClass, "flex-1")} value={val(scope + key, value)} placeholder={placeholder} onChange={(e) => setEdits({ ...edits, [scope + key]: e.target.value })} />
      <Button size="sm" disabled={edits[scope + key] === undefined} onClick={() => save(scope, key, edits[scope + key] ?? "")}>
        Save
      </Button>
    </div>
  );
  const head = (t: string) => <div className="text-[12px] font-semibold tracking-wider text-fg2 mt-4 mb-2">{t}</div>;
  if (!a) return <Dialog open onClose={onClose} title="Accounts" width={620}>{q.isError ? String(q.error) : "Loading…"}</Dialog>;
  return (
    <Dialog open onClose={onClose} title="Accounts" width={620}>
      <Integrations />
      {head("IDENTITY (EVERY REPOSITORY)")}
      {row("Name", "user.name", "global", a.name, "Your Name")}
      {row("Email", "user.email", "global", a.email, "you@example.com")}
      {path && head("THIS REPOSITORY ONLY (OVERRIDES THE ABOVE)")}
      {path && row("Name", "user.name", "local", a.repo_name, "(same as above)")}
      {path && row("Email", "user.email", "local", a.repo_email, "(same as above)")}

      {head("HTTPS CREDENTIALS")}
      <div className="text-[12.5px] text-fg mb-2">
        Credential helper: <span className="mono text-white">{a.credential_helper || "none"}</span>
        {a.gcm ? <span className="text-good"> · Git Credential Manager is installed</span> : <span className="text-fg3"> · Git Credential Manager not found</span>}
      </div>
      {a.gcm && a.credential_helper !== "manager" && (
        <Button size="sm" onClick={() => save("global", "credential.helper", "manager")}>
          Use Git Credential Manager (signs in to GitHub, GitLab, Bitbucket and Azure DevOps in your browser)
        </Button>
      )}

      {head("SSH KEYS")}
      <div className="text-[12px] text-fg3 mb-1">{a.ssh_dir}</div>
      {a.ssh_keys.length === 0 && <div className="text-[12.5px] text-fg3 mb-2">No SSH keys yet.</div>}
      {a.ssh_keys.map((k) => (
        <div key={k.path} className="flex items-center gap-2 mb-1.5 text-[12.5px]">
          <span className="mono text-white/85 truncate flex-1" title={k.public}>
            {k.path.split(/[\\/]/).pop()} <span className="text-fg3">({k.kind})</span>
          </span>
          <Button size="sm" onClick={() => navigator.clipboard.writeText(k.public).then(() => s.toast(true, "Public key copied — paste it into your Git host's SSH keys page"))}>
            Copy public key
          </Button>
        </div>
      ))}
      <Button
        size="sm"
        disabled={!a.ssh_keygen}
        title={a.ssh_keygen ? "Creates ~/.ssh/id_ed25519 without a passphrase" : "ssh-keygen was not found"}
        onClick={async () => {
          try {
            const pub = await api.generateSshKey(a.email || "gitcraft", "id_ed25519");
            await navigator.clipboard.writeText(pub);
            s.toast(true, "SSH key created and its public key copied");
            q.refetch();
          } catch (e) {
            s.toast(false, String(e));
          }
        }}
      >
        Generate a new SSH key
      </Button>

      {head("COMMIT SIGNING (GPG)")}
      {!a.gpg && <div className="text-[12.5px] text-fg3 mb-2">GPG was not found. Install Gpg4win (or use SSH signing) to sign commits.</div>}
      {a.gpg_keys.length > 0 && (
        <div className="flex items-center gap-2 mb-2">
          <span className="w-[150px] text-[12.5px] text-fg2 shrink-0">Signing key</span>
          <select className={cn(inputClass, "flex-1")} value={a.signing_key} onChange={(e) => save("global", "user.signingkey", e.target.value)}>
            <option value="">(none)</option>
            {a.gpg_keys.map((k) => (
              <option key={k.id} value={k.id}>
                {k.id} — {k.uid}
              </option>
            ))}
          </select>
        </div>
      )}
      {a.gpg_keys.length === 0 && row("Signing key", "user.signingkey", "global", a.signing_key, "key id")}
      <Check label="Sign every commit (commit.gpgsign)" value={a.sign_commits} onChange={(v) => save("global", "commit.gpgsign", v ? "true" : "")} />
    </Dialog>
  );
}

/** Profiles: separate settings and Git identity, switched from the profile button. */
export function ProfilesPanel() {
  const s = useStore();
  const [name, setName] = useState("");
  const [gitName, setGitName] = useState("");
  const [email, setEmail] = useState("");
  return (
    <>
      {s.profiles.map((p) => (
        <div key={p.id} className="flex items-center gap-2 h-[34px] border-b border-white/5 text-[13px]">
          <span className={cn("flex-1 truncate", p.id === s.activeProfile ? "text-white font-semibold" : "text-fg")}>
            {p.name}
            {p.gitEmail && <span className="text-fg3 font-normal"> · {p.gitName} &lt;{p.gitEmail}&gt;</span>}
          </span>
          {p.id === s.activeProfile ? (
            <span className="text-[11.5px] text-good">current</span>
          ) : (
            <>
              <Button size="sm" onClick={() => switchProfile(p.id)}>
                Switch
              </Button>
              <Button size="sm" variant="danger" onClick={() => removeProfile(p.id)}>
                Delete
              </Button>
            </>
          )}
        </div>
      ))}
      <div className="text-[12px] font-semibold tracking-wider text-fg2 mt-4 mb-2">NEW PROFILE</div>
      <div className="flex gap-2 mb-2">
        <input className={cn(inputClass, "flex-1")} placeholder="Profile name (e.g. Work)" value={name} onChange={(e) => setName(e.target.value)} />
      </div>
      <div className="flex gap-2 mb-2">
        <input className={cn(inputClass, "flex-1")} placeholder="Git name" value={gitName} onChange={(e) => setGitName(e.target.value)} />
        <input className={cn(inputClass, "flex-1")} placeholder="Git email" value={email} onChange={(e) => setEmail(e.target.value)} />
      </div>
      <Button
        variant="primary"
        disabled={!name.trim()}
        onClick={() => {
          addProfile(name, gitName, email);
          setName("");
          setGitName("");
          setEmail("");
        }}
      >
        Add Profile
      </Button>
      <div className="text-[12px] text-fg3 mt-3">A new profile starts with a copy of the current settings. Switching sets your global Git name and email to the profile's.</div>
    </>
  );
}

function ProfilesDialog({ onClose }: { onClose: () => void }) {
  return (
    <Dialog open onClose={onClose} title="Profiles" width={520}>
      <ProfilesPanel />
    </Dialog>
  );
}

/** Local workspaces: named groups of repositories you open together. */
function WorkspacesDialog({ onClose }: { onClose: () => void }) {
  const s = useStore();
  const [name, setName] = useState("");
  const [picked, setPicked] = useState<string[]>([]);
  const candidates = [...new Set([...s.tabs.filter((t) => t.kind === "repo").map((t) => t.path!), ...s.settings.recents])];
  return (
    <Dialog open onClose={onClose} title="Workspaces" width={560}>
      {s.settings.workspaces.length === 0 && <div className="text-[12.5px] text-fg3 mb-2">No workspaces yet.</div>}
      {s.settings.workspaces.map((w) => (
        <div key={w.id} className="flex items-center gap-2 h-[34px] border-b border-white/5 text-[13px]">
          <span className="flex-1 truncate text-fg">
            {w.name} <span className="text-fg3">· {w.repos.length} repositories</span>
          </span>
          <Button
            size="sm"
            variant="primary"
            onClick={() => {
              onClose();
              openWorkspace(w.id);
            }}
          >
            Open all
          </Button>
          <Button size="sm" variant="danger" onClick={() => deleteWorkspace(w.id)}>
            Delete
          </Button>
        </div>
      ))}
      <div className="text-[12px] font-semibold tracking-wider text-fg2 mt-4 mb-2">NEW WORKSPACE</div>
      <input className={cn(inputClass, "w-full mb-2")} placeholder="Workspace name" value={name} onChange={(e) => setName(e.target.value)} />
      <div className="max-h-[180px] overflow-auto rounded border border-white/5 mb-2">
        {candidates.length === 0 && <div className="p-2 text-[12px] text-fg3">Open some repositories first.</div>}
        {candidates.map((p) => (
          <label key={p} className="flex items-center gap-2 px-2 h-[26px] text-[12.5px] text-fg cursor-pointer hover:bg-hoverrow">
            <input type="checkbox" className="accent-[#80B1EA]" checked={picked.includes(p)} onChange={() => setPicked(picked.includes(p) ? picked.filter((x) => x !== p) : [...picked, p])} />
            <span className="truncate">{p}</span>
          </label>
        ))}
      </div>
      <Button
        variant="primary"
        disabled={!name.trim() || picked.length === 0}
        onClick={() => {
          saveWorkspace(name, picked);
          setName("");
          setPicked([]);
        }}
      >
        Create Workspace
      </Button>
    </Dialog>
  );
}

/** GitHub / GitLab tokens, with a shortcut to shared AI connection preferences. */
function Integrations() {
  const s = useStore();
  const q = useQuery({ queryKey: ["hostingStatus", s.settings.gitlabHost], queryFn: hostingApi.status });
  const [vals, setVals] = useState<Record<string, string>>({});
  const save = async (name: "github-token" | "gitlab-token", value: string) => {
    try {
      await hostingApi.setSecret(name, value);
      setVals({ ...vals, [name]: "" });
      s.toast(true, value ? "Saved to the system credential store" : "Removed");
      q.refetch();
    } catch (e) {
      s.toast(false, String(e));
    }
  };
  const row = (name: "github-token" | "gitlab-token", label: string, connected: string | null | boolean, error: string | null, help: string) => (
    <div className="mb-3">
      <div className="flex items-center gap-2">
        <span className="w-[150px] text-[12.5px] text-fg2 shrink-0">{label}</span>
        <input
          type="password"
          className={cn(inputClass, "flex-1")}
          placeholder={connected ? "•••••••• (saved)" : "paste token"}
          value={vals[name] ?? ""}
          onChange={(e) => setVals({ ...vals, [name]: e.target.value })}
        />
        <Button size="sm" disabled={!vals[name]} onClick={() => save(name, vals[name] ?? "")}>
          Save
        </Button>
        {connected && (
          <Button size="sm" variant="danger" onClick={() => save(name, "")}>
            Remove
          </Button>
        )}
      </div>
      <div className="text-[11.5px] ml-[158px] mt-0.5">
        {error ? <span className="text-[#ef7a76]">{error}</span> : connected ? <span className="text-good">{typeof connected === "string" ? `Connected as ${connected}` : "Saved"}</span> : <span className="text-fg3">{help}</span>}
      </div>
    </div>
  );
  const st = q.data;
  return (
    <>
      <div className="text-[12px] font-semibold tracking-wider text-fg2 mt-1 mb-2">INTEGRATIONS</div>
      {row("github-token", "GitHub token", st?.github ?? null, st?.github_error ?? null, "A personal access token with repo access (github.com › Settings › Developer settings).")}
      {row("gitlab-token", "GitLab token", st?.gitlab ?? null, st?.gitlab_error ?? null, "A personal access token with the api scope.")}
      <div className="flex items-center gap-2 mb-3">
        <span className="w-[150px] text-[12.5px] text-fg2 shrink-0">Self-managed GitLab</span>
        <input className={cn(inputClass, "flex-1")} placeholder="gitlab.example.com (optional)" value={s.settings.gitlabHost} onChange={(e) => s.setSettings({ gitlabHost: e.target.value.trim() })} />
      </div>
      <Button size="sm" onClick={() => s.setDialog({ kind: "prefs", section: "ai" })}>Configure local or hosted AI</Button>
      <div className="text-[11.5px] text-fg3 mb-2">Tokens and keys are stored in the system's credential store, not in GitCraft's settings file.</div>
    </>
  );
}
