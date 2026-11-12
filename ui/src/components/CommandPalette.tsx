import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "@/lib/api";
import { COMMANDS, fuzzy, keysOf, runCommand, type Command } from "@/lib/commands";
import { act, openHistory, openRepoPath, repoData } from "@/lib/repo";
import { useStore } from "@/lib/store";
import { THEMES, switchProfile, openWorkspace } from "@/lib/prefs";
import { cn } from "@/lib/utils";

interface Item {
  key: string;
  label: string;
  hint?: string;
  shortcut?: string;
  run: () => void;
}

const PLACEHOLDER: Record<string, string> = {
  root: "Search for commands and actions (e.g., Open Repo)",
  openRepo: "Search for a repository to open",
  checkout: "Search for a branch to checkout",
  history: "Search for a file to view its history / blame",
  blame: "Search for a file to view who did what (i.e. its blame)",
  theme: "Search for a theme to switch to",
  profile: "Search for a profile to switch to",
  workspace: "Search for a workspace to open",
};

/** Command palette (Ctrl+Shift+P) and its pickers, like the original's "Fuzzy Finder". */
export function CommandPalette() {
  const s = useStore();
  const mode = s.palette;
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const tab = s.tabs.find((t) => t.id === s.activeId);
  const path = tab?.kind === "repo" ? tab.path : undefined;
  const files = useQuery({ queryKey: ["tracked", path], queryFn: () => api.trackedFiles(path!), enabled: !!path && (mode === "history" || mode === "blame") });

  useEffect(() => {
    setQ("");
    setSel(0);
    setTimeout(() => input.current?.focus(), 0);
  }, [mode]);

  const close = () => s.setPalette(null);
  const items: Item[] = useMemo(() => {
    if (!mode) return [];
    const done = (f: () => void) => () => {
      close();
      f();
    };
    switch (mode) {
      case "root":
        return COMMANDS.filter((c) => !c.repo || path).map((c: Command) => ({
          key: c.id,
          label: c.label,
          hint: [c.group, c.keywords].filter(Boolean).join(" "),
          shortcut: keysOf(c),
          run: c.picker ? () => s.setPalette(c.picker!) : done(() => runCommand(c)),
        }));
      case "openRepo": {
        const open = s.tabs.filter((t) => t.kind === "repo").map((t) => t.path!);
        const all = [...new Set([...open, ...s.settings.recents])];
        return all.map((p) => ({ key: p, label: p.split(/[\\/]/).pop()!, hint: p, run: done(() => openRepoPath(p)) }));
      }
      case "checkout": {
        const repo = path ? repoData(path) : undefined;
        return (repo?.refs ?? [])
          .filter((r) => r.kind !== "Tag" && !(r.kind === "Local" && r.name === repo?.head_branch))
          .map((r) => ({
            key: r.full,
            label: r.name,
            hint: r.kind === "Remote" ? "remote branch" : "local branch",
            run: done(() => act(path!, r.kind === "Remote" ? { type: "checkoutRemote", remoteBranch: r.name } : { type: "checkout", target: r.name })),
          }));
      }
      case "history":
      case "blame":
        return (files.data ?? []).map((f) => ({ key: f, label: f.split("/").pop()!, hint: f, run: done(() => openHistory(f, mode)) }));
      case "theme":
        return THEMES.map((t) => ({ key: t.id, label: t.label, hint: s.settings.theme === t.id ? "current" : "", run: done(() => s.setSettings({ theme: t.id })) }));
      case "profile":
        return s.profiles.map((p) => ({ key: p.id, label: p.name, hint: p.id === s.activeProfile ? "current" : "", run: done(() => switchProfile(p.id)) }));
      case "workspace":
        return s.settings.workspaces.map((w) => ({ key: w.id, label: w.name, hint: `${w.repos.length} repositories`, run: done(() => openWorkspace(w.id)) }));
    }
    return [];
  }, [mode, path, files.data, s.tabs, s.settings, s.profiles, s.activeProfile]);

  const shown = useMemo(() => {
    return items
      .map((it) => ({ it, score: Math.max(fuzzy(q, it.label) * 2, fuzzy(q, it.hint ?? "")) }))
      .filter((x) => x.score > 0)
      .sort((a, b) => b.score - a.score)
      .slice(0, 80)
      .map((x) => x.it);
  }, [items, q]);

  if (!mode) return null;
  return (
    <div className="fixed inset-0 z-[65]">
      <button type="button" tabIndex={-1} aria-label="Close command palette" className="absolute inset-0 cursor-default" onClick={close} />
      <div className="absolute left-1/2 top-[64px] -translate-x-1/2 w-[560px] max-w-[90vw] bg-[var(--menu-bg)] border border-white/15 rounded-md shadow-2xl overflow-hidden">
        <input
          ref={input}
          aria-label={PLACEHOLDER[mode]}
          value={q}
          placeholder={PLACEHOLDER[mode]}
          onChange={(e) => (setQ(e.target.value), setSel(0))}
          onKeyDown={(e) => {
            if (e.key === "Escape") return close();
            if (e.key === "ArrowDown") return (e.preventDefault(), setSel((x) => Math.min(shown.length - 1, x + 1)));
            if (e.key === "ArrowUp") return (e.preventDefault(), setSel((x) => Math.max(0, x - 1)));
            if (e.key === "Enter" && shown[sel]) return (e.preventDefault(), shown[sel].run());
            if (e.key === "Backspace" && !q && mode !== "root") s.setPalette("root");
          }}
          className="w-full h-[38px] px-3 bg-field text-[13.5px] text-white outline-none border-b border-white/10 placeholder:text-fg3"
        />
        <div className="max-h-[360px] overflow-auto py-1">
          {mode !== "root" && (mode === "history" || mode === "blame") && files.isLoading && <div className="px-3 py-2 text-fg3 text-[12px]">Loading files…</div>}
          {shown.length === 0 && <div className="px-3 py-2 text-fg3 text-[12.5px]">No matches</div>}
          {shown.map((it, i) => (
            <button
              key={it.key}
              onMouseEnter={() => setSel(i)}
              onClick={it.run}
              className={cn("w-full flex items-center gap-2 px-3 h-[30px] text-left text-[13px]", i === sel ? "bg-accent text-white" : "text-fg")}
            >
              <span className="truncate">{it.label}</span>
              {mode !== "root" && it.hint && <span className={cn("truncate text-[11.5px]", i === sel ? "text-white/80" : "text-fg3")}>{it.hint}</span>}
              {it.shortcut && <span className={cn("ml-auto mono text-[11px] shrink-0", i === sel ? "text-white/85" : "text-fg3")}>{it.shortcut}</span>}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
