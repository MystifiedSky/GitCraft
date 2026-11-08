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

/** Mix a #rrggbb colour with the app background. */
export function mix(hex: string, amount: number, bg = "#1c1e23") {
  const p = (h: string, i: number) => parseInt(h.slice(i, i + 2), 16);
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

export function relTime(unix: number) {
  const s = Math.max(0, Date.now() / 1000 - unix);
  if (s < 60) return "<1 minute ago";
  if (s < 3600) return `${Math.floor(s / 60)} minutes ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} hours ago`;
  return `${Math.floor(s / 86400)} days ago`;
}

export function fullDate(unix: number) {
  const d = new Date(unix * 1000);
  return d.toLocaleString(undefined, { year: "numeric", month: "numeric", day: "numeric", hour: "numeric", minute: "2-digit" });
}

export const isMac = navigator.platform.toLowerCase().includes("mac");
export const MOD = isMac ? "⌘" : "Ctrl";
