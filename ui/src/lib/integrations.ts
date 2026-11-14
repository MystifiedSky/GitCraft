// Preferences back end: integrations (all services), runtime Git settings, AI configuration, Git config,
// GPG and credential helpers. Settings that change how Git runs are pushed to Rust whenever they change.
import { invoke } from "@tauri-apps/api/core";
import { invokeAi } from "./ai-connection";
import { useQuery } from "@tanstack/react-query";
import { queryClient } from "./repo";
import { useStore, type AiSettings, type Settings } from "./store";

export interface ServiceStatus {
  id: string;
  label: string;
  host: string;
  connected: boolean;
  account: string | null;
  error: string | null;
}

export interface HostedRepo {
  name: string;
  full_name: string;
  description: string;
  private: boolean;
  https_url: string;
  ssh_url: string;
  updated: string;
}

export interface ServiceField {
  key: keyof Settings["integrations"];
  label: string;
  placeholder?: string;
  help?: string;
}

export interface ServiceInfo {
  id: string;
  label: string;
  kind: "hosting" | "issues";
  /** Browser sign-in through Git Credential Manager. */
  browser?: boolean;
  /** Can reuse the GitHub CLI's sign-in. */
  gh?: boolean;
  fields: ServiceField[];
  /** Where to create a token, from the integration settings. */
  tokenUrl: (i: Settings["integrations"]) => string;
  tokenHelp: string;
  /** Needs these settings before connecting. */
  needs?: (keyof Settings["integrations"])[];
}

