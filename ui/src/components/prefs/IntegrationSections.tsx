import { signingOptions } from "@/lib/signing-keys";
import { invoke } from "@tauri-apps/api/core";
import { sameFolder } from "@/lib/agents";
import { useQuery } from "@tanstack/react-query";
import { CheckCircle2, CircleDashed, ExternalLink, AlertTriangle } from "lucide-react";
import { useRef, useState } from "react";
import { api } from "@/lib/api";
import { hosting } from "@/lib/hosting";
import { useStore, DEFAULT_AI, type AiSettings, type Settings } from "@/lib/store";
import { SERVICES, aiApi, integrations, tools, type AiModel, type ServiceInfo } from "@/lib/integrations";
import { cn } from "@/lib/utils";
import { Area, Btn, H, Note, PathField, Row, Select, Text, Toggle, field } from "./controls";

const useSet = () => {
  const s = useStore();
  return { set: s.settings, put: (p: Partial<Settings>) => s.setSettings(p), toast: s.toast };
};

export function IntegrationsSection({ initial }: { initial?: string }) {
  const { set, put, toast } = useSet();
  const [sel, setSel] = useState(initial ?? "github");
  const statuses = useQuery({ queryKey: ["integrationStatuses", set.integrations], queryFn: () => integrations.statuses(SERVICES.map((x) => x.id)) });
  const st = (id: string) => statuses.data?.find((x) => x.id === id);
  const svc = SERVICES.find((x) => x.id === sel) ?? SERVICES[0];
  return (
    <>
      <H>Integrations</H>
      <Note>
        Connect your services to see pull requests and issues in the left panel and the Launchpad, clone your repositories, create pull requests, add SSH keys, and let Git push and pull over
        HTTPS with the same sign-in. Tokens are kept in the system's credential store.
      </Note>
      <div className="flex gap-4 min-h-[420px]">
        <div className="w-[230px] shrink-0 rounded border border-white/8 bg-white/[0.02] py-1 self-start">
          {(["hosting", "issues"] as const).map((kind) => (
            <div key={kind}>
              <div className="px-3 pt-2 pb-1 text-[11px] font-semibold tracking-wider text-fg3">{kind === "hosting" ? "HOSTING SERVICES" : "ISSUE TRACKERS"}</div>
              {SERVICES.filter((x) => x.kind === kind).map((x) => {
                const s2 = st(x.id);
                return (
                  <button key={x.id} onClick={() => setSel(x.id)} className={cn("w-full flex items-center gap-2 px-3 h-[30px] text-[13px] text-left", sel === x.id ? "bg-selrow text-white" : "text-fg hover:bg-hoverrow")}>
                    {s2?.connected ? s2.error ? <AlertTriangle size={13} className="text-warn" /> : <CheckCircle2 size={13} className="text-good" /> : <CircleDashed size={13} className="text-fg3" />}
                    <span className="truncate">{x.label}</span>
                  </button>
                );
              })}
            </div>
          ))}
        </div>
        <div className="flex-1 min-w-0">
          <ServicePane key={svc.id} svc={svc} status={st(svc.id)} loading={statuses.isLoading} onChanged={() => statuses.refetch()} set={set} put={put} toast={toast} />
        </div>
      </div>
    </>
  );
}

