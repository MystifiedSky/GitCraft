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

/** First check after start-up, then every few hours while the app stays open. */
export const FIRST_CHECK_MS = 10000;
export const CHECK_EVERY_MS = 14400000;
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

/** Look for a newer release; when found, download it in the background (if allowed) so it is ready to install. */
export async function checkForUpdates(manual: boolean) {
  const st = useStore.getState();
  if (busy || st.screenshot) return;
  if (!(await configured())) {
    useUpdates.setState({ phase: "off" });
    if (manual) st.toast(false, "Automatic updates aren't set up in this build. See RELEASING.md.");
    return;
  }
  busy = true;
  useUpdates.setState({ phase: "checking", error: null });
  try {
    const u = await check();
    useUpdates.setState({ lastChecked: Date.now() });
    if (!u) {
      useUpdates.setState({ phase: "none" });
      if (manual) st.toast(true, `GitCraft ${st.version} is the newest version.`);
      return;
    }
    pending = u;
    useUpdates.setState({ phase: "available", version: u.version, notes: u.body ?? "" });
    if (manual || st.settings.autoUpdate) await download();
  } catch (e) {
    useUpdates.setState({ phase: "error", error: String(e), lastChecked: Date.now() });
    if (manual) st.toast(false, `Could not check for updates: ${e}`);
  } finally {
    busy = false;
  }
}

export async function download() {
  const u = pending;
  if (!u) return;
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
    });
    useUpdates.setState({ phase: "ready", percent: 100 });
  } catch (e) {
    useUpdates.setState({ phase: "error", error: String(e) });
    useStore.getState().toast(false, `The update could not be downloaded: ${e}`);
  }
}

/** "Restart and Install Update". On Windows the installer closes GitCraft and opens it again by itself. */
export async function installAndRestart() {
  if (!pending) return;
  try {
    await pending.install();
    await relaunch();
  } catch (e) {
    useStore.getState().toast(false, `The update could not be installed: ${e}`);
  }
}

/** Start the background schedule (called once from the app shell). */
export function startUpdateChecks() {
  if (useStore.getState().screenshot) return () => {};
  const first = setTimeout(() => void checkForUpdates(false), FIRST_CHECK_MS);
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
