// Coloured ticks beside the graph's scrollbar showing where labelled commits sit in the whole history.
// Geometry, lanes and colours follow the graph component's marker table and drawing routine.
export type MarkerType = "stashes" | "tags" | "remoteBranches" | "localBranches" | "highlights" | "upstream" | "pullRequests" | "head" | "selection";
export type MarkerShape = "block" | "fullLine" | "thinLine";

/** Draw order, lane(s) and shape of each marker type (three lanes across the strip). */
export const MARKER_TABLE: Record<MarkerType, { lanes: number[]; shape: MarkerShape; color: string }> = {
  stashes: { lanes: [0], shape: "block", color: "#fb94ff" },
  tags: { lanes: [2], shape: "block", color: "#affcff" },
  remoteBranches: { lanes: [2], shape: "block", color: "#ffd68b" },
  localBranches: { lanes: [0], shape: "block", color: "orange" },
  highlights: { lanes: [1], shape: "block", color: "#e6e62b" },
  upstream: { lanes: [1, 2], shape: "block", color: "#9bff9b" },
  pullRequests: { lanes: [2], shape: "block", color: "#ff8f18" },
  head: { lanes: [0, 1], shape: "block", color: "#16c216" },
  selection: { lanes: [0, 1, 2], shape: "fullLine", color: "#0821ff" },
};
export const MARKER_LANES = 3;
/** One row of the graph in the original's units; heights scale from these. */
const ROW_UNITS = 28;
/** Shape sizes: base height (in row units) and the clamp, in pixels. */
const SHAPES: Record<MarkerShape, { base: number; min: number; max: number }> = {
  block: { base: 1 * ROW_UNITS, min: 5, max: 20 },
  fullLine: { base: 0.5 * ROW_UNITS, min: 2, max: 4 },
  thinLine: { base: 0.25 * ROW_UNITS, min: 1, max: 2 },
};
/** Markers shown by default: an estimate, the default list was not found. */
export const DEFAULT_MARKERS: MarkerType[] = ["head", "localBranches", "remoteBranches", "tags", "upstream", "selection"];

/** Height and offset of one tick: the height scales with the strip, clamped; a short tick is centred in its row's slice. */
export function tickSize(type: MarkerType, totalRows: number, viewportHeight: number): { height: number; offset: number } {
  const { base, min, max } = SHAPES[MARKER_TABLE[type].shape];
  const rowSlice = viewportHeight / totalRows;
  const height = Math.min(max, Math.max(min, (viewportHeight / (totalRows * ROW_UNITS)) * base));
  return { height, offset: height >= rowSlice ? 0 : (rowSlice - height) / 2 };
}

export interface MarkerRect {
  type: MarkerType;
  top: number;
  height: number;
  left: number;
  width: number;
  color: string;
}

/**
 * The rectangles to draw. A type is drawn when enabled; remote branches are also drawn (at the rows where a remote branch is
 * a local branch's upstream) when only local branches are enabled.
 */
export function markerRects(
  enabled: MarkerType[],
  rows: Partial<Record<MarkerType, number[]>>,
  upstreamRows: number[],
  totalRows: number,
  viewportHeight: number,
  stripWidth: number,
): MarkerRect[] {
  if (totalRows <= 0 || viewportHeight <= 0) return [];
  const laneWidth = stripWidth / MARKER_LANES;
  const out: MarkerRect[] = [];
  for (const type of Object.keys(MARKER_TABLE) as MarkerType[]) {
    if (!(enabled.includes(type) || (type === "remoteBranches" && enabled.includes("localBranches")))) continue;
    const list = type === "remoteBranches" && !enabled.includes("remoteBranches") ? upstreamRows : rows[type] ?? [];
    const { lanes, color } = MARKER_TABLE[type];
    const { height, offset } = tickSize(type, totalRows, viewportHeight);
    for (const row of list) {
      out.push({ type, top: (row * viewportHeight) / totalRows + offset - 0.5 * height, height, left: lanes[0] * laneWidth, width: laneWidth * lanes.length, color });
    }
  }
  return out;
}