function ServicePane({
  svc,
  status,
  loading,
  onChanged,
  set,
  put,
  toast,
}: {
  svc: ServiceInfo;
  status: { connected: boolean; account: string | null; error: string | null } | undefined;
  loading: boolean;
  onChanged: () => void;
  set: Settings;
  put: (p: Partial<Settings>) => void;
  toast: (ok: boolean, text: string) => void;
}) {
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const acc = useQuery({ queryKey: ["accounts", null], queryFn: () => api.accountInfo(null) });
  const ready = (svc.needs ?? []).every((k) => set.integrations[k].trim());
  const run = async (what: string, f: () => Promise<unknown>, ok: string) => {
    setBusy(what);
    try {
      await f();
      toast(true, ok);
      onChanged();
    } catch (e) {
      toast(false, String(e));
    } finally {
      setBusy(null);
    }
  };
  const selectedPublic = useQuery({ queryKey: ["sshPublicKey", set.sshPublicKey], queryFn: () => invoke<string>("read_ssh_public_key", { path: set.sshPublicKey }), enabled: !!set.sshPublicKey });
  const pubKey = () => {
    const keys = acc.data?.ssh_keys ?? [];
    if (set.sshPublicKey) {
      const chosen = keys.find(k => sameFolder(k.path.replace(/\.pub$/, ""), set.sshPublicKey.replace(/\.pub$/, "")));
      return selectedPublic.data ? { path: set.sshPublicKey, public: selectedPublic.data } : chosen;
    }
    return keys.find(k => /id_ed25519/.test(k.path)) ?? keys[0];
  };
  return (
    <div>
      <div className="flex items-center gap-2 mb-3">
        <div className="text-[16px] font-semibold text-white">{svc.label}</div>
        <div className="flex-1" />
        {loading ? (
          <span className="text-[12px] text-fg3">Checking…</span>
        ) : status?.connected ? (
          status.error ? <span className="text-[12px] text-warn">{status.error}</span> : <span className="text-[12px] text-good">Connected{status.account ? ` as ${status.account}` : ""}</span>
        ) : (
          <span className="text-[12px] text-fg3">{svc.label} is not connected</span>
        )}
      </div>
      {svc.fields.map((f) => (
        <Row key={f.key} label={f.label} help={f.help}>
          <Text value={set.integrations[f.key]} placeholder={f.placeholder} onSave={(v) => put({ integrations: { ...set.integrations, [f.key]: v.trim() }, ...(f.key === "gitlabHost" ? { gitlabHost: v.trim() } : {}) })} />
        </Row>
      ))}
      {!status?.connected && (svc.browser || svc.gh) && (
        <Row label="Connect" help={svc.browser ? "Opens your browser to sign in (through Git Credential Manager, which Git uses too)." : undefined}>
          {svc.browser && (
            <Btn primary disabled={!!busy || !ready} onClick={() => run("browser", () => integrations.signIn(svc.id, "browser"), `Connected to ${svc.label}`)}>
              {busy === "browser" ? "Waiting for the browser…" : `Connect ${svc.label}`}
            </Btn>
          )}
          {svc.gh && (
            <Btn disabled={!!busy} onClick={() => run("gh", () => integrations.signIn(svc.id, "gh"), `Connected to ${svc.label} with the GitHub CLI sign-in`)}>
              Use GitHub CLI sign-in
            </Btn>
          )}
        </Row>
      )}
      <Row
        label={status?.connected ? "Replace token" : "Personal Access Token"}
        help={
          <>
            {svc.tokenHelp}{" "}
            <button className="text-link hover:underline inline-flex items-center gap-1" disabled={!ready} onClick={() => hosting.openUrl(svc.tokenUrl(set.integrations))}>
              Generate a token here. <ExternalLink size={11} />
            </button>
          </>
        }
      >
        <input type="password" className={cn(field, "flex-1")} placeholder="paste token" value={token} onChange={(e) => setToken(e.target.value)} />
        <Btn
          disabled={!token.trim() || !ready || !!busy}
          onClick={() =>
            run("token", async () => {
              await integrations.setToken(svc.id, token.trim());
              setToken("");
            }, "Token saved")
          }
        >
          Save Token
        </Btn>
      </Row>
      {status?.connected && (
        <Row label="Connection">
          <Btn danger onClick={() => run("disconnect", () => integrations.setToken(svc.id, ""), `Disconnected from ${svc.label}`)}>
            Disconnect
          </Btn>
        </Row>
      )}
      {svc.kind === "hosting" && (
        <Row
          label="SSH Key"
          help={pubKey() ? `This computer's key: ${pubKey()!.path}` : "No SSH settings detected. You can generate an SSH key in Preferences > SSH."}
        >
          <Btn
            disabled={!status?.connected || !pubKey() || !!busy}
            title={svc.id === "azure" ? "Azure DevOps doesn't let other apps add keys" : undefined}
            onClick={() => run("ssh", () => integrations.addSshKey(svc.id, `GitCraft (${navigator.platform})`, pubKey()!.public), `SSH Key added to ${svc.label}!`)}
          >
            Add key to {svc.label}
          </Btn>
          <Btn
            disabled={!pubKey() || !ready}
            onClick={async () => {
              await navigator.clipboard.writeText(pubKey()!.public).catch(() => undefined);
              hosting.openUrl(await integrations.sshPage(svc.id));
              toast(true, "Public key copied - paste it on the page that opened");
            }}
          >
            Copy public key and open {svc.label}
          </Btn>
        </Row>
      )}
    </div>
  );
}

