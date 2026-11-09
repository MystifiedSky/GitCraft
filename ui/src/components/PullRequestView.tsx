import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ExternalLink, GitPullRequest, CircleDot, X } from "lucide-react";
import { useState } from "react";
import type { RepoView } from "@/lib/api";
import { useStore } from "@/lib/store";
import { act, confirmThen } from "@/lib/repo";
import { branchFromIssue, hosting } from "@/lib/hosting";
import { Button } from "./ui/button";
import { Avatar } from "./Icons";
import { timeSince } from "@/lib/utils";

function Since({ iso }: { iso: string }) {
  const units = useStore((s) => s.metrics.time_units);
  if (!iso) return null;
  const t = timeSince(Date.parse(iso) / 1000, units);
  return <span style={{ color: t.color }}>{t.text}</span>;
}

/** A pull request (merge request): description, comments, checkout, merge, comment. */
export function PullRequestView({ repo, tabId, number }: { repo: RepoView; tabId: string; number: number }) {
  const s = useStore();
  const qc = useQueryClient();
  const prs = useQuery({ queryKey: ["prs", repo.path], queryFn: () => hosting.prs(repo.path), staleTime: 60_000 });
  const pr = prs.data?.find((p) => p.number === number);
  const comments = useQuery({ queryKey: ["prComments", repo.path, number], queryFn: () => hosting.comments(repo.path, number) });
  const [text, setText] = useState("");
  const [method, setMethod] = useState("merge");
  const [busy, setBusy] = useState(false);
  const close = () => s.setUi(tabId, { pr: null });
  const remoteName = repo.remotes.includes("origin") ? "origin" : repo.remotes[0];

  const send = async () => {
    if (!text.trim()) return;
    setBusy(true);
    try {
      await hosting.comment(repo.path, number, text.trim());
      setText("");
      comments.refetch();
    } catch (e) {
      s.toast(false, String(e));
    } finally {
      setBusy(false);
    }
  };
  const merge = () =>
    confirmThen(`Merge #${number}?`, `This merges ${pr?.source} into ${pr?.target} on the server (${method}).`, "Merge", async () => {
      try {
        await hosting.merge(repo.path, number, method);
        s.toast(true, `Merged #${number}`);
        qc.invalidateQueries({ queryKey: ["prs", repo.path] });
        act(repo.path, { type: "fetch" }, true);
        close();
      } catch (e) {
        s.toast(false, String(e));
      }
    });

  return (
    <div className="h-full flex flex-col bg-app min-w-0">
      <div className="flex items-center gap-2 h-[32px] pl-3 pr-2 bg-toolbar/60 border-b border-black/30 shrink-0">
        <GitPullRequest size={14} className="text-good" />
        <span className="text-[13px] text-white font-semibold truncate flex-1">
          #{number} {pr?.title ?? ""}
        </span>
        {pr && (
          <button className="text-fg2 hover:text-white p-1" title="Open in browser" onClick={() => hosting.openUrl(pr.url)}>
            <ExternalLink size={14} />
          </button>
        )}
        <button onClick={close} className="text-fg2 hover:text-white p-1" title="Close (Esc)">
          <X size={16} />
        </button>
      </div>
      {!pr && <div className="p-4 text-fg3 text-[13px]">{prs.isLoading ? "Loading…" : prs.isError ? String(prs.error) : "This pull request is no longer open."}</div>}
      {pr && (
        <div className="flex-1 overflow-auto p-4">
          <div className="flex flex-wrap items-center gap-2 text-[12.5px] text-fg2 mb-3">
            <Avatar name={pr.author} email={pr.author} size={20} />
            <span className="text-white">{pr.author}</span> wants to merge
            <span className="mono px-1.5 rounded bg-white/10 text-white">{pr.source}</span> into
            <span className="mono px-1.5 rounded bg-white/10 text-white">{pr.target}</span>
            {pr.draft && <span className="px-1.5 rounded bg-white/10 text-fg">Draft</span>}
            <span>
              · updated <Since iso={pr.updated} />
            </span>
          </div>
          <div className="flex flex-wrap gap-2 mb-4">
            <Button size="sm" onClick={() => remoteName && act(repo.path, { type: "checkoutRemote", remoteBranch: `${remoteName}/${pr.source}` })}>
              Checkout {pr.source}
            </Button>
            <select value={method} onChange={(e) => setMethod(e.target.value)} className="h-7 px-1 rounded-[4px] bg-field border border-white/10 text-[12.5px] text-fg outline-none">
              <option value="merge">Merge commit</option>
              <option value="squash">Squash and merge</option>
              <option value="rebase">Rebase and merge</option>
            </select>
            <Button size="sm" variant="success" disabled={pr.draft} onClick={merge} title={pr.draft ? "Draft pull requests can't be merged" : ""}>
              Merge Pull Request
            </Button>
          </div>
          <div className="rounded border border-white/10 bg-panel p-3 text-[13px] text-fg whitespace-pre-wrap break-words mb-4 select-text">{pr.body || <span className="text-fg3">No description.</span>}</div>
          <div className="text-[12px] font-semibold tracking-wider text-fg2 mb-2">COMMENTS</div>
          {comments.isLoading && <div className="text-fg3 text-[12.5px]">Loading comments…</div>}
          {comments.data?.length === 0 && <div className="text-fg3 text-[12.5px] mb-2">No comments yet.</div>}
          {comments.data?.map((c, i) => (
            <div key={i} className="flex gap-2 mb-3">
              <Avatar name={c.author} email={c.author} size={24} />
              <div className="flex-1 min-w-0">
                <div className="text-[12px] text-fg2">
                  <span className="text-white">{c.author}</span> · <Since iso={c.created} />
                </div>
                <div className="text-[13px] text-fg whitespace-pre-wrap break-words select-text">{c.body}</div>
              </div>
            </div>
          ))}
          <textarea
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder="Leave a comment"
            className="w-full h-[80px] bg-field border border-white/10 rounded-[3px] px-2 py-1.5 text-[13px] text-white outline-none focus:border-accent resize-none"
          />
          <Button size="sm" variant="primary" className="mt-2" disabled={busy || !text.trim()} onClick={send}>
            Comment
          </Button>
        </div>
      )}
    </div>
  );
}

