import { describe, expect, it } from "vitest";
import { ensureSyntaxTree } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { declarationLine, declaresSelector, findDefinition, resolveImport, targetAt } from "./definition";
import { scriptWithTemplates } from "./embedded";

const component = [
  `import { Component } from "@angular/core";`,
  `import { UserService as Users } from "../services/user.service";`,
  ``,
  `@Component({`,
  `  selector: "app-user-list",`,
  "  template: `<div class=\"list\" (click)=\"save()\"><app-user-card [user]=\"user\"></app-user-card></div>`,",
  "  styles: [`.list { color: red; }`],",
  `})`,
  `export class UserListComponent {`,
  `  user = null;`,
  `  constructor(private users: Users) {}`,
  `  save() { this.users.store(this.user); }`,
  `}`,
].join("\n");

function stateOf(doc: string) {
  const state = EditorState.create({ doc, extensions: scriptWithTemplates({ typescript: true }) });
  ensureSyntaxTree(state, doc.length, 5000);
  return state;
}

const at = (doc: string, needle: string, offset = 1) => doc.indexOf(needle) + offset;

describe("template inline", () => {
  const state = stateOf(component);
  const nodeAt = (needle: string, offset = 1) => ensureSyntaxTree(state, component.length)!.resolveInner(at(component, needle, offset), 1).name;

  it("parses the Angular template as HTML", () => {
    expect(nodeAt("<div")).toBe("TagName");
    expect(nodeAt(" class=")).toBe("AttributeName");
  });

  it("parses the component styles as CSS", () => {
    expect(nodeAt("color:", 0)).toBe("PropertyName");
  });
});

describe("findDefinition", () => {
  const state = stateOf(component);
  const definitionAt = (needle: string, offset = 1) => {
    const target = targetAt(state, at(component, needle, offset));
    return target && findDefinition(state, target);
  };

  it("jumps from a template call to the class method", () => {
    expect(definitionAt("save()\"")).toEqual({ kind: "local", from: component.indexOf("save() {") });
  });

  it("jumps from this.property to the class member", () => {
    expect(definitionAt("user);")).toEqual({ kind: "local", from: component.indexOf("user = null") });
  });

  it("follows a renamed import to the exported name", () => {
    expect(definitionAt("Users)")).toEqual({ kind: "import", source: "../services/user.service", name: "UserService" });
  });

  it("looks up a custom element by selector", () => {
    expect(definitionAt("app-user-card [", 3)).toEqual({ kind: "selector", selector: "app-user-card" });
  });

  it("does nothing on the declaration itself", () => {
    expect(definitionAt("save() {")).toBeNull();
  });
});

describe("resolveImport", () => {
  const files = ["C:\\p\\src\\app\\list.ts", "C:\\p\\src\\services\\user.service.ts", "C:\\p\\src\\shared\\index.ts"];

  it("resolves relative paths with the script extension", () => {
    expect(resolveImport("C:\\p\\src\\app\\list.ts", "../services/user.service", files)).toBe(files[1]);
  });

  it("resolves folders to their index", () => {
    expect(resolveImport("C:\\p\\src\\app\\list.ts", "../shared", files)).toBe(files[2]);
  });

  it("resolves aliases by suffix", () => {
    expect(resolveImport("C:\\p\\src\\app\\list.ts", "@app/services/user.service", files)).toBe(files[1]);
  });

  it("ignores packages", () => {
    expect(resolveImport("C:\\p\\src\\app\\list.ts", "@angular/core", files)).toBeNull();
  });
});

describe("declarationLine / declaresSelector", () => {
  it("finds the exported declaration", () => {
    expect(declarationLine("import x from 'y';\n\nexport class UserService {}", "UserService")).toBe(3);
  });

  it("matches the exact selector only", () => {
    expect(declaresSelector(`  selector: 'app-user-card',`, "app-user-card")).toBe(true);
    expect(declaresSelector(`  selector: 'app-user-card-large',`, "app-user-card")).toBe(false);
  });
});