export function SigningSection() {
  const s = useStore();
  const path = s.tabs.find((t) => t.id === s.activeId)?.path ?? null;
  const keys = ["gpg.program", "gpg.format", "user.signingkey", "commit.gpgsign", "tag.gpgSign", "gpg.ssh.program", "gpg.ssh.allowedSignersFile"];
  const cfg = useQuery({ queryKey: ["gpgConfig"], queryFn: () => tools.configGet(null, "global", keys) });
  const acc = useQuery({ queryKey: ["accounts", null], queryFn: () => api.accountInfo(null) });
  const local = useQuery({ queryKey: ["gpgLocal", path], queryFn: () => tools.configGet(path, "local", ["gpg.ssh.allowedSignersFile"]), enabled: !!path });
  const [pass, setPass] = useState("");
  const [busy, setBusy] = useState(false);
  const c = cfg.data ?? {};
  const save = async (k: string, v: string) => {
    await api.setGitConfig(null, "global", k, v);
    cfg.refetch();
    await acc.refetch();
  };
  const ssh = c["gpg.format"] === "ssh";
  const program = c["gpg.program"] || "gpg";
  return (
    <>
      <H>Commit Signing</H>
      <Row label="GPG Format">
        <Select value={ssh ? "ssh" : "openpgp"} onChange={(v) => save("gpg.format", v === "ssh" ? "ssh" : "")} options={[["openpgp", "OPENPGP"], ["ssh", "SSH"]]} />
      </Row>
      {!ssh && (
        <>
          <Row label="GPG Program" help={'This is usually "gpg" or "gpg2" depending on the version you are using.\nYou can also browse for the path directly.'} warning={!acc.data?.gpg ? "Select a GPG program to sign with." : undefined}>
            <PathField title="Path to gpg" value={c["gpg.program"] ?? ""} placeholder="gpg" onSave={(v) => save("gpg.program", v)} />
          </Row>
          <Row label="Signing Key" warning={!c["user.signingkey"] ? "A key is required for GPG signing." : undefined}>
            <Select
              value={c["user.signingkey"] ?? ""}
              onChange={(v) => save("user.signingkey", v)}
              options={signingOptions(acc.data?.gpg_keys ?? [], c["user.signingkey"] ?? "")}
            />
            <Btn onClick={() => acc.refetch()}>Reload GPG keys</Btn>
          </Row>
          <Row label="Copy GPG Public Key">
            <Btn
              disabled={!c["user.signingkey"]}
              onClick={async () => {
                try {
                  await navigator.clipboard.writeText(await tools.gpgExport(program, c["user.signingkey"]));
                  s.toast(true, "Public GPG key copied successfully");
                } catch (e) {
                  s.toast(false, String(e));
                }
              }}
            >
              Copy
            </Btn>
          </Row>
          <Row label="Generate new GPG Key" help={"This will create a 4096 bit key with the name and email in your current profile that expires in 2 years.\nThis will also be set as your active signing key."}>
            <input type="password" className={cn(field, "w-[200px]")} placeholder="Passphrase (optional)" value={pass} onChange={(e) => setPass(e.target.value)} />
            <Btn
              disabled={busy || !acc.data?.gpg}
              onClick={async () => {
                setBusy(true);
                try {
                  const fpr = await tools.gpgGenerate(program, acc.data?.name ?? "", acc.data?.email ?? "", pass);
                  await save("user.signingkey", fpr);
                  setPass("");
                  s.toast(true, "GPG Key Created Successfully");
                  acc.refetch();
                } catch (e) {
                  s.toast(false, String(e));
                } finally {
                  setBusy(false);
                }
              }}
            >
              {busy ? "Generating…" : "Generate"}
            </Btn>
          </Row>
        </>
      )}
      {ssh && (
        <>
          <Row label="Signing Key" help="The private key will be used for signing (or the SSH agent, for a public key or key::)." warning={!c["user.signingkey"] ? "A key is required for SSH signing." : undefined}>
            <PathField title="Path to signing Key" value={c["user.signingkey"] ?? ""} placeholder="~/.ssh/id_ed25519.pub" onSave={(v) => save("user.signingkey", v)} />
          </Row>
          <Row label="GPG SSH Program" help="Usually empty (ssh-keygen).">
            <PathField title="Path to GPG SSH program" value={c["gpg.ssh.program"] ?? ""} onSave={(v) => save("gpg.ssh.program", v)} />
          </Row>
          <Row
            label="Allowed Signers File"
            help="File used for SSH signature verification. If empty, it will use the global git configuration value."
            warning={local.data?.["gpg.ssh.allowedSignersFile"] ? `A local 'gpg.ssh.allowedSignersFile' config value in this repository is overriding this setting. Local value: ${local.data["gpg.ssh.allowedSignersFile"]}` : undefined}
          >
            <PathField title="Path to allowed signers file" value={c["gpg.ssh.allowedSignersFile"] ?? ""} onSave={(v) => save("gpg.ssh.allowedSignersFile", v)} />
          </Row>
        </>
      )}
      <Row label="Sign Commits by Default">
        <Toggle value={c["commit.gpgsign"] === "true"} onChange={(v) => save("commit.gpgsign", v ? "true" : "")} />
      </Row>
      <Row label="Sign Tags by Default">
        <Toggle value={c["tag.gpgSign"] === "true"} onChange={(v) => save("tag.gpgSign", v ? "true" : "")} />
      </Row>
    </>
  );
}

