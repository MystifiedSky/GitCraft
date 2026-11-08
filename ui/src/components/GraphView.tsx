import { Check, Cloud, Laptop, MessageSquare, Network, Settings2, Tag, X, ChevronUp, ChevronDown } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { Commit, GitRef, RepoView } from "@/lib/api";
import { useStore, useTabUi } from "@/lib/store";
import { act } from "@/lib/repo";
import { branchMenu, commitMenu } from "@/lib/menus";
import { CtxMenu } from "./ui/menu";
import { Button } from "./ui/button";
import { cn, initials, laneColor, mix } from "@/lib/utils";


interface Props {
  repo: RepoView;
  tabId: string;
}

export function GraphView({ repo, tabId }: Props) {
  // Sizes from the original's graph constants (src/metrics.rs, MECHANICS.md).
  const m = useStore((s) => s.metrics);
  const ROW = m.row;
  const NODE = m.node;
  const LANE = m.lane;
  const GUTTER = m.gutter;
  const PAD = m.pad;
  const HEADER = m.header;
  const ui = useTabUi(tabId);
  const setUi = useStore((s) => s.setUi);
  const scrollRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewH, setViewH] = useState(400);
  const [paneW, setPaneW] = useState(300);

  const wip = repo.status.unstaged.length + repo.status.staged.length > 0;
  const off = wip ? 1 : 0;
  const total = repo.commits.length + off;

  const refsBySha = useMemo(() => {
    const m = new Map<string, GitRef[]>();
    const order = (r: GitRef) => (r.kind === "Local" && r.name === repo.head_branch ? 0 : r.kind === "Local" ? 1 : r.kind === "Remote" ? 2 : 3);
    for (const r of repo.refs) m.set(r.sha, [...(m.get(r.sha) ?? []), r]);
    for (const [k, v] of m) m.set(k, v.sort((a, b) => order(a) - order(b)));
    return m;
  }, [repo]);

  const headIdx = repo.commits.findIndex((c) => c.sha === repo.head_sha);
  const headLane = headIdx >= 0 ? repo.graph.rows[headIdx]?.col ?? 0 : 0;

  // Search (Ctrl+F)
  const q = ui.search.trim().toLowerCase();
  const matches = useMemo(() => {
    if (!q) return [] as number[];
    const out: number[] = [];
    repo.commits.forEach((c, i) => {
      if (c.summary.toLowerCase().includes(q) || c.author.toLowerCase().includes(q) || c.sha.startsWith(q) || c.body.toLowerCase().includes(q)) out.push(i);
    });
    return out;
  }, [repo, q]);
  const matchSet = useMemo(() => new Set(matches), [matches]);

  // Zone widths: default widths when the pane has room for all three zones,
  // otherwise the branch/tag and graph zones shrink like the original's narrow layout.
  const lanesW = GUTTER + Math.max(1, repo.graph.width) * LANE + PAD * 2;
  const roomy = paneW >= m.ref_zone + m.graph_zone + m.message_zone;
  // Narrow: the graph keeps its lanes; the branch/tag zone gives way first (down to REF_ZONE_MIN_WIDTH),
  // and the message zone keeps at least COMMIT_MESSAGE_ZONE_MIN_WIDTH.
  const graphW = roomy ? Math.max(m.graph_zone, lanesW) : Math.max(0, Math.min(lanesW, paneW - m.ref_zone_min - m.message_min));
  const refW = roomy
    ? m.ref_zone
    : Math.round(Math.max(m.ref_zone_min, Math.min(m.ref_zone, paneW * 0.42, paneW - graphW - m.message_min)));
  const msgW = paneW - refW - graphW;
  const laneX = (l: number) => refW + GUTTER + l * LANE + LANE / 2;

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      setViewH(el.clientHeight);
      setPaneW(el.clientWidth);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const first = Math.max(0, Math.floor(scrollTop / ROW) - 4);
  const last = Math.min(total - 1, Math.ceil((scrollTop + viewH) / ROW) + 4);

  // Keep the selected row in view.
  const selIdx = ui.selection.type === "wip" ? (wip ? 0 : -1) : (repo.commits.findIndex((c) => c.sha === (ui.selection as { sha: string }).sha) + off);
  useEffect(() => {
    const el = scrollRef.current;
    if (!el || selIdx < 0) return;
    const top = selIdx * ROW;
    if (top < el.scrollTop) el.scrollTop = top;
    else if (top + ROW > el.scrollTop + el.clientHeight) el.scrollTop = top + ROW - el.clientHeight;
  }, [selIdx]);

  // Draw the visible part of the graph on the canvas.
  useLayoutEffect(() => {
    const cv = canvasRef.current;
    if (!cv) return;
    const dpr = window.devicePixelRatio || 1;
    const w = refW + graphW;
    cv.width = Math.ceil(w * dpr);
    cv.height = Math.ceil(viewH * dpr);
    cv.style.width = `${w}px`;
    cv.style.height = `${viewH}px`;
    const g = cv.getContext("2d")!;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, w, viewH);
    g.lineWidth = 2;
    g.lineCap = "round";

    // Lines change lane with a rounded corner of COMMIT_ZONE_EDGE_ARC_RADIUS.
    const R = m.arc_radius;
    const curve = (x1: number, y1: number, x2: number, y2: number, color: string, downward: boolean) => {
      g.strokeStyle = color;
      g.beginPath();
      g.moveTo(x1, y1);
      if (x1 === x2) g.lineTo(x2, y2);
      else if (downward) {
        // across from the node, then turn down into the other lane
        g.arcTo(x2, y1, x2, y2, R);
        g.lineTo(x2, y2);
      } else {
        // down the lane, then turn across into the node
        g.arcTo(x1, y2, x2, y2, R);
        g.lineTo(x2, y2);
      }
      g.stroke();
    };

    for (let d = first; d <= last; d++) {
      const y0 = d * ROW - scrollTop;
      const yc = y0 + ROW / 2;
      const y1 = y0 + ROW;
      if (d < off) {
        // WIP row: dashed node joined to the checked-out commit.
        const x = laneX(headLane);
        g.setLineDash([3, 3]);
        g.strokeStyle = laneColor(headLane);
        g.beginPath();
        g.arc(x, yc, NODE / 2 - 1, 0, Math.PI * 2);
        g.stroke();
        if (headIdx >= 0) {
          g.beginPath();
          g.moveTo(x, yc + NODE / 2);
          g.lineTo(x, Math.min(viewH, (headIdx + off) * ROW - scrollTop + ROW / 2));
          g.stroke();
        }
        g.setLineDash([]);
        continue;
      }
      const i = d - off;
      const row = repo.graph.rows[i];
      if (!row) continue;
      for (const s of row.up) curve(laneX(s.from), y0, laneX(s.to), yc, laneColor(s.color), false);
      for (const s of row.down) curve(laneX(s.from), yc, laneX(s.to), y1, laneColor(s.color), true);
    }
    // Nodes on top of the lines; connector from the branch label to its node.
    for (let d = Math.max(first, off); d <= last; d++) {
      const i = d - off;
      const row = repo.graph.rows[i];
      const c = repo.commits[i];
      if (!row || !c) continue;
      const yc = d * ROW - scrollTop + ROW / 2;
      const x = laneX(row.col);
      const col = laneColor(row.color);
      if (refsBySha.has(c.sha)) {
        g.strokeStyle = col;
        g.beginPath();
        g.moveTo(refW - m.ref_margin_right, yc);
        g.lineTo(x - NODE / 2, yc);
        g.stroke();
      }
      if (row.merge) {
        g.fillStyle = col;
        g.beginPath();
        g.arc(x, yc, 6, 0, Math.PI * 2);
        g.fill();
      } else {
        g.fillStyle = col;
        g.beginPath();
        g.arc(x, yc, NODE / 2, 0, Math.PI * 2);
        g.fill();
        // Avatar inside the ring: initials on the default avatar background.
        g.fillStyle = m.avatar_bg;
        g.beginPath();
        g.arc(x, yc, m.avatar_inner / 2, 0, Math.PI * 2);
        g.fill();
        g.fillStyle = "#ffffff";
        g.font = "600 8.5px 'Open Sans', 'Segoe UI', sans-serif";
        g.textAlign = "center";
        g.textBaseline = "middle";
        g.fillText(initials(c.author), x, yc + 0.5);
      }
    }
  }, [repo, first, last, scrollTop, viewH, refW, graphW, off, headLane, headIdx, refsBySha, m]);

  const select = (sel: { type: "wip" } | { type: "commit"; sha: string }) => setUi(tabId, { selection: sel, diff: null });

  const jump = (dir: 1 | -1) => {
    if (!matches.length) return;
    const cur = ui.selection.type === "commit" ? repo.commits.findIndex((c) => c.sha === (ui.selection as { sha: string }).sha) : -1;
    let next = dir === 1 ? matches.find((m) => m > cur) : [...matches].reverse().find((m) => m < cur);
    if (next === undefined) next = dir === 1 ? matches[0] : matches[matches.length - 1];
    select({ type: "commit", sha: repo.commits[next].sha });
  };

  const rows = [];
  for (let d = first; d <= last; d++) {
    if (d < off) {
      const sel = ui.selection.type === "wip";
      rows.push(
        <div
          key="wip"
          className={cn("absolute left-0 right-0 flex items-center", sel ? "bg-selrow" : "hover:bg-white/[0.03]")}
          style={{ top: d * ROW, height: ROW }}
          onClick={() => select({ type: "wip" })}
        >
          <div style={{ width: refW + graphW }} className="shrink-0" />
          <div
            className="flex-1 min-w-0 flex items-center mr-[4px] rounded-[2px] border border-dashed border-white/30 bg-panel text-[13px] text-white/85 italic"
            style={{ height: m.row_inner, paddingLeft: m.summary_margin }}
          >
            <span className="truncate">// WIP</span>
          </div>
        </div>,
      );
      continue;
    }
    const c = repo.commits[d - off];
    const gr = repo.graph.rows[d - off];
    if (!c || !gr) continue;
    const sel = ui.selection.type === "commit" && ui.selection.sha === c.sha;
    const col = laneColor(gr.color);
    rows.push(
      <CommitRow
        key={c.sha}
        repo={repo}
        commit={c}
        top={d * ROW}
        refW={refW}
        graphW={graphW}
        color={col}
        selected={sel}
        match={matchSet.has(d - off)}
        dim={q.length > 0 && !matchSet.has(d - off)}
        refs={refsBySha.get(c.sha)}
        rowH={ROW}
        onSelect={() => select({ type: "commit", sha: c.sha })}
      />,
    );
  }

  return (
    <div className="h-full flex flex-col bg-app min-w-0">
      <div className="flex items-center shrink-0 bg-panel border-b border-black/30 text-[10.5px] tracking-wider text-fg2 font-semibold" style={{ height: HEADER }}>
        <div style={{ width: refW }} className="pl-[15px] truncate">
          BRANCH / TAG
        </div>
        <div style={{ width: graphW }} className="flex items-center justify-center text-white/70" title="GRAPH">
          <Network size={13} />
        </div>
        <div className="flex-1 truncate" style={{ paddingLeft: m.summary_margin }}>
          {msgW >= 130 ? "COMMIT MESSAGE" : ""}
        </div>
        <button className="px-1.5 text-white/70 hover:text-white" title="Show commit description in graph (Stage 2)">
          <MessageSquare size={13} />
        </button>
        <button className="px-1.5 text-white/70 hover:text-white" title="Column settings (Stage 2)">
          <Settings2 size={13} />
        </button>
      </div>
      {ui.searchOpen && (
        <div className="flex items-center gap-2 h-[34px] px-2 bg-panel border-b border-black/30 shrink-0">
          <input
            autoFocus
            value={ui.search}
            onChange={(e) => setUi(tabId, { search: e.target.value })}
            onKeyDown={(e) => {
              if (e.key === "Enter") jump(e.shiftKey ? -1 : 1);
              if (e.key === "Escape") setUi(tabId, { searchOpen: false, search: "" });
            }}
            placeholder="Search commits by message, author or sha"
            className="flex-1 h-[24px] px-2 rounded-[3px] bg-field border border-white/10 outline-none focus:border-accent text-[12.5px] min-w-0"
          />
          <span className="text-[11.5px] text-fg2 whitespace-nowrap">{q ? `${matches.length} found` : ""}</span>
          <button className="text-fg2 hover:text-white" onClick={() => jump(-1)} title="Previous (Shift+Enter)">
            <ChevronUp size={15} />
          </button>
          <button className="text-fg2 hover:text-white" onClick={() => jump(1)} title="Next (Enter)">
            <ChevronDown size={15} />
          </button>
          <button className="text-fg2 hover:text-white" onClick={() => setUi(tabId, { searchOpen: false, search: "" })} title="Close (Esc)">
            <X size={15} />
          </button>
        </div>
      )}
      {repo.state_note && <StateBanner repo={repo} />}
      <div ref={scrollRef} className="flex-1 overflow-y-auto overflow-x-hidden relative" onScroll={(e) => setScrollTop((e.target as HTMLDivElement).scrollTop)}>
        <div className="relative" style={{ height: total * ROW }}>
          {rows}
          <canvas ref={canvasRef} className="absolute left-0 pointer-events-none" style={{ top: scrollTop }} />
        </div>
        {total === 0 && <div className="absolute inset-0 flex items-center justify-center text-fg3">No commits yet — make the first one on the right.</div>}
      </div>
    </div>
  );
}

