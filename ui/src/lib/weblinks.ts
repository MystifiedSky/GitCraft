import type { MenuEntry } from "@/components/ui/menu";
import { api, type RepoView } from "./api";
import { invoke } from "@tauri-apps/api/core";
import { copy } from "./repo";
import { useStore } from "./store";

/** What a link points at on the hosting site. */
export type WebTarget = { kind: "repo" } | { kind: "commit"; sha: string } | { kind: "branch"; name: string } | { kind: "tag"; name: string };

/**
 * The hosting site's web address for a remote URL (`git@host:owner/repo.git`, `ssh://git@host:22/owner/repo.git`,
 * `https://user@host/owner/repo.git`, Azure's `git@ssh.dev.azure.com:v3/org/project/repo`), or null when it isn't one.
 */
export function webBase(remoteUrl: string): string | null {
  let url = remoteUrl.trim();
  if (!url) return null;
  // scp-like ssh: user@host:path
  const scp = /^[\w.-]+@([^:/]+):(?!\/)(.+)$/.exec(url);
  let host: string;
  let path: string;
  if (scp) {
    host = scp[1];
    path = scp[2];
  } else {
    const m = /^(?:ssh|git|https?|git\+ssh):\/\/(?:[^@/]+@)?([^/:]+)(?::\d+)?\/(.+)$/i.exec(url);
    if (!m) return null;
    host = m[1];
    path = m[2];
    if (/^https?:/i.test(url)) {
      // keep a non-default https port (self-hosted servers)
      const port = /^https?:\/\/(?:[^@/]+@)?[^/:]+(:\d+)\//i.exec(url)?.[1] ?? "";
      host += port;
    }
  }
  if (path.endsWith(".git/")) path = path.slice(0, -5);
  else if (path.endsWith(".git")) path = path.slice(0, -4);
  let end = path.length;
  while (end > 0 && path[end - 1] === "/") end--;
  path = path.slice(0, end);
  // Azure DevOps: ssh.dev.azure.com:v3/org/project/repo -> dev.azure.com/org/project/_git/repo
  if (/(^|\.)dev\.azure\.com$/i.test(host) || /visualstudio\.com$/i.test(host)) {
    const parts = path.replace(/^v3\//, "").split("/");
    if (path.startsWith("v3/") && parts.length >= 3) return `https://dev.azure.com/${parts[0]}/${parts[1]}/_git/${parts.slice(2).join("/")}`;
    return `https://${host.replace(/^ssh\./, "")}/${path}`;
  }
  host = host.replace(/^ssh\./, "");
  if (!path.includes("/")) return null;
  return `https://${host}/${path}`;
}

/** The page for a commit, branch or tag on the remote's site (GitHub style unless the host says otherwise). */
export function webLink(remoteUrl: string, target: WebTarget): string | null {
  const base = webBase(remoteUrl);
  if (!base) return null;
  const enc = (s: string) => s.split("/").map(encodeURIComponent).join("/");
  if (target.kind === "repo") return base;
  if (/dev\.azure\.com|visualstudio\.com/i.test(base)) {
    if (target.kind === "commit") return `${base}/commit/${target.sha}`;
    return `${base}?version=${target.kind === "branch" ? "GB" : "GT"}${encodeURIComponent(target.name)}`;
  }
  if (/gitlab/i.test(base)) {
    if (target.kind === "commit") return `${base}/-/commit/${target.sha}`;
    return target.kind === "branch" ? `${base}/-/tree/${enc(target.name)}` : `${base}/-/tags/${enc(target.name)}`;
  }
  if (/bitbucket/i.test(base)) {
    if (target.kind === "commit") return `${base}/commits/${target.sha}`;
    return `${base}/src/${enc(target.name)}`;
  }
  if (target.kind === "commit") return `${base}/commit/${target.sha}`;
  return target.kind === "branch" ? `${base}/tree/${enc(target.name)}` : `${base}/releases/tag/${enc(target.name)}`;
}

async function linkFor(path: string, remote: string, target: WebTarget): Promise<string | null> {
  const r = (await api.remotesList(path).catch(() => [])).find((x) => x.name === remote);
  const link = r ? webLink(r.fetch_url || r.push_url, target) : null;
  if (!link) useStore.getState().toast(false, `${remote} isn't on a website GitCraft can link to.`);
  return link;
}

export async function copyWebLink(path: string, remote: string, target: WebTarget) {
  const link = await linkFor(path, remote, target);
  if (link) copy(link);
}

export async function openWebLink(path: string, remote: string, target: WebTarget) {
  const link = await linkFor(path, remote, target);
  if (link) await invoke("open_url", { url: link }).catch((e) => useStore.getState().toast(false, String(e)));
}

/**
 * Menu entries for copying links: one remote gives "<label> on remote: <name>",
 * several give "<label>" with a submenu "On remote: <name>". No remote, no entry.
 */
export function linkItems(repo: RepoView, label: string, target: WebTarget, remotes: string[] = repo.remotes): MenuEntry[] {
  if (remotes.length === 0) return [];
  if (remotes.length === 1) return [{ label: `${label} on remote: ${remotes[0]}`, onSelect: () => copyWebLink(repo.path, remotes[0], target) }];
  return [{ label, children: remotes.map((r) => ({ label: `On remote: ${r}`, onSelect: () => copyWebLink(repo.path, r, target) })) }];
}
