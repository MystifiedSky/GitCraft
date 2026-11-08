import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import { useStore } from "@/lib/store";
import { openRepoPath, refresh } from "@/lib/repo";
import { Dialog, Field, inputClass } from "./ui/dialog";
import { Button } from "./ui/button";
import { AppMark } from "./Icons";
import { cn, MOD } from "@/lib/utils";

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
            <div className="text-fg2 text-[12.5px]">Version {useStore.getState().version} · Stage 1</div>
            <div className="text-fg3 text-[12px] text-center mt-2">A graphical Git client built with Tauri, Rust and React. Every action runs the real Git command line.</div>
          </div>
        </Dialog>
      );
    case "plan":
      return <PlanDialog onClose={close} />;
    case "prefs":
      return <PrefsDialog onClose={close} />;
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
  const go = async () => {
    if (!url.trim() || !dir.trim()) return;
    setBusy(true);
    setError("");
    s.setBusy("Cloning…");
    try {
      const path = await api.cloneRepo(url, dir);
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
      <Field label="URL">
        <input autoFocus className={inputClass} value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://github.com/owner/project.git" />
      </Field>
      <FolderField label="Where to clone to" value={dir} onChange={setDir} title="Choose where to clone to" />
      {url && dir && <div className="text-[12px] text-fg3 -mt-1 mb-2">Full path: {dir.replace(/[\\/]$/, "")}\{url.replace(/\/$/, "").split(/[/\\:]/).pop()?.replace(/\.git$/, "")}</div>}
      {error && <div className="text-[12px] text-[#ef7a76] whitespace-pre-wrap">{error}</div>}
    </Dialog>
  );
}

function InitDialog({ onClose }: { onClose: () => void }) {
  const s = useStore();
  const [name, setName] = useState("");
  const [dir, setDir] = useState(s.settings.defaultCloneFolder);
  const [error, setError] = useState("");
  const go = async () => {
    if (!name.trim() || !dir.trim()) return;
    try {
      const path = await api.initRepo(dir, name);
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
      <div className="text-[12px] text-fg3">Default branch name: main. .gitignore and licence templates come in Stage 4.</div>
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
  return (
    <Dialog open onClose={onClose} title="Keyboard Shortcuts" width={520}>
      <table className="w-full text-[13px]">
        <tbody>
          {SHORTCUTS.map(([k, v]) => (
            <tr key={k} className="border-b border-white/5">
              <td className="py-1.5 pr-4 text-fg">{v}</td>
              <td className="py-1.5 text-right">
                <span className="mono text-[11.5px] px-1.5 py-0.5 rounded bg-white/8 border border-white/10">{k}</span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="text-[12px] text-fg3 mt-3">A shortcut editor and command palette arrive in Stage 5.</div>
    </Dialog>
  );
}

const STAGES: [string, string, string[]][] = [
  ["Stage 1", "Main window and everyday Git — done", ["Tabs, toolbar, panels, commit graph", "Stage, commit, amend, discard", "Branches, tags, stashes", "Fetch, pull, push", "Undo / redo, search, activity log"]],
  ["Stage 2", "Deeper graph and diffs (Monaco)", ["Hunk / split diffs, line staging", "File history and blame", "More graph columns, solo / hide branches", "Compare commits"]],
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
          <div key={n} className={cn("rounded border px-3 py-2", i === 0 ? "border-good/60 bg-good/10" : "border-white/10 bg-field")}>
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

function PrefsDialog({ onClose }: { onClose: () => void }) {
  const s = useStore();
  const set = s.settings;
  const tab = s.tabs.find((t) => t.id === s.activeId);
  return (
    <Dialog open onClose={onClose} title="Preferences" width={480}>
      <div className="text-[12px] font-semibold tracking-wider text-fg2 mb-2">GENERAL</div>
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
      <div className="text-[12px] text-fg3 mt-3">Auto-fetch, SSH, GPG, external tools, themes and profiles come in Stages 4 and 5.</div>
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
  return (
    <div className="fixed right-3 bottom-9 z-[70] flex flex-col gap-2 w-[300px] pointer-events-none">
      {toasts.map((t) => (
        <div
          key={t.id}
          onClick={() => drop(t.id)}
          className={cn("pointer-events-auto rounded-md px-3 py-2 text-[12.5px] shadow-xl border bg-[#2b2e35] whitespace-pre-wrap break-words max-h-[160px] overflow-auto", t.ok ? "border-good/60" : "border-bad/80")}
        >
          <span className={t.ok ? "text-good" : "text-[#ef7a76]"}>{t.ok ? "✓ " : "✗ "}</span>
          {t.text}
        </div>
      ))}
    </div>
  );
}
