import { afterEach, expect, it, vi } from "vitest";
import { api } from "./api";
import { switchProfile, saveProfileIdentity } from "./prefs";
import { DEFAULT_SETTINGS, useStore } from "./store";
const original=useStore.getState();
afterEach(() => {useStore.setState(original,true);vi.restoreAllMocks();});
function setup() {
  let name="Default Name", email="default@example.test";
  const read=vi.spyOn(api,"gitConfig").mockImplementation(async () => ({"user.name":name,"user.email":email}));
  const write=vi.spyOn(api,"setGitConfig").mockImplementation(async (_path,_scope,key,value) => {if(key==="user.name") name=value; if(key==="user.email") email=value;});
  useStore.setState({ settings:{...DEFAULT_SETTINGS,keepGitconfigUpdated:true},activeProfile:"default",profiles:[{id:"default",name:"Default",gitName:"",gitEmail:"",settings:null},{id:"work",name:"Work",gitName:"Work Name",gitEmail:"work@example.test",settings:{keepGitconfigUpdated:true}}]});
  return {read,write,identity:()=>[name,email]};
}
it("restores Default identity after a round trip without rediscovering tools", async () => {
  const fixture=setup(); const tools=vi.spyOn(api,"accountInfo");
  await switchProfile("work"); expect(fixture.identity()).toEqual(["Work Name","work@example.test"]);
  await switchProfile("default"); expect(fixture.identity()).toEqual(["Default Name","default@example.test"]);
  expect(tools).not.toHaveBeenCalled(); expect(fixture.read).toHaveBeenCalledTimes(2);
});
it("persists edited identity and rolls back a failed switch", async () => {
  const fixture=setup(); await saveProfileIdentity("user.name","Edited");
  expect(useStore.getState().profiles[0].gitName).toBe("Edited");
  fixture.write.mockRejectedValueOnce(new Error("read only"));
  await switchProfile("work");expect(useStore.getState().activeProfile).toBe("default");expect(fixture.identity()).toEqual(["Edited","default@example.test"]);
});
it("explicitly clears identity when a blank profile is selected", async () => {
  const fixture=setup();useStore.setState({profiles:useStore.getState().profiles.map(p=>p.id==="work"?{...p,gitName:"",gitEmail:""}:p)});
  await switchProfile("work");expect(fixture.identity()).toEqual(["",""]);
});