/** An issue: description, labels, create a branch for it. */
export function IssueView({ repo, tabId, number }: { repo: RepoView; tabId: string; number: number }) {
  const s = useStore();
  const issues = useQuery({ queryKey: ["issues", repo.path], queryFn: () => hosting.issues(repo.path), staleTime: 60_000 });
  const issue = issues.data?.find((i) => i.number === number);
  return (
    <div className="h-full flex flex-col bg-app min-w-0">
      <div className="flex items-center gap-2 h-[32px] pl-3 pr-2 bg-toolbar/60 border-b border-black/30 shrink-0">
        <CircleDot size={14} className="text-good" />
        <span className="text-[13px] text-white font-semibold truncate flex-1">
          #{number} {issue?.title ?? ""}
        </span>
        {issue && (
          <button className="text-fg2 hover:text-white p-1" title="Open in browser" onClick={() => hosting.openUrl(issue.url)}>
            <ExternalLink size={14} />
          </button>
        )}
        <button onClick={() => s.setUi(tabId, { issue: null })} className="text-fg2 hover:text-white p-1" title="Close (Esc)">
          <X size={16} />
        </button>
      </div>
      {!issue && <div className="p-4 text-fg3 text-[13px]">{issues.isLoading ? "Loading…" : "This issue is no longer open."}</div>}
      {issue && (
        <div className="flex-1 overflow-auto p-4">
          <div className="flex flex-wrap items-center gap-2 text-[12.5px] text-fg2 mb-3">
            opened by <span className="text-white">{issue.author}</span>
            {issue.labels.map((l) => (
              <span key={l} className="px-1.5 rounded bg-accent/20 text-white text-[11.5px]">
                {l}
              </span>
            ))}
          </div>
          <Button size="sm" variant="primary" className="mb-4" onClick={() => branchFromIssue(repo.path, issue)}>
            Create branch for this issue
          </Button>
          <div className="rounded border border-white/10 bg-panel p-3 text-[13px] text-fg whitespace-pre-wrap break-words select-text">{issue.body || <span className="text-fg3">No description.</span>}</div>
        </div>
      )}
    </div>
  );
}
