import { useStore } from "./store";
import { invoke } from "@tauri-apps/api/core";

export type RefKind = "Local" | "Remote" | "Tag";

export interface Commit {
  sha: string;
  parents: string[];
  author: string;
  email: string;
  time: number;
  summary: string;
  body: string;
}

export interface GitRef {
  name: string;
  full: string;
  sha: string;
  kind: RefKind;
  upstream: string;
  ahead: number;
  behind: number;
}

export interface Stash {
  index: number;
  sha: string;
  message: string;
}

export interface FileChange {
  path: string;
  status: string; // M A D R ? U
}

export interface Seg {
  from: number;
  to: number;
  color: number;
}

export interface GraphRow {
  col: number;
  color: number;
  up: Seg[];
  down: Seg[];
  merge: boolean;
}

export interface RepoView {
  path: string;
  name: string;
  commits: Commit[];
  refs: GitRef[];
  stashes: Stash[];
  worktrees: { path: string; branch: string; locked: boolean }[];
  submodules: string[];
  remotes: string[];
  head_branch: string | null;
  head_sha: string;
  status: { unstaged: FileChange[]; staged: FileChange[]; conflicted: boolean };
  state_note: string | null;
  has_more: boolean;
  merge_msg: string | null;
  conflict_labels: [string, string] | null;
  graph: { rows: GraphRow[]; width: number };
  undo: { undo: string | null; redo: string | null };
}

export type DiffSource =
  | { kind: "Unstaged" }
  | { kind: "Staged" }
  | { kind: "Commit"; sha: string }
  /** Between two points; the second may be WORKDIR (the working folder). */
  | { kind: "Range"; sha: [string, string] };

export const WORKDIR = "WORKDIR";

export interface FileVersions {
  old: string | null;
  new: string | null;
  binary: boolean;
  old_image: string | null;
  new_image: string | null;
  encoding: string;
  editable: boolean;
}

export interface HistoryEntry extends Commit {
  path: string;
  status: string;
}

export interface BlameLine {
  sha: string;
  author: string;
  time: number;
  summary: string;
  line: number;
  text: string;
}

export interface Stats {
  sha: string;
  files: number;
  added: number;
  deleted: number;
}

export interface RefView {
  hidden: string[];
  solo: string[];
  pinned?: string | null;
  smart?: boolean;
}

export type PatchOp = "stage" | "unstage" | "discard";

/** The four interactive rebase actions. */
export type Step = "pick" | "reword" | "squash" | "drop";

export interface TodoItem {
  action: Step;
  sha: string;
  message: string | null;
}

export interface AccountInfo {
  name: string;
  email: string;
  repo_name: string;
  repo_email: string;
  credential_helper: string;
  gcm: boolean;
  ssh_dir: string;
  ssh_keys: { path: string; kind: string; public: string }[];
  ssh_keygen: boolean;
  gpg: boolean;
  gpg_keys: { id: string; uid: string }[];
  signing_key: string;
  sign_commits: boolean;
  signing_format: string;
}

export interface JobProgress {
  id: string;
  text: string;
  percent: number | null;
}

export interface LfsInfo {
  available: boolean;
  version: string;
  enabled: boolean;
  patterns: string[];
  locks: { id: string; path: string; owner: string }[];
}

export interface InitOptions {
  branch: string;
  gitignore: string[];
  license: string;
  readme: boolean;
}

export interface GitflowConfig {
  master: string;
  develop: string;
  feature: string;
  release: string;
  hotfix: string;
  versiontag: string;
  initialized: boolean;
}

export interface CleanupBranch {
  name: string;
  time: number;
  upstream: string;
  merged: boolean;
}

export interface RemoteInfo {
  name: string;
  fetch_url: string;
  push_url: string;
}

export interface SubmoduleInfo {
  path: string;
  sha: string;
  state: "in-sync" | "out-of-sync" | "uninitialized" | "conflict";
  url: string;
}

