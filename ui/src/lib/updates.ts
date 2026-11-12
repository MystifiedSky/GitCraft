// Automatic updates from GitHub Releases (Tauri updater). Installed copies read latest.json from the newest
// release, check its signature against the public key built into the app, download in the background and
// offer "Update Ready (Restart GitCraft)" like the original.
import { invoke } from "@tauri-apps/api/core";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { create } from "zustand";
import { useStore } from "./store";

export type UpdatePhase = "idle" | "checking" | "none" | "available" | "downloading" | "ready" | "error" | "off";

interface UpdateState {
  phase: UpdatePhase;
  version: string | null;
  notes: string;
  percent: number | null;
  lastChecked: number | null;
  error: string | null;
}

export const useUpdates = create<UpdateState>(() => ({ phase: "idle", version: null, notes: "", percent: null, lastChecked: null, error: null }));

/** Check at launch and once an hour while the app stays open. */
export const CHECK_EVERY_MS = 60 * 60 * 1000;
const CHECK_TIMEOUT_MS = 30_000;
const DOWNLOAD_TIMEOUT_MS = 10 * 60 * 1000;
export const RELEASES_URL = "https://github.com/MystifiedSky/GitCraft/releases";

let pending: Update | null = null;
let busy = false;

async function configured() {
  try {
    return (await invoke<{ configured: boolean }>("update_config")).configured;
  } catch {
    return false;
  }
}

async function replacePending(update: Update | null) {
  const previous = pending;
  pending = update;
  if (previous) await previous.close().catch(() => undefined);
}

/** Look for a newer release; when found, download it in the background (if allowed) so it is ready to install. */
export async function checkForUpdates(manual: boolean) {
  const st = useStore.getState();
  // A downloaded update is kept until restart, even if a scheduled check comes due.
  if (busy || st.screenshot || (!manual && !st.settings.autoUpdate) || useUpdates.getState().phase === "ready") return;
  busy = true;
  const { phase: previousPhase, error: previousError } = useUpdates.getState();
  useUpdates.setState({ phase: "checking", error: null });
  try {
    const canUpdate = await configured();
    if (!manual && !useStore.getState().settings.autoUpdate) {
      useUpdates.setState({ phase: previousPhase, error: previousError });
      return;
    }
    if (!canUpdate) {
      await replacePending(null);
      useUpdates.setState({ phase: "off", version: null, notes: "", percent: null });
      if (manual) st.toast(false, "Automatic updates aren't set up in this build. See RELEASING.md.");
      return;
    }
    const u = await check({ timeout: CHECK_TIMEOUT_MS });
    await replacePending(u);
    useUpdates.setState({ lastChecked: Date.now() });
    if (!u) {
      useUpdates.setState({ phase: "none", version: null, notes: "", percent: null });
      if (manual) st.toast(true, `GitCraft ${st.version} is the newest version.`);
      return;
    }
    useUpdates.setState({ phase: "available", version: u.version, notes: u.body ?? "", percent: null });
    if (manual || useStore.getState().settings.autoUpdate) await downloadPending(u, manual);
  } catch (e) {
    useUpdates.setState({ phase: "error", error: String(e), lastChecked: Date.now() });
    if (manual) st.toast(false, `Could not check for updates: ${e}`);
  } finally {
    busy = false;
  }
}

export async function download() {
  const u = pending;
  if (!u || busy || useUpdates.getState().phase !== "available") return;
  busy = true;
  try {
    await downloadPending(u, true);
  } finally {
    busy = false;
  }
}

async function downloadPending(u: Update, manual: boolean) {
  let total = 0;
  let got = 0;
  useUpdates.setState({ phase: "downloading", percent: 0 });
  try {
    await u.download((ev) => {
      if (ev.event === "Started") total = ev.data.contentLength ?? 0;
      else if (ev.event === "Progress") {
        got += ev.data.chunkLength;
        useUpdates.setState({ percent: total ? Math.min(100, Math.round((got / total) * 100)) : null });
      }
    }, { timeout: DOWNLOAD_TIMEOUT_MS });
    useUpdates.setState({ phase: "ready", percent: 100 });
  } catch (e) {
    useUpdates.setState({ phase: "error", error: String(e) });
    if (manual) useStore.getState().toast(false, `The update could not be downloaded: ${e}`);
  }
}

/** "Restart and Install Update". On Windows the installer closes GitCraft and opens it again by itself. */
export async function installAndRestart() {
  const u = pending;
  if (!u || busy || useUpdates.getState().phase !== "ready") return;
  busy = true;
  try {
    await u.install();
    await relaunch();
  } catch (e) {
    useStore.getState().toast(false, `The update could not be installed: ${e}`);
  } finally {
    busy = false;
  }
}

/** Start the background schedule (called once from the app shell). */
export function startUpdateChecks() {
  const { screenshot, settings } = useStore.getState();
  if (screenshot || !settings.autoUpdate) return () => {};
  const first = setTimeout(() => void checkForUpdates(false), 0);
  const every = setInterval(() => void checkForUpdates(false), CHECK_EVERY_MS);
  return () => {
    clearTimeout(first);
    clearInterval(every);
  };
}

function ago(t: number): string {
  const m = Math.floor((Date.now() - t) / 60000);
  if (m < 1) return "a few seconds ago";
  if (m < 60) return m === 1 ? "a minute ago" : `${m} minutes ago`;
  const h = Math.floor(m / 60);
  return h === 1 ? "an hour ago" : `${h} hours ago`;
}

/** Menu label like the original's ("Check for Update", "Update available (1.2.0)", "Downloading Update..."). */
export function updateMenuLabel(s: UpdateState): string {
  switch (s.phase) {
    case "checking":
      return "Checking for updates...";
    case "available":
      return `Update available (${s.version})`;
    case "downloading":
      return "Downloading Update...";
    case "ready":
      return "Restart and Install Update";
    case "error":
      return "Update Errored — Check Again";
    default:
      return s.lastChecked ? `Check for Update (Last checked ${ago(s.lastChecked)})` : "Check for Update";
  }
}
