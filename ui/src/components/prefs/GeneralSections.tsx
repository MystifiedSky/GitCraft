import { useQuery } from "@tanstack/react-query";
import { enableDesktopNotifications } from "@/lib/desktop-notify";
import { useState } from "react";
import { api } from "@/lib/api";
import { useStore, type Settings } from "@/lib/store";
import { tools, formatDate } from "@/lib/integrations";
import { THEMES } from "@/lib/prefs";
import { Area, Btn, H, Note, Num, PathField, Row, Select, Text, Toggle } from "./controls";

const useSet = () => {
  const s = useStore();
  return { set: s.settings, put: (p: Partial<Settings>) => s.setSettings(p), toast: s.toast };
};

/** Global Git config values shown on General (core.autocrlf, core.longpaths). */
function useGlobalConfig(keys: string[]) {
  const q = useQuery({ queryKey: ["gitConfigGlobal", keys.join()], queryFn: () => tools.configGet(null, "global", keys) });
  const save = async (key: string, value: string) => {
    await api.setGitConfig(null, "global", key, value);
    q.refetch();
  };
  return { cfg: q.data ?? {}, save };
}

export function GeneralSection() {
  const { set, put, toast } = useSet();
  const { cfg, save } = useGlobalConfig(["core.autocrlf", "core.longpaths"]);
  const isWin = /Win/.test(navigator.platform);
  return (
    <>
      <H>General</H>
      <Row label="Auto-Fetch Interval" help="In minutes, between 0 and 60 (0 disables auto-fetch). Fetches all visible remotes." warning={set.autoFetchMin > 0 && set.autoFetchMin < 2 ? "If you have many visible remotes, this may impact performance." : undefined}>
        <Num value={set.autoFetchMin} min={0} max={60} onSave={(v) => put({ autoFetchMin: v })} />
      </Row>
      <Row label="Auto-Prune">
        <Toggle value={set.autoPrune} onChange={(v) => put({ autoPrune: v })} />
      </Row>
      <Row label="Keep submodules up to date" help="Automatically update all submodules after performing a Git action">
        <Toggle value={set.autoUpdateSubmodules} onChange={(v) => put({ autoUpdateSubmodules: v })} />
      </Row>
      <Row label="Default Branch Name" help="Sets the default branch name when initializing new repositories." error={/[\s~^:?*[\\]/.test(set.defaultBranch) ? "That isn't a valid branch name." : undefined}>
        <Text value={set.defaultBranch} placeholder="main" onSave={(v) => put({ defaultBranch: v.trim() })} />
      </Row>
      <Row label={'Delete ".orig" files after merging'}>
        <Toggle value={set.deleteOrigAfterMerge} onChange={(v) => put({ deleteOrigAfterMerge: v })} />
      </Row>
      <Row label="Git Config Default: core.autocrlf" help="Sets core.autocrlf in your global Git Config.">
        <Select value={(cfg["core.autocrlf"] || "unset") as string} onChange={(v) => save("core.autocrlf", v === "unset" ? "" : v)} options={[["unset", "(not set)"], ["true", "true"], ["input", "input"], ["false", "false"]]} />
      </Row>
      {isWin && (
        <Row label="Longpaths" help="Sets core.longpaths in your global Git Config. By default, Windows only supports file paths less than 260 characters. Enable support for longer file paths.">
          <Toggle value={cfg["core.longpaths"] === "true"} onChange={(v) => save("core.longpaths", v ? "true" : "false")} />
        </Row>
      )}
      <Row label="Use In-App Terminal Instead" help="Open External Terminal (Alt+T) and the Terminal button use the built-in terminal.">
        <Toggle value={set.useInAppTerminal} onChange={(v) => put({ useInAppTerminal: v })} />
      </Row>
      <Row label="Remember tabs">
        <Toggle value={set.rememberTabs} onChange={(v) => put({ rememberTabs: v })} />
      </Row>
      <Row
        label="Initial Commits in Graph"
        help="Minimum 500. If commit lazy loading is enabled, GitCraft will load additional commits if you reach the earliest commit in the Graph."
        warning={set.initialCommits > 2000 ? "Increasing this value may adversely affect performance." : undefined}
      >
        <Num value={set.initialCommits} min={500} max={100000} step={100} onSave={(v) => put({ initialCommits: v })} />
      </Row>
      <Row label="Lazy Load Commits in Graph">
        <Toggle value={set.lazyLoad} onChange={(v) => put({ lazyLoad: v })} />
      </Row>
      <Row label="Show All Commits in Graph" warning={set.showAllCommits ? "Enabling this option may adversely affect performance." : undefined}>
        <Toggle value={set.showAllCommits} onChange={(v) => put({ showAllCommits: v })} />
      </Row>
      <Row label="Use extended logging in activity log" help="The activity log keeps Git's full output (standard output and errors) for every command.">
        <Toggle value={set.extendedLog} onChange={(v) => put({ extendedLog: v })} />
      </Row>
      <Row label="Enable proactive conflict detection with target branch" help="Unchecking this will disable proactive conflict detection for all repositories. You can disable an individual repository from Repo-Specific Preferences > Conflict Prevention.">
        <Toggle value={set.conflictDetection} onChange={(v) => put({ conflictDetection: v })} />
      </Row>
      <Row label="Automatically download GitCraft updates" help="Checks GitHub Releases and installs on restart.">
        <Toggle value={set.autoUpdate} onChange={(v) => put({ autoUpdate: v })} />
      </Row>
      <Row label="Default folder for clone and init">
        <PathField folder title="Default folder" value={set.defaultCloneFolder} onSave={(v) => put({ defaultCloneFolder: v })} />
      </Row>
      <Row label="Default pull behaviour">
        <Select
          value={set.pullMode}
          onChange={(v) => put({ pullMode: v })}
          options={[
            ["ff", "Pull (fast-forward if possible)"],
            ["ffonly", "Pull (fast-forward only)"],
            ["rebase", "Pull (rebase)"],
            ["fetch", "Fetch All"],
          ]}
        />
      </Row>
      <Row label="Forget all Usernames and Passwords" help="Asks Git Credential Manager to forget what it saved for the connected services.">
        <Btn
          onClick={async () => {
            const hosts = ["github.com", "gitlab.com", "bitbucket.org", "dev.azure.com", set.integrations.githubEnterpriseHost, set.integrations.gitlabHost, set.integrations.bitbucketServerHost].filter(Boolean);
            const n = await tools.forgetCredentials(hosts).catch(() => 0);
            toast(true, n ? `Forgot saved credentials for ${n} host${n === 1 ? "" : "s"}` : "GitCraft has no stored user/pass credentials to remove");
          }}
        >
          Forget All
        </Btn>
      </Row>
      <Row label="Usage data and bug reports" help="GitCraft doesn't send usage data or bug reports anywhere.">
        <span className="text-[12.5px] text-fg3">Nothing is sent</span>
      </Row>
    </>
  );
}

export function SshSection() {
  const { set, put, toast } = useSet();
  const acc = useQuery({ queryKey: ["accounts", null], queryFn: () => api.accountInfo(null) });
  const [busy, setBusy] = useState(false);
  return (
    <>
      <H>SSH</H>
      <Note>Used when a remote's address starts with git@ or ssh://. With the local SSH agent, Git uses your system's SSH setup (~/.ssh/config and the agent); otherwise it uses the key below.</Note>
      <Row label="Use local SSH agent">
        <Toggle value={set.sshUseAgent} onChange={(v) => put({ sshUseAgent: v })} />
      </Row>
      <Row label="SSH Private Key">
        <PathField title="SSH Private Key" value={set.sshPrivateKey} placeholder="~/.ssh/id_ed25519" onSave={(v) => put({ sshPrivateKey: v, sshPublicKey: set.sshPublicKey || `${v}.pub` })} />
      </Row>
      <Row label="SSH Public Key">
        <PathField title="SSH Public Key" value={set.sshPublicKey} placeholder="~/.ssh/id_ed25519.pub" onSave={(v) => put({ sshPublicKey: v })} />
      </Row>
      <Row label="Generate new Private/Public key" help="This will create new keys and configure GitCraft to use them. Add the public key to your services under Integrations.">
        <Btn
          disabled={busy || !acc.data?.ssh_keygen}
          onClick={async () => {
            setBusy(true);
            try {
              const name = `gitcraft_${Date.now().toString(36)}`;
              const pub = await api.generateSshKey(acc.data?.email || "gitcraft", name);
              const priv = `${acc.data?.ssh_dir}${acc.data?.ssh_dir?.endsWith("\\") || acc.data?.ssh_dir?.endsWith("/") ? "" : "/"}${name}`;
              put({ sshUseAgent: false, sshPrivateKey: priv, sshPublicKey: `${priv}.pub` });
              await navigator.clipboard.writeText(pub).catch(() => undefined);
              toast(true, "New SSH key created, selected, and its public key copied");
              acc.refetch();
            } catch (e) {
              toast(false, String(e));
            } finally {
              setBusy(false);
            }
          }}
        >
          Generate
        </Btn>
      </Row>
      <Row label="Use default Git Credential Manager" help="Select this to have Git use the credential manager in your Git config to access repositories over HTTPS.">
        <Toggle value={acc.data?.credential_helper === "manager"} disabled={!acc.data?.gcm} onChange={(v) => api.setGitConfig(null, "global", "credential.helper", v ? "manager" : "").then(() => acc.refetch())} />
      </Row>
      {(acc.data?.ssh_keys.length ?? 0) > 0 && (
        <Row label="Keys on this computer" wide>
          <div className="flex flex-col gap-1 w-full">
            {acc.data!.ssh_keys.map((k) => (
              <div key={k.path} className="flex items-center gap-2 text-[12.5px]">
                <span className="mono truncate flex-1 text-fg">{k.path}</span>
                <span className="text-fg3">{k.kind}</span>
                <Btn onClick={() => navigator.clipboard.writeText(k.public).then(() => toast(true, "Public key copied"))}>Copy public key</Btn>
                <Btn onClick={() => put({ sshUseAgent: false, sshPrivateKey: k.path.replace(/\.pub$/, ""), sshPublicKey: k.path.endsWith(".pub") ? k.path : `${k.path}.pub` })}>Use</Btn>
              </div>
            ))}
          </div>
        </Row>
      )}
    </>
  );
}

const NOW = new Date(2026, 10, 9, 14, 23, 5);

export function UiSection() {
  const { set, put } = useSet();
  const cols = set.columns;
  const col = (k: keyof typeof cols, label: string) => (
    <Row label={label} key={k}>
      <Toggle value={cols[k]} onChange={(v) => put({ columns: { ...cols, [k]: v } })} />
    </Row>
  );
  return (
    <>
      <H>UI Customization</H>
      <Row label="Theme">
        <Select value={set.theme} onChange={(v) => put({ theme: v })} options={THEMES.map((t) => [t.id, t.label] as [string, string])} />
      </Row>
      <Row label="Notification Location">
        <Select
          value={set.notificationLocation}
          onChange={(v) => put({ notificationLocation: v })}
          options={[
            ["top-left", "Top Left"],
            ["top-right", "Top Right"],
            ["bottom-left", "Bottom Left"],
            ["bottom-right", "Bottom Right"],
          ]}
        />
      </Row>
      <Row label="Language" help="GitCraft is available in English.">
        <Select value="en" onChange={() => undefined} options={[["en", "English"]]} />
      </Row>
      <H sub>Date/Time</H>
      {(
        [
          ["dateFormat", "Date Format"],
          ["dateTimeFormat", "Date/Time Format"],
          ["dateVerboseFormat", "Date verbose Format"],
        ] as const
      ).map(([k, label]) => (
        <Row key={k} label={label} help={`Example: ${formatDate(NOW, set[k])}. Tokens: YYYY MM M DD D MMMM MMM dddd HH H hh h mm ss A; [text] is kept as written.`}>
          <Text value={set[k]} onSave={(v) => put({ [k]: v || (k === "dateFormat" ? "MM/DD/YYYY" : k === "dateTimeFormat" ? "MM/DD/YYYY @ h:mm A" : "MMMM D, YYYY h:mm A") } as Partial<Settings>)} mono />
        </Row>
      ))}
      <H sub>Graph</H>
      <Row label="Branch visibility in commit graph" help="Smart Branch Visibility shows the checked-out branch, its upstream and the branch it will merge into.">
        <Select value={set.smartBranches ? "smart" : "all"} onChange={(v) => put({ smartBranches: v === "smart" })} options={[["all", "All"], ["smart", "Smart Branch Visibility"]]} />
      </Row>
      {col("refs", "Show branches and tags in graph")}
      {col("graph", "Show commit tree in graph")}
      {col("description", "Show commit description in graph")}
      {col("author", "Show commit author in graph")}
      {col("date", "Show commit date/time in graph")}
      {col("sha", "Show commit sha in graph")}
      {col("changes", "Show commit changes in graph")}
      <Row label="Compact Graph Column">
        <div className="flex gap-2">
          <Btn onClick={() => put({ columns: { refs: true, graph: true, description: true, author: false, date: false, sha: false, changes: false } })}>Reset columns to default layout</Btn>
          <Btn onClick={() => put({ columns: { refs: true, graph: true, description: false, author: false, date: false, sha: false, changes: false } })}>Reset columns to compact layout</Btn>
        </div>
      </Row>
      <Row label="Display author initials and generic remote icons instead of avatars">
        <Toggle value={set.authorInitials} onChange={(v) => put({ authorInitials: v })} />
      </Row>
      <Row label="Show ghost branch/tag when hovering over or selecting a commit">
        <Toggle value={set.ghostRefs} onChange={(v) => put({ ghostRefs: v })} />
      </Row>
      <Row label="Highlight associated rows when hovering over a branch">
        <Toggle value={set.highlightRowsOnHover} onChange={(v) => put({ highlightRowsOnHover: v })} />
      </Row>
      <Row label="Show markers beside the graph scrollbar" help="Coloured ticks for branches, tags, the checked-out commit and the selection. Off by default.">
        <Toggle value={set.scrollMarkers} onChange={(v) => put({ scrollMarkers: v })} />
      </Row>
      <H sub>Panels</H>
      <Row label="Show toolbar icon labels">
        <Toggle value={set.toolbarLabels} onChange={(v) => put({ toolbarLabels: v })} />
      </Row>
      <Row label="Show Agents view in left panel">
        <Toggle value={set.showAgentsView} onChange={(v) => put({ showAgentsView: v, leftView: v ? set.leftView : "list" })} />
      </Row>
      <Row label="Automatically group worktree tabs together">
        <Toggle value={set.autogroupWorktreeTabs} onChange={(v) => put({ autogroupWorktreeTabs: v })} />
      </Row>
      <Row label="Enable spell checking">
        <Toggle value={set.spellcheck} onChange={(v) => put({ spellcheck: v })} />
      </Row>
    </>
  );
}

export function NotificationsSection() {
  const { set, put, toast } = useSet();
  return (
    <>
      <H>Notifications</H>
      <Row label="Enable Desktop Notifications" help="Allow GitCraft to send desktop notifications.">
        <Toggle
          value={set.desktopNotifications}
          onChange={async (v) => {
            put({ desktopNotifications: v });
            // A native "Desktop notifications enabled" toast is shown; without permission an in-app one stands in.
            if (v && !(await enableDesktopNotifications())) toast(true, "Desktop notifications enabled");
          }}
        />
      </Row>
      <Row label="Show a notice after successful actions" help="Errors are always shown.">
        <Toggle value={set.notifySuccess} onChange={(v) => put({ notifySuccess: v })} />
      </Row>
      <Row label="Receive Marketing Notifications" help="GitCraft has no marketing notifications.">
        <Toggle value={false} disabled onChange={() => undefined} />
      </Row>
    </>
  );
}

const MONO_FONTS = ["Cascadia Code", "Cascadia Mono", "Consolas", "Courier New", "Fira Code", "JetBrains Mono", "Lucida Console", "Menlo", "Monaco", "Source Code Pro", "Ubuntu Mono"];
const OTHER_FONTS = ["Arial", "Calibri", "Open Sans", "Segoe UI", "Tahoma", "Verdana"];

function FontSelect({ value, onChange, monoOnly }: { value: string; onChange: (v: string) => void; monoOnly: boolean }) {
  const fonts = (monoOnly ? [...MONO_FONTS] : [...MONO_FONTS, ...OTHER_FONTS]).sort((a, b) => a.localeCompare(b));
  return <Select value={value} onChange={onChange} options={[["", "Default"], ...fonts.map((f) => [f, f] as [string, string])]} />;
}

export function EditorSection() {
  const { set, put } = useSet();
  return (
    <>
      <H>Editor</H>
      <Row label="Font">
        <FontSelect value={set.editorFontFamily} monoOnly={set.monospaceOnly} onChange={(v) => put({ editorFontFamily: v })} />
      </Row>
      <Row label="Show Only Monospace Fonts">
        <Toggle value={set.monospaceOnly} onChange={(v) => put({ monospaceOnly: v })} />
      </Row>
      <Row label="Font Size">
        <Num value={set.editorFontSize} min={8} max={32} onSave={(v) => put({ editorFontSize: v })} />
      </Row>
      <Row label="Tab Size">
        <Num value={set.editorTabSize} min={1} max={16} onSave={(v) => put({ editorTabSize: v })} />
      </Row>
      <Row label="End of Line Character" help="Used for new lines you type in the file editor.">
        <Select value={set.editorEol} onChange={(v) => put({ editorEol: v })} options={[["lf", "LF"], ["crlf", "CRLF"]]} />
      </Row>
      <Row label="Show Line Numbers">
        <Toggle value={set.editorLineNumbers} onChange={(v) => put({ editorLineNumbers: v })} />
      </Row>
      <Row label="Syntax Highlighting">
        <Toggle value={set.editorSyntax} onChange={(v) => put({ editorSyntax: v })} />
      </Row>
      <Row label="Word Wrap">
        <Toggle value={set.wordWrap} onChange={(v) => put({ wordWrap: v })} />
      </Row>
    </>
  );
}

export function TerminalSection() {
  const { set, put } = useSet();
  const isWin = /Win/.test(navigator.platform);
  return (
    <>
      <H>In-App Terminal</H>
      <Row label="Default In-App Terminal" help="Changes to this setting will only apply to new tabs or after restarting GitCraft">
        <Select
          value={set.termShell}
          onChange={(v) => put({ termShell: v })}
          options={
            isWin
              ? [
                  ["powershell", "Windows PowerShell"],
                  ["pwsh", "PowerShell 7"],
                  ["cmd", "Command Prompt"],
                  ["gitbash", "Git Bash"],
                  ["wsl", "WSL"],
                ]
              : [
                  ["default", "Default shell"],
                  ["bash", "bash"],
                  ["zsh", "zsh"],
                  ["fish", "fish"],
                ]
          }
        />
      </Row>
      <Row label="Font">
        <FontSelect value={set.termFontFamily} monoOnly onChange={(v) => put({ termFontFamily: v })} />
      </Row>
      <Row label="Font Size">
        <Num value={set.termFontSize} min={8} max={32} onSave={(v) => put({ termFontSize: v })} />
      </Row>
      <Row label="Line Height">
        <Num value={set.termLineHeight} min={1} max={2} step={0.1} onSave={(v) => put({ termLineHeight: v })} />
      </Row>
      <Row label="Cursor Style">
        <Select value={set.termCursor} onChange={(v) => put({ termCursor: v })} options={[["block", "Block"], ["underline", "Underline"], ["bar", "Bar"]]} />
      </Row>
      <Row label="Dim terminal when unfocused">
        <Toggle value={set.termDim} onChange={(v) => put({ termDim: v })} />
      </Row>
    </>
  );
}

export const EDITORS: [string, string][] = [
  ["vscode", "Visual Studio Code"],
  ["cursor", "Cursor"],
  ["windsurf", "Windsurf"],
  ["zed", "Zed"],
  ["sublime", "Sublime Text"],
  ["notepadpp", "Notepad++"],
  ["idea", "IntelliJ IDEA"],
  ["webstorm", "WebStorm"],
  ["rider", "Rider"],
  ["vs", "Visual Studio"],
  ["custom", "Other (custom)"],
];

export const AGENTS: [string, string][] = [
  ["claude", "Claude Code"],
  ["codex", "Codex"],
  ["copilot", "Copilot CLI"],
  ["opencode", "OpenCode"],
  ["gemini", "Gemini CLI"],
  ["none", "None"],
];

const DIFF_TOOLS = ["vscode", "meld", "kdiff3", "bc", "p4merge", "winmerge", "tortoisemerge", "vimdiff", "araxis", "deltawalker", "diffmerge", "ecmerge", "emerge", "examdiff", "guiffy", "opendiff", "smerge", "tkdiff", "xxdiff"];

export function ToolsSection() {
  const { set, put } = useSet();
  const isWin = /Win/.test(navigator.platform);
  return (
    <>
      <H>External Tools</H>
      <H sub>Merge Tool</H>
      <Row label="External Merge Tool" help="Opened with Ctrl+D on a conflicted file (git mergetool).">
        <Select value={set.mergeTool} onChange={(v) => put({ mergeTool: v })} options={[["", "Git's configured tool"], ...DIFF_TOOLS.map((t) => [t, t] as [string, string])]} />
      </Row>
      <H sub>Diff Tool</H>
      <Row label="External Diff Tool" help="Opened with Ctrl+D on a changed file (git difftool).">
        <Select value={set.diffTool} onChange={(v) => put({ diffTool: v })} options={[["", "Git's configured tool"], ...DIFF_TOOLS.map((t) => [t, t] as [string, string])]} />
      </Row>
      <H sub>Editor</H>
      <Row label="External Editor" help="File > Open Repo in … and Open file in external editor.">
        <Select value={set.externalEditor} onChange={(v) => put({ externalEditor: v })} options={EDITORS} />
      </Row>
      {set.externalEditor === "custom" && (
        <>
          <Row label="Editor Executable">
            <PathField title="Select an editor executable to use" value={set.editorPath} onSave={(v) => put({ editorPath: v })} />
          </Row>
          <Row label="Open File Command" help="Arguments; $FILE is the file, $REPO_PATH the repository folder.">
            <Text value={set.editorFileArgs} placeholder='"$FILE"' onSave={(v) => put({ editorFileArgs: v })} mono />
          </Row>
          <Row label="Open Repo Command" help="Arguments; $REPO_PATH is the repository folder.">
            <Text value={set.editorRepoArgs} placeholder='"$REPO_PATH"' onSave={(v) => put({ editorRepoArgs: v })} mono />
          </Row>
        </>
      )}
      <H sub>Terminal</H>
      <Row label="External Terminal">
        <Select
          value={set.externalTerminal}
          onChange={(v) => put({ externalTerminal: v })}
          options={
            isWin
              ? [
                  ["default", "Default (Windows Terminal if installed)"],
                  ["wt", "Windows Terminal"],
                  ["powershell", "Windows PowerShell"],
                  ["pwsh", "PowerShell 7"],
                  ["cmd", "Command Prompt"],
                  ["gitbash", "Git Bash"],
                  ["custom", "Other (custom)"],
                ]
              : [
                  ["default", "Default"],
                  ["custom", "Other (custom)"],
                ]
          }
        />
      </Row>
      {set.externalTerminal === "custom" && (
        <>
          <Row label="Terminal Executable">
            <PathField title="Terminal executable" value={set.terminalPath} onSave={(v) => put({ terminalPath: v })} />
          </Row>
          <Row label="Terminal Arguments" help="$REPO_PATH is the repository folder.">
            <Text value={set.terminalArgs} onSave={(v) => put({ terminalArgs: v })} mono />
          </Row>
        </>
      )}
      <H sub>Coding Agent</H>
      <Row label="Coding Agent" help="Started from the Agents view in the in-app terminal.">
        <Select value={set.codingAgent} onChange={(v) => put({ codingAgent: v })} options={AGENTS} />
      </Row>
    </>
  );
}

export function AgentsSection() {
  const { set, put } = useSet();
  return (
    <>
      <H>Agents</H>
      <Row label="Setup commands (1 per line)" help="Commands run in sequence when a new agent session starts in a worktree. Use this to install dependencies and prepare your environment. Put each command on a new line." wide>
        <Area value={set.agentSetup} placeholder={"npm install\npnpm build"} onSave={(v) => put({ agentSetup: v })} mono />
      </Row>
      <Row label="CLI arguments" help="Arguments appended to the agent command when starting a session.">
        <Text value={set.agentArgs} onSave={(v) => put({ agentArgs: v })} mono />
      </Row>
      <Row label="Show worktree WIP nodes in the graph" help="When enabled, the graph shows a WIP row for every worktree with uncommitted changes or an active agent session.">
        <Toggle value={set.showWorktreeWip} onChange={(v) => put({ showWorktreeWip: v })} />
      </Row>
    </>
  );
}

export function CommitSection() {
  const { set, put } = useSet();
  return (
    <>
      <H>Commit</H>
      <Note>Create and apply a default initial message for commits to maintain consistency in commit summaries and descriptions.</Note>
      <Row label="Apply this template to commit messages">
        <Toggle value={set.useCommitTemplate} onChange={(v) => put({ useCommitTemplate: v })} />
      </Row>
      <Row label="Summary">
        <Text value={set.templateSummary} placeholder="e.g. feat: " onSave={(v) => put({ templateSummary: v })} />
      </Row>
      <Row label="Description" wide>
        <Area value={set.templateDescription} onSave={(v) => put({ templateDescription: v })} />
      </Row>
      <Row label="Remove comments from commit messages" help="Lines starting with # are left out of the commit message.">
        <Toggle value={set.stripComments} onChange={(v) => put({ stripComments: v })} />
      </Row>
      <Row label="Push after each commit">
        <Toggle value={set.pushAfterCommit} onChange={(v) => put({ pushAfterCommit: v })} />
      </Row>
      <Row label="Skip git hooks by default when committing">
        <Toggle value={set.skipHooks} onChange={(v) => put({ skipHooks: v })} />
      </Row>
      <H sub>Merge Behavior</H>
      <Row label="Squash" help="When enabled, merging branches locally will stage all changes without creating a merge commit automatically. You will need to commit manually.">
        <Toggle value={set.squashMerge} onChange={(v) => put({ squashMerge: v })} />
      </Row>
    </>
  );
}

export function ExperimentalSection() {
  const { set, put } = useSet();
  const v = useQuery({ queryKey: ["toolVersions", set.gitPath], queryFn: () => tools.versions("gpg") });
  const ver = /(?:^|\D)(\d+\.\d+\.\d+)/.exec(v.data?.git ?? "")?.[1];
  const old = ver && ver.localeCompare("2.40.0", undefined, { numeric: true }) < 0;
  return (
    <>
      <H>Experimental</H>
      <Note>GitCraft always uses the Git executable (it never had a separate Git library), so SSH commit signing and every advanced Git feature are available.</Note>
      <H sub>Git Executable</H>
      <Row
        label="Git Executable"
        help={v.data ? `The Git executable GitCraft will use to execute Git commands.\n${v.data.git || "not found"} · ${v.data.git_path}\nDetected OpenSSH version: ${v.data.ssh || "not found"}` : "Checking…"}
        warning={old ? `Selected Git Executable version is lower than recommended: ${ver} < 2.40.0. Using this version of Git may produce unexpected results.` : !v.data?.ssh && v.data ? "OpenSSH not detected. Interacting with SSH remotes while not using an SSH agent might fail." : undefined}
      >
        <PathField title="Git executable" value={set.gitPath} placeholder="git on your PATH" onSave={(p) => put({ gitPath: p })} />
      </Row>
    </>
  );
}