export interface ConflictInfo {
  working: string | null;
  base: string | null;
  ours: string | null;
  theirs: string | null;
  ours_label: string;
  theirs_label: string;
  binary: boolean;
}

export interface DiffLine {
  kind: "Context" | "Added" | "Removed" | "Hunk" | "Meta";
  old_no: number | null;
  new_no: number | null;
  text: string;
}

export interface LogEntry {
  time: string;
  command: string;
  ok: boolean;
  output: string;
}

export type Action =
  | { type: "stage"; paths: string[] }
  | { type: "unstage"; paths: string[] }
  | { type: "stageAll" }
  | { type: "unstageAll" }
  | { type: "discard"; paths: string[] }
  | { type: "discardAll" }
  | { type: "commit"; summary: string; description: string; amend: boolean; stageAll: boolean; skipHooks?: boolean; stripComments?: boolean }
  | { type: "checkout"; target: string }
  | { type: "checkoutRemote"; remoteBranch: string }
  | { type: "createBranch"; name: string; at: string | null; checkout: boolean }
  | { type: "deleteBranch"; name: string }
  | { type: "deleteRemoteBranch"; remoteBranch: string }
  | { type: "renameBranch"; old: string; new: string }
  | { type: "createTag"; name: string; at: string | null; message: string | null }
  | { type: "deleteTag"; name: string }
  | { type: "pushTag"; name: string; remote: string }
  | { type: "deleteRemoteTag"; name: string; remote: string }
  | { type: "fetch" }
  | { type: "pull"; mode: string }
  | { type: "push"; force: boolean; lease?: boolean }
  | { type: "pushBranch"; branch: string }
  | { type: "skipRebase" }
  | { type: "interactiveRebase"; base: string; items: TodoItem[] }
  | { type: "interactiveCherryPick"; items: TodoItem[] }
  | { type: "takeSide"; paths: string[]; side: "ours" | "theirs" }
  | { type: "markResolved"; paths: string[] }
  | { type: "markConflicted"; paths: string[] }
  | { type: "saveResolved"; file: string; content: string }
  | { type: "fastForward"; branch: string; to: string }
  | { type: "setUpstream"; branch: string; upstream: string }
  | { type: "stashRename"; index: number; message: string }
  | { type: "autoFetch"; prune: boolean }
  | { type: "fetchRemote"; remote: string }
  | { type: "remoteEdit"; name: string; newName: string; url: string; pushUrl: string }
  | { type: "remoteRemove"; name: string }
  | { type: "worktreeAdd"; path: string; branch: string; newBranch: boolean; start: string | null }
  | { type: "worktreeRemove"; path: string; force: boolean }
  | { type: "worktreeLock"; path: string; message: string }
  | { type: "worktreeUnlock"; path: string }
  | { type: "submoduleAdd"; url: string; path: string }
  | { type: "submoduleUpdate"; paths: string[] }
  | { type: "submoduleRemove"; path: string }
  | { type: "deleteBranches"; names: string[]; remotes: string[] }
  | { type: "gitflowInit"; config: GitflowConfig }
  | { type: "lfsInstall" }
  | { type: "composeCommits"; commits: { summary: string; description: string; files: string[] }[] }
  | { type: "applyPatchFile"; file: string }
  | { type: "stashFiles"; paths: string[]; message: string }
  | { type: "stashTakeFile"; index: number; path: string }
  | { type: "revertHunk"; sha: string; parent: string | null; file: string; hunk: number; lines: number[] | null }
  | { type: "mergeSquash"; branch: string }
  | { type: "externalDiff"; file: string; staged: boolean; tool: string }
  | { type: "externalMerge"; file: string; tool: string }
  | { type: "lfsTrack"; pattern: string }
  | { type: "lfsUntrack"; pattern: string }
  | { type: "lfsPull" }
  | { type: "lfsLock"; path: string }
  | { type: "lfsUnlock"; path: string; force: boolean }
  | { type: "ignore"; pattern: string; untrack: string[] }
  | { type: "sparseSet"; patterns: string[] }
  | { type: "sparseDisable" }
  | { type: "gitflowStart"; kind: string; name: string }
  | { type: "gitflowFinish"; kind: string; name: string; keep: boolean; tagMessage: string }
  | { type: "stash"; message: string }
  | { type: "stashPop"; index: number }
  | { type: "stashApply"; index: number }
  | { type: "stashDrop"; index: number }
  | { type: "cherryPick"; sha: string; mainline?: number }
  | { type: "revert"; sha: string; mainline?: number }
  | { type: "reset"; sha: string; mode: string }
  | { type: "merge"; branch: string }
  | { type: "rebase"; onto: string }
  | { type: "abortMerge" }
  | { type: "abortRebase" }
  | { type: "continueRebase" }
  | { type: "addRemote"; name: string; url: string }
  | { type: "patch"; file: string; untracked: boolean; op: PatchOp; hunk: number; lines: number[] | null }
  | { type: "cherryPickMany"; shas: string[] }
  | { type: "undo" }
  | { type: "redo" };

