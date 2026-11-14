import { useQuery } from "@tanstack/react-query";
import {
  ArrowDown,
  ArrowUp,
  Columns2,
  Pencil,
  Pilcrow,
  Rows3,
  Save,
  TextQuote,
  WrapText,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { DiffLine, DiffSource, PatchOp, RepoView } from "@/lib/api";
import { api, WORKDIR } from "@/lib/api";
import type { DiffTarget } from "@/lib/store";
import { useStore, usePanelStore } from "@/lib/store";
import { act, confirmThen, refresh } from "@/lib/repo";
import { Button } from "./ui/button";
import { StatusIcon } from "./Icons";
import { MonacoDiff, MonacoFile, type DiffHandle } from "./MonacoViews";
import { cn } from "@/lib/utils";

export function sourceLabel(src: DiffSource, shaLen: number) {
  switch (src.kind) {
    case "Unstaged":
      return "Unstaged";
    case "Staged":
      return "Staged";
    case "Commit":
      return src.sha.slice(0, shaLen);
    case "Range":
      return `${src.sha[0].slice(0, shaLen)} → ${src.sha[1] === WORKDIR ? "Working folder" : src.sha[1].slice(0, shaLen)}`;
  }
}

/** A file's changes, opened over the graph: Hunk / Inline / Split diff, or File View. */
export function DiffView({ repo, tabId, target }: { repo: RepoView; tabId: string; target: DiffTarget }) {
  const s = usePanelStore();
  const set = s.settings;
  const m = s.metrics;
  const [view, setView] = useState<"diff" | "file">("diff");
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<string | null>(null);
  const handle = useRef<DiffHandle | null>(null);
  const hunkScroll = useRef<HTMLDivElement>(null);
  const close = () => s.setUi(tabId, { diff: null });

  const statusKey = target.source.kind === "Commit" ? "" : JSON.stringify(repo.status);
  const versions = useQuery({
    queryKey: ["versions", repo.path, JSON.stringify(target), statusKey],
    queryFn: () => api.fileVersions(repo.path, target.source, target.file, target.untracked, target.parent),
  });
  const lines = useQuery({
    queryKey: ["diff", repo.path, JSON.stringify(target), statusKey, set.ignoreWhitespace],
    queryFn: () => api.fileDiff(repo.path, target.source, target.file, target.untracked, target.parent, set.ignoreWhitespace),
    enabled: set.diffMode === "hunk",
  });

  useEffect(() => {
    setEditing(false);
    setDraft(null);
  }, [target.file, JSON.stringify(target.source)]);

  const v = versions.data;
  const editable = !!v?.editable && !v.binary;
  const added = (lines.data ?? []).filter((l) => l.kind === "Added").length;
  const removed = (lines.data ?? []).filter((l) => l.kind === "Removed").length;
  const inList = (list: { path: string }[]) => list.some((f) => f.path === target.file);
  const canStage = target.source.kind === "Unstaged" && inList(repo.status.unstaged);
  const canUnstage = target.source.kind === "Staged" && inList(repo.status.staged);
  const dir = target.file.includes("/") ? target.file.slice(0, target.file.lastIndexOf("/") + 1) : "";
  const name = target.file.slice(dir.length);
  const status =
    (target.source.kind === "Unstaged" ? repo.status.unstaged : target.source.kind === "Staged" ? repo.status.staged : []).find((f) => f.path === target.file)?.status ??
    (target.untracked ? "?" : "M");

  const save = async () => {
    if (draft === null) return;
    try {
      await api.saveFile(repo.path, target.file, draft);
      s.toast(true, `Saved ${name}`);
      setDraft(null);
      setEditing(false);
      await refresh(repo.path);
    } catch (e) {
      s.toast(false, String(e));
    }
  };

  const next = (dir: 1 | -1) => {
    if (view === "diff" && set.diffMode !== "hunk") return dir === 1 ? handle.current?.next() : handle.current?.prev();
    const el = hunkScroll.current;
    if (!el) return;
    const hunks = [...el.querySelectorAll<HTMLElement>("[data-hunk]")];
    const top = el.scrollTop + 4;
    const target = dir === 1 ? hunks.find((h) => h.offsetTop > top) : [...hunks].reverse().find((h) => h.offsetTop < top - 8);
    if (target) el.scrollTop = target.offsetTop;
  };

  const modeBtn = (mode: "hunk" | "inline" | "split", icon: React.ReactNode, title: string) => (
    <IconToggle active={view === "diff" && set.diffMode === mode} title={title} onClick={() => (setView("diff"), s.setSettings({ diffMode: mode }))}>
      {icon}
    </IconToggle>
  );

  let body: React.ReactNode;
  if (versions.isLoading) body = <Note>Loading…</Note>;
  else if (versions.isError) body = <Note error>{String(versions.error)}</Note>;
  else if (v && (v.old_image || v.new_image)) body = <ImageDiff before={v.old_image} after={v.new_image} />;
  else if (v?.binary) body = <Note>This is a binary file, so there is no text to compare.</Note>;
  else if (v && view === "file")
    body = (
      <MonacoFile
        value={draft ?? v.new ?? v.old ?? ""}
        file={target.file}
        readOnly={!editing}
        wordWrap={set.wordWrap}
        onChange={(t) => editing && setDraft(t)}
        onSave={save}
      />
    );
  else if (v && set.diffMode !== "hunk")
    body = (
      <MonacoDiff
        original={v.old ?? ""}
        modified={draft ?? v.new ?? ""}
        file={target.file}
        sideBySide={set.diffMode === "split"}
        wordWrap={set.wordWrap}
        ignoreWhitespace={set.ignoreWhitespace}
        editable={editing}
        onChange={(t) => editing && setDraft(t)}
        onSave={save}
        handle={handle}
      />
    );
  else if (lines.isError) body = <Note error>{String(lines.error)}</Note>;
  else
    body = (
      <HunkView
        repo={repo}
        target={target}
        lines={lines.data ?? []}
        loading={lines.isLoading}
        wordWrap={set.wordWrap}
        actions={(canStage || canUnstage || target.source.kind === "Commit") && !set.ignoreWhitespace}
        scrollRef={hunkScroll}
      />
    );

  return (
    <div className="h-full flex flex-col bg-app min-w-0">
      <div className="flex items-center gap-2 h-[30px] pl-3 pr-2 bg-toolbar/60 border-b border-black/30 shrink-0">
        <StatusIcon status={status} />
        <span className="text-[13px] truncate min-w-0 flex-1" title={target.file}>
          <span className="text-fg2">{dir}</span>
          <span className="text-white">{name}</span>
        </span>
        {set.diffMode === "hunk" && view === "diff" && lines.data && (
          <span className="text-[11.5px] whitespace-nowrap">
            <span className="text-good">+{added}</span> <span className="text-[#ef7a76]">−{removed}</span>
          </span>
        )}
        <span className="text-[11px] text-fg3">{v?.encoding ?? "UTF-8"}</span>
        {canStage && (
          <Button size="sm" variant="success" onClick={() => act(repo.path, { type: "stage", paths: [target.file] }, true)}>
            Stage File
          </Button>
        )}
        {canUnstage && (
          <Button size="sm" variant="danger" onClick={() => act(repo.path, { type: "unstage", paths: [target.file] }, true)}>
            Unstage File
          </Button>
        )}
        <button onClick={close} className="text-fg2 hover:text-white p-1" title="Close (Esc)">
          <X size={16} />
        </button>
      </div>
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1 min-h-[32px] py-1 px-2 bg-app border-b border-white/5 shrink-0">
        <div className="flex-1 flex items-center gap-1.5 min-w-0">
          {editable && !editing && (
            <Button size="sm" variant="primary" className="h-[22px]" onClick={() => (setEditing(true), setView("file"))} title="Edit the working-folder copy of this file">
              <Pencil size={12} /> {target.source.kind === "Staged" ? "Edit in Working Directory" : "Edit This File"}
            </Button>
          )}
          {editing && (
            <>
              <Button size="sm" variant="success" className="h-[22px]" disabled={draft === null} onClick={save} title="Save (Ctrl+S)">
                <Save size={12} /> Save Changes
              </Button>
              <Button size="sm" className="h-[22px]" onClick={() => (setEditing(false), setDraft(null))}>
                Cancel
              </Button>
            </>
          )}
        </div>
        <div className="flex rounded-[3px] overflow-hidden border border-white/10 text-[12px] shrink-0">
          <span className="px-2.5 py-[2px] bg-accent/25 text-white whitespace-nowrap">{sourceLabel(target.source, m.sha_len)}</span>
          <button className={cn("px-2.5 py-[2px]", view === "file" ? "bg-white/15 text-white" : "text-fg2 hover:text-white")} onClick={() => setView("file")}>
            File View
          </button>
          <button className={cn("px-2.5 py-[2px]", view === "diff" ? "bg-white/15 text-white" : "text-fg2 hover:text-white")} onClick={() => setView("diff")}>
            Diff View
          </button>
        </div>
        <div className="flex-1 flex items-center justify-end gap-0.5 min-w-0">
          <IconToggle title="Previous change" onClick={() => next(-1)}>
            <ArrowUp size={14} />
          </IconToggle>
          <IconToggle title="Next change" onClick={() => next(1)}>
            <ArrowDown size={14} />
          </IconToggle>
          <span className="w-2" />
          {modeBtn("hunk", <Rows3 size={14} />, "Hunk View")}
          {modeBtn("inline", <TextQuote size={14} />, "Inline View")}
          {modeBtn("split", <Columns2 size={14} />, "Split View")}
          <span className="w-2" />
          <IconToggle active={set.ignoreWhitespace} title="Ignore Leading/Trailing Whitespace" onClick={() => s.setSettings({ ignoreWhitespace: !set.ignoreWhitespace })}>
            <Pilcrow size={14} />
          </IconToggle>
          <IconToggle active={set.wordWrap} title="Word Wrap" onClick={() => s.setSettings({ wordWrap: !set.wordWrap })}>
            <WrapText size={14} />
          </IconToggle>
        </div>
      </div>
      <div className="flex-1 min-h-0">{body}</div>
    </div>
  );
}

function IconToggle({ children, title, onClick, active }: { children: React.ReactNode; title: string; onClick: () => void; active?: boolean }) {
  return (
    <button aria-label={title} aria-pressed={active} title={title} onClick={onClick} className={cn("w-[24px] h-[22px] rounded-[3px] flex items-center justify-center", active ? "bg-accent/30 text-white" : "text-fg2 hover:text-white hover:bg-white/10")}>
      {children}
    </button>
  );
}

function Note({ children, error }: { children: React.ReactNode; error?: boolean }) {
  return <div className={cn("p-4 text-[13px]", error ? "text-[#ef7a76]" : "text-fg3")}>{children}</div>;
}

function ImageDiff({ before, after }: { before: string | null; after: string | null }) {
  const pane = (label: string, src: string | null, color: string) => (
    <div className="flex-1 flex flex-col items-center gap-2 min-w-0 p-4">
      <span className="text-[12px] font-semibold" style={{ color }}>
        {label}
      </span>
      <div className="flex-1 w-full flex items-center justify-center rounded border border-white/10 bg-[repeating-conic-gradient(#2a2d34_0%_25%,#22252b_0%_50%)] bg-[length:16px_16px] min-h-[120px]">
        {src ? <img src={src} alt={`${label} version`} className="max-w-full max-h-[60vh] object-contain" /> : <span className="text-fg3 text-[12px]">No image</span>}
      </div>
    </div>
  );
  return (
    <div className="h-full flex overflow-auto">
      {pane("Before", before, "#ef7a76")}
      {pane("After", after, "#5cb85c")}
    </div>
  );
}

interface Hunk {
  header: string;
  lines: DiffLine[]; // body lines, indexes match the patch builder in src/files.rs
}

function splitHunks(lines: DiffLine[]): { pre: DiffLine[]; hunks: Hunk[] } {
  const pre: DiffLine[] = [];
  const hunks: Hunk[] = [];
  for (const l of lines) {
    if (l.kind === "Hunk") hunks.push({ header: l.text, lines: [] });
    else if (hunks.length) hunks[hunks.length - 1].lines.push(l);
    else pre.push(l);
  }
  return { pre, hunks };
}

/** Hunk View: each change block with Stage / Unstage / Discard, and line picking (click, Shift+click). */
function HunkView({
  repo,
  target,
  lines,
  loading,
  wordWrap,
  actions,
  scrollRef,
}: {
  repo: RepoView;
  target: DiffTarget;
  lines: DiffLine[];
  loading: boolean;
  wordWrap: boolean;
  actions: boolean;
  scrollRef: React.RefObject<HTMLDivElement | null>;
}) {
  const { pre, hunks } = useMemo(() => splitHunks(lines), [lines]);
  const [sel, setSel] = useState<{ hunk: number; lines: number[]; anchor: number } | null>(null);
  useEffect(() => setSel(null), [lines]);
  const staged = target.source.kind === "Staged";

  const run = (op: PatchOp, hunk: number, picked: number[] | null) => {
    const go = () => {
      void act(repo.path, { type: "patch", file: target.file, untracked: target.untracked, op, hunk, lines: picked }).then(() => setSel(null));
    };
    if (op === "discard")
      confirmThen(
        picked ? `Discard ${picked.length} line${picked.length === 1 ? "" : "s"}?` : "Discard this hunk?",
        "These changes are removed from the working folder. You can bring them back with Undo.",
        "Discard",
        go,
      );
    else go();
  };

  const pick = (h: number, i: number, shift: boolean) => {
    if (!actions) return;
    setSel((cur) => {
      if (!cur || cur.hunk !== h) return { hunk: h, lines: [i], anchor: i };
      if (shift) {
        const [a, b] = [Math.min(cur.anchor, i), Math.max(cur.anchor, i)];
        const range = hunks[h].lines.map((l, k) => ({ l, k })).filter(({ l, k }) => k >= a && k <= b && (l.kind === "Added" || l.kind === "Removed")).map(({ k }) => k);
        return { ...cur, lines: range };
      }
      const has = cur.lines.includes(i);
      const ls = has ? cur.lines.filter((x) => x !== i) : [...cur.lines, i];
      return ls.length ? { hunk: h, lines: ls, anchor: i } : null;
    });
  };

  if (loading) return <Note>Loading diff…</Note>;
  if (lines.length === 0) return <Note>No changes to show (the file may be empty, or only its mode changed).</Note>;

  return (
    <div ref={scrollRef} className="h-full overflow-auto mono text-[12px] leading-[19px] relative">
      {pre.map((l, i) => (
        <div key={i} className="px-3 py-1 text-fg3 font-sans">
          {l.text}
        </div>
      ))}
      {hunks.map((h, hi) => {
        const picked = sel?.hunk === hi ? sel.lines : [];
        const n = picked.length;
        return (
          <div key={hi} data-hunk className="mb-2">
            <div className="sticky top-0 z-10 flex items-center gap-2 h-[28px] px-3 bg-panel2 border-y border-white/5 font-sans">
              <span className="text-[11.5px] text-fg2 truncate flex-1 mono">{h.header}</span>
              {actions && target.source.kind === "Unstaged" && (
                <>
                  <Button size="sm" variant="danger" className="h-[20px] text-[11px]" onClick={() => run("discard", hi, n ? picked : null)}>
                    {n ? `Discard ${n} Line${n === 1 ? "" : "s"}` : "Discard Hunk"}
                  </Button>
                  <Button size="sm" variant="success" className="h-[20px] text-[11px]" onClick={() => run("stage", hi, n ? picked : null)}>
                    {n ? `Stage ${n} Line${n === 1 ? "" : "s"}` : "Stage Hunk"}
                  </Button>
                </>
              )}
              {actions && target.source.kind === "Commit" && (
                <Button
                  size="sm"
                  className="h-[20px] text-[11px]"
                  title="Apply the inverse of this hunk to your working directory"
                  onClick={async () => {
                    const sha = (target.source as { kind: "Commit"; sha: string }).sha;
                    await act(repo.path, { type: "revertHunk", sha, parent: target.parent, file: target.file, hunk: hi, lines: n ? picked : null });
                    setSel(null);
                  }}
                >
                  {n ? `Revert ${n} Line${n === 1 ? "" : "s"}` : "Revert Hunk"}
                </Button>
              )}
              {actions && staged && (
                <Button size="sm" variant="danger" className="h-[20px] text-[11px]" onClick={() => run("unstage", hi, n ? picked : null)}>
                  {n ? `Unstage ${n} Line${n === 1 ? "" : "s"}` : "Unstage Hunk"}
                </Button>
              )}
            </div>
            <table className="border-collapse min-w-full">
              <tbody>
                {h.lines.map((l, i) => {
                  const change = l.kind === "Added" || l.kind === "Removed";
                  const on = picked.includes(i);
                  return (
                    <tr
                      key={i}
                      onClick={(e) => change && pick(hi, i, e.shiftKey)}
                      className={cn(
                        l.kind === "Added" && "bg-good/20",
                        l.kind === "Removed" && "bg-bad/20",
                        l.kind === "Meta" && "text-fg3",
                        on && "outline outline-1 -outline-offset-1 outline-accent bg-accent/25",
                        change && actions && "cursor-pointer hover:brightness-125",
                      )}
                      title={change && actions ? "Click to pick this line, Shift+click to pick a range" : undefined}
                    >
                      <td className="w-[44px] text-right pr-2 text-fg3 select-none align-top">{l.old_no ?? ""}</td>
                      <td className="w-[44px] text-right pr-2 text-fg3 select-none align-top border-r border-white/5">{l.new_no ?? ""}</td>
                      <td className={cn("pl-2 pr-4 text-white/85 select-text", wordWrap ? "whitespace-pre-wrap break-all" : "whitespace-pre")}>
                        <span className="select-none inline-block w-3 text-fg3">{l.kind === "Added" ? "+" : l.kind === "Removed" ? "-" : " "}</span>
                        {l.text}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        );
      })}
    </div>
  );
}
