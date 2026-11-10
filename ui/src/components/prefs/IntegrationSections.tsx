import { useQuery } from "@tanstack/react-query";
import { CheckCircle2, CircleDashed, ExternalLink, AlertTriangle } from "lucide-react";
import { useState } from "react";
import { api } from "@/lib/api";
import { hosting } from "@/lib/hosting";
import { useStore, DEFAULT_AI, type AiSettings, type Settings } from "@/lib/store";
import { SERVICES, aiApi, integrations, tools, type ServiceInfo } from "@/lib/integrations";
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
  const pubKey = () => {
    const keys = acc.data?.ssh_keys ?? [];
    const chosen = set.sshPublicKey ? keys.find((k) => k.path.replace(/\\/g, "/").endsWith(set.sshPublicKey.replace(/\\/g, "/").split("/").pop() ?? "")) : undefined;
    return chosen ?? keys.find((k) => /id_ed25519/.test(k.path)) ?? keys[0];
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
              options={[["", "(none)"], ...(acc.data?.gpg_keys ?? []).map((k) => [k.id, `${k.id} - ${k.uid}`] as [string, string])]}
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

const KEY_NAME: Record<AiSettings["provider"], string> = { anthropic: "anthropic-key", openai: "openai-key", google: "google-key", ollama: "ollama-key", custom: "custom-ai-key" };
const KEY_PAGE: Partial<Record<AiSettings["provider"], string>> = {
  anthropic: "https://console.anthropic.com/settings/keys",
  openai: "https://platform.openai.com/api-keys",
  google: "https://aistudio.google.com/app/apikey",
};

export function AiSection() {
  const { set, put, toast } = useSet();
  const ai = { ...DEFAULT_AI, ...set.ai };
  const upd = (p: Partial<AiSettings>) => put({ ai: { ...ai, ...p } });
  const [key, setKey] = useState("");
  const [testing, setTesting] = useState(false);
  const label = { anthropic: "Anthropic", openai: "OpenAI", google: "Google", ollama: "Ollama", custom: "Custom" }[ai.provider];
  return (
    <>
      <H>AI</H>
      <Row label="Enable AI" help={ai.enabled ? "AI features like Commit Message Generation, Explain Commits, Compose and Conflict Resolution will be available. Uncheck to disable all AI features." : "AI features disabled. You can re-enable anytime."}>
        <Toggle value={ai.enabled} onChange={(v) => upd({ enabled: v })} />
      </Row>
      <Row label="Provider" help="GitCraft makes requests directly to the selected provider with your own API key. Nothing is sent unless you press an AI button.">
        <Select
          value={ai.provider}
          onChange={(v) => upd({ provider: v })}
          options={[
            ["anthropic", "Anthropic (Claude)"],
            ["openai", "OpenAI"],
            ["google", "Google (Gemini)"],
            ["ollama", "Ollama"],
            ["custom", "<Custom URL> (OpenAI-compatible)"],
          ]}
        />
      </Row>
      <Row
        label={ai.provider === "ollama" ? "Authorization Key (Optional)" : "Provider API Key"}
        help={
          <>
            {ai.provider === "ollama" ? "You may need this if your Ollama server is behind an authenticated proxy." : "Your API Key used to make the API call. Kept in the system's credential store."}{" "}
            {KEY_PAGE[ai.provider] && (
              <button className="text-link hover:underline" onClick={() => hosting.openUrl(KEY_PAGE[ai.provider]!)}>
                Generate an API key for {label} here.
              </button>
            )}
          </>
        }
      >
        <input type="password" className={cn(field, "flex-1")} placeholder="paste key" value={key} onChange={(e) => setKey(e.target.value)} />
        <Btn
          disabled={!key.trim()}
          onClick={async () => {
            await hosting.setSecret(KEY_NAME[ai.provider] as "anthropic-key", key.trim()).catch((e) => toast(false, String(e)));
            setKey("");
            toast(true, "Saved to the system credential store");
          }}
        >
          Save
        </Btn>
      </Row>
      <Row
        label={ai.provider === "ollama" ? "Ollama URL" : ai.provider === "custom" ? "API Endpoint URL" : `${label} Endpoint URL (optional)`}
        help={ai.provider === "custom" ? "The full URL of your provider's OpenAI API compatible endpoint" : ai.provider === "ollama" ? "The full URL of your Ollama API (default http://localhost:11434/v1)" : "Most of the time, you don't need to change this."}
      >
        <Text value={ai.endpoint} onSave={(v) => upd({ endpoint: v.trim() })} mono />
      </Row>
      <Row label="Test Connection">
        <Btn
          disabled={testing}
          onClick={async () => {
            setTesting(true);
            try {
              toast(true, await aiApi.test());
            } catch (e) {
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
      <Row label="Custom Instructions" help="These instructions apply to all AI features unless overridden below." wide>
        <Area value={ai.instructions} onSave={(v) => upd({ instructions: v })} rows={3} />
      </Row>
      {AI_FEATURES.map(([id, title, modelHelp]) => (
        <div key={id}>
          <H sub>{title}</H>
          <Row label="Model" help={modelHelp + (ai.provider === "anthropic" ? " Empty uses claude-opus-5-5." : " Required for this provider.")}>
            <Text value={ai.models[id] ?? ""} placeholder={ai.provider === "anthropic" ? "claude-opus-5-5" : "model name"} onSave={(v) => upd({ models: { ...ai.models, [id]: v.trim() } })} mono />
          </Row>
          <Row label="Custom Instructions" wide>
            <Area value={ai.featureInstructions[id] ?? ""} rows={2} onSave={(v) => upd({ featureInstructions: { ...ai.featureInstructions, [id]: v } })} />
          </Row>
        </div>
      ))}
    </>
  );
}
