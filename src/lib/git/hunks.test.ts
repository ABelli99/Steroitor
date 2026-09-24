import { describe, expect, it } from "vitest";
import { Chunk } from "@codemirror/merge";
import { EditorState } from "@codemirror/state";
import { rollbackChange, stagedContent, toText } from "./hunks";

const index = toText("uno\ndue\ntre\nquattro\ncinque\n");
const buffer = toText("UNO\ndue\ntre\nquattro\nCINQUE\n");

describe("stagedContent", () => {
  it("stages only the change that touches the given lines", () => {
    expect(stagedContent(index, buffer, 1, 1)).toEqual({ content: "UNO\ndue\ntre\nquattro\ncinque\n", chunks: 1 });
    expect(stagedContent(index, buffer, 5, 5).content).toBe("uno\ndue\ntre\nquattro\nCINQUE\n");
  });

  it("stages every change in a wider selection", () => {
    expect(stagedContent(index, buffer, 1, 5).content).toBe(buffer.toString());
  });

  it("stages nothing when the lines are unchanged", () => {
    expect(stagedContent(index, buffer, 3, 3)).toEqual({ content: index.toString(), chunks: 0 });
  });

  it("handles insertions and deletions", () => {
    const inserted = toText("uno\nnuova\ndue\n");
    expect(stagedContent(toText("uno\ndue\n"), inserted, 2, 2).content).toBe("uno\nnuova\ndue\n");
    const removed = toText("uno\ntre\n");
    expect(stagedContent(toText("uno\ndue\ntre\n"), removed, 2, 2).content).toBe("uno\ntre\n");
  });

  it("handles a last line without trailing newline", () => {
    expect(stagedContent(toText("a\nb"), toText("a\nB\nc"), 2, 3).content).toBe("a\nB\nc");
    expect(stagedContent(toText("a\nb\n"), toText("a\nB"), 2, 2).content).toBe("a\nB");
  });
});

describe("rollbackChange", () => {
  const rollback = (head: string, current: string) => {
    const headText = toText(head);
    let state = EditorState.create({ doc: current });
    const [chunk] = Chunk.build(headText, state.doc);
    state = state.update({ changes: rollbackChange(headText, state.doc, chunk) }).state;
    return state.doc.toString();
  };

  it("restores the HEAD version of a chunk", () => {
    expect(rollback("uno\ndue\ntre\n", "uno\nDUE\ntre\n")).toBe("uno\ndue\ntre\n");
    expect(rollback("uno\ntre\n", "uno\ndue\ntre\n")).toBe("uno\ntre\n");
    expect(rollback("uno\ndue\ntre\n", "uno\ntre\n")).toBe("uno\ndue\ntre\n");
  });

  it("handles a missing trailing newline on either side", () => {
    expect(rollback("a\nb\n", "a\nB")).toBe("a\nb\n");
    expect(rollback("a\nb", "a\nB\n")).toBe("a\nb");
  });
});
