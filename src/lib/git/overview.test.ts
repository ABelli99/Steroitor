import { describe, expect, it } from "vitest";
import { Chunk } from "@codemirror/merge";
import { Text } from "@codemirror/state";
import { buildMarkers } from "./gutter";
import { mergeSpans } from "./overview";

const doc = (...lines: string[]) => Text.of(lines);

function spansByLine(baseline: Text, current: Text) {
  return mergeSpans(buildMarkers(baseline, Chunk.build(baseline, current), current), current).map(({ kind, from, to }) => ({
    kind,
    from: current.lineAt(from).number,
    to: current.lineAt(to).number,
  }));
}

describe("mergeSpans", () => {
  const base = doc("uno", "due", "tre", "quattro");

  it("merges consecutive lines of the same kind", () => {
    expect(spansByLine(base, doc("uno", "a", "b", "due", "tre", "quattro"))).toEqual([{ kind: "added", from: 2, to: 3 }]);
  });

  it("keeps separate changes apart", () => {
    expect(spansByLine(base, doc("UNO", "due", "TRE", "quattro"))).toEqual([
      { kind: "modified", from: 1, to: 1 },
      { kind: "modified", from: 3, to: 3 },
    ]);
  });
});
