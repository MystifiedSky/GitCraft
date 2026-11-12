import { useStore } from "./store";
import { formatDate } from "./integrations";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/** The 10 graph lane colours of the original's dark theme. */
export const GRAPH_COLORS = [
  "#15a0bf",
  "#0669f7",
  "#8e00c2",
  "#c517b6",
  "#d90171",
  "#cd0101",
  "#f25d2e",
  "#f2ca33",
  "#7bd938",
  "#2ece9d",
];

export function laneColor(i: number) {
  return GRAPH_COLORS[((i % 10) + 10) % 10];
}

/** The app background of the current theme (for tints). */
export function themeBg() {
  const t = typeof document !== "undefined" ? document.documentElement.dataset.theme ?? "dark" : "dark";
  return t.startsWith("light") ? "#fcfcfc" : t === "dark-hc" ? "#090a0b" : "#1c1e23";
}

/** Mix a #rrggbb colour with the app background. */
export function mix(hex: string, amount: number, bg = themeBg()) {
  const p = (h: string, i: number) => Number.parseInt(h.slice(i, i + 2), 16);
  const r = Math.round(p(hex, 1) * amount + p(bg, 1) * (1 - amount));
  const g = Math.round(p(hex, 3) * amount + p(bg, 3) * (1 - amount));
  const b = Math.round(p(hex, 5) * amount + p(bg, 5) * (1 - amount));
  return `rgb(${r},${g},${b})`;
}

export function initials(name: string) {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return "?";
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
}

/** Stable colour for an author, from the lane palette. */
export function authorColor(email: string) {
  let h = 0;
  for (const ch of email) h = (h * 31 + ch.charCodeAt(0)) >>> 0;
  return GRAPH_COLORS[h % 10];
}

const TIME_COLORS: Record<string, string> = { red: "#ef7a76", orange: "#de9b43", green: "#5cb85c" };

/** The original's timeSince: first unit that fits, e.g. "3d", coloured red / orange / green by age.
 *  Units come from src/metrics.rs (year 31556736 s, month 2630016, week 604800, day 86400, hour 3600, minute 60). */
export function timeSince(unix: number, units: [number, string, string][], now = Date.now() / 1000): { text: string; color: string | undefined } {
  const secs = Math.floor(now - unix);
  for (const [len, suffix, color] of units) if (secs >= len) return { text: `${Math.floor(secs / len)}${suffix}`, color: TIME_COLORS[color] };
  return secs > 0 ? { text: `${secs}s`, color: TIME_COLORS.green } : { text: "--", color: undefined };
}

export function relTime(unix: number) {
  const s = Math.max(0, Date.now() / 1000 - unix);
  if (s < 60) return "<1 minute ago";
  if (s < 3600) return `${Math.floor(s / 60)} minutes ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} hours ago`;
  return `${Math.floor(s / 86400)} days ago`;
}

/** A commit's date and time in the UI Customization > Date/Time Format (default "MM/DD/YYYY @ h:mm A"). */
export function fullDate(unix: number) {
  const fmt = useStore.getState().settings.dateTimeFormat || "MM/DD/YYYY @ h:mm A";
  return formatDate(new Date(unix * 1000), fmt);
}

/** "Date verbose Format" (default "MMMM D, YYYY h:mm A"). */
export function verboseDate(unix: number) {
  return formatDate(new Date(unix * 1000), useStore.getState().settings.dateVerboseFormat || "MMMM D, YYYY h:mm A");
}

export const isMac = navigator.platform.toLowerCase().includes("mac");
export const MOD = isMac ? "⌘" : "Ctrl";

/** Add "Co-authored-by:" trailers (GitHub / GitLab credit every listed person). */
export function withCoauthors(description: string, list: string): string {
  const people = list
    .split(",")
    .map((p) => p.trim())
    .filter((p) => /^.+<[^<>@\s]+@[^<>\s]+>$/.test(p));
  if (!people.length) return description;
  const trailers = people.map((p) => `Co-authored-by: ${p}`).join("\n");
  return description.trim() ? `${description.trimEnd()}\n\n${trailers}` : trailers;
}
