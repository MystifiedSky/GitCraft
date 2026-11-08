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
  worktrees: { path: string; branch: string }[];
  submodules: string[];
  remotes: string[];
  head_branch: string | null;
  head_sha: string;
  status: { unstaged: FileChange[]; staged: FileChange[]; conflicted: boolean };
  state_note: string | null;
  graph: { rows: GraphRow[]; width: number };
  undo: { undo: string | null; redo: string | null };
}

export type DiffSource = { kind: "Unstaged" } | { kind: "Staged" } | { kind: "Commit"; sha: string };

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
  | { type: "commit"; summary: string; description: string; amend: boolean; stageAll: boolean }
  | { type: "checkout"; target: string }
  | { type: "checkoutRemote"; remoteBranch: string }
  | { type: "createBranch"; name: string; at: string | null; checkout: boolean }
  | { type: "deleteBranch"; name: string }
  | { type: "deleteRemoteBranch"; remoteBranch: string }
  | { type: "renameBranch"; old: string; new: string }
  | { type: "createTag"; name: string; at: string | null; message: string | null }
  | { type: "deleteTag"; name: string }
  | { type: "fetch" }
  | { type: "pull"; mode: string }
  | { type: "push"; force: boolean }
  | { type: "stash"; message: string }
  | { type: "stashPop"; index: number }
  | { type: "stashApply"; index: number }
  | { type: "stashDrop"; index: number }
  | { type: "cherryPick"; sha: string }
  | { type: "revert"; sha: string }
  | { type: "reset"; sha: string; mode: string }
  | { type: "merge"; branch: string }
  | { type: "rebase"; onto: string }
  | { type: "abortMerge" }
  | { type: "abortRebase" }
  | { type: "continueRebase" }
  | { type: "addRemote"; name: string; url: string }
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
  auto_fetch_min: number;
  zoom_factors: number[];
  default_zoom: number;
}

export interface ActionResult {
  ok: boolean;
  message: string;
}

export const api = {
  launchOptions: () => invoke<{ screenshot: boolean; version: string; metrics: Metrics }>("launch_options"),
  openRepo: (path: string) => invoke<RepoView>("open_repo", { path }),
  commitFiles: (path: string, sha: string) => invoke<FileChange[]>("commit_files", { path, sha }),
  fileDiff: (path: string, source: DiffSource, file: string, untracked: boolean, parent: string | null) =>
    invoke<DiffLine[]>("file_diff", { path, source, file, untracked, parent }),
  run: (path: string, action: Action) => invoke<ActionResult>("run_action", { path, action }),
  activityLog: () => invoke<LogEntry[]>("activity_log"),
  sampleRepo: () => invoke<string>("sample_repo"),
  lastCommitMessage: (path: string) => invoke<[string, string]>("last_commit_message", { path }),
  pickFolder: (title: string) => invoke<string | null>("pick_folder", { title }),
  initRepo: (parent: string, name: string) => invoke<string>("init_repo", { parent, name }),
  cloneRepo: (url: string, parent: string) => invoke<string>("clone_repo", { url, parent }),
  openTerminal: (path: string) => invoke<void>("open_terminal", { path }),
  openFileManager: (path: string) => invoke<void>("open_file_manager", { path }),
  loadSettings: () => invoke<unknown>("load_settings"),
  saveSettings: (settings: unknown) => invoke<void>("save_settings", { settings }),
  watchRepo: (path: string) => invoke<void>("watch_repo", { path }),
  unwatchRepo: (path: string) => invoke<void>("unwatch_repo", { path }),
  screenshotReady: () => invoke<void>("screenshot_ready"),
};