/** Measured layout values, sent by the Rust side (src/metrics.rs). */
export interface Metrics {
  node: number;
  avatar_inner: number;
  lane: number;
  gutter: number;
  pad: number;
  graph_zone: number;
  ref_zone: number;
  ref_zone_min: number;
  ref_margin_left: number;
  ref_margin_right: number;
  ref_max: number;
  message_zone: number;
  message_min: number;
  summary_margin: number;
  header: number;
  row: number;
  row_inner: number;
  arc_radius: number;
  min_alpha: number;
  sha_len: number;
  avatar_bg: string;
  summary_counter: number;
  toolbar_left_max: number;
  toolbar_center: number;
  /** Tab slide time when tabs make room for a dragged tab (ms). */
  tab_slide_ms: number;
  /** Width the "// WIP" box leaves for the change counts (px). */
  wip_stats_space: number;
  auto_fetch_min: number;
  zoom_factors: number[];
  default_zoom: number;
  lazy_offset: number;
  conflict_a: string;
  conflict_b: string;
  conflict_output: string;
  time_units: [number, string, string][];
  initial_commits: number;
  author_zone: number;
  date_zone: number;
  sha_zone: number;
  changes_zone: number;
  stats_added: string;
  stats_deleted: string;
  stats_files: string;
}

export interface ActionResult {
  ok: boolean;
  message: string;
}

