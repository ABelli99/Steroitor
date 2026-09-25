import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Entry } from "../backend";

const disk: Record<string, Entry[]> = {};

vi.mock("../backend", () => ({
  listDir: async (path: string) => {
    if (!disk[path]) throw new Error(`missing ${path}`);
    return disk[path];
  },
  listFiles: async () => [],
  watchFolder: async () => {},
}));

const { FileTree } = await import("./tree.svelte");

const dir = (path: string): Entry => ({ name: path.split("/").pop()!, path, isDir: true });
const file = (path: string): Entry => ({ name: path.split("/").pop()!, path, isDir: false });

describe("FileTree compact folders", () => {
  beforeEach(() => {
    for (const key of Object.keys(disk)) delete disk[key];
    disk["/r"] = [dir("/r/dir1"), dir("/r/other"), file("/r/readme.md")];
    disk["/r/dir1"] = [dir("/r/dir1/dir2")];
    disk["/r/dir1/dir2"] = [file("/r/dir1/dir2/file1"), file("/r/dir1/dir2/file2")];
    disk["/r/other"] = [file("/r/other/a"), dir("/r/other/sub")];
  });

  const labels = (tree: InstanceType<typeof FileTree>) => tree.rows.map((row) => `${row.depth}:${row.label}`);

  it("merges a chain of single folders into one row when expanded", async () => {
    const tree = new FileTree("p");
    await tree.open("/r");
    await tree.expand("/r/dir1");
    expect(labels(tree)).toEqual(["0:dir1/dir2", "1:file1", "1:file2", "0:other", "0:readme.md"]);

    const [chain] = tree.rows;
    expect(chain.head).toBe("/r/dir1");
    expect(chain.entry.path).toBe("/r/dir1/dir2");
  });

  it("collapses the whole chain from its head and restores it on expand", async () => {
    const tree = new FileTree("p");
    await tree.open("/r");
    await tree.expand("/r/dir1");
    tree.collapse("/r/dir1");
    expect(labels(tree)[0]).toBe("0:dir1");
    await tree.expand("/r/dir1");
    expect(labels(tree)[0]).toBe("0:dir1/dir2");
  });

  it("does not compact folders with more than one child", async () => {
    const tree = new FileTree("p");
    await tree.open("/r");
    await tree.expand("/r/other");
    expect(labels(tree)).toContain("0:other");
    expect(labels(tree)).toContain("1:sub");
  });
});
