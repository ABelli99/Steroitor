import type { Branch } from "./api";

export type BranchNode =
  | { kind: "folder"; name: string; key: string; children: BranchNode[] }
  | { kind: "branch"; name: string; branch: Branch };

export interface BranchRow {
  node: BranchNode;
  depth: number;
}

type MutableFolder = { kind: "folder"; name: string; key: string; children: BranchNode[]; folders: Map<string, MutableFolder> };

const byName = (a: BranchNode, b: BranchNode) =>
  a.kind === b.kind ? a.name.localeCompare(b.name, undefined, { sensitivity: "base" }) : a.kind === "folder" ? -1 : 1;

function sortTree(nodes: BranchNode[]): BranchNode[] {
  return nodes
    .map((node) => (node.kind === "folder" ? { kind: node.kind, name: node.name, key: node.key, children: sortTree(node.children) } : node))
    .sort(byName);
}

/** `feature/ui/login` → cartella feature › cartella ui › branch login; i prefissi comuni si fondono. */
export function buildBranchTree(branches: Branch[], keyPrefix = ""): BranchNode[] {
  const root: MutableFolder = { kind: "folder", name: "", key: keyPrefix, children: [], folders: new Map() };
  for (const branch of branches) {
    const parts = branch.name.split("/");
    const leaf = parts.pop()!;
    let folder = root;
    for (const part of parts) {
      let next = folder.folders.get(part);
      if (!next) {
        next = { kind: "folder", name: part, key: `${folder.key}/${part}`, children: [], folders: new Map() };
        folder.folders.set(part, next);
        folder.children.push(next);
      }
      folder = next;
    }
    folder.children.push({ kind: "branch", name: leaf, branch });
  }
  return sortTree(root.children);
}

/** Righe visibili: le cartelle in `collapsed` nascondono i loro figli. */
export function flattenBranchTree(nodes: BranchNode[], collapsed: ReadonlySet<string>, depth = 0): BranchRow[] {
  return nodes.flatMap((node) => {
    const row = { node, depth };
    if (node.kind === "branch" || collapsed.has(node.key)) return [row];
    return [row, ...flattenBranchTree(node.children, collapsed, depth + 1)];
  });
}

/** Chiavi di tutte le cartelle, tranne quelle sul percorso del branch corrente (restano aperte). */
export function foldersToCollapse(nodes: BranchNode[]): string[] {
  const keys: string[] = [];
  const visit = (node: BranchNode): boolean => {
    if (node.kind === "branch") return node.branch.current;
    const containsCurrent = node.children.map(visit).some(Boolean);
    if (!containsCurrent) keys.push(node.key);
    return containsCurrent;
  };
  nodes.forEach(visit);
  return keys;
}
