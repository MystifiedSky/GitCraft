import { updateEditorTheme } from "./editor-theme";
// Themes, profiles and local workspaces.
import { api } from "./api";
import { queryClient } from "./repo";
import { openRepoPath } from "./repo";
import { DEFAULT_SETTINGS, newId, useStore, type Profile, type Workspace } from "./store";

/** Themes: Dark / Light and their high-contrast versions. */
export const THEMES = [
  { id: "dark", label: "Dark" },
  { id: "light", label: "Light" },
  { id: "dark-hc", label: "Dark (High Contrast)" },
  { id: "light-hc", label: "Light (High Contrast)" },
] as const;

export function applyTheme(id: string) {
  document.documentElement.dataset.theme = id;
  updateEditorTheme();
}

const st = () => useStore.getState();

/** Switch profile: keep the current profile's settings, load the other's, and apply its Git identity. */
let switchingProfile = false;
export async function switchProfile(id: string) {
  if (switchingProfile) return;
  const s = st();
  const target = s.profiles.find((p) => p.id === id);
  if (id === s.activeProfile || !target) return;
  switchingProfile = true;
  try {
    const values = await api.gitConfig(null, "global", ["user.name", "user.email"]);
    const identity = { name: values["user.name"] ?? "", email: values["user.email"] ?? "" };
    const profiles = s.profiles.map((p) => p.id === s.activeProfile ? { ...p, settings: s.settings, gitName: identity.name, gitEmail: identity.email } : p);
    const settings = { ...DEFAULT_SETTINGS, ...(target.settings ?? {}), recents: s.settings.recents };
    if (settings.keepGitconfigUpdated) {
      try {
        await api.setGitConfig(null, "global", "user.name", target.gitName);
        await api.setGitConfig(null, "global", "user.email", target.gitEmail);
      } catch (error) {
        const rollback = await Promise.allSettled([api.setGitConfig(null, "global", "user.name", identity.name), api.setGitConfig(null, "global", "user.email", identity.email)]);
        if (rollback.some((result) => result.status === "rejected")) throw new Error(`${error}. Check your global Git identity; restoring it also failed.`);
        throw error;
      }
    }
    useStore.setState({ profiles, activeProfile: id, settings });
    await queryClient.invalidateQueries({ queryKey: ["accounts"] });
    s.toast(true, `Switched to profile ${target.name}`);
  } catch (error) { s.toast(false, `Could not switch profile: ${error}`); }
  finally { switchingProfile = false; }
}

export async function saveProfileIdentity(key: "user.name" | "user.email", value: string) {
  await api.setGitConfig(null, "global", key, value);
  const s = st();
  useStore.setState({ profiles: s.profiles.map((p) => p.id === s.activeProfile ? { ...p, [key === "user.name" ? "gitName" : "gitEmail"]: value } : p) });
  await queryClient.invalidateQueries({ queryKey: ["accounts"] });
}

export function addProfile(name: string, gitName: string, gitEmail: string, options: { icon?: number; copySettings?: boolean } = {}) {
  const s = st();
  const p: Profile = { id: newId(), name: name.trim() || "Profile", gitName: gitName.trim(), gitEmail: gitEmail.trim(), icon: options.icon ?? 0, settings: options.copySettings === false ? null : { ...s.settings } };
  useStore.setState({ profiles: [...s.profiles, p] });
  return p;
}

export function removeProfile(id: string) {
  const s = st();
  if (id === s.activeProfile || s.profiles.length <= 1) return s.toast(false, "Switch to another profile before deleting this one");
  useStore.setState({ profiles: s.profiles.filter((p) => p.id !== id) });
}

export function saveWorkspace(name: string, repos: string[], id?: string) {
  const s = st();
  const ws: Workspace = { id: id ?? newId(), name: name.trim() || "Workspace", repos };
  const list = id ? s.settings.workspaces.map((w) => (w.id === id ? ws : w)) : [...s.settings.workspaces, ws];
  s.setSettings({ workspaces: list });
  return ws;
}

export function deleteWorkspace(id: string) {
  st().setSettings({ workspaces: st().settings.workspaces.filter((w) => w.id !== id) });
}

/** "Open all in workspace": every repository of the workspace in its own tab. */
export async function openWorkspace(id: string) {
  const ws = st().settings.workspaces.find((w) => w.id === id);
  if (!ws) return;
  for (const p of ws.repos) await openRepoPath(p);
}
