import { describe, expect, it } from "vitest";
import { EditorState, Text, type RangeSet } from "@codemirror/state";
import type { GutterMarker } from "@codemirror/view";
import { blameField, blameMarkers } from "./blame";

const line = (hash: string, author: string) => ({ hash, author, timestamp: 1_700_000_000, summary: "msg" });

function authorsByLine(doc: Text, set: RangeSet<GutterMarker>) {
  const result: Record<number, string> = {};
  set.between(0, doc.length, (from, _to, marker) => {
    result[doc.lineAt(from).number] = (marker as unknown as { line: { author: string } }).line.author;
  });
  return result;
}

describe("blameMarkers", () => {
  it("puts one marker per blamed line", () => {
    const doc = Text.of(["a", "b"]);
    expect(authorsByLine(doc, blameMarkers([line("1", "Ada"), line("2", "Bob")], doc))).toEqual({ 1: "Ada", 2: "Bob" });
  });

  it("ignores blame lines beyond the document", () => {
    const doc = Text.of(["solo"]);
    expect(authorsByLine(doc, blameMarkers([line("1", "Ada"), line("2", "Bob")], doc))).toEqual({ 1: "Ada" });
  });
});

describe("blameField", () => {
  it("keeps annotations attached to their lines when a line is inserted above", () => {
    const field = blameField([line("1", "Ada"), line("2", "Bob")]);
    let state = EditorState.create({ doc: "uno\ndue", extensions: field });
    state = state.update({ changes: { from: 0, insert: "nuova\n" } }).state;
    expect(authorsByLine(state.doc, state.field(field))).toEqual({ 2: "Ada", 3: "Bob" });
  });
});
