import type { FileChange } from "./api";
export interface TreeNode {
  name: string; path: string; dirs: Map<string, TreeNode>; files: FileChange[]; counts: Record<string, number>;
}
export function buildTree(files: FileChange[]): TreeNode {
  const create = (name: string, path: string): TreeNode => ({ name, path, dirs:new Map(), files:[], counts:{} });
  const root = create("", "");
  for (const file of files) {
    let node = root;
    for (const part of file.path.split("/").slice(0,-1)) {
      const path = node.path ? `${node.path}/${part}` : part;
      if (!node.dirs.has(part)) node.dirs.set(part, create(part,path));
      node = node.dirs.get(part)!;
    }
    node.files.push(file);
  }
  const rollup = (node: TreeNode) => {
    for (const file of node.files) {
      const status = file.status === "?" ? "A" : ["R","U"].includes(file.status) ? "M" : file.status;
      node.counts[status] = (node.counts[status] ?? 0)+1;
    }
    for (const child of node.dirs.values()) {
      rollup(child);
      for (const [status,count] of Object.entries(child.counts)) node.counts[status]=(node.counts[status] ?? 0)+count;
    }
  };
  rollup(root);
  return root;
}
