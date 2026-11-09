// GitHub / GitLab: pull requests, issues, Launchpad. Tokens stay in the OS credential store.
import { invoke } from "@tauri-apps/api/core";
import { act, repoData } from "./repo";
import { useStore } from "./store";

export interface RemoteRepo {
  provider: "github" | "gitlab";
  host: string;
  path: string;
  web: string;
}

export interface PullRequest {
  number: number;
  title: string;
  author: string;
  source: string;
  target: string;
  draft: boolean;
  state: string;
  url: string;
  updated: string;
  body: string;
  repo: string;
}

export interface Issue {
  number: number;
  title: string;
  author: string;
  state: string;
  url: string;
  labels: string[];
  updated: string;
  body: string;
  repo: string;
}

export interface Comment {
  author: string;
  body: string;
  created: string;
}

export interface HostingStatus {
  github: string | null;
  gitlab: string | null;
  github_error: string | null;
  gitlab_error: string | null;
  ai: boolean;
}

const gh = () => useStore.getState().settings.gitlabHost ?? "";

export const hosting = {
  status: () => invoke<HostingStatus>("hosting_status", { gitlabHost: gh() }),
  setSecret: (name: "github-token" | "gitlab-token" | "anthropic-key", value: string) => invoke<void>("set_secret", { name, value }),
  remote: (path: string) => invoke<RemoteRepo | null>("repo_remote", { path, gitlabHost: gh() }),
  prs: (path: string) => invoke<PullRequest[]>("list_prs", { path, gitlabHost: gh() }),
  issues: (path: string) => invoke<Issue[]>("list_issues", { path, gitlabHost: gh() }),
  comments: (path: string, number: number) => invoke<Comment[]>("pr_comments", { path, gitlabHost: gh(), number }),
  comment: (path: string, number: number, body: string) => invoke<void>("pr_comment", { path, gitlabHost: gh(), number, body }),
  create: (path: string, source: string, target: string, title: string, body: string, draft: boolean) =>
    invoke<PullRequest>("create_pr", { path, gitlabHost: gh(), source, target, title, body, draft }),
  merge: (path: string, number: number, method: string) => invoke<void>("merge_pr", { path, gitlabHost: gh(), number, method }),
  launchpad: () => invoke<{ pull_requests: PullRequest[]; issues: Issue[]; errors: string[] }>("launchpad", { gitlabHost: gh() }),
  issueBranch: (number: number, title: string) => invoke<string>("issue_branch_name", { number, title }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
};

/** "Start a pull request": push the branch if needed, then create the PR / MR. */
export function startPullRequest(path: string, branch: string, target?: string) {
  const s = useStore.getState();
  const repo = repoData(path);
  const ref = repo?.refs.find((r) => r.kind === "Local" && r.name === branch);
  const head = repo?.commits.find((c) => c.sha === ref?.sha);
  const remoteDefault = repo?.refs.find((r) => r.kind === "Remote" && /\/(main|master|develop)$/.test(r.name))?.name.split("/").slice(1).join("/");
  s.setDialog({
    kind: "form",
    title: `Start a pull request from ${branch}`,
    fields: [
      { key: "target", label: "Into branch", value: target ?? remoteDefault ?? "main" },
      { key: "title", label: "Title", value: head?.summary ?? branch },
      { key: "body", label: "Description", value: head?.body ?? "" },
      { key: "draft", label: "Create as a draft", value: false, type: "checkbox" },
    ],
    confirm: "Create Pull Request",
    help: "The branch is pushed first if the remote doesn't have it yet.",
    onSubmit: async (v) => {
      if (!ref?.upstream || (ref.ahead ?? 0) > 0) {
        const ok = await act(path, { type: "push", force: false }, true);
        if (!ok) return;
      }
      try {
        s.setBusy("Creating pull request…");
        const pr = await hosting.create(path, branch, String(v.target).trim(), String(v.title).trim(), String(v.body), !!v.draft);
        s.toast(true, `Created #${pr.number}: ${pr.title}`);
        const tab = s.tabs.find((t) => t.id === s.activeId);
        if (tab) s.setUi(tab.id, { pr: pr.number, issue: null, diff: null, history: null });
      } catch (e) {
        s.toast(false, String(e));
      } finally {
        s.setBusy(null);
      }
    },
  });
}

/** "Create branch from issue": issue-<n>-<title>, checked out. */
export async function branchFromIssue(path: string, issue: Issue) {
  const name = await hosting.issueBranch(issue.number, issue.title);
  useStore.getState().setDialog({
    kind: "prompt",
    title: `Create a branch for #${issue.number}`,
    label: "Branch name",
    value: name,
    confirm: "Create and Checkout",
    onSubmit: (v) => act(path, { type: "createBranch", name: v, at: null, checkout: true }),
  });
}