const host = (h: string) => h.trim().replace(/^https?:\/\//, "").replace(/\/$/, "");

/** The supported integrations, in display order. */
export const SERVICES: ServiceInfo[] = [
  {
    id: "github",
    label: "GitHub",
    kind: "hosting",
    browser: true,
    gh: true,
    fields: [],
    tokenUrl: () => "https://github.com/settings/tokens/new?scopes=repo,read:org,workflow,write:public_key&description=GitCraft",
    tokenHelp: "A classic token with the repo scope (write:public_key lets GitCraft add SSH keys).",
  },
  {
    id: "github-enterprise",
    label: "GitHub Enterprise Server",
    kind: "hosting",
    fields: [{ key: "githubEnterpriseHost", label: "Host Domain", placeholder: "github.mycompany.com" }],
    needs: ["githubEnterpriseHost"],
    tokenUrl: (i) => `https://${host(i.githubEnterpriseHost)}/settings/tokens/new?scopes=repo,read:org,write:public_key&description=GitCraft`,
    tokenHelp: "A personal access token with the repo scope.",
  },
  {
    id: "gitlab",
    label: "GitLab",
    kind: "hosting",
    browser: true,
    fields: [],
    tokenUrl: () => "https://gitlab.com/-/user_settings/personal_access_tokens?name=GitCraft&scopes=api,read_user",
    tokenHelp: "A personal access token with the api and read_user scopes.",
  },
  {
    id: "gitlab-self",
    label: "GitLab Self-Managed",
    kind: "hosting",
    fields: [{ key: "gitlabHost", label: "Host Domain", placeholder: "gitlab.mycompany.com" }],
    needs: ["gitlabHost"],
    tokenUrl: (i) => `https://${host(i.gitlabHost)}/-/user_settings/personal_access_tokens?name=GitCraft&scopes=api,read_user`,
    tokenHelp: "A personal access token with the api and read_user scopes.",
  },
  {
    id: "bitbucket",
    label: "Bitbucket",
    kind: "hosting",
    browser: true,
    fields: [{ key: "bitbucketUser", label: "Username", placeholder: "only for an app password / API token", help: "Leave blank for an access token or a browser sign-in." }],
    tokenUrl: () => "https://bitbucket.org/account/settings/app-passwords/new",
    tokenHelp: "An app password (with your username) or an access token, with repository, pull request and account permissions.",
  },
  {
    id: "bitbucket-server",
    label: "Bitbucket Data Center",
    kind: "hosting",
    fields: [
      { key: "bitbucketServerHost", label: "Host Domain", placeholder: "bitbucket.mycompany.com" },
      { key: "bitbucketServerUser", label: "Username", placeholder: "for pushing over HTTPS" },
    ],
    needs: ["bitbucketServerHost"],
    tokenUrl: (i) => `https://${host(i.bitbucketServerHost)}/plugins/servlet/access-tokens/manage`,
    tokenHelp: "An HTTP access token with Project (Admin) and Repositories (Admin) permissions.",
  },
  {
    id: "azure",
    label: "Azure DevOps",
    kind: "hosting",
    browser: true,
    fields: [{ key: "azureOrg", label: "Azure Organization", placeholder: "mycompany" }],
    needs: ["azureOrg"],
    tokenUrl: (i) => `https://dev.azure.com/${i.azureOrg.trim()}/_usersSettings/tokens`,
    tokenHelp: "A personal access token with Code (read, write & manage), Work Items (read & write), Project and Team (read), Identity (read).",
  },
  {
    id: "jira",
    label: "Jira Cloud",
    kind: "issues",
    fields: [
      { key: "jiraSite", label: "Site", placeholder: "mycompany.atlassian.net" },
      { key: "jiraEmail", label: "Account email" },
    ],
    needs: ["jiraSite", "jiraEmail"],
    tokenUrl: () => "https://id.atlassian.com/manage-profile/security/api-tokens",
    tokenHelp: "An Atlassian API token for the email above.",
  },
  {
    id: "jira-server",
    label: "Jira Data Center",
    kind: "issues",
    fields: [{ key: "jiraServerHost", label: "Host Domain", placeholder: "jira.mycompany.com" }],
    needs: ["jiraServerHost"],
    tokenUrl: (i) => `https://${host(i.jiraServerHost)}/secure/ViewProfile.jspa`,
    tokenHelp: "A personal access token (Profile > Personal Access Tokens).",
  },
  {
    id: "trello",
    label: "Trello",
    kind: "issues",
    fields: [{ key: "trelloKey", label: "API key", placeholder: "from trello.com/power-ups/admin", help: "Create a Power-Up to get an API key." }],
    needs: ["trelloKey"],
    tokenUrl: (i) => `https://trello.com/1/authorize?expiration=never&scope=read,write&response_type=token&name=GitCraft&key=${i.trelloKey.trim()}`,
    tokenHelp: "Press the link to authorise GitCraft on Trello, then paste the token it shows.",
  },
];

export const integrations = {
  statuses: (ids: string[]) => invoke<ServiceStatus[]>("integration_statuses", { ids }),
  setToken: (id: string, token: string) => invoke<void>("set_secret", { name: `${id}-token`, value: token }),
  signIn: (id: string, method: "browser" | "gh") => invoke<string>("integration_sign_in", { id, method }),
  repos: (id: string) => invoke<HostedRepo[]>("integration_repos", { id }),
  addSshKey: (id: string, title: string, key: string) => invoke<void>("integration_add_ssh_key", { id, title, key }),
  sshPage: (id: string) => invoke<string>("integration_ssh_page", { id }),
  createIssue: (path: string | null, tracker: { kind: string; project: string } | null, title: string, body: string) =>
    invoke<{ url: string; number: number; key: string }>("create_issue", { path, gitlabHost: useStore.getState().settings.gitlabHost ?? "", tracker, title, body }),
};

export const tools = {
  versions: (gpgProgram: string) => invoke<{ git: string; git_path: string; ssh: string; gpg: string }>("tool_versions", { gpgProgram }),
  configGet: (path: string | null, scope: "global" | "local", keys: string[]) => invoke<Record<string, string>>("git_config_get", { path, scope, keys }),
  gpgGenerate: (program: string, name: string, email: string, passphrase: string) => invoke<string>("gpg_generate_key", { program, name, email, passphrase }),
  gpgExport: (program: string, key: string) => invoke<string>("gpg_export_key", { program, key }),
  forgetCredentials: (hosts: string[]) => invoke<number>("forget_credentials", { hosts }),
  deleteOrig: (path: string) => invoke<string[]>("delete_orig_files", { path }),
  pickFile: (title: string) => invoke<string | null>("pick_file", { title }),
};

export const aiApi = {
  test: () => invokeAi<string>("ai_test_connection"),
  listModels: (config: AiSettings) => invoke<AiModel[]>("ai_list_models", { config }),
  findProgram: (provider: AiSettings["provider"]) => invoke<{ path: string; version: string }>("ai_find_program", { provider }),
  pullRequest: (path: string, source: string, target: string) => invokeAi<[string, string]>("ai_pull_request", { path, source, target }),
  resolveConflict: (base: string, ours: string, theirs: string, file: string) => invokeAi<string>("ai_resolve_conflict", { base, ours, theirs, file }),
};

export interface AiModel {
  id: string;
  name: string;
}

/** Push the settings that change how the Rust side works (Git executable, SSH key, logging, services, AI). */
export function syncBackendSettings(st: Settings) {
  const encodings = Object.fromEntries(Object.entries(st.repoPrefs ?? {}).filter(([, p]) => p.encoding).map(([path, p]) => [path, p.encoding!]));
  void invoke("set_runtime_settings", { settings: { gitPath: st.gitPath, sshUseAgent: st.sshUseAgent, sshPrivateKey: st.sshPrivateKey, extendedLog: st.extendedLog, encodings } }).catch(() => undefined);
  void invoke("set_integration_config", { config: { ...st.integrations, gitlabHost: st.integrations.gitlabHost || st.gitlabHost } }).catch(() => undefined);
  void invoke("set_ai_config", { config: st.ai }).catch(() => undefined);
}

/** Keep the Rust side in step with the settings (called once at start-up). */
export function startBackendSync() {
  syncBackendSettings(useStore.getState().settings);
  useStore.subscribe((st, prev) => {
    const a = st.settings;
    const b = prev.settings;
    if (a.gitPath !== b.gitPath || a.sshUseAgent !== b.sshUseAgent || a.sshPrivateKey !== b.sshPrivateKey || a.extendedLog !== b.extendedLog || a.integrations !== b.integrations || a.ai !== b.ai || a.gitlabHost !== b.gitlabHost || a.repoPrefs !== b.repoPrefs)
      syncBackendSettings(a);
  });
}

/** Simple moment-style date formatting for the Date/Time preferences (YYYY, MMMM, MMM, MM, M, DD, D, dddd, ddd, HH, H, hh, h, mm, ss, A, a). */
export function formatDate(d: Date, fmt: string, locale = "en-US"): string {
  const month = (style: "long" | "short") => d.toLocaleString(locale, { month: style });
  const day = (style: "long" | "short") => d.toLocaleString(locale, { weekday: style });
  const h12 = d.getHours() % 12 || 12;
  const pad = (n: number) => String(n).padStart(2, "0");
  const tokens: Record<string, () => string> = {
    YYYY: () => String(d.getFullYear()),
    YY: () => String(d.getFullYear()).slice(-2),
    MMMM: () => month("long"),
    MMM: () => month("short"),
    MM: () => pad(d.getMonth() + 1),
    M: () => String(d.getMonth() + 1),
    dddd: () => day("long"),
    ddd: () => day("short"),
    DD: () => pad(d.getDate()),
    D: () => String(d.getDate()),
    HH: () => pad(d.getHours()),
    H: () => String(d.getHours()),
    hh: () => pad(h12),
    h: () => String(h12),
    mm: () => pad(d.getMinutes()),
    ss: () => pad(d.getSeconds()),
    A: () => (d.getHours() < 12 ? "AM" : "PM"),
    a: () => (d.getHours() < 12 ? "am" : "pm"),
  };
  const formatTokens = (part: string) => part.replace(/YYYY|YY|MMMM|MMM|MM|M|dddd|ddd|DD|D|HH|H|hh|h|mm|ss|A|a/g, (m) => tokens[m]());
  let result = "";
  let start = 0;
  // Bracketed text is literal. Stop looking for literals once an opening bracket has no closer.
  for (;;) {
    const open = fmt.indexOf("[", start);
    if (open === -1) break;
    const close = fmt.indexOf("]", open + 1);
    if (close === -1) break;
    result += formatTokens(fmt.slice(start, open)) + fmt.slice(open + 1, close);
    start = close + 1;
  }
  return result + formatTokens(fmt.slice(start));
}


/** Services with a saved sign-in. Shared by the status bar ("Select an integration" when none) and File > Sign Out. */
const SIGNED_IN_KEY = ["signedInServices"];
const fetchSignedIn = () =>
  integrations.statuses(SERVICES.filter((x) => x.kind === "hosting").map((x) => x.id)).then((all) => all.filter((x) => x.connected && !x.error));
export function useSignedInServices() {
  return useQuery({ queryKey: SIGNED_IN_KEY, queryFn: fetchSignedIn, staleTime: 5 * 60_000, retry: false });
}

/** Removes every saved service token (File > Sign Out). Returns how many were removed. */
export async function signOutAll(): Promise<number> {
  const connected = await queryClient.fetchQuery({ queryKey: SIGNED_IN_KEY, queryFn: fetchSignedIn, staleTime: 0 });
  await Promise.all(connected.map((x) => integrations.setToken(x.id, "")));
  queryClient.invalidateQueries({ queryKey: SIGNED_IN_KEY });
  queryClient.invalidateQueries({ queryKey: ["integrationStatuses"] });
  return connected.length;
}