export const api = {
  launchOptions: () => invoke<{ screenshot: boolean; shot_view: string | null; version: string; metrics: Metrics }>("launch_options"),
  repoStatus: (path: string) => invoke<RepoView["status"]>("repo_status", { path }),
  openRepo: (path: string, max?: number, view?: RefView) => invoke<RepoView>("open_repo", { path, max: max ?? null, view: view ?? null }),
  commitFiles: (path: string, sha: string) => invoke<FileChange[]>("commit_files", { path, sha }),
  fileVersions: (path: string, source: DiffSource, file: string, untracked: boolean, parent: string | null) =>
    invoke<FileVersions>("file_versions", { path, source, file, untracked, parent }),
  saveFile: (path: string, file: string, content: string) => invoke<void>("save_file", { path, file, content }),
  fileHistory: (path: string, file: string) => invoke<HistoryEntry[]>("file_history", { path, file }),
  blame: (path: string, file: string, rev: string | null) => invoke<BlameLine[]>("blame", { path, file, rev }),
  commitStats: (path: string, shas: string[]) => invoke<Stats[]>("commit_stats", { path, shas }),
  rangeFiles: (path: string, from: string, to: string) => invoke<FileChange[]>("range_files", { path, from, to }),
  trackedFiles: (path: string) => invoke<string[]>("tracked_files", { path }),
  /** Preferences > External Tools > Editor. */
  openInEditor: (path: string, file?: string) => {
    const t = useStore.getState().settings;
    return invoke<void>("open_in_editor", { path, file: file ?? null, tool: t.externalEditor, exe: t.editorPath, fileArgs: t.editorFileArgs, repoArgs: t.editorRepoArgs });
  },
  accountInfo: (path: string | null) => invoke<AccountInfo>("account_info", { path }),
  setGitConfig: (path: string | null, scope: "global" | "local", key: string, value: string) => invoke<void>("set_git_config", { path, scope, key, value }),
  generateSshKey: (email: string, name: string) => invoke<string>("generate_ssh_key", { email, name }),
  cancelJob: (id: string) => invoke<boolean>("cancel_job", { id }),
  lfsInfo: (path: string) => invoke<LfsInfo>("lfs_info", { path }),
  sparseInfo: (path: string) => invoke<{ enabled: boolean; patterns: string[] }>("sparse_info", { path }),
  gitignoreTemplates: () => invoke<string[]>("gitignore_templates"),
  gitflowConfig: (path: string) => invoke<GitflowConfig>("gitflow_config", { path }),
  cleanupList: (path: string) => invoke<CleanupBranch[]>("cleanup_list", { path }),
  remotesList: (path: string) => invoke<RemoteInfo[]>("remotes_list", { path }),
  submodulesList: (path: string) => invoke<SubmoduleInfo[]>("submodules_list", { path }),
  rebaseCommits: (path: string, base: string) => invoke<Commit[]>("rebase_commits", { path, base }),
  conflictInfo: (path: string, file: string) => invoke<ConflictInfo>("conflict_info", { path, file }),
  fileDiff: (path: string, source: DiffSource, file: string, untracked: boolean, parent: string | null, ignoreWs = false) =>
    invoke<DiffLine[]>("file_diff", { path, source, file, untracked, parent, ignoreWs }),
  run: (path: string, action: Action) => invoke<ActionResult>("run_action", { path, action }),
  activityLog: () => invoke<LogEntry[]>("activity_log"),
  sampleRepo: () => invoke<string>("sample_repo"),
  conflictSample: () => invoke<string>("conflict_sample"),
  lastCommitMessage: (path: string) => invoke<[string, string]>("last_commit_message", { path }),
  pickFolder: (title: string) => invoke<string | null>("pick_folder", { title }),
  initRepo: (parent: string, name: string, options?: InitOptions) => invoke<string>("init_repo", { parent, name, options: options ?? null }),
  cloneRepo: (url: string, parent: string, depth?: number) => invoke<string>("clone_repo", { url, parent, depth: depth ?? null }),
  /** Preferences > External Tools > Terminal; General > Use In-App Terminal Instead opens the built-in one. */
  openTerminal: (path: string) => {
    const t = useStore.getState().settings;
    if (t.useInAppTerminal) {
      useStore.getState().setSettings({ showTerminal: true });
      return Promise.resolve();
    }
    return invoke<void>("open_terminal", { path, tool: t.externalTerminal, exe: t.terminalPath, args: t.terminalArgs });
  },
  openFileManager: (path: string) => invoke<void>("open_file_manager", { path }),
  /** Open a repository file with the system's usual app for it. */
  openDefault: (path: string, file: string) => invoke<void>("open_default", { path, file }),
  /** Show a repository file selected in the file manager. */
  revealFile: (path: string, file: string) => invoke<void>("reveal_file", { path, file }),
  /** A file's text from the working folder, or as it was in `rev`. */
  fileText: (path: string, file: string, rev?: string) => invoke<string>("file_text", { path, file, rev: rev ?? null }),
  loadSettings: () => invoke<unknown>("load_settings"),
  saveSettings: (settings: unknown) => invoke<void>("save_settings", { settings }),
  watchRepo: (path: string) => invoke<void>("watch_repo", { path }),
  unwatchRepo: (path: string) => invoke<void>("unwatch_repo", { path }),
  screenshotReady: () => invoke<void>("screenshot_ready"),
};