const AI_FEATURES: [string, string, string][] = [
  ["commit", "Commit Message Generation", "The model GitCraft will use to generate commit messages."],
  ["compose", "Commit Composer", "The model GitCraft will use to compose commits."],
  ["stash", "Stash Message Generation", "The model GitCraft will use to generate stash messages."],
  ["explain", "Explain Changes", "The model GitCraft will use to explain changes."],
  ["pr", "Pull Request", "The model GitCraft will use to generate Pull Request title and description."],
  ["conflict", "Conflict Resolution", "The model GitCraft will use to auto-resolve conflicts."],
];

const KEY_NAME: Partial<Record<AiSettings["provider"], string>> = { anthropic: "anthropic-key", openai: "openai-key", google: "google-key", ollama: "ollama-key", custom: "custom-ai-key" };
const KEY_PAGE: Partial<Record<AiSettings["provider"], string>> = {
  anthropic: "https://console.anthropic.com/settings/keys",
  openai: "https://platform.openai.com/api-keys",
  google: "https://aistudio.google.com/app/apikey",
};
const CLI_SETUP = {
  codex: { name: "Codex CLI", login: "codex login", folder: "~/.codex", variable: "CODEX_HOME", docs: "https://developers.openai.com/codex/cli" },
  claude_code: { name: "Claude Code", login: "claude auth login", folder: "~/.claude", variable: "CLAUDE_CONFIG_DIR", docs: "https://code.claude.com/docs/en/quickstart" },
} as const;

function modelSource(a: AiSettings) {
  return JSON.stringify([a.provider, a.endpoint, a.cliPath, a.cliConfigDir]);
}

function ModelField({ models, value, fallback, label, onChange }: {
  models: AiModel[]; value: string; fallback: string; label: string; onChange: (value: string) => void;
}) {
  const [manual, setManual] = useState(false);
  const options: [string, string][] = [["", fallback], ...models.map((m): [string, string] => [m.id, m.name === m.id ? m.name : `${m.name} (${m.id})`])];
  if (value && !models.some((m) => m.id === value)) options.push([value, `${value} (saved)`]);
  return (
    <div className="flex min-w-0 flex-1 items-center gap-2">
      {models.length > 0 && !manual
        ? <Select label={label} value={value} options={options} onChange={onChange} />
        : <Text label={label} value={value} placeholder={fallback} onSave={(v) => onChange(v.trim())} mono />}
      {models.length > 0 && <Btn onClick={() => setManual(!manual)}>{manual ? "Use List" : "Enter ID"}</Btn>}
    </div>
  );
}

