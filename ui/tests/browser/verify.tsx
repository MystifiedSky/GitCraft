import React from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import { QueryClientProvider } from "@tanstack/react-query";
import "../../src/index.css";

const calls: { command: string; args: any }[] = [];
let handlers: Record<string, (args: any) => any> = {};
let callbackId = 0;
(window as any).__TAURI_INTERNALS__ = {
  metadata: {currentWindow:{label:"main"},currentWebview:{label:"main"}},
  transformCallback: () => ++callbackId, unregisterCallback: () => {}, convertFileSrc: () => "about:blank",
  invoke: async (command: string, args: any) => {calls.push({command,args}); return handlers[command]?.(args);},
};
const {useStore,DEFAULT_SETTINGS,DEFAULT_COLUMNS}=await import("../../src/lib/store");
const {queryClient,refresh,act,openRepoPath,WIP_COMMIT_EVENT}=await import("../../src/lib/repo");
const {monaco}=await import("../../src/lib/monaco");
const {MonacoDiff,MonacoFile}=await import("../../src/components/MonacoViews");
const {VirtualFileRows}=await import("../../src/components/VirtualFileRows");
const {Dialogs}=await import("../../src/components/Dialogs");
const {promptUpstream}=await import("../../src/lib/menus");
const {RightPanel}=await import("../../src/components/RightPanel");
const {GraphView}=await import("../../src/components/GraphView");
const {LeftPanel}=await import("../../src/components/LeftPanel");
const {IntegrationsSection,SigningSection}=await import("../../src/components/prefs/IntegrationSections");
const {RepoManagementDialog}=await import("../../src/components/RepoManagement");
const {AgentsView}=await import("../../src/components/Stage6Panels");
const {topMatches}=await import("../../src/lib/ranking");
const {PullRequestView}=await import("../../src/components/PullRequestView");
const {HistoryView}=await import("../../src/components/HistoryView");
const {Row:PrefRow,Toggle:PrefToggle,Num:PrefNum,Area:PrefArea,Text:PrefText,Select:PrefSelect,PathField:PrefPath}=await import("../../src/components/prefs/controls");
const {DiffView}=await import("../../src/components/DiffView");
const {ConflictView}=await import("../../src/components/ConflictView");
const initial=useStore.getState();
const sleep=(ms:number)=>new Promise(resolve=>setTimeout(resolve,ms));
function assert(value:unknown,message:string):asserts value {if(!value)throw new Error(message);}
async function until(condition:()=>boolean,label:string){for(let i=0;i<250;i++){if(condition())return;await sleep(20);}throw new Error(`Timed out: ${label}`);}
let root=createRoot(document.getElementById("test-root")!);
const mount=(node:React.ReactNode)=>flushSync(()=>root.render(<QueryClientProvider client={queryClient}>{node}</QueryClientProvider>));
const button=(text:string)=>[...document.querySelectorAll<HTMLButtonElement>("button")].find(button=>button.textContent?.trim()===text);
function input(element:HTMLInputElement,value:string){Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,"value")!.set!.call(element,value);element.dispatchEvent(new Event("input",{bubbles:true}));}
const sha=(c:string)=>c.repeat(40);
const commit=(id:string)=>({sha:sha(id),parents:[],author:"Fixture",email:"fixture@example.invalid",time:1,summary:`Commit ${id}`,body:""});
let repo:any;
let tabId:string;
function reset(){
  root.unmount();queryClient.clear();document.getElementById("test-root")!.innerHTML="";root=createRoot(document.getElementById("test-root")!);
  useStore.setState({...initial,settings:{...DEFAULT_SETTINGS,autoUpdate:false,autoFetchMin:0,wordWrap:true,keepGitconfigUpdated:false},tabs:[],ui:{},drafts:{},dialog:null,notices:[],toasts:[]},true);
  useStore.getState().openRepoTab("/verification/repo","verification");tabId=useStore.getState().activeId;
  repo={path:"/verification/repo",name:"verification",commits:[commit("a")],refs:[],stashes:[],worktrees:[],submodules:[],remotes:["origin"],head_branch:"main",head_sha:sha("a"),status:{staged:[],unstaged:[],conflicted:false},state_note:null,has_more:false,history_cursor:null,merge_msg:null,conflict_labels:null,graph:{rows:[{col:0,color:0,up:[],down:[],merge:false}],width:1},undo:{undo:null,redo:null}};
  calls.length=0;
  handlers={open_repo:()=>repo,repo_status:()=>repo.status,run_action:()=>({ok:true,message:"Fixture operation complete"}),repo_remote:()=>null,lfs_info:()=>({available:false,patterns:[],locks:[]}),integration_statuses:({ids})=>ids.map((id:string)=>({id,connected:true,account:"fixture",error:null})),account_info:()=>({name:"Fixture",email:"fixture@example.invalid",ssh_keys:[],gpg_keys:[],gpg:true}),git_config_get:()=>({}),pull_requests:()=>[],issues:()=>[]};
}
const results:any[]=[];
async function test(id:string,run:()=>Promise<void>){reset();const start=performance.now();try{await run();results.push({id,status:"passed",milliseconds:Math.round(performance.now()-start)});}catch(error){results.push({id,status:"failed",error:String(error)});} }

