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
}

export type PatchOp = "stage" | "unstage" | "discard";

/** The original's four interactive rebase actions. */
export type Step = "pick" | "reword" | "squash" | "drop";

export interface TodoItem {
  action: Step;
  sha: string;
  message: string | null;
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
  | { type: "push"; force: boolean; lease?: boolean }
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
  openInEditor: (path: string) => invoke<void>("open_in_editor", { path }),
  rebaseCommits: (path: string, base: string) => invoke<Commit[]>("rebase_commits", { path, base }),
  conflictInfo: (path: string, file: string) => invoke<ConflictInfo>("conflict_info", { path, file }),
  fileDiff: (path: string, source: DiffSource, file: string, untracked: boolean, parent: string | null, ignoreWs = false) =>
    invoke<DiffLine[]>("file_diff", { path, source, file, untracked, parent, ignoreWs }),
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
