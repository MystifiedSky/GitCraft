import { invoke } from "@tauri-apps/api/core";
import { DEFAULT_AI, useStore } from "./store";

/** Bind every operation to its own connection snapshot, including connection tests. */
export async function invokeAi<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const config = { ...DEFAULT_AI, ...useStore.getState().settings.ai };
  return invoke<T>(command, { ...args, config });
}
