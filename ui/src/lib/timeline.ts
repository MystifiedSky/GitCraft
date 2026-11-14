// Graph "timeline" labels ("20 hours ago", "yesterday", ...): which row gets which label, and when they show.

export interface TimelineMark {
  /** Milliseconds since the epoch: rows older than this come after the line. */
  date: number;
  unit: "hour" | "day" | "week" | "month" | "year";
  value: number;
  text: string;
}

/** How many marks of each unit exist. */
export const TIMELINE_COUNTS = { hour: 23, day: 6, week: 4, month: 11, year: 6 } as const;
/** The labels are rebuilt every 900000 ms (15 minutes). */
export const TIMELINE_REFRESH_MS = 9e5;

const plural = (n: number, one: string, many: string) => (n === 1 ? one : many.replace("{0}", String(n)));

/** The marks in order: hours, days, weeks, months, years - each from the newest to the oldest. */
export function buildTimeline(now: Date = new Date()): TimelineMark[] {
  const out: TimelineMark[] = [];
  const t = now.getTime();
  for (let n = 1; n <= TIMELINE_COUNTS.hour; n++) out.push({ date: t - 36e5 * n, unit: "hour", value: n, text: plural(n, "1 hour ago", "{0} hours ago") });
  for (let n = 1; n <= TIMELINE_COUNTS.day; n++) out.push({ date: t - 864e5 * n, unit: "day", value: n, text: plural(n, "yesterday", "{0} days ago") });
  for (let n = 1; n <= TIMELINE_COUNTS.week; n++) out.push({ date: t - 6048e5 * n, unit: "week", value: n, text: plural(n, "a week ago", "{0} weeks ago") });
  // Months and years step back by calendar, at local midnight.
  for (let n = 1; n <= TIMELINE_COUNTS.month; n++) {
    const month = new Date(now.getFullYear(), now.getMonth() - n, 1);
    month.setDate(Math.min(now.getDate(), new Date(month.getFullYear(), month.getMonth() + 1, 0).getDate()));
    month.setHours(0, 0, 0, 0);
    out.push({ date: month.getTime(), unit: "month", value: n, text: plural(n, "a month ago", "{0} months ago") });
  }
  for (let n = 1; n <= TIMELINE_COUNTS.year; n++) {
    const year = new Date(now.getFullYear() - n, now.getMonth(), 1);
    year.setDate(Math.min(now.getDate(), new Date(year.getFullYear(), year.getMonth() + 1, 0).getDate()));
    year.setHours(0, 0, 0, 0);
    const text = n === 1 ? "a year ago" : n === TIMELINE_COUNTS.year ? `${n}+ years ago` : `${n} years ago`;
    out.push({ date: year.getTime(), unit: "year", value: n, text });
  }
  return out;
}

/**
 * updateTimelines: for each mark in order, the FIRST row (newest first) whose date is older than the mark gets that mark;
 * a later mark for the same row replaces an earlier one. `rowTimes` are the commit times in list order, in milliseconds.
 */
export function assignTimeline(rowTimes: number[], now: Date = new Date()): Map<number, string> {
  const out = new Map<number, string>();
  if (!rowTimes.length) return out;
  for (const mark of buildTimeline(now)) {
    const i = rowTimes.findIndex((t) => t < mark.date);
    if (i >= 0) out.set(i, mark.text);
  }
  return out;
}

/**
 * hasTimeline: labels show while the Date / Time column is hidden, or always when the commit list is filtered; never on the first row.
 */
export function showsTimeline(rowIndex: number, dateColumnShown: boolean, filtered: boolean): boolean {
  // (The caller also requires the list to be taller than its view, as the labels sit in the scrollbar column.)
  return rowIndex > 0 && (filtered || !dateColumnShown);
}