await test("B039 real Monaco cached file switches",async()=>{
  for(const file of ["a.txt","b.txt","a.txt"]){
    mount(<MonacoDiff file={file} original="old\n" modified={"new ".repeat(80)} sideBySide={false} wordWrap ignoreWhitespace editable/>);
    await until(()=>monaco.editor.getDiffEditors().length>0,`diff created: ${monaco.editor.getEditors().length} editors, ${document.querySelectorAll(".monaco-diff-editor").length} DOM`);await sleep(100);
    const diff=monaco.editor.getDiffEditors()[0];
    assert(diff.getModifiedEditor().getRawOptions().wordWrap==="on",`${file}: modified wrap lost`);
    assert(diff.getOriginalEditor().getRawOptions().wordWrap==="on",`${file}: original wrap lost`);
    assert(!diff.getModifiedEditor().getOption(monaco.editor.EditorOption.readOnly),`${file}: editability lost`);
    assert(!diff.getContainerDomNode().classList.contains("side-by-side"),`${file}: inline layout lost`);
  }
  for(const file of ["a.txt","b.txt","a.txt"]){mount(<MonacoFile file={file} value="long text" readOnly={false} wordWrap/>);await sleep(100);const editor=monaco.editor.getEditors()[0];assert(editor.getRawOptions().wordWrap==="on",`${file}: file-view wrapping lost`);}
});
await test("O006 10000-file scrolling and keyboard",async()=>{
  const files=Array.from({length:10000},(_,i)=>({path:`file-${i}`,status:"M"}));
  mount(<div id="scroller" style={{height:300,overflowY:"auto"}}><VirtualFileRows files={files} render={file=><button>{file.path}</button>}/></div>);
  await sleep(100);assert(document.querySelectorAll('[role="listitem"]').length<60,"Unbounded file DOM");
  const first=document.querySelector<HTMLButtonElement>('[data-file-row="0"] button')!;first.focus();first.dispatchEvent(new KeyboardEvent("keydown",{key:"End",bubbles:true}));
  await until(()=>document.activeElement?.textContent==="file-9999","End focus");
  document.activeElement!.dispatchEvent(new KeyboardEvent("keydown",{key:"Home",bubbles:true}));await until(()=>document.activeElement?.textContent==="file-0","Home focus");
  const scroller=document.getElementById("scroller")!;scroller.scrollTop=24*5000;scroller.dispatchEvent(new Event("scroll"));await sleep(100);assert(document.querySelector('[data-file-row="5000"]'),"Scrolled row absent");
});
await test("B025 empty upstream submission",async()=>{
  mount(<Dialogs/>);promptUpstream(repo,{name:"main",full:"refs/heads/main",kind:"Local",sha:sha("a"),upstream:"origin/main",ahead:0,behind:0});
  await until(()=>!!document.querySelector('input'),"upstream prompt");input(document.querySelector('input')!,"");await sleep(30);button("Set Upstream")!.click();
  await until(()=>calls.some(call=>call.command==="run_action"&&call.args.action.type==="setUpstream"),"upstream action");
  assert(calls.find(call=>call.command==="run_action")!.args.action.upstream==="","Empty upstream was rejected");
});
await test("B035 delayed commit retains next draft",async()=>{
  repo.status.staged=[{path:"a.txt",status:"M"}];useStore.getState().setDraft(repo.path,{summary:"First draft",description:"",amend:false});
  let release!:(value:any)=>void;handlers.run_action=()=>new Promise(resolve=>{release=resolve;});
  mount(<RightPanel repo={repo} tabId={tabId}/>);await until(()=>!!button("Commit Changes to 1 File"),"commit button");button("Commit Changes to 1 File")!.click();
  await until(()=>!!release,"commit held");const field=document.querySelector<HTMLInputElement>('input[placeholder="Commit summary"]')!;assert(field,"Summary input missing");input(field,"Next draft");await sleep(30);
  assert(useStore.getState().drafts[repo.path].summary==="Next draft","Typing did not update draft");release({ok:true,message:"Committed"});await until(()=>useStore.getState().busy===null,"commit complete");
  assert(useStore.getState().drafts[repo.path].summary==="Next draft","Newer draft cleared");
});
await test("B028 statistics reload at unchanged viewport",async()=>{
  useStore.getState().setSettings({columns:{...DEFAULT_COLUMNS,changes:true}});handlers.commit_stats=({shas})=>shas.map((sha:string)=>({sha,files:1,added:2,deleted:1}));
  mount(<GraphView repo={repo} tabId={tabId}/>);await until(()=>calls.some(call=>call.command==="commit_stats"&&call.args.shas.includes(sha("a"))),"first statistics");
  repo={...repo,commits:[commit("b")],head_sha:sha("b")};mount(<GraphView repo={repo} tabId={tabId}/>);
  await until(()=>calls.some(call=>call.command==="commit_stats"&&call.args.shas.includes(sha("b"))),"replacement statistics");
});
await test("B020 remote detection refresh without renaming",async()=>{
  mount(<LeftPanel repo={repo} tabId={tabId} onSelectSha={()=>{}}/>);await until(()=>calls.some(call=>call.command==="repo_remote"),"initial remote");
  await until(()=>queryClient.getQueriesData({queryKey:["remote",repo.path]}).length>0 && !queryClient.isFetching({queryKey:["remote",repo.path]}),"initial remote settled");
  handlers.repo_remote=()=>({provider:"github",service:"github",host:"github.com",path:"fixture/repo",web:"https://github.com/fixture/repo"});await refresh(repo.path);
  await until(()=>calls.filter(call=>call.command==="repo_remote").length>=2,`updated remote detection: ${JSON.stringify(useStore.getState().tabs)} active ${useStore.getState().activeId}`);
  assert(queryClient.getQueriesData({queryKey:["remote",repo.path]}).some(([,data]:any)=>data?.path==="fixture/repo"),"Old unsupported remote remained cached");
});
await test("B047 selected SSH bytes reach copy and upload",async()=>{
  const selected="C:/fixture/second/work.pub",publicKey="ssh-ed25519 SYNTHETICSECOND selected@example.invalid";let copied="";
  useStore.getState().setSettings({sshPublicKey:selected});handlers.read_ssh_public_key=()=>publicKey;
  handlers.account_info=()=>({ssh_keys:[{path:"C:/fixture/first/work",public:"ssh-ed25519 SYNTHETICFIRST"},{path:"C:/fixture/second/work",public:publicKey}],gpg_keys:[]});
  Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:async(text:string)=>{copied=text;}}});handlers.integration_ssh_page=()=>"https://example.invalid/ssh";
  mount(<IntegrationsSection initial="github"/>);await until(()=>!!button("Add key to GitHub")&&!button("Add key to GitHub")!.disabled,"selected key ready");
  button("Add key to GitHub")!.click();await until(()=>calls.some(call=>call.command==="integration_add_ssh_key"),"key upload IPC");
  assert(calls.find(call=>call.command==="integration_add_ssh_key")!.args.key===publicKey,"Wrong public key sent");
  await until(()=>!button("Copy public key and open GitHub")?.disabled,"copy ready");button("Copy public key and open GitHub")!.click();await until(()=>copied===publicKey,"correct clipboard bytes");
});
await test("B067 configured fingerprint selects its key",async()=>{
  const fingerprint="123456789012345678901234ABCDEF0123456789";
  handlers.git_config_get=()=>({"user.signingkey":fingerprint});handlers.account_info=()=>({name:"Fixture",email:"fixture@example.invalid",ssh_keys:[],gpg_keys:[{id:"ABCDEF0123456789",fingerprint,uid:"Fixture"}],gpg:true});
  mount(<SigningSection/>);await until(()=>[...document.querySelectorAll('select')].some(select=>select.value===fingerprint),"fingerprint option selected");
  const selected=[...document.querySelectorAll('select')].find(select=>select.value===fingerprint)!;assert(selected.selectedOptions[0].textContent?.includes("Fixture"),"Configured fingerprint appears unset");
});
await test("O014 local LFS remains visible while server is unavailable",async()=>{
  handlers.lfs_info=()=>({available:true,enabled:true,patterns:["*.fixture"],locks:[]});
  let rejectLocks!:(error:Error)=>void;handlers.lfs_locks=()=>new Promise((_,reject)=>{rejectLocks=reject;});
  mount(<LeftPanel repo={repo} tabId={tabId} onSelectSha={()=>{}}/>);
  await until(()=>[...document.querySelectorAll('button')].some(b=>b.textContent?.includes("LFS")),"LFS section");
  [...document.querySelectorAll('button')].find(b=>b.textContent?.includes("LFS"))!.click();
  await until(()=>!!rejectLocks,"server request held");assert(document.body.textContent?.includes("*.fixture"),"Local patterns waited for server");
  for(let i=0;i<10;i++){repo={...repo,status:{...repo.status,unstaged:[{path:`edit${i}`,status:"M"}]}};mount(<LeftPanel repo={repo} tabId={tabId} onSelectSha={()=>{}}/>);}
  assert(calls.filter(call=>call.command==="lfs_locks").length===1,"Status edits repeated server calls");
  rejectLocks(new Error("Lock server unavailable"));await sleep(50);assert(document.body.textContent?.includes("*.fixture"),"Local patterns vanished on server error");
});
await test("O023 300-repository summary concurrency and filter cancellation",async()=>{
  useStore.getState().setSettings({favorites:Array.from({length:300},(_,i)=>`/fixture/repo-${i}`),recents:[]});
  let running=0,peak=0;const held:(()=>void)[]=[];
  handlers.repo_summary=()=>new Promise(resolve=>{running++;peak=Math.max(peak,running);held.push(()=>{running--;resolve({exists:true,branch:"main",changes:0,ahead:0,behind:0});});});
  mount(<RepoManagementDialog onClose={()=>{}}/>);await until(()=>held.length===4,"four active summary reads");
  input(document.querySelector<HTMLInputElement>('[aria-label="Search repositories"]')!,"repo-299");await sleep(50);
  while(held.length)held.shift()!();await until(()=>calls.some(c=>c.command==="repo_summary"&&c.args.path==="/fixture/repo-299"),"filtered row prioritized");
  while(held.length)held.shift()!();await sleep(50);
  assert(peak===4,`Expected 4 summary workers, saw ${peak}`);assert(calls.filter(c=>c.command==="repo_summary").length<=5,"Hidden queued summaries were started");
});
await test("B046 merge cleanup waits and reviews exact candidates",async()=>{
  useStore.getState().setSettings({deleteOrigAfterMerge:true});mount(<Dialogs/>);
  handlers.orig_cleanup_candidates=()=>[{path:"file.orig",expected:"fixture-token"}];
  await act(repo.path,{type:"markResolved",paths:["file"]} as any,true);
  assert(!calls.some(c=>c.command==="orig_cleanup_candidates"),"Resolving a file requested cleanup too early");
  await act(repo.path,{type:"merge",target:"feature"} as any,true);
  await until(()=>!!button("Delete backups"),"cleanup review");assert(document.body.textContent?.includes("file.orig"),"Candidate not shown");
  assert(!calls.some(c=>c.command==="delete_orig_files"),"Deleted before confirmation");button("Delete backups")!.click();
  await until(()=>calls.some(c=>c.command==="delete_orig_files"),"confirmed cleanup");
  assert(calls.find(c=>c.command==="delete_orig_files")!.args.candidates[0].expected==="fixture-token","Candidate identity lost");
});
await test("O009 bulk fetch overlaps mixed delays with three workers",async()=>{
  const paths=Array.from({length:7},(_,i)=>`/fixture/bulk-${i}`);useStore.getState().setSettings({favorites:paths,recents:[]});
  handlers.repo_summary=()=>({exists:true,branch:"main",changes:0,ahead:0,behind:0});
  let running=0,peak=0,done=0;const active=new Set<string>();
  handlers.run_action=async({path})=>{assert(!active.has(path),"Overlapping actions for one checkout");active.add(path);running++;peak=Math.max(peak,running);await sleep(path.endsWith("0")?150:30);running--;active.delete(path);done++;return{ok:true,message:"Fetched"};};
  mount(<RepoManagementDialog onClose={()=>{}}/>);await until(()=>document.querySelectorAll('[title="Fetch this repository"]').length>0 || queryClient.getQueriesData({queryKey:["repoSummary"]}).filter(([,data]:any)=>data?.exists).length===8,"summaries ready");
  await until(()=>!queryClient.isFetching({queryKey:["repoSummary"]}),"summaries settled");
  button("Fetch all")!.click();await until(()=>done===8&&useStore.getState().busy===null,"bulk complete");
  assert(peak===3,`Expected three mutation workers, saw ${peak}`);assert(done===8,"A repository was skipped");
});
await test("O015 bounded ranking agrees with full sort at 100000 files",async()=>{
  const files=Array.from({length:100000},(_,id)=>({id,score:(id*7919)%1009}));
  const started=performance.now();const bounded=topMatches(files,item=>item.score);const boundedMs=performance.now()-started;
  const fullStarted=performance.now();const full=files.filter(item=>item.score>0).sort((a,b)=>b.score-a.score).slice(0,80);const fullMs=performance.now()-fullStarted;
  assert(JSON.stringify(bounded)===JSON.stringify(full),"Ranking/tie order changed");
  results.push({id:"O015 timing sample",status:"measurement",boundedMs,fullSortMs:fullMs,entries:files.length});
});
await test("O017 session grouping reused across filter edits",async()=>{
  repo.worktrees=Array.from({length:100},(_,i)=>({path:`/fixture/worktree-${i}`,branch:`branch-${i}`,locked:false}));
  let pathReads=0;const sessions=Array.from({length:5000},(_,i)=>({id:`session-${i}`,agent:"codex",title:`Session ${i}`,status:"finished",time:1,get path(){pathReads++;return `/fixture/worktree-${i%100}`;}}));
  handlers.agent_sessions=()=>sessions;handlers.repo_status=()=>repo.status;
  mount(<AgentsView repo={repo} tabId={tabId}/>);await until(()=>document.body.textContent?.includes("Session 4999")??false,"session cards");
  const reads=pathReads;document.querySelector<HTMLButtonElement>('[title="Filter worktrees and sessions"]')!.click();await sleep(20);
  input(document.querySelector<HTMLInputElement>('[aria-label="Filter worktrees and sessions"]')!,"Session 4999");await sleep(20);
  assert(pathReads===reads,`Filter regrouped ${pathReads-reads} session paths`);assert(document.body.textContent?.includes("Session 4999"),"Matching session hidden");
});
await test("B074 server-default merge is explicit in selector, confirmation and payload",async()=>{
  handlers.repo_remote=()=>({provider:"bitbucket",service:"bitbucket-server",host:"example.invalid",path:"P/repo",web:"https://example.invalid"});
  handlers.pr_details=()=>({number:1,title:"Fixture PR",author:"Fixture",source:"feature",target:"main",draft:false,state:"open",body:"Fixture",updated:"",url:"https://example.invalid/pr/1"});handlers.pr_comments=()=>[];
  mount(<><PullRequestView repo={repo} tabId={tabId} number={1}/><Dialogs/></>);
  await until(()=>!!button("Merge Pull Request")&&!button("Merge Pull Request")!.disabled,"merge ready");
  const select=document.querySelector('select')!;assert(select.options.length===1&&select.value==="default","Unsupported merge choices offered");
  button("Merge Pull Request")!.click();await until(()=>!!button("Merge"),"confirmation");assert(document.body.textContent?.includes("server default strategy"),"Confirmation promised a specific strategy");
  button("Merge")!.click();await until(()=>calls.some(c=>c.command==="merge_pr"),"merge IPC");assert(calls.find(c=>c.command==="merge_pr")!.args.method==="default","Default choice lost");
});
await test("B079 signing Off writes explicit false for both defaults",async()=>{
  handlers.git_config_get=()=>({"commit.gpgsign":"true","tag.gpgSign":"true"});
  mount(<SigningSection/>);await until(()=>document.querySelectorAll<HTMLInputElement>('input[type="checkbox"]:checked').length===2,"signing defaults loaded");
  const checkboxes=[...document.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')];checkboxes[0].click();await until(()=>calls.some(c=>c.command==="set_git_config"&&c.args.key==="commit.gpgsign"),"commit signing write");
  checkboxes[1].click();await until(()=>calls.some(c=>c.command==="set_git_config"&&c.args.key==="tag.gpgSign"),"tag signing write");
  assert(calls.filter(c=>c.command==="set_git_config").every(c=>c.args.value==="false"),"Off removed an override instead of disabling it");
});
await test("B083 force choice retains its reviewed branch and destination",async()=>{
  const review={branch:"refs/heads/main",head:sha("a"),remote:"origin",remoteRef:"refs/heads/main",url:"/fixture/remote.git",remoteOid:sha("c"),trackingOid:sha("c"),config:"fixture-config"};
  handlers.push_review=()=>review;handlers.run_action=({action})=>action.force?{ok:true,message:"Pushed"}:{ok:false,message:"rejected non-fast-forward"};
  await openRepoPath(repo.path);mount(<Dialogs/>);await act(repo.path,{type:"push",force:false},true);
  await until(()=>!!button("Force Push"),"reviewed force choices");
  repo={...repo,head_branch:"other",head_sha:sha("b")};await openRepoPath(repo.path);button("Force Push")!.click();
  await until(()=>calls.some(c=>c.command==="run_action"&&c.args.action.force),"reviewed force IPC");
  const pushed=calls.find(c=>c.command==="run_action"&&c.args.action.force)!.args.action;
  assert(pushed.review.branch==="refs/heads/main"&&pushed.review.head===sha("a")&&pushed.review.remoteRef==="refs/heads/main","Force push was rebound to the later checkout");
});
await test("B085 all commit shortcuts refuse unresolved conflicts",async()=>{
  repo.status={staged:[],unstaged:[{path:"a.txt",status:"U"}],conflicted:true};useStore.getState().setDraft(repo.path,{summary:"Unresolved",description:"Details",amend:false});
  mount(<RightPanel repo={repo} tabId={tabId}/>);await sleep(30);
  for(const field of [document.querySelector('input[placeholder="Commit summary"]')!,document.querySelector('textarea[placeholder="Description"]')!]) {
    for(const shiftKey of [false,true])field.dispatchEvent(new KeyboardEvent("keydown",{key:"Enter",ctrlKey:true,shiftKey,bubbles:true}));
  }
  for(const all of [false,true])window.dispatchEvent(new CustomEvent(WIP_COMMIT_EVENT,{detail:{path:repo.path,all}}));await sleep(30);
  assert(!calls.some(c=>c.command==="run_action"),"A conflicted shortcut dispatched a commit or push");
});
for(const amend of [false,true])for(const stripComments of [false,true])await test(`B086 comment cleanup forwarded, amend=${amend}, strip=${stripComments}`,async()=>{
  repo.status.staged=[{path:"a.txt",status:"M"}];useStore.getState().setSettings({stripComments});useStore.getState().setDraft(repo.path,{summary:"Subject",description:"# Template comment",amend});
  mount(<RightPanel repo={repo} tabId={tabId}/>);await until(()=>!!button(amend?"Amend Previous Commit":"Commit Changes to 1 File"),"commit control");button(amend?"Amend Previous Commit":"Commit Changes to 1 File")!.click();
  await until(()=>calls.some(c=>c.command==="run_action"&&c.args.action.type==="commit")&&useStore.getState().busy===null,"commit completed");
  const action=calls.find(c=>c.command==="run_action"&&c.args.action.type==="commit")!.args.action;assert(action.stripComments===stripComments&&action.amend===amend,"Cleanup preference lost");
});
await test("B072 historical blame follows repeated renames",async()=>{
  handlers.file_history=()=>["after.txt","middle.txt","before.txt"].map((path,i)=>({...commit(["c","b","a"][i]),path}));handlers.blame=()=>[];handlers.file_versions=()=>({old:"before",new:"after",binary:false,editable:false});
  for(const [id,file] of [["a","before.txt"],["b","middle.txt"],["c","after.txt"]]) {
    mount(<HistoryView repo={repo} tabId={tabId} file="after.txt" mode="history" sha={sha(id)}/>);await until(()=>calls.some(c=>c.command==="file_versions"&&c.args.file===file),"historical diff path");
    mount(<HistoryView repo={repo} tabId={tabId} file="after.txt" mode="blame" sha={sha(id)}/>);await until(()=>calls.some(c=>c.command==="blame"&&c.args.file===file&&c.args.rev===sha(id)),"historical blame path");
  }
  mount(<HistoryView repo={repo} tabId={tabId} file="after.txt" mode="blame" sha={null}/>);await until(()=>calls.some(c=>c.command==="blame"&&c.args.file==="after.txt"&&c.args.rev===null),"working-folder path");
  assert(!calls.some(c=>c.command==="blame"&&c.args.rev===sha("a")&&c.args.file!=="before.txt"),"Premature blame used current filename");
});
await test("B095 shared preference fields inherit captions and descriptions",async()=>{
  mount(<><PrefRow label="Enabled" help="Enable help"><PrefToggle value={false} onChange={()=>{}}/></PrefRow><PrefRow label="Count"><PrefNum value={1} onSave={()=>{}}/></PrefRow><PrefRow label="Details"><PrefArea value="" onSave={()=>{}}/></PrefRow><PrefRow label="Text" help="Text help" warning="Text warning" error="Text problem"><PrefText value="" onSave={()=>{}}/></PrefRow><PrefRow label="Choice"><PrefSelect value="one" onChange={()=>{}} options={[["one","One"]]}/></PrefRow><PrefRow label="Path" help="Path help"><PrefPath value="" onSave={()=>{}} title="Program file"/></PrefRow></>);
  const fields=[...document.querySelectorAll<HTMLInputElement|HTMLSelectElement|HTMLTextAreaElement>('input,textarea,select')];assert(fields.length===6,"Shared controls missing");
  const names=fields.map(field=>field.getAttribute("aria-label")??document.getElementById(field.getAttribute("aria-labelledby")??"")?.textContent);assert(JSON.stringify(names)===JSON.stringify(["Enabled","Count","Details","Text","Choice","Program file"]),`Unnamed preferences: ${names}`);
  const text=fields[3];assert(text.getAttribute("aria-invalid")==="true","Validation state not associated");assert(text.getAttribute("aria-describedby")!.split(" ").every(id=>!!document.getElementById(id)),"Description references missing");
  const browse=button("Browse")!;assert(browse.getAttribute("aria-label")==="Browse: Program file","Browse name ambiguous");for(const field of [...fields,browse]){field.focus();assert(document.activeElement===field,"Control lost focusability");}
});
for(const what of ["Fetch all","Pull all"])await test(`B097 ${what} includes unknown and filtered summaries`,async()=>{
  const paths=["/fixture/bulk-0","/fixture/bulk-1","/fixture/bulk-2"];useStore.getState().setSettings({favorites:paths,recents:[]});
  const held:(()=>void)[]=[];handlers.repo_summary=({path})=>path===paths[0]?{exists:true,branch:"main",changes:0,ahead:0,behind:0}:new Promise(resolve=>held.push(()=>resolve({exists:true,branch:"main",changes:0,ahead:0,behind:0})));
  const visited:string[]=[];handlers.run_action=({path})=>{visited.push(path);return{ok:path!==paths[1],message:path===paths[1]?"Repository unavailable":"Completed"};};
  mount(<RepoManagementDialog onClose={()=>{}}/>);await until(()=>queryClient.getQueriesData({queryKey:["repoSummary",paths[0]]}).some(([,data]:any)=>data?.exists),"visible summary");
  input(document.querySelector<HTMLInputElement>('[aria-label="Search repositories"]')!,"bulk-0");await sleep(30);button(what)!.click();await until(()=>visited.length===4&&useStore.getState().busy===null,"every requested action attempted");
  assert(paths.every(path=>visited.includes(path)),"Filtered/unknown repository skipped");assert(useStore.getState().toasts.some(toast=>toast.text.includes("3 of 4 repositories; 1 failed")),"Bulk result omitted failures");
  while(held.length)held.shift()!();await sleep(30);
});
await test("B098 lossy preview requires a reviewed conversion",async()=>{
  handlers.file_versions=()=>({old:"old",new:"replacement text",binary:false,editable:false,decoding_loss:true,new_token:"original-token",encoding:"UTF-8"});handlers.file_diff=()=>[];
  mount(<><DiffView repo={repo} tabId={tabId} target={{source:{kind:"Unstaged"},file:"file.txt",untracked:false,parent:null}}/><Dialogs/></>);await until(()=>document.body.textContent?.includes("Some bytes could not be decoded")??false,"decoding warning");
  assert(!button("Edit This File"),"Lossy preview was ordinarily editable");assert(!calls.some(c=>c.command==="save_file"),"Preview saved without consent");button("Convert preview to UTF-8...")!.click();await until(()=>!!button("Convert preview to UTF-8"),"conversion review");button("Convert preview to UTF-8")!.click();
  await until(()=>calls.some(c=>c.command==="save_file"),"explicit conversion");const save=calls.find(c=>c.command==="save_file")!.args;assert(save.utf8===true&&save.expected==="original-token","Conversion consent or freshness token lost");
});
await test("B098 conflict output requires conversion consent",async()=>{
  handlers.conflict_info=()=>({working_token:"conflict-token",working:"<<<<<<< ours\nreplacement\n=======\ntheirs\n>>>>>>> theirs\n",ours:"replacement",theirs:"theirs",base:"base",ours_label:"ours",theirs_label:"theirs",binary:false,decoding_loss:true});
  mount(<><ConflictView repo={repo} tabId={tabId} file="file.txt"/><Dialogs/></>);await until(()=>!!button("Create Conflict Chunks")||!!button("Save Output"),"conflict ready");(button("Create Conflict Chunks")??button("Save Output"))!.click();
  await until(()=>!!button("Convert output to UTF-8"),"lossy conflict confirmation");assert(!calls.some(c=>c.command==="run_action"||c.command==="save_file"),"Conflict saved before consent");button("Convert output to UTF-8")!.click();await until(()=>calls.some(c=>c.command==="save_file"||c.command==="run_action"),"explicit conflict save");
  const save=calls.find(c=>c.command==="save_file"||c.command==="run_action")!.args;assert(save.utf8===true||save.action?.utf8===true,"Conflict conversion flag lost");
});
await test("O026 dense hunk rows stay bounded and preserve original indexes",async()=>{
  useStore.getState().setSettings({diffMode:"hunk",wordWrap:false});repo.status.unstaged=[{path:"dense.txt",status:"M"}];
  handlers.file_versions=()=>({old:"",new:"dense\n".repeat(4000),binary:false,editable:true,new_token:"dense"});
  handlers.file_diff=()=>[{kind:"Hunk",old_no:null,new_no:null,text:"@@ -0,0 +1,4000 @@"},...Array.from({length:4000},(_,i)=>({kind:"Added",old_no:null,new_no:i+1,text:`line-${i}`}))];
  const started=performance.now();mount(<DiffView repo={repo} tabId={tabId} target={{source:{kind:"Unstaged"},file:"dense.txt",untracked:false,parent:null}}/>);
  await until(()=>!!document.querySelector('tbody tr[title]'),"virtual rows");assert(document.querySelectorAll('tbody tr').length<100,`All rows mounted: ${document.querySelectorAll("tbody tr").length}, viewport ${document.querySelector("[data-hunk]")?.parentElement?.clientHeight}`);
  const scroll=document.querySelector<HTMLDivElement>('[data-hunk]')!.parentElement!;scroll.scrollTop=2500*19;scroll.dispatchEvent(new Event("scroll"));
  await until(()=>[...document.querySelectorAll('tbody tr')].some(row=>row.textContent?.endsWith("line-2500")),"middle row");
  [...document.querySelectorAll<HTMLTableRowElement>('tbody tr')].find(row=>row.textContent?.endsWith("line-2500"))!.dispatchEvent(new KeyboardEvent("keydown",{key:"Enter",bubbles:true}));await until(()=>!!button("Stage 1 Line"),"keyboard selection");button("Stage 1 Line")!.click();await until(()=>calls.some(c=>c.command==="run_action"&&c.args.action.type==="patch"),"patch");
  const patch=calls.find(c=>c.command==="run_action"&&c.args.action.type==="patch")!.args.action;assert(patch.hunk===0&&patch.lines[0]===2500&&patch.expected.length===4001,"Row indexes changed");assert(document.querySelectorAll('tbody tr').length<100,"Scroll expanded DOM");
  results.push({id:"O026 dense diff",status:"measurement",milliseconds:Math.round(performance.now()-started),rows:document.querySelectorAll('tbody tr').length});
  useStore.getState().setSettings({wordWrap:true});await until(()=>!!document.querySelector('.monaco-diff-editor'),"wrapped virtual comparison");assert(!button("Stage Hunk"),"Wrapped fallback retained partial actions");
});
await test("O026 limit marker offers virtual comparison and safe full-file stage",async()=>{
  useStore.getState().setSettings({diffMode:"hunk"});repo.status.unstaged=[{path:"dense.txt",status:"M"}];
  handlers.file_versions=()=>({old:"",new:"dense\n".repeat(100000),binary:false,editable:true,new_token:"dense"});handlers.file_diff=()=>[{kind:"Limit",old_no:null,new_no:null,text:"Large diff"}];
  mount(<DiffView repo={repo} tabId={tabId} target={{source:{kind:"Unstaged"},file:"dense.txt",untracked:true,parent:null}}/>);
  await until(()=>!!document.querySelector('.monaco-diff-editor')&&!!button("Stage File"),"large fallback");assert(!button("Stage Hunk"),"Truncated patch offered");button("Stage File")!.click();await until(()=>calls.some(c=>c.command==="run_action"&&c.args.action.type==="stage"),"full stage");assert(!calls.some(c=>c.command==="run_action"&&c.args.action.type==="patch"),"Limit submitted as patch");
});
root.unmount();queryClient.clear();
(document.body as HTMLElement).innerHTML="";const report=document.createElement("pre");report.id="verification-results";report.textContent=JSON.stringify({results,passed:results.filter(result=>result.status==="passed").length,failed:results.filter(result=>result.status==="failed").length});document.body.append(report);
