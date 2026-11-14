import {afterEach,expect,it,vi} from "vitest";
import {COMMANDS} from "./commands";
import * as repo from "./repo";
import {useStore} from "./store";
const original=useStore.getState();
afterEach(()=>{useStore.setState(original,true);vi.restoreAllMocks();});
it.each(["Commit","Range","Staged","Unstaged"])("handles %s external comparisons explicitly",async(kind)=>{
  const action=vi.spyOn(repo,"act").mockResolvedValue(true);useStore.getState().openRepoTab("/fixture/repo","fixture");
  const state=useStore.getState();state.setUi(state.activeId,{diff:{file:"[id].txt",source:kind==="Range"?{kind,sha:["from","to"]}:kind==="Commit"?{kind,sha:"saved"}:{kind} as any,untracked:false,parent:null}});
  await COMMANDS.find(command=>command.id==="repo:externalDiff")!.run!("/fixture/repo");
  if(kind==="Commit"||kind==="Range"){expect(action).not.toHaveBeenCalled();expect(useStore.getState().toasts.at(-1)?.text).toContain("not available");}
  else expect(action).toHaveBeenCalledWith("/fixture/repo",expect.objectContaining({type:"externalDiff",file:"[id].txt",staged:kind==="Staged"}));
});
