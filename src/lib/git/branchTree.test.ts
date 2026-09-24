import { describe, expect, it } from "vitest";
import type { Branch } from "./api";
import { buildBranchTree, flattenBranchTree, foldersToCollapse, type BranchNode } from "./branchTree";

const branch = (name: string, current = false): Branch => ({ name, current, remote: false, upstream: null, hash: "abc" });

const shape = (nodes: BranchNode[]): unknown[] =>
  nodes.map((node) => (node.kind === "branch" ? node.name : { [node.name]: shape(node.children) }));

describe("buildBranchTree", () => {
  it("nests names by slash and merges common prefixes", () => {
    const tree = buildBranchTree([branch("feature/ui/login"), branch("main"), branch("feature/ui/menu"), branch("feature/api")]);
    expect(shape(tree)).toEqual([{ feature: [{ ui: ["login", "menu"] }, "api"] }, "main"]);
  });

  it("lists folders before branches, alphabetically and case-insensitively", () => {
    const tree = buildBranchTree([branch("zeta"), branch("Alpha"), branch("fix/b"), branch("beta")]);
    expect(shape(tree)).toEqual([{ fix: ["b"] }, "Alpha", "beta", "zeta"]);
  });

  it("uses the prefix to keep local and remote folder keys apart", () => {
    const [folder] = buildBranchTree([branch("origin/main")], "remote");
    expect(folder.kind === "folder" && folder.key).toBe("remote/origin");
  });
});

describe("flattenBranchTree", () => {
  const tree = buildBranchTree([branch("feature/a"), branch("feature/b"), branch("main")], "local");

  it("shows everything when nothing is collapsed", () => {
    expect(flattenBranchTree(tree, new Set()).map((row) => `${row.depth}:${row.node.name}`)).toEqual(["0:feature", "1:a", "1:b", "0:main"]);
  });

  it("hides the children of collapsed folders", () => {
    expect(flattenBranchTree(tree, new Set(["local/feature"])).map((row) => row.node.name)).toEqual(["feature", "main"]);
  });
});

describe("foldersToCollapse", () => {
  it("keeps open only the folders leading to the current branch", () => {
    const tree = buildBranchTree([branch("feature/ui/login", true), branch("feature/api/x"), branch("fix/y")], "local");
    expect(foldersToCollapse(tree).sort()).toEqual(["local/feature/api", "local/fix"]);
  });
});
