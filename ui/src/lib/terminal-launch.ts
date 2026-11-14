import { useStore } from "./store";

type TerminalRun = { command: string; title: string; cwd?: string; agentEnv?: Record<string, string> };
export const pendingRuns: TerminalRun[] = [];

/** Open a new in-app terminal tab that runs `command` (shows the terminal panel). */
export function runInTerminal(command: string, title: string, cwd?: string, agentEnv?: Record<string, string>) {
  pendingRuns.push({ command, title, cwd, agentEnv });
  useStore.getState().setSettings({ showTerminal: true });
  window.dispatchEvent(new Event("gitcraft-terminal-run"));
}

