import { afterEach, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { VirtualFileRows, fileWindow } from "../components/VirtualFileRows";
import { topMatches } from "./ranking";
import { literalIgnorePath } from "./ignore-pattern";
import { useStore } from "./store";
import type { FileChange } from "./api";

const original = useStore.getState();
afterEach(() => { useStore.setState(original, true); vi.restoreAllMocks(); });
it("ranks only the best 80 while preserving full-sort tie ordering", () => {
  const items = Array.from({length: 20000}, (_, id) => ({id, score: id % 27}));
  const expected = items.filter(i => i.score > 0).sort((a,b) => b.score-a.score).slice(0,80);
  expect(topMatches(items, i => i.score)).toEqual(expected);
  expect(topMatches(items, () => 0)).toEqual([]);
});
it("bounds a 10000-file list and computes windows at either end", () => {
  const files = Array.from({length:10000}, (_, i) => ({path: `file-${i}`, status: "M"}) as FileChange);
  let rendered = 0;
  renderToStaticMarkup(<VirtualFileRows files={files} render={file => { rendered++; return <button>{file.path}</button>; }} />);
  expect(rendered).toBeLessThan(50);
  expect(fileWindow(10000, 9999*24, 600).end).toBe(10000);
  expect(fileWindow(10000, 0, 600).start).toBe(0);
});
it("escapes exact ignore paths without interpreting brackets or spaces", () => {
  expect(literalIgnorePath("[ab].txt")).toBe("/\\[ab\\].txt");
  expect(literalIgnorePath(" leading trailing ")).toBe("/\\ leading\\ trailing\\ ");
});
it("keeps case-distinct POSIX repositories and releases closed-tab UI state", () => {
  useStore.getState().openRepoTab("/repos/Project", "Project");
  useStore.getState().openRepoTab("/repos/project", "project");
  expect(useStore.getState().tabs.filter(t => t.kind === "repo")).toHaveLength(2);
  for (let i=0;i<100;i++) {
    useStore.getState().openRepoTab(`/repos/temp-${i}`, `temp-${i}`);
    const id=useStore.getState().activeId;
    useStore.getState().setUi(id,{ multi:Array(100).fill("sha") });
    useStore.getState().closeTab(id);
    expect(useStore.getState().ui[id]).toBeUndefined();
  }
  expect(Object.keys(useStore.getState().ui)).toHaveLength(2);
  expect(useStore.getState().closed).toHaveLength(10);
});


it("computes tree totals once while preserving status counts", async () => {
  const { buildTree }=await import("./file-tree");
  let reads=0;
  const files=Array.from({length:1000},(_,i)=>({path:`a/b/c/d/e/f/g/h/i/j/file-${i}`,get status(){reads++;return i%2?"?":"M";}}));
  const tree=buildTree(files);
  expect(tree.counts).toEqual({A:500,M:500});
  const before=reads;
  let node=tree;while(node.dirs.size){node=[...node.dirs.values()][0];expect(node.counts).toEqual(tree.counts);}
  expect(reads).toBe(before);expect(reads).toBeLessThan(4000);
});


it("keeps full signing fingerprints, long IDs, external forms and missing keys visible", async () => {
  const { signingOptions }=await import("./signing-keys");
  const key={id:"ABCDEF0123456789",fingerprint:"123456789012345678901234ABCDEF0123456789",uid:"Test"};
  for(const configured of [key.id,key.fingerprint,"0x"+key.id,key.fingerprint+"!"]) expect(signingOptions([key],configured).some(option=>option[0]===configured)).toBe(true);
  expect(signingOptions([key],"missing").at(-1)).toEqual(["missing","missing (configured key not found)"]);
});


it("bounds summary reads and discards cancelled queued work", async () => {
  const { createReadPool }=await import("./read-pool"); const run=createReadPool(3);
  let active=0,peak=0,started=0;const releases:(()=>void)[]=[];
  const work=()=>new Promise<number>(resolve=>{started++;active++;peak=Math.max(peak,active);releases.push(()=>{active--;resolve(started);});});
  const requests=Array.from({length:12},()=>run(work));
  const cancelled=new AbortController();const rejected=run(work,cancelled.signal).catch(error=>error.name);cancelled.abort();
  for(let i=0;i<12;i++){await new Promise(resolve=>setTimeout(resolve,0));releases.shift()!();}
  await Promise.all(requests);expect(await rejected).toBe("AbortError");expect(started).toBe(12);expect(peak).toBe(3);
});


it("distinguishes same-number tracker issues and proposed branch names", async () => {
  const { issueIdentity, branchFromIssue }=await import("./hosting");
  const first={number:42,key:"AAA-42",tracker:"jira",repo:"AAA",title:"Fix it"} as import("./hosting").Issue;
  const second={...first,key:"BBB-42",repo:"BBB"};
  expect(issueIdentity(first)).not.toBe(issueIdentity(second));
  await branchFromIssue("/repo",first);const initial=useStore.getState().dialog;
  await branchFromIssue("/repo",second);const latest=useStore.getState().dialog;
  expect(initial?.kind).toBe("prompt");expect(latest?.kind).toBe("prompt");
  if(initial?.kind==="prompt"&&latest?.kind==="prompt") expect(initial.value).not.toBe(latest.value);
});
