import { describe, expect, it } from "vitest";
import { ensureSyntaxTree } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { scriptWithTemplates } from "./embedded";
import { syntaxErrors } from "./module";

function errorsIn(doc: string) {
  const state = EditorState.create({ doc, extensions: scriptWithTemplates({}) });
  ensureSyntaxTree(state, doc.length, 5000);
  return syntaxErrors(state);
}

describe("syntaxErrors", () => {
  it("accepts valid JavaScript, including inline Angular templates", () => {
    expect(errorsIn("const a = 1;\nconst t = { template: `<p (click)=\"go()\">{{ a }}</p>` };")).toEqual([]);
  });

  it("reports a broken statement", () => {
    expect(errorsIn("const a = ;").length).toBeGreaterThan(0);
  });
});
