import { Eye, EyeOff, MessageSquare, Network, Settings2, X, ChevronUp, ChevronDown, Loader2 } from "lucide-react";
import { assignTimeline, showsTimeline, TIMELINE_REFRESH_MS } from "@/lib/timeline";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { Commit, GitRef, RepoView, Stats } from "@/lib/api";
import { api } from "@/lib/api";
import { DEFAULT_COLUMNS, useStore, useTabUi, type GraphColumns } from "@/lib/store";
import { act, clearSolo, showAllRefs, toggleHidden, WIP_COMMIT_EVENT } from "@/lib/repo";
import { commitMenu } from "@/lib/menus";
import { CommitRefs } from "./CommitRefs";
import { CtxMenu, DropMenu, type MenuEntry } from "./ui/menu";
import { Button } from "./ui/button";
import { Avatar } from "./Icons";
import { cn, fullDate, initials, laneColor, mix } from "@/lib/utils";

interface Props {
  repo: RepoView;
  tabId: string;
}

type Extra = "author" | "date" | "sha" | "changes";

export function GraphView({ repo, tabId }: Props) {
  // Sizes from the graph constants (src/metrics.rs).
  const m = useStore((s) => s.metrics);
  const cols = useStore((s) => s.settings.columns) ?? DEFAULT_COLUMNS;
  const lazyLoad = useStore((s) => s.settings.lazyLoad);
  const hidden = useStore((s) => s.settings.hidden[repo.path]) ?? [];
  const solo = useStore((s) => s.settings.solo[repo.path]) ?? [];
  const setSettings = useStore((s) => s.setSettings);
  const commitIndex = useMemo(() => new Map(repo.commits.map((c, i) => [c.sha, i])), [repo.commits]);
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
  const searchInput = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (ui.searchOpen) searchInput.current?.focus();
  }, [ui.searchOpen]);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewH, setViewH] = useState(400);
  const [paneW, setPaneW] = useState(300);
  const [hoverRef, setHoverRef] = useState<number | null>(null);
  const [hoverRow, setHoverRow] = useState<number | null>(null);
  const ghostOn = useStore((s) => s.settings.ghostRefs);
  /** Nearest label above a commit in the same lane (the branch it belongs to). */
  const ghostFor = (i: number): GitRef | undefined => {
    const lane = repo.graph.rows[i]?.col;
    for (let j = i - 1; j >= 0; j--) {
      if (repo.graph.rows[j]?.col !== lane) continue;
      const r = refsBySha.get(repo.commits[j].sha);
      if (r?.length) return r[0];
    }
    return undefined;
  };

  const wipFiles = useMemo(() => {
    const all = new Map<string, string>();
    for (const f of [...repo.status.staged, ...repo.status.unstaged]) if (!all.has(f.path)) all.set(f.path, f.status);
    return [...all.values()];
  }, [repo]);
  const wip = wipFiles.length > 0;
  const off = wip ? 1 : 0;
  const total = repo.commits.length + off + (repo.has_more ? 1 : 0);

  const refsBySha = useMemo(() => {
    const map = new Map<string, GitRef[]>();
    const order = (r: GitRef) => (r.kind === "Local" && r.name === repo.head_branch ? 0 : r.kind === "Local" ? 1 : r.kind === "Remote" ? 2 : 3);
    for (const r of repo.refs) {
      if (hidden.includes(r.full)) continue; // hidden branches don't get a label
      map.set(r.sha, [...(map.get(r.sha) ?? []), r]);
    }
    for (const [k, v] of map) map.set(k, v.sort((a, b) => order(a) - order(b)));
    return map;
  }, [repo, hidden]);

  const headIdx = commitIndex.get(repo.head_sha) ?? -1;
  const headLane = headIdx >= 0 ? repo.graph.rows[headIdx]?.col ?? 0 : 0;

  // Search (Ctrl+F)
  const q = ui.search.trim().toLowerCase();
  // Date lines ("20 hours ago", "yesterday") between rows, rebuilt every 15 minutes.
  const [timelineNow, setTimelineNow] = useState(() => new Date());
  useEffect(() => {
    const id = setInterval(() => setTimelineNow(new Date()), TIMELINE_REFRESH_MS);
    return () => clearInterval(id);
  }, []);
  const timelines = useMemo(() => assignTimeline(repo.commits.map((c) => c.time * 1000), timelineNow), [repo.commits, timelineNow]);
  const matches = useMemo(() => {
    if (!q) return [] as number[];
    const out: number[] = [];
    repo.commits.forEach((c, i) => {
      if (c.summary.toLowerCase().includes(q) || c.author.toLowerCase().includes(q) || c.sha.startsWith(q) || c.body.toLowerCase().includes(q)) out.push(i);
    });
    return out;
  }, [repo, q]);
  const matchSet = useMemo(() => new Set(matches), [matches]);

  // Rows that belong to a hovered branch label: its first-parent chain while it stays in the label's lane.
  const highlighted = useMemo(() => {
    const set = new Set<number>();
    // UI Customization > Highlight associated rows when hovering over a branch.
    if (hoverRef === null || useStore.getState().settings.highlightRowsOnHover === false) return set;
    let i = hoverRef;
    const lane = repo.graph.rows[i]?.col;
    while (i >= 0 && i < repo.commits.length && repo.graph.rows[i]?.col === lane) {
      set.add(i);
      const p = repo.commits[i].parents[0];
      if (!p) break;
      const next = commitIndex.get(p);
      if (next === undefined || next <= i) break;
      i = next;
    }
    return set;
  }, [hoverRef, repo]);

  // Zone widths: default widths when the pane has room, otherwise the branch/tag zone gives way
  // first (down to REF_ZONE_MIN_WIDTH) and the message zone keeps COMMIT_MESSAGE_ZONE_MIN_WIDTH.
  const lanesW = GUTTER + Math.max(1, repo.graph.width) * LANE + PAD * 2;
  const extraWidth: Record<Extra, number> = { author: m.author_zone, date: m.date_zone, sha: m.sha_zone, changes: m.changes_zone };
  const wanted = (["author", "date", "sha", "changes"] as Extra[]).filter((k) => cols[k]);
  const baseRef = cols.refs ? m.ref_zone : 0;
  const baseGraph = cols.graph ? m.graph_zone : 0;
  // Extra columns that don't fit next to a minimum-width message are left out (rightmost first).
  const extras: Extra[] = [];
  for (const k of wanted) {
    const used = extras.reduce((a, e) => a + extraWidth[e], 0) + extraWidth[k];
    if (paneW - used - (cols.refs ? m.ref_zone_min : 0) - (cols.graph ? lanesW : 0) >= m.message_min) extras.push(k);
  }
  const extrasW = extras.reduce((a, e) => a + extraWidth[e], 0);
  const roomy = paneW - extrasW >= baseRef + baseGraph + m.message_zone;
  const graphW = !cols.graph ? 0 : roomy ? Math.max(m.graph_zone, lanesW) : Math.max(0, Math.min(lanesW, paneW - extrasW - (cols.refs ? m.ref_zone_min : 0) - m.message_min));
  const refW = !cols.refs
    ? 0
    : roomy
      ? m.ref_zone
      : Math.round(Math.max(m.ref_zone_min, Math.min(m.ref_zone, (paneW - extrasW) * 0.42, paneW - extrasW - graphW - m.message_min)));
  const msgW = paneW - refW - graphW - extrasW;
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

  // Lazy loading: read more history once the view is GRAPH_ROW_LAZY_LOAD_COMMITS_OFFSET rows from the end.
  const loadingMore = useRef(false);
  useEffect(() => {
    loadingMore.current = false;
  }, [repo.commits.length]);
  useEffect(() => {
    if (!lazyLoad || !repo.has_more || loadingMore.current) return;
    if (last >= repo.commits.length - m.lazy_offset) {
      loadingMore.current = true;
      setUi(tabId, { limit: repo.commits.length + m.initial_commits });
    }
  }, [last, repo.has_more, repo.commits.length, lazyLoad]);

  // Keep the selected row in view.
  const selIdx = ui.selection.type === "wip" ? (wip ? 0 : -1) : (commitIndex.get((ui.selection as { sha: string }).sha) ?? -1) + off;
  useEffect(() => {
    const el = scrollRef.current;
    if (!el || selIdx < 0) return;
    const top = selIdx * ROW;
    if (top < el.scrollTop) el.scrollTop = top;
    else if (top + ROW > el.scrollTop + el.clientHeight) el.scrollTop = top + ROW - el.clientHeight;
  }, [selIdx]);

  // CHANGES column: counts for the visible commits, fetched in small batches and kept.
  const [stats, setStats] = useState<Map<string, Stats>>(new Map());
  useEffect(() => {
    if (!extras.includes("changes")) return;
    const need: string[] = [];
    for (let d = Math.max(first, off); d <= last; d++) {
      const c = repo.commits[d - off];
      if (c && !stats.has(c.sha)) need.push(c.sha);
    }
    if (!need.length) return;
    const t = setTimeout(() => {
      api.commitStats(repo.path, need).then((res) =>
        setStats((old) => {
          const n = new Map(old);
          for (const s of res) n.set(s.sha, s);
          return n;
        }),
      ).catch(() => undefined);
    }, 80);
    return () => clearTimeout(t);
  }, [first, last, extras.join(), repo.path]);

  // Draw the visible part of the graph on the canvas.
  useLayoutEffect(() => {
    const cv = canvasRef.current;
    if (!cv) return;
    const dpr = window.devicePixelRatio || 1;
    const w = refW + graphW;
    cv.width = Math.ceil(Math.max(1, w) * dpr);
    cv.height = Math.ceil(viewH * dpr);
    cv.style.width = `${w}px`;
    cv.style.height = `${viewH}px`;
    const g = cv.getContext("2d")!;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, w, viewH);
    if (!cols.graph) return;
    g.save();
    g.beginPath();
    g.rect(refW, 0, graphW, viewH); // the graph never draws over the branch labels
    g.clip();
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
        g.arcTo(x2, y1, x2, y2, R);
        g.lineTo(x2, y2);
      } else {
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
    g.restore();
    // Nodes on top of the lines; connector from the branch label to its node.
    for (let d = Math.max(first, off); d <= last; d++) {
      const i = d - off;
      const row = repo.graph.rows[i];
      const c = repo.commits[i];
      if (!row || !c) continue;
      const yc = d * ROW - scrollTop + ROW / 2;
      const x = laneX(row.col);
      if (x > refW + graphW) continue;
      const col = laneColor(row.color);
      g.globalAlpha = q && !matchSet.has(i) ? m.min_alpha / 100 : 1;
      if (refsBySha.has(c.sha) && cols.refs) {
        g.strokeStyle = col;
        g.lineWidth = 2;
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
      g.globalAlpha = 1;
    }
  }, [repo, first, last, scrollTop, viewH, refW, graphW, off, headLane, headIdx, refsBySha, m, cols.graph, cols.refs, q, matchSet]);

  const select = (sel: { type: "wip" } | { type: "commit"; sha: string }) => setUi(tabId, { selection: sel, diff: null, multi: [], compareWorkdir: null, history: null });

  // Ctrl+click adds or removes a commit; Shift+click picks a range (two picked = compare them).
  const clickCommit = (c: Commit, i: number, e: React.MouseEvent) => {
    const cur = ui.selection.type === "commit" ? ui.selection.sha : null;
    if (e.ctrlKey || e.metaKey) {
      const base = ui.multi.length ? ui.multi : cur ? [cur] : [];
      const multi = base.includes(c.sha) ? base.filter((x) => x !== c.sha) : [...base, c.sha];
      return setUi(tabId, { multi: multi.length > 1 ? multi : [], selection: { type: "commit", sha: multi[multi.length - 1] ?? c.sha }, diff: null, compareWorkdir: null, history: null });
    }
    if (e.shiftKey && cur) {
      const a = repo.commits.findIndex((x) => x.sha === cur);
      if (a >= 0) {
        const [lo, hi] = [Math.min(a, i), Math.max(a, i)];
        const multi = repo.commits.slice(lo, hi + 1).map((x) => x.sha);
        return setUi(tabId, { multi: multi.length > 1 ? multi : [], diff: null, compareWorkdir: null, history: null });
      }
    }
    select({ type: "commit", sha: c.sha });
  };

  const jump = (dir: 1 | -1) => {
    if (!matches.length) return;
    const cur = ui.selection.type === "commit" ? repo.commits.findIndex((c) => c.sha === (ui.selection as { sha: string }).sha) : -1;
    let next = dir === 1 ? matches.find((x) => x > cur) : [...matches].reverse().find((x) => x < cur);
    if (next === undefined) next = dir === 1 ? matches[0] : matches[matches.length - 1];
    select({ type: "commit", sha: repo.commits[next].sha });
  };

  const multiSet = new Set(ui.multi);
  const rows = [];
  for (let d = first; d <= last; d++) {
    if (d < off) {
      const sel = ui.selection.type === "wip";
      const count = (k: string[]) => wipFiles.filter((s) => k.includes(s)).length;
      const [mod, add, del] = [count(["M", "R", "U"]), count(["A", "?"]), count(["D"])];
      rows.push(
        <div
          key="wip"
          className={cn("absolute left-0 right-0 flex items-center", sel ? "bg-selrow" : "hover:bg-white/[0.03]")}
          style={{ top: d * ROW, height: ROW }}
        >
          <button type="button" aria-label="Select working changes" aria-pressed={sel} className="absolute inset-0 focus-visible:outline focus-visible:-outline-offset-2 focus-visible:outline-accent" onClick={() => select({ type: "wip" })} />
          <div style={{ width: refW + graphW }} className="shrink-0" />
          {/* A "// WIP" box to type the commit summary in, sized to its text,
              with the change counts beside it (not inside it). */}
          <div className="flex-1 min-w-0 flex items-center gap-[10px] mr-[4px] text-[13px]" style={{ height: m.row_inner, paddingLeft: m.summary_margin }}>
            <WipInput path={repo.path} onFocus={() => select({ type: "wip" })} />
            <span className="flex items-center gap-[10px] text-[13px] font-semibold whitespace-nowrap overflow-hidden shrink-0">
              {mod > 0 && <span className="text-[#de9b43]">✎ {mod}</span>}
              {add > 0 && <span className="text-good">+ {add}</span>}
              {del > 0 && <span className="text-[#ef7a76]">− {del}</span>}
            </span>
          </div>
          <div style={{ width: extrasW }} className="shrink-0" />
        </div>,
      );
      continue;
    }
    const i = d - off;
    const c = repo.commits[i];
    const gr = repo.graph.rows[i];
    if (!c || !gr) {
      if (repo.has_more && i === repo.commits.length)
        rows.push(
          <div key="more" className="absolute left-0 right-0 flex items-center justify-center gap-2 text-[12px] text-fg3" style={{ top: d * ROW, height: ROW }}>
            <Loader2 size={13} className="spin" /> Loading more commits…
          </div>,
        );
      continue;
    }
    const sel = (ui.selection.type === "commit" && ui.selection.sha === c.sha && ui.multi.length === 0) || multiSet.has(c.sha);
    rows.push(
      <CommitRow
        key={c.sha}
        repo={repo}
        commit={c}
        index={i}
        top={d * ROW}
        refW={refW}
        graphW={graphW}
        extras={extras}
        extraWidth={extraWidth}
        color={laneColor(gr.color)}
        selected={sel}
        highlighted={highlighted.has(i)}
        match={matchSet.has(i)}
        dim={q.length > 0 && !matchSet.has(i)}
        refs={refsBySha.get(c.sha)}
        ghost={ghostOn && !refsBySha.get(c.sha) && (hoverRow === i || sel) ? ghostFor(i) : undefined}
        hovered={hoverRow === i}
        onHoverRow={setHoverRow}
        rowH={ROW}
        timeline={showsTimeline(d, cols.date, q.length > 0) ? timelines.get(i) : undefined}
        showDesc={cols.description}
        stats={stats.get(c.sha)}
        onClick={clickCommit}
        onHoverRef={setHoverRef}
      />,
    );
  }

  const columnItems: MenuEntry[] = [
    { heading: "Columns" },
    ...(
      [
        ["refs", "Branch / Tag"],
        ["graph", "Graph"],
        ["description", "Commit Description"],
        ["author", "Author"],
        ["date", "Commit Date / Time"],
        ["sha", "Sha"],
        ["changes", "Changes"],
      ] as [keyof GraphColumns, string][]
    ).map(([k, label]) => ({ label: `${cols[k] ? "✓ " : "   "}${label}`, onSelect: () => setSettings({ columns: { ...cols, [k]: !cols[k] } }) })),
    { separator: true },
    { label: "Reset columns to default layout", onSelect: () => setSettings({ columns: DEFAULT_COLUMNS }) },
    { label: "Reset columns to compact layout", onSelect: () => setSettings({ columns: { ...DEFAULT_COLUMNS, description: false } }) },
    { separator: true },
    { label: `${lazyLoad ? "✓ " : "   "}Lazy Load Commits in Graph`, onSelect: () => setSettings({ lazyLoad: !lazyLoad }) },
  ];
  const hiddenRefs = repo.refs.filter((r) => hidden.includes(r.full));
  const hiddenItems: MenuEntry[] = [
    { heading: "Hidden Branches / Tags" },
    ...hiddenRefs.map((r) => ({ label: `Show ${r.name}`, icon: <Eye size={13} />, onSelect: () => toggleHidden(repo.path, r.full) })),
    { separator: true },
    { label: "Show all", onSelect: () => showAllRefs(repo.path) },
  ];

  return (
    <div className="h-full flex flex-col bg-app min-w-0">
      <div className="flex items-center shrink-0 bg-panel border-b border-black/30 text-[10.5px] tracking-wider text-fg2 font-semibold" style={{ height: HEADER }}>
        {cols.refs && (
          <div style={{ width: refW }} className="pl-[15px] truncate shrink-0">
            BRANCH / TAG
          </div>
        )}
        {cols.graph && (
          <div style={{ width: graphW }} className="flex items-center justify-center text-white/70 shrink-0" title="GRAPH">
            <Network size={13} />
          </div>
        )}
        <div className="flex-1 truncate min-w-0" style={{ paddingLeft: m.summary_margin }}>
          {msgW >= 130 ? "COMMIT MESSAGE" : ""}
        </div>
        {hiddenRefs.length > 0 && (
          <DropMenu
            align="end"
            items={hiddenItems}
            trigger={
              <button className="px-1.5 flex items-center gap-1 text-warn hover:text-white outline-none" title="Hidden Branches / Tags">
                <EyeOff size={13} /> {hiddenRefs.length}
              </button>
            }
          />
        )}
        <button
          className={cn("px-1.5", cols.description ? "text-white" : "text-white/50 hover:text-white")}
          title="Show commit description in graph"
          onClick={() => setSettings({ columns: { ...cols, description: !cols.description } })}
        >
          <MessageSquare size={13} />
        </button>
        <DropMenu
          align="end"
          items={columnItems}
          trigger={
            <button className="px-1.5 text-white/70 hover:text-white outline-none" title="Column settings">
              <Settings2 size={13} />
            </button>
          }
        />
        {extras.map((k) => (
          <div key={k} style={{ width: extraWidth[k] }} className="shrink-0 pl-2 truncate border-l border-white/5">
            {{ author: "AUTHOR", date: "COMMIT DATE / TIME", sha: "SHA", changes: "CHANGES" }[k]}
          </div>
        ))}
      </div>
      {ui.searchOpen && (
        <div className="flex items-center gap-2 h-[34px] px-2 bg-panel border-b border-black/30 shrink-0">
          <input
            ref={searchInput}
            aria-label="Search commits"
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
      {solo.length > 0 && (
        <div className="flex items-center gap-2 px-3 h-[28px] bg-warn/20 border-b border-warn/40 text-[12px] shrink-0">
          <span className="flex-1 truncate">
            Soloing {solo.length} branch{solo.length === 1 ? "" : "es"}: {solo.map((f) => f.replace(/^refs\/(heads|remotes|tags)\//, "")).join(", ")}
          </span>
          <Button size="sm" className="h-[20px]" onClick={() => clearSolo(repo.path)}>
            Stop soloing
          </Button>
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
  index,
  top,
  refW,
  graphW,
  extras,
  extraWidth,
  color,
  selected,
  highlighted,
  match,
  dim,
  refs,
  ghost,
  onHoverRow,
  hovered,
  rowH,
  timeline,
  showDesc,
  stats,
  onClick,
  onHoverRef,
}: {
  repo: RepoView;
  commit: Commit;
  index: number;
  top: number;
  refW: number;
  graphW: number;
  extras: Extra[];
  extraWidth: Record<Extra, number>;
  color: string;
  selected: boolean;
  highlighted: boolean;
  match: boolean;
  dim: boolean;
  refs: GitRef[] | undefined;
  ghost: GitRef | undefined;
  hovered: boolean;
  onHoverRow: (i: number | null) => void;
  rowH: number;
  /** The timeline entry for this row ("yesterday"): a line along the row's top edge with a label at its right end. */
  timeline?: string;
  showDesc: boolean;
  stats: Stats | undefined;
  onClick: (c: Commit, i: number, e: React.MouseEvent) => void;
  onHoverRef: (i: number | null) => void;
}) {
  const m = useStore((st) => st.metrics);
  const first = refs?.[0];
  const desc = showDesc && c.body ? c.body.replace(/\r?\n+/g, " | ") : "";
  const bg = selected ? mix(color, 0.28) : highlighted ? mix(color, 0.14) : undefined;
  return (
    <CtxMenu items={() => commitMenu(repo, c)}>
      <div
        className="absolute left-0 right-0 flex items-center"
        style={{ top, height: rowH, background: bg, opacity: dim ? m.min_alpha / 100 : undefined }}
        onClick={(e) => onClick(c, index, e)}
        onMouseEnter={(e) => {
          onHoverRow(index);
          if (!bg) e.currentTarget.style.background = "rgba(77,136,255,0.10)";
        }}
        onMouseLeave={(e) => {
          onHoverRow(null);
          if (!bg) e.currentTarget.style.background = "";
        }}
      >
        {timeline && (
          <>
            <div className="absolute left-0 right-0 top-0 h-0 border-t border-line pointer-events-none" aria-hidden />
            <span
              className="absolute right-0 -top-[8px] z-[3] px-[6px] pb-px rounded-[2px] bg-panel text-fg2 text-[10px] leading-[13px] whitespace-nowrap pointer-events-none"
              data-timeline={timeline}
            >
              {timeline}
            </span>
          </>
        )}
        <button
          type="button"
          aria-label={c.summary}
          aria-pressed={selected}
          tabIndex={selected || index === 0 ? 0 : -1}
          className="absolute inset-0 focus-visible:outline focus-visible:-outline-offset-2 focus-visible:outline-accent"
        />
        {refW > 0 && (
          <div style={{ width: refW, paddingLeft: m.ref_margin_left, paddingRight: m.ref_margin_right }} className="shrink-0 h-full flex items-center min-w-0">
            {!first && ghost && (
              <div
                className="flex items-center gap-[4px] px-[5px] rounded-[3px] min-w-0 max-w-full text-[12.5px] text-white border border-dashed opacity-60"
                style={{ borderColor: color, height: m.row_inner }}
                title={`On ${ghost.name}`}
              >
                <span className="truncate">{ghost.name}</span>
              </div>
            )}
            {first && (
              <CommitRefs
                repo={repo}
                refs={refs!}
                color={color}
                hovered={hovered}
                selected={selected}
                roomy={refW >= 120}
                onSelect={(e) => onClick(c, index, e)}
                onHover={(active) => onHoverRef(active ? index : null)}
              />
            )}
          </div>
        )}
        {graphW > 0 && <div style={{ width: graphW }} className="shrink-0" />}
        <div
          className="flex-1 min-w-0 flex items-center pr-2 text-[13px] text-white/90 bg-panel/70"
          style={{ height: m.row_inner, paddingLeft: m.summary_margin, boxShadow: `inset 3px 0 0 ${color}`, background: bg ? "transparent" : undefined }}
        >
          {match && <span className="w-1.5 h-1.5 rounded-full bg-[#f2ca33] mr-1.5 shrink-0" />}
          <span className="truncate">
            {c.summary}
            {desc && <span className="text-white/50 ml-2">{desc}</span>}
          </span>
        </div>
        {extras.map((k) => (
          <div key={k} style={{ width: extraWidth[k] }} className="shrink-0 h-full flex items-center gap-1.5 pl-2 pr-1 text-[12px] text-fg min-w-0 border-l border-white/5">
            {k === "author" && (
              <>
                <Avatar name={c.author} email={c.email} size={18} />
                <span className="truncate">{c.author}</span>
              </>
            )}
            {k === "date" && <span className="truncate">{fullDate(c.time)}</span>}
            {k === "sha" && <span className="mono truncate text-fg2">{c.sha.slice(0, m.sha_len)}</span>}
            {k === "changes" && <ChangesBar stats={stats} />}
          </div>
        ))}
      </div>
    </CtxMenu>
  );
}

/** CHANGES column: files changed plus a green / red bar of added and removed lines. */
function ChangesBar({ stats }: { stats: Stats | undefined }) {
  const m = useStore((st) => st.metrics);
  if (!stats) return <span className="text-fg3">…</span>;
  const total = stats.added + stats.deleted || 1;
  return (
    <span className="flex items-center gap-1.5 w-full min-w-0" title={`${stats.files} files, +${stats.added} −${stats.deleted}`}>
      <span className="whitespace-nowrap text-[11px]" style={{ color: m.stats_files }}>
        {stats.files} file{stats.files === 1 ? "" : "s"}
      </span>
      <span className="flex-1 h-[6px] rounded-sm overflow-hidden flex bg-white/5 min-w-[20px]">
        <span style={{ width: `${(stats.added / total) * 100}%`, background: m.stats_added }} />
        <span style={{ width: `${(stats.deleted / total) * 100}%`, background: m.stats_deleted }} />
      </span>
    </span>
  );
}

function StateBanner({ repo }: { repo: RepoView }) {
  const merging = repo.state_note === "merging";
  const rebasing = repo.state_note === "rebasing";
  return (
    <div className="flex items-center gap-2 px-3 h-[34px] bg-warn/25 border-b border-warn/50 text-[12.5px] shrink-0">
      <span className="flex-1 truncate">
        {repo.conflict_labels
          ? merging
            ? `Merging ${repo.conflict_labels[1]} into ${repo.conflict_labels[0]}`
            : rebasing
              ? `Rebasing ${repo.conflict_labels[1]} onto ${repo.conflict_labels[0]}`
              : `${repo.state_note} in progress`
          : merging
            ? "Merge in progress"
            : rebasing
              ? "Rebase in progress"
              : `${repo.state_note} in progress`}
        {repo.status.conflicted ? " — merge conflicts detected" : ""}
      </span>
      {rebasing && (
        <Button size="sm" onClick={() => act(repo.path, { type: "skipRebase" })}>
          Skip Commit
        </Button>
      )}
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

/** Extra width beside the typed text: the input wrapper is the text width + 35 px. */
const WIP_INPUT_PAD = 35;
/** .work-dir-input{min-width:100px; max-width:calc(100% - 75px)}, input height 22px. */
const WIP_INPUT_MIN = 100;
const WIP_INPUT_HEIGHT = 22;

/**
 * The "// WIP" row's box: types the commit summary (the same text as the Commit panel's summary), grows with the text
 * Ctrl+Enter commits (the app's shortcut), and other keys stay in the box.
 */
function WipInput({ path, onFocus }: { path: string; onFocus: () => void }) {
  const summary = useStore((s) => s.drafts[path]?.summary ?? "");
  const setDraft = useStore((s) => s.setDraft);
  const measure = useRef<HTMLSpanElement>(null);
  const box = useRef<HTMLSpanElement>(null);
  const [width, setWidth] = useState(WIP_INPUT_MIN);
  // max-width: the row's message area minus metrics.wip_stats_space (src/metrics.rs WIP_BOX_STATS_SPACE), kept up to date as panels are resized.
  const [maxWidth, setMaxWidth] = useState<number | undefined>(undefined);
  useLayoutEffect(() => {
    setWidth(Math.max(WIP_INPUT_MIN, (measure.current?.offsetWidth ?? 0) + WIP_INPUT_PAD));
  }, [summary]);
  useLayoutEffect(() => {
    const area = box.current?.parentElement;
    if (!area) return;
    const fit = () => setMaxWidth(Math.max(WIP_INPUT_MIN, area.clientWidth - useStore.getState().metrics.wip_stats_space));
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(area);
    return () => ro.disconnect();
  }, []);
  return (
    <span ref={box} className="relative z-[1] inline-block shrink min-w-0" style={{ width, minWidth: WIP_INPUT_MIN, maxWidth }}>
      <input
        value={summary}
        aria-label="Commit summary for working changes"
        placeholder="// WIP"
        spellCheck={false}
        onFocus={onFocus}
        onMouseDown={(e) => e.stopPropagation()}
        onClick={(e) => e.stopPropagation()}
        onChange={(e) => setDraft(path, { summary: e.target.value })}
        onKeyDown={(e) => {
          if (e.key === "Escape") (e.currentTarget.blur(), e.stopPropagation());
          else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
            e.preventDefault();
            e.stopPropagation();
            window.dispatchEvent(new CustomEvent(WIP_COMMIT_EVENT, { detail: { path, all: e.shiftKey } }));
          } else e.stopPropagation();
        }}
        className="block w-[calc(100%-3px)] ml-[3px] px-[6px] rounded-[3px] bg-field border border-line2 text-[12.5px] tracking-[0.02em] text-fg placeholder:text-fg3 outline-none focus:border-accent focus:shadow-[0_0_0_1px_var(--color-accent)]"
        style={{ height: WIP_INPUT_HEIGHT }}
      />
      <span ref={measure} aria-hidden className="absolute invisible whitespace-pre text-[12.5px] tracking-[0.02em] px-[6px]">
        {summary}
      </span>
    </span>
  );
}
