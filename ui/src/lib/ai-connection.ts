import { invoke } from "@tauri-apps/api/core";
import { DEFAULT_AI, useStore } from "./store";

/** Await the current settings before any AI request, including connection tests. */
export async function invokeAi<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  await invoke("set_ai_config", { config: { ...DEFAULT_AI, ...useStore.getState().settings.ai } });
  return invoke<T>(command, args);
}
