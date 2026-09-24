import { describe, expect, it } from "vitest";
import { parseConflicts, resolution, resolveAll } from "./conflicts";

const merge = [
  "prima",
  "<<<<<<< HEAD",
  "nostra 1",
  "nostra 2",
  "=======",
  "loro",
  ">>>>>>> feature",
  "dopo",
  "",
].join("\n");

describe("parseConflicts", () => {
  it("finds the ours and theirs sections of a merge conflict", () => {
    const [conflict] = parseConflicts(merge);
    expect(merge.slice(conflict.ours.from, conflict.ours.to)).toBe("nostra 1\nnostra 2\n");
    expect(merge.slice(conflict.theirs.from, conflict.theirs.to)).toBe("loro\n");
    expect(conflict.base).toBeNull();
    expect(merge.slice(conflict.from, conflict.to).startsWith("<<<<<<< HEAD")).toBe(true);
    expect(merge.slice(conflict.to)).toBe("dopo\n");
  });

  it("supports the diff3 base section", () => {
    const text = "<<<<<<< ours\na\n||||||| base\nb\n=======\nc\n>>>>>>> theirs\n";
    const [conflict] = parseConflicts(text);
    expect(text.slice(conflict.ours.from, conflict.ours.to)).toBe("a\n");
    expect(text.slice(conflict.base!.from, conflict.base!.to)).toBe("b\n");
    expect(text.slice(conflict.theirs.from, conflict.theirs.to)).toBe("c\n");
  });

  it("handles several conflicts, CRLF and a block at the end of the file", () => {
    const text = "<<<<<<< a\r\n1\r\n=======\r\n2\r\n>>>>>>> b\r\nmezzo\r\n<<<<<<< a\n3\n=======\n4\n>>>>>>> b";
    const conflicts = parseConflicts(text);
    expect(conflicts).toHaveLength(2);
    expect(conflicts[1].to).toBe(text.length);
  });

  it("ignores unterminated blocks and look-alike lines", () => {
    expect(parseConflicts("<<<<<<< a\nx\n=======\ny\n")).toEqual([]);
    expect(parseConflicts("========\n<<<<<<<<\n")).toEqual([]);
  });
});

describe("resolution", () => {
  const [conflict] = parseConflicts(merge);

  it("builds each choice", () => {
    expect(resolution(merge, conflict, "ours")).toBe("nostra 1\nnostra 2\n");
    expect(resolution(merge, conflict, "theirs")).toBe("loro\n");
    expect(resolution(merge, conflict, "ours-theirs")).toBe("nostra 1\nnostra 2\nloro\n");
    expect(resolution(merge, conflict, "theirs-ours")).toBe("loro\nnostra 1\nnostra 2\n");
  });

  it("resolves every block at once", () => {
    const twice = merge + merge;
    expect(resolveAll(twice, "theirs")).toBe("prima\nloro\ndopo\nprima\nloro\ndopo\n");
    expect(parseConflicts(resolveAll(twice, "ours"))).toEqual([]);
  });
});