function CommitRow({
  repo,
  commit: c,
  top,
  refW,
  graphW,
  color,
  selected,
  match,
  dim,
  refs,
  rowH,
  onSelect,
}: {
  repo: RepoView;
  commit: Commit;
  top: number;
  refW: number;
  graphW: number;
  color: string;
  selected: boolean;
  match: boolean;
  dim: boolean;
  refs: GitRef[] | undefined;
  rowH: number;
  onSelect: () => void;
}) {
  const m = useStore((st) => st.metrics);
  const first = refs?.[0];
  const localNames = new Set(repo.refs.filter((r) => r.kind === "Local" && r.sha === c.sha).map((r) => r.name));
  const remoteSame = first && first.kind === "Local" && repo.refs.some((r) => r.kind === "Remote" && r.sha === c.sha && r.name.endsWith("/" + first.name));
  const isHead = first?.kind === "Local" && first.name === repo.head_branch;
  const more = (refs?.length ?? 0) - 1 - (remoteSame ? 1 : 0);
  const roomy = refW >= 120; // icons only when the branch/tag zone has room, the name comes first
  return (
    <CtxMenu items={() => commitMenu(repo, c)}>
      <div
        className="absolute left-0 right-0 flex items-center"
        style={{ top, height: rowH, background: selected ? mix(color, 0.28) : undefined, opacity: dim ? m.min_alpha / 100 : undefined }}
        onClick={onSelect}
        onMouseEnter={(e) => !selected && (e.currentTarget.style.background = "rgba(77,136,255,0.10)")}
        onMouseLeave={(e) => !selected && (e.currentTarget.style.background = "")}
      >
        <div style={{ width: refW, paddingLeft: m.ref_margin_left, paddingRight: m.ref_margin_right }} className="shrink-0 h-full flex items-center min-w-0">
          {first && (
            <CtxMenu items={() => branchMenu(repo, first)}>
              <div
                onDoubleClick={(e) => {
                  e.stopPropagation();
                  if (first.kind === "Local" && !isHead) act(repo.path, { type: "checkout", target: first.name });
                  if (first.kind === "Remote") act(repo.path, { type: "checkoutRemote", remoteBranch: first.name });
                }}
                title={refs!.slice(0, m.ref_max).map((r) => r.name).join("\n")}
                className="flex items-center gap-[4px] px-[5px] rounded-[3px] min-w-0 max-w-full text-[12.5px] text-white"
                style={{ background: mix(color, 0.45), height: m.row_inner }}
              >
                {isHead && <Check size={13} strokeWidth={3} className="shrink-0" />}
                {first.kind === "Tag" && <Tag size={12} className="shrink-0" />}
                <span className="truncate font-semibold min-w-[14px]">{first.name}</span>
                {roomy && first.kind === "Local" && <Laptop size={13} className="shrink-0 opacity-90" />}
                {(first.kind === "Remote" || remoteSame) && <Cloud size={13} className={cn("shrink-0 opacity-90", !roomy && "w-[11px]")} />}
                {roomy && more > 0 && <span className="shrink-0 text-[11px] opacity-80">+{more}</span>}
              </div>
            </CtxMenu>
          )}
          {!first && localNames.size === 0 && null}
        </div>
        <div style={{ width: graphW }} className="shrink-0" />
        <div
          className="flex-1 min-w-0 flex items-center pr-2 text-[13px] text-white/90 bg-panel/70"
          style={{ height: m.row_inner, paddingLeft: m.summary_margin, boxShadow: `inset 3px 0 0 ${color}`, background: selected ? "transparent" : undefined }}
        >
          {match && <span className="w-1.5 h-1.5 rounded-full bg-[#f2ca33] mr-1.5 shrink-0" />}
          <span className="truncate">{c.summary}</span>
        </div>
      </div>
    </CtxMenu>
  );
}

function StateBanner({ repo }: { repo: RepoView }) {
  const merging = repo.state_note === "merging";
  const rebasing = repo.state_note === "rebasing";
  return (
    <div className="flex items-center gap-2 px-3 h-[34px] bg-warn/25 border-b border-warn/50 text-[12.5px] shrink-0">
      <span className="flex-1 truncate">
        {merging ? "Merge in progress" : rebasing ? "Rebase in progress" : `${repo.state_note} in progress`}
        {repo.status.conflicted ? " — merge conflicts detected" : ""}
      </span>
      {rebasing && (
        <Button size="sm" variant="success" onClick={() => act(repo.path, { type: "continueRebase" })}>
          Continue Rebase
        </Button>
      )}
      <Button size="sm" variant="danger" onClick={() => act(repo.path, merging ? { type: "abortMerge" } : { type: "abortRebase" })}>
        {merging ? "Abort Merge" : "Abort Rebase"}
      </Button>
    </div>
  );
}
