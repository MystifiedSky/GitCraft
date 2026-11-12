import { create } from "zustand";
import type { Action } from "./api";

/** Toolbar buttons that turn into a spinning circle while their action runs. From render.bundle.js ToolbarButton
 *  (`icon: spinAction ? ["far","circle-notch"] : icon, spin: spinAction`) and its callers:
 *  Pull = getIsPulling, Push = getIsPushing (ref.pushPromise), Stash = getIsStashing (stashPromise),
 *  Pop = getIsApplyingStash (applyStashPromise). See MECHANICS.md "Toolbar busy spinner". */
export type SpinButton = "pull" | "push" | "stash" | "pop";

/** One full turn of the circle: styles.css `.fa-spin{animation:fa-spin .5s infinite linear}`. */
export const SPIN_TURN_MS = 500;
/** Shortest time the circle stays, so an action that finishes at once still shows it was pressed.
 *  The original ties the circle only to the running action; this minimum is GitCraft's (estimated). */
export const SPIN_MIN_MS = 500;

/** Which toolbar button spins for an action (null: none). A quiet background fetch doesn't spin Pull. */
export function spinButtonFor(action: Action, quiet: boolean): SpinButton | null {
  switch (action.type) {
    case "pull":
      return "pull";
    case "fetch":
      return quiet ? null : "pull";
    case "push":
    case "pushBranch":
    case "pushTag":
    case "deleteRemoteTag":
      return "push";
    case "stash":
      return "stash";
    case "stashPop":
      return "pop";
    default:
      return null;
  }
}

/** Running count per repository and button (a button can be pressed again from the palette or keyboard). */
const useSpinStore = create<{ running: Record<string, number> }>(() => ({ running: {} }));
const key = (path: string, button: SpinButton) => `${button}\n${path}`;

function change(k: string, by: number) {
  useSpinStore.setState((s) => {
    const n = (s.running[k] ?? 0) + by;
    const running = { ...s.running };
    if (n > 0) running[k] = n;
    else delete running[k];
    return { running };
  });
}

/** Run `work` with `button` spinning for `path`. The answer isn't held back: only the circle lingers to SPIN_MIN_MS. */
export async function withSpin<T>(path: string, button: SpinButton | null, work: () => Promise<T>): Promise<T> {
  if (!button) return work();
  const k = key(path, button);
  const started = performance.now();
  change(k, 1);
  try {
    return await work();
  } finally {
    const left = SPIN_MIN_MS - (performance.now() - started);
    if (left > 0) setTimeout(() => change(k, -1), left);
    else change(k, -1);
  }
}

/** Whether a toolbar button is spinning right now (outside React). */
export function isSpinning(path: string, button: SpinButton): boolean {
  return !!useSpinStore.getState().running[key(path, button)];
}

/** Whether a toolbar button is spinning for this repository. */
export function useSpinning(path: string | undefined, button: SpinButton): boolean {
  return useSpinStore((s) => !!path && !!s.running[key(path, button)]);
}
