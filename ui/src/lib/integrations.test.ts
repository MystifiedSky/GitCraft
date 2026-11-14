import { afterEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { integrations, signOutAll, SERVICES } from "./integrations";
vi.mock("@tauri-apps/api/core", () => ({ invoke:vi.fn() }));
afterEach(() => vi.mocked(invoke).mockReset());
it("clears every service without filtering on network health and syncs Git", async () => {
  vi.mocked(invoke).mockResolvedValue(undefined);
  await signOutAll();
  for (const service of SERVICES) expect(invoke).toHaveBeenCalledWith("set_secret",{name:`${service.id}-token`,value:""});
  expect(invoke).toHaveBeenCalledWith("sync_git_credentials",expect.anything());
  expect(invoke).not.toHaveBeenCalledWith("integration_statuses",expect.anything());
});
it("continues clearing other services after one failure and reports the failure", async () => {
  vi.mocked(invoke).mockImplementation((name,args) => name === "set_secret" && (args as {name:string}).name === `${SERVICES[0].id}-token` ? Promise.reject(new Error("locked")) : Promise.resolve(undefined));
  await expect(signOutAll()).rejects.toThrow("Could not remove credentials");
  expect(invoke).toHaveBeenCalledTimes(SERVICES.length+1);
});
it("syncs Git immediately after an individual disconnect", async () => {
  vi.mocked(invoke).mockResolvedValue(undefined);
  await integrations.setToken("github", "");
  expect(vi.mocked(invoke).mock.calls.map(call => call[0])).toEqual(["set_secret","sync_git_credentials"]);
});
