import { useQuery } from "@tanstack/react-query";
import { ArrowDown, ArrowUp, Pencil, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { RepoView } from "@/lib/api";
import { api } from "@/lib/api";
import { useStore } from "@/lib/store";
import { act } from "@/lib/repo";
import { Button } from "./ui/button";
import { cn } from "@/lib/utils";

type Segment = { kind: "common"; lines: string[] } | { kind: "conflict"; ours: string[]; base: string[]; theirs: string[]; oursMark: string; theirsMark: string };

/** Split a file with conflict markers into shared text and conflict chunks. */
export function parseConflicts(text: string): Segment[] {
  const lines = text.replace(/\r\n/g, "\n").split("\n");
  if (lines[lines.length - 1] === "") lines.pop();
  const out: Segment[] = [];
  let common: string[] = [];
  let i = 0;
  while (i < lines.length) {
    if (lines[i].startsWith("<<<<<<<")) {
      if (common.length) out.push({ kind: "common", lines: common });
      common = [];
      const oursMark = lines[i];
      const ours: string[] = [];
      const base: string[] = [];
      const theirs: string[] = [];
      let part: string[] = ours;
      i++;
      let theirsMark = ">>>>>>>";
      while (i < lines.length) {
        const l = lines[i];
        if (l.startsWith("|||||||")) part = base;
        else if (l.startsWith("=======") && part !== theirs) part = theirs;
        else if (l.startsWith(">>>>>>>")) {
          theirsMark = l;
          break;
        } else part.push(l);
        i++;
      }
      out.push({ kind: "conflict", ours, base, theirs, oursMark, theirsMark });
      i++;
    } else {
      common.push(lines[i]);
      i++;
    }
  }
  if (common.length) out.push({ kind: "common", lines: common });
  return out;
}

interface Pick {
  a: boolean[];
  b: boolean[];
  touched: boolean;
}

/** Merge conflict tool: A (current) and B (incoming) on top, Output below, like the original. */
export function ConflictView({ repo, tabId, file }: { repo: RepoView; tabId: string; file: string }) {
  const s = useStore();
  const m = s.metrics;
  const q = useQuery({ queryKey: ["conflict", repo.path, file, JSON.stringify(repo.status)], queryFn: () => api.conflictInfo(repo.path, file) });
  const info = q.data;
  const segments = useMemo(() => (info?.working ? parseConflicts(info.working) : []), [info?.working]);
  const chunks = segments.map((seg, i) => ({ seg, i })).filter((x) => x.seg.kind === "conflict");
  const [picks, setPicks] = useState<Record<number, Pick>>({});
  const [manual, setManual] = useState<string | null>(null);
  const [current, setCurrent] = useState(0);
  const panes = useRef<(HTMLDivElement | null)[]>([]);
  const crlf = !!info?.working?.includes("\r\n");
  useEffect(() => {
    setPicks({});
    setManual(null);
    setCurrent(0);
  }, [file, info?.working]);

  const pickOf = (i: number): Pick => {
    const seg = segments[i] as Extract<Segment, { kind: "conflict" }>;
    return picks[i] ?? { a: seg.ours.map(() => false), b: seg.theirs.map(() => false), touched: false };
  };
  const setPick = (i: number, p: Pick) => setPicks((old) => ({ ...old, [i]: { ...p, touched: true } }));
  const takeAll = (side: "a" | "b", on: boolean) => {
    const next: Record<number, Pick> = { ...picks };
    for (const { i } of chunks) {
      const p = pickOf(i);
      next[i] = { ...p, [side]: p[side].map(() => on), touched: true };
    }
    setPicks(next);
  };

  // Output: shared text plus, for each chunk, the taken A lines then the taken B lines.
  // Chunks nobody has touched yet stay as conflict markers.
  const built = useMemo(() => {
    const out: string[] = [];
    segments.forEach((seg, i) => {
      if (seg.kind === "common") return out.push(...seg.lines);
      const p = picks[i];
      if (!p?.touched) {
        out.push(seg.oursMark, ...seg.ours, "=======", ...seg.theirs, seg.theirsMark);
        return;
      }
      seg.ours.forEach((l, k) => p.a[k] && out.push(l));
      seg.theirs.forEach((l, k) => p.b[k] && out.push(l));
    });
    return out.join("\n") + "\n";
  }, [segments, picks]);
  const output = manual ?? built;
  const unresolved = chunks.filter(({ i }) => !picks[i]?.touched).length;
  const allDone = manual !== null || unresolved === 0;

  const save = async () => {
    const text = crlf ? output.replace(/\n/g, "\r\n") : output;
    if (allDone) {
      const ok = await act(repo.path, { type: "saveResolved", file, content: text });
      if (ok) s.setUi(tabId, { conflict: null });
    } else {
      // Like "Create Conflict Chunks": keep markers for the chunks not decided yet.
      await api.saveFile(repo.path, file, text);
      s.toast(true, `Saved ${file}; ${unresolved} conflict${unresolved === 1 ? "" : "s"} still to resolve`);
    }
  };

  const go = (dir: 1 | -1) => {
    if (!chunks.length) return;
    const next = (current + dir + chunks.length) % chunks.length;
    setCurrent(next);
    for (const el of panes.current) el?.querySelector(`[data-chunk="${chunks[next].i}"]`)?.scrollIntoView({ block: "center" });
  };

  const [ours, theirs] = info ? [info.ours_label, info.theirs_label] : ["current", "incoming"];
  const close = () => s.setUi(tabId, { conflict: null });

  if (q.isLoading) return <div className="p-4 text-fg3 text-[13px]">Loading conflict…</div>;
  if (!info) return null;

  const header = (
    <div className="flex items-center gap-2 h-[32px] pl-3 pr-2 bg-toolbar/60 border-b border-black/30 shrink-0">
      <span className="w-[14px] h-[14px] rounded-[3px] text-[10px] font-bold flex items-center justify-center bg-bad/30 text-[#ef7a76]">!</span>
      <span className="text-[13px] text-white truncate flex-1">{file}</span>
      {chunks.length > 0 && (
        <>
          <span className="text-[12px] text-fg2 whitespace-nowrap">
            conflict {Math.min(current + 1, chunks.length)} of {chunks.length}
          </span>
          <button className="text-fg2 hover:text-white" title="Previous conflict" onClick={() => go(-1)}>
            <ArrowUp size={14} />
          </button>
          <button className="text-fg2 hover:text-white" title="Next conflict" onClick={() => go(1)}>
            <ArrowDown size={14} />
          </button>
        </>
      )}
      <Button size="sm" className="h-[22px]" onClick={() => act(repo.path, { type: "takeSide", paths: [file], side: "ours" }).then((ok) => ok && close())}>
        Take current ({ours})
      </Button>
      <Button size="sm" className="h-[22px]" onClick={() => act(repo.path, { type: "takeSide", paths: [file], side: "theirs" }).then((ok) => ok && close())}>
        Take incoming ({theirs})
      </Button>
      <button onClick={close} className="text-fg2 hover:text-white p-1" title="Close (Esc)">
        <X size={16} />
      </button>
    </div>
  );

  if (info.binary || segments.every((x) => x.kind === "common")) {
    return (
      <div className="h-full flex flex-col bg-app min-w-0">
        {header}
        <div className="p-5 text-[13px] text-fg2 leading-relaxed">
          {info.binary
            ? "This is a binary file, so its lines can't be merged here. Choose which version to keep with Take current or Take incoming."
            : "This file has no conflict markers (for example one side deleted it). Choose which version to keep, or mark it resolved from the file list."}
        </div>
      </div>
    );
  }

  const pane = (side: "a" | "b", label: string, color: string, idx: number) => (
    <div className="flex-1 flex flex-col min-w-0 border-r border-black/30 last:border-r-0">
      <div className="flex items-center gap-2 h-[26px] px-2 text-[12px] border-b border-white/5 shrink-0" style={{ boxShadow: `inset 0 -2px 0 ${color}` }}>
        <span className="font-bold" style={{ color }}>
          {side.toUpperCase()}
        </span>
        <span className="text-fg truncate flex-1">{label}</span>
        <label className="flex items-center gap-1 text-fg2 cursor-pointer">
          <input type="checkbox" className="accent-[#80B1EA]" onChange={(e) => takeAll(side, e.target.checked)} /> Take all
        </label>
      </div>
      <div ref={(el) => void (panes.current[idx] = el)} className="flex-1 overflow-auto mono text-[12px] leading-[19px]">
        {segments.map((seg, i) =>
          seg.kind === "common" ? (
            seg.lines.map((l, k) => (
              <div key={`${i}-${k}`} className="pl-7 pr-3 whitespace-pre text-white/55">
                {l || " "}
              </div>
            ))
          ) : (
            <div key={i} data-chunk={i} className="my-[2px]" style={{ background: color + "40", boxShadow: `inset 3px 0 0 ${color}` }}>
              {(side === "a" ? seg.ours : seg.theirs).map((l, k) => {
                const p = pickOf(i);
                const on = p[side][k];
                return (
                  <label key={k} className="flex items-start gap-1.5 pl-1.5 pr-3 cursor-pointer hover:bg-white/5" title="Take this line">
                    <input
                      type="checkbox"
                      checked={on}
                      className="mt-[3px] accent-[#80B1EA]"
                      onChange={() => setPick(i, { ...p, [side]: p[side].map((x, j) => (j === k ? !x : x)) })}
                    />
                    <span className="whitespace-pre text-white/90">{l || " "}</span>
                  </label>
                );
              })}
              {(side === "a" ? seg.ours : seg.theirs).length === 0 && <div className="pl-7 text-[11px] text-fg3 italic">(no lines on this side)</div>}
            </div>
          ),
        )}
      </div>
    </div>
  );

  return (
    <div className="h-full flex flex-col bg-app min-w-0">
      {header}
      <div className="flex-1 flex min-h-0">
        {pane("a", ours, m.conflict_a, 0)}
        {pane("b", theirs, m.conflict_b, 1)}
      </div>
      <div className="h-[45%] flex flex-col border-t-2 min-h-0" style={{ borderColor: m.conflict_output }}>
        <div className="flex items-center gap-2 h-[30px] px-2 text-[12px] border-b border-white/5 shrink-0">
          <span className="font-bold" style={{ color: m.conflict_output }}>
            Output
          </span>
          <span className="text-fg3 flex-1">{manual !== null ? "edited by hand" : unresolved ? `${unresolved} conflict${unresolved === 1 ? "" : "s"} left` : "all conflicts decided"}</span>
          {manual === null && (
            <Button size="sm" className="h-[22px]" onClick={() => setManual(built)} title="Type in the output directly">
              <Pencil size={11} /> Edit
            </Button>
          )}
          <Button size="sm" className="h-[22px]" onClick={() => (setPicks({}), setManual(null))}>
            Reset
          </Button>
          <Button size="sm" variant="success" className="h-[22px]" onClick={save}>
            {allDone ? "Save Output" : "Create Conflict Chunks"}
          </Button>
        </div>
        {manual !== null ? (
          <textarea
            value={manual}
            onChange={(e) => setManual(e.target.value)}
            spellCheck={false}
            className="flex-1 bg-app mono text-[12px] leading-[19px] text-white/90 p-2 outline-none resize-none"
          />
        ) : (
          <div className="flex-1 overflow-auto mono text-[12px] leading-[19px] py-1">
            {output.split("\n").map((l, k) => (
              <div key={k} className={cn("px-3 whitespace-pre", /^(<<<<<<<|=======|>>>>>>>)/.test(l) ? "text-[#ef7a76] bg-bad/10" : "text-white/85")}>
                {l || " "}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
