import { describe, expect, it } from "vitest";
import { layoutGraph } from "./graph";

const c = (hash: string, ...parents: string[]) => ({ hash, parents });

describe("layoutGraph", () => {
  it("keeps a linear history on one lane", () => {
    const rows = layoutGraph([c("c", "b"), c("b", "a"), c("a")]);
    expect(rows.map((row) => row.lane)).toEqual([0, 0, 0]);
    expect(rows.every((row) => row.width === 1)).toBe(true);
    expect(rows[2].segments.filter((s) => s.half === "bottom")).toEqual([]);
  });

  it("opens a lane for the second parent of a merge and joins it back", () => {
    //   m        merge di f in main
    //   | \
    //   b  f
    //   | /
    //   a
    const rows = layoutGraph([c("m", "b", "f"), c("b", "a"), c("f", "a"), c("a")]);
    expect(rows.map((row) => row.lane)).toEqual([0, 0, 1, 0]);

    const mergeBottom = rows[0].segments.filter((s) => s.half === "bottom");
    expect(mergeBottom).toContainEqual(expect.objectContaining({ from: 0, to: 1 }));

    const joinTop = rows[3].segments.filter((s) => s.half === "top");
    expect(joinTop).toContainEqual(expect.objectContaining({ from: 1, to: 0 }));
    expect(rows[3].width).toBe(2);
  });

  it("gives separate branch tips their own lanes and colours", () => {
    const rows = layoutGraph([c("x", "a"), c("y", "a"), c("a")]);
    expect(rows[0].lane).toBe(0);
    expect(rows[1].lane).toBe(1);
    expect(rows[0].color).not.toBe(rows[1].color);
    expect(rows[2].lane).toBe(0);
  });

  it("frees lanes so later branches reuse them", () => {
    const rows = layoutGraph([c("m", "b", "f"), c("f", "a"), c("b", "a"), c("a"), c("z")]);
    expect(rows[4].lane).toBe(0);
    expect(rows[4].width).toBe(1);
  });
});
