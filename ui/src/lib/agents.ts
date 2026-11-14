import type { Settings } from "./store";

export const AGENT_LABEL: Record<string, string> = { claude: "Claude Code", codex: "Codex CLI", copilot: "Copilot CLI", opencode: "OpenCode", gemini: "Gemini CLI" };
export const AGENT_COMMAND: Record<string, string> = { claude: "claude", codex: "codex", copilot: "copilot", opencode: "opencode", gemini: "gemini" };

export function agentConnection(settings: Settings, agent: string) {
  const provider = agent === "claude" ? "claude_code" : agent === "codex" ? "codex" : null;
  return provider ? (settings.ai.provider === provider ? settings.ai : settings.ai.connections?.[provider]) : undefined;
}

export function folderKey(s: string) {
  const p = s.replaceAll("\\", "/").replace(/^\/\/\?\/UNC\//, "//").replace(/^\/\/\?\//, "").replace(/\/$/, "");
  return /^[a-z]:\//i.test(p) || p.startsWith("//") ? p.toLowerCase() : p;
}
export function sameFolder(a: string, b: string) { return folderKey(a) === folderKey(b); }

/** Paths are shell-quoted; repository folders are passed directly to the PTY, never typed as cd. */
export function agentLaunch(settings: Settings, agent: string, resume?: string) {
  const program = AGENT_COMMAND[agent];
  if (!program) throw new Error("Choose a coding agent in Preferences > External Tools.");
  if (resume && !/^[a-z0-9_-]+$/i.test(resume)) throw new Error("This session has an invalid resume id.");
  const connection = agentConnection(settings, agent);
  const path = connection?.cliPath?.trim();
  const windows = /Win/.test(navigator.platform);
  const ps = settings.termShell === "pwsh" || (windows && (!settings.termShell || settings.termShell === "powershell"));
  const quote = (s: string) => {
    if (settings.termShell === "cmd") {
      if (/["%!^&|<>\r\n]/.test(s)) throw new Error("Choose PowerShell in Terminal Preferences to use this executable path.");
      return '"' + s + '"';
    }
    return "'" + s.replaceAll("'", ps ? "''" : "'\\''") + "'";
  };
  const executable = path ? (path.endsWith(".js") ? "node " + quote(path) : (ps ? "& " : "") + quote(path)) : program;
  const args = resume ? (agent === "claude" ? "--resume " : "resume ") + resume : settings.agentArgs.trim();
  const env: Record<string, string> = {};
  if (connection?.cliConfigDir?.trim()) env[agent === "claude" ? "CLAUDE_CONFIG_DIR" : "CODEX_HOME"] = connection.cliConfigDir.trim();
  return { command: [executable, args].filter(Boolean).join(" "), env };
}