export function AiSection() {
  const { set, put, toast } = useSet();
  const ai = { ...DEFAULT_AI, ...set.ai };
  const upd = (p: Partial<AiSettings>) => {
    const current = { ...DEFAULT_AI, ...useStore.getState().settings.ai };
    put({ ai: { ...current, ...p } });
  };
  const [key, setKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [result, setResult] = useState<{ config: string; ok: boolean; text: string } | null>(null);
  const [catalog, setCatalog] = useState<{ source: string; models: AiModel[]; loading: boolean; error?: string }>();
  const [programResult, setProgramResult] = useState<{ provider: AiSettings["provider"]; path: string; found?: string; error?: string }>();
  const [finding, setFinding] = useState(false);
  const catalogRequest = useRef(0);
  const signature = JSON.stringify(ai);
  const source = modelSource(ai);
  const currentCatalog = catalog?.source === source ? catalog : undefined;
  const models = currentCatalog?.models ?? [];
  const program = programResult?.provider === ai.provider && programResult.path === ai.cliPath ? programResult : undefined;
  const label = { anthropic: "Anthropic", openai: "OpenAI", codex: "ChatGPT", claude_code: "Claude", google: "Google", ollama: "Ollama", custom: "compatible server" }[ai.provider];
  const cli = ai.provider === "codex" || ai.provider === "claude_code" ? CLI_SETUP[ai.provider] : undefined;
  const optionalKey = ai.provider === "ollama" || ai.provider === "custom";
  const defaultLabel = cli ? `Use ${cli.name} default` : ai.provider === "anthropic" ? "Use Anthropic default" : "Select a model";
  const connection = (a: AiSettings) => ({ endpoint: a.endpoint, cliPath: a.cliPath, cliConfigDir: a.cliConfigDir, defaultModel: a.defaultModel, responseFormat: a.responseFormat, models: a.models });
  const choose = (provider: AiSettings["provider"], preset?: string) => {
    const current = { ...DEFAULT_AI, ...useStore.getState().settings.ai };
    const connections = { ...current.connections, [current.provider]: connection(current) };
    const saved = { ...connection(DEFAULT_AI), ...connections[provider] };
    upd({ provider, connections, ...saved, ...(preset ? { endpoint: preset, defaultModel: "", models: {}, responseFormat: "auto" as const } : {}) });
    setKey("");
    setResult(null);
  };
  const saveKey = async (value: string) => {
    const name = KEY_NAME[ai.provider];
    if (!name) return;
    setSaving(true);
    setResult(null);
    try {
      await hosting.setSecret(name, value);
      setKey("");
      if (useStore.getState().settings.ai.provider === ai.provider) {
        catalogRequest.current++;
        setCatalog(undefined);
      }
      toast(true, value ? "Saved to the system credential store" : "Saved API key removed");
    } catch (e) {
      toast(false, String(e));
    } finally {
      setSaving(false);
    }
  };
  const fetchModels = async () => {
    const config = { ...DEFAULT_AI, ...useStore.getState().settings.ai };
    const from = modelSource(config);
    const request = ++catalogRequest.current;
    setCatalog((prev) => ({ source: from, loading: true, models: prev?.source === from ? prev.models : [] }));
    try {
      const list = await aiApi.listModels(config);
      if (request === catalogRequest.current) setCatalog({ source: from, loading: false, models: list });
    } catch (e) {
      if (request === catalogRequest.current) setCatalog((prev) => ({ source: from, loading: false, models: prev?.models ?? [], error: String(e) }));
    }
  };
  const findProgram = async () => {
    const { provider, cliPath } = { ...DEFAULT_AI, ...useStore.getState().settings.ai };
    setFinding(true);
    setProgramResult(undefined);
    try {
      const found = await aiApi.findProgram(provider);
      const current = useStore.getState().settings.ai;
      if (current.provider === provider && (current.cliPath ?? "") === cliPath) {
        upd({ cliPath: found.path });
        setProgramResult({ provider, path: found.path, found: found.version });
      }
    } catch (e) {
      setProgramResult({ provider, path: cliPath, error: String(e) });
    } finally {
      setFinding(false);
    }
  };
  return (
    <>
      <H>AI</H>
      <Note>Use your ChatGPT or Claude subscription, a hosted API, or a local server for GitCraft’s AI helpers.</Note>
      <Row label="Enable AI" help="Commit and stash messages, commit composition, change explanations, pull request drafts and conflict resolution all use this connection.">
        <Toggle value={ai.enabled} onChange={(v) => upd({ enabled: v })} />
      </Row>
      <Row label="Provider" help="Requests go directly to this connection when you press an AI button. A local connection keeps your code on your server.">
        <Select
          value={ai.provider}
          onChange={(v) => choose(v)}
          options={[
            ["anthropic", "Anthropic API (Claude)"],
            ["openai", "OpenAI API"],
            ["codex", "ChatGPT subscription (Codex CLI)"],
            ["claude_code", "Claude subscription (Claude Code)"],
            ["google", "Google (Gemini)"],
            ["ollama", "Ollama (local AI)"],
            ["custom", "Local / OpenAI-compatible API"],
          ]}
        />
      </Row>
      <Row label="Local AI presets" help="Start your local server, select its preset here, then fetch its models below. Presets reset model overrides for the new server.">
        <div className="flex gap-2 flex-wrap">
          <Btn onClick={() => choose("ollama", "http://localhost:11434/v1")}>Ollama</Btn>
          <Btn onClick={() => choose("custom", "http://localhost:1234/v1")}>LM Studio</Btn>
          <Btn onClick={() => choose("custom", "http://localhost:8080/v1")}>llama.cpp</Btn>
        </div>
      </Row>
      {cli ? (
        <>
          <Row label={`${label} sign-in`} help={<>Install {cli.name}, then run <code>{cli.login}</code> in a terminal and sign in to your {label} account. Requests use that CLI’s allowance. GitCraft passes the context needed for the helper to {cli.name}.</>}>
            <Btn onClick={() => void hosting.openUrl(cli.docs)}>Setup Guide</Btn>
          </Row>
          <Row label="Program" help={program?.found ? <span role="status">Found and verified: {program.found}</span> : `Find ${cli.name} on this computer or browse to its executable.`}
            error={program?.error}>
            <div className="flex w-full flex-wrap items-center gap-2">
              <PathField key={ai.provider} value={ai.cliPath} onSave={(v) => upd({ cliPath: v.trim() })} placeholder="Program path" title={`Choose the ${cli.name} executable`} />
              <Btn disabled={finding} onClick={() => void findProgram()}>{finding ? "Finding…" : "Find Automatically"}</Btn>
            </div>
          </Row>
          <Row label={`${cli.name} folder`} help={<>Optional. Choose the folder you used when signing in with <code>{cli.variable}</code>. Leave empty to use the CLI’s usual folder.</>}>
            <PathField key={ai.provider} value={ai.cliConfigDir} onSave={(v) => upd({ cliConfigDir: v.trim() })} placeholder={cli.folder} folder title={`Choose the ${cli.name} config folder`} />
          </Row>
        </>
      ) : (
        <>
          <Row
            label={optionalKey ? "API Key (optional)" : "Provider API Key"}
            help={
              <>
                {optionalKey ? "Leave empty for a local server without authentication, or save a key if your server requires one. Keys stay in the system credential store." : "Your API key stays in the system credential store."}{" "}
                {KEY_PAGE[ai.provider] && (
                  <button className="text-link hover:underline" onClick={() => hosting.openUrl(KEY_PAGE[ai.provider]!)}>
                    Get an API key for {label}.
                  </button>
                )}
              </>
            }
          >
            <input type="password" autoComplete="off" aria-label="AI API key" className={cn(field, "flex-1")} placeholder={optionalKey ? "optional key" : "paste key"} value={key} onChange={(e) => setKey(e.target.value)} />
            <Btn disabled={!key.trim() || saving} onClick={() => void saveKey(key.trim())}>Save</Btn>
            <Btn disabled={saving} onClick={() => void saveKey("")} title="Remove this provider's key from the credential store">Remove Key</Btn>
          </Row>
          <Row
            label="API Endpoint URL"
            help={ai.provider === "anthropic" ? "Optional API base or full Messages URL. Empty uses Anthropic." : "Enter a server address, a /v1 API base, or the full /chat/completions URL. Empty uses the provider default; a compatible server needs its own URL."}
          >
            <Text key={ai.provider} value={ai.endpoint} onSave={(v) => upd({ endpoint: v.trim() })} placeholder={ai.provider === "ollama" ? "http://localhost:11434/v1" : ai.provider === "custom" ? "http://localhost:1234/v1" : "provider default"} mono />
          </Row>
        </>
      )}
      <Row label={cli ? "Default Model (optional)" : "Default Model"} error={currentCatalog?.error}
        help={<>Fetch the models available on this connection. The default is used by every feature unless you choose an override.
          {currentCatalog && !currentCatalog.loading && !currentCatalog.error && (
            <span role="status" className="block mt-1">{models.length ? `${models.length} models found.` : "No models were listed. You can still enter a model ID."}</span>
          )}
        </>}>
        <div className="flex w-full flex-wrap items-center gap-2">
          <ModelField key={source} label="Default Model" models={models} value={ai.defaultModel} fallback={defaultLabel} onChange={(v) => upd({ defaultModel: v })} />
          <Btn disabled={currentCatalog?.loading || saving} onClick={() => void fetchModels()}>{currentCatalog?.loading ? "Fetching…" : models.length ? "Refresh Models" : "Fetch Models"}</Btn>
        </div>
      </Row>
      {ai.provider !== "anthropic" && !cli && (
        <Row label="Response Mode" help="Automatic tries structured output, then JSON mode, then a JSON prompt only if the server rejects the format. Every mode supports every feature; answers are validated before use.">
          <Select
            value={ai.responseFormat}
            onChange={(v) => upd({ responseFormat: v })}
            options={[["auto", "Automatic (recommended)"], ["json_schema", "Structured output"], ["json_object", "JSON mode"], ["text", "JSON prompt only (maximum compatibility)"]]}
          />
        </Row>
      )}
      <Row label="Test Connection" help={result?.config === signature ? result.text : cli ? "Checks your subscription sign-in and makes a short test request without repository data." : "Tests the default model with the same connection and response mode used by AI features."} error={result?.config === signature && !result.ok ? "Connection test failed. Check the settings above and try again." : undefined}>
        <Btn
          disabled={testing || !ai.enabled}
          onClick={async () => {
            setTesting(true);
            try {
              const text = await aiApi.test();
              setResult({ config: signature, ok: true, text });
              toast(true, text);
            } catch (e) {
              setResult({ config: signature, ok: false, text: String(e) });
              toast(false, String(e));
            } finally {
              setTesting(false);
            }
          }}
        >
          {testing ? "Testing…" : "Test Connection"}
        </Btn>
      </Row>
      <H sub>Global Instructions</H>
      <Row label="Custom Instructions" help="Applied to every AI feature, together with any feature instructions below." wide>
        <Area value={ai.instructions} onSave={(v) => upd({ instructions: v })} rows={3} />
      </Row>
      {AI_FEATURES.map(([id, title, modelHelp]) => (
        <div key={id}>
          <H sub>{title}</H>
          <Row label="Model Override (optional)" help={modelHelp + " Leave empty to use the Default Model."}>
            <ModelField key={source} label={`${title} Model`} models={models} value={ai.models[id] ?? ""} fallback="Use Default Model" onChange={(v) => upd({ models: { ...useStore.getState().settings.ai.models, [id]: v } })} />
          </Row>
          <Row label="Custom Instructions" wide>
            <Area value={ai.featureInstructions[id] ?? ""} rows={2} onSave={(v) => upd({ featureInstructions: { ...useStore.getState().settings.ai.featureInstructions, [id]: v } })} />
          </Row>
        </div>
      ))}
    </>
  );
}
