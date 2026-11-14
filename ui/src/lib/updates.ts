// Automatic updates from GitHub Releases (Tauri updater). Installed copies read latest.json from the newest
// release, check its signature against the public key built into the app, download in the background and
// offer "Update Ready (Restart GitCraft)".
import { invoke } from "@tauri-apps/api/core";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { create } from "zustand";
import { fromNow } from "./relative-time";
import { useStore } from "./store";

export type UpdatePhase = "idle" | "checking" | "none" | "available" | "downloading" | "ready" | "error" | "off";

interface UpdateState {
  phase: UpdatePhase;
  version: string | null;
  notes: string;
  percent: number | null;
  /** When a check last found nothing newer; cleared by a failure. */
  lastChecked: number | null;
  /** When the last check or download failed; cleared by a check that succeeds. */
  errorAt: number | null;
  error: string | null;
}

export const useUpdates = create<UpdateState>(() => ({ phase: "idle", version: null, notes: "", percent: null, lastChecked: null, errorAt: null, error: null }));

/** Check at launch and every hour (3,600,000 ms) while the app stays open. */
export const CHECK_EVERY_MS = 3_600_000;
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
    if (!u) {
      useUpdates.setState({ phase: "none", version: null, notes: "", percent: null, lastChecked: Date.now(), errorAt: null });
      if (manual) st.toast(true, `GitCraft ${st.version} is the newest version.`);
      return;
    }
    useUpdates.setState({ phase: "available", version: u.version, notes: u.body ?? "", percent: null, errorAt: null });
    if (manual || useStore.getState().settings.autoUpdate) await downloadPending(u, manual);
  } catch (e) {
    useUpdates.setState({ phase: "error", error: String(e), lastChecked: null, errorAt: Date.now() });
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
    useUpdates.setState({ phase: "error", error: String(e), lastChecked: null, errorAt: Date.now() });
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

/** "(Last checked 5 minutes ago)": the time in brackets after an optional prefix; empty when there is no time. */
export function whenText(at: number | null, prefix = "", now: number = Date.now()): string {
  return at ? `(${[prefix, fromNow(at, now)].filter(Boolean).join(" ")})` : "";
}

export interface UpdateMenuItem {
  label: string;
  disabled: boolean;
  /** What choosing it does; null for a status line. */
  run: "install" | "download" | "check" | null;
}

/**
 * The update lines of the File menu, in this order, each only in its states:
 * "Checking for updates..." (checking), "Restart and Install Update" (ready), "Update available (x)" (found, not yet downloading),
 * "Check for Update (Last checked …)" (idle, nothing newer, or failed), "Update Errored (… ago)" (failed),
 * "Downloading Update..." (downloading). A failure clears "Last checked", so the check line then has no time.
 */
export function updateMenuItems(s: Pick<UpdateState, "phase" | "version" | "lastChecked" | "errorAt">, now: number = Date.now()): UpdateMenuItem[] {
  const items: UpdateMenuItem[] = [];
  const canCheck = s.phase === "idle" || s.phase === "none" || s.phase === "error" || s.phase === "off";
  if (s.phase === "checking") items.push({ label: "Checking for updates...", disabled: true, run: null });
  if (s.phase === "ready") items.push({ label: "Restart and Install Update", disabled: false, run: "install" });
  if (s.phase === "available") items.push({ label: `Update available (${s.version})`, disabled: false, run: "download" });
  if (canCheck) items.push({ label: `Check for Update ${whenText(s.lastChecked, "Last checked", now)}`.trimEnd(), disabled: false, run: "check" });
  if (s.phase === "error") items.push({ label: `Update Errored ${whenText(s.errorAt, "", now)}`.trimEnd(), disabled: true, run: null });
  if (s.phase === "downloading") items.push({ label: "Downloading Update...", disabled: true, run: null });
  return items;
}

/** One-line status for places that show a single update message (the notification menu). */
export function updateMenuLabel(s: UpdateState): string {
  const items = updateMenuItems(s);
  const status = items.find((i) => i.run !== "check") ?? items[0];
  return status?.label ?? "Check for Update";
}
