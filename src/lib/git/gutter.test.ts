import { describe, expect, it } from "vitest";
import { Chunk } from "@codemirror/merge";
import { Text } from "@codemirror/state";
import { buildMarkers } from "./gutter";

const doc = (...lines: string[]) => Text.of(lines);

function markersByLine(baseline: Text, current: Text) {
  const result: Record<number, string> = {};
  buildMarkers(baseline, Chunk.build(baseline, current), current).between(0, current.length, (from, _to, marker) => {
    result[current.lineAt(from).number] = (marker as unknown as { kind: string }).kind;
  });
  return result;
}

describe("buildMarkers", () => {
  const base = doc("uno", "due", "tre", "quattro");

  it("marks nothing when the document matches HEAD", () => {
    expect(markersByLine(base, base)).toEqual({});
  });

  it("marks inserted lines as added", () => {
    expect(markersByLine(base, doc("uno", "nuova", "due", "tre", "quattro"))).toEqual({ 2: "added" });
  });

  it("marks changed lines as modified", () => {
    expect(markersByLine(base, doc("uno", "DUE", "TRE", "quattro"))).toEqual({ 2: "modified", 3: "modified" });
  });

  it("marks the line after a deletion", () => {
    expect(markersByLine(base, doc("uno", "quattro"))).toEqual({ 2: "deleted" });
  });

  it("marks every line of a new file as added", () => {
    expect(markersByLine(Text.empty, doc("a", "b"))).toEqual({ 1: "added", 2: "added" });
  });
});
