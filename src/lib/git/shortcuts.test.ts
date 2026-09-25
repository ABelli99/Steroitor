import { describe, expect, it } from "vitest";
import { keyOf } from "../shortcuts";
import type { Workspace } from "../workspace/workspace.svelte";
import { gitShortcuts } from "./shortcuts";

const key = (key: string, modifiers: Partial<Record<"ctrlKey" | "altKey" | "shiftKey", boolean>> = {}, code = "") =>
  keyOf({ ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, key, code, ...modifiers });

describe("keyOf", () => {
  it("normalises modifiers and letters", () => {
    expect(key("k", { ctrlKey: true, altKey: true })).toBe("Ctrl+Alt+K");
    expect(key("K", { ctrlKey: true, shiftKey: true })).toBe("Ctrl+Shift+K");
  });

  it("maps the key left of 1 to backquote on any layout", () => {
    expect(key("\\", { altKey: true }, "Backquote")).toBe("Alt+`");
  });
});

describe("gitShortcuts", () => {
  const bindings = gitShortcuts({ repo: () => null, workspace: {} as Workspace, openVcsMenu: () => {}, openTerminal: () => {} });

  it("leaves Ctrl+D to the editor (select next occurrence) outside Git panels", () => {
    expect(bindings["Ctrl+D"].git).toBeTypeOf("function");
    expect(bindings["Ctrl+D"].editor).toBeUndefined();
  });

  it("uses Ctrl+T for a new terminal outside Git panels and for Update in Git panels", () => {
    expect(bindings["Ctrl+T"].editor).toBeTypeOf("function");
    expect(bindings["Ctrl+T"].terminal).toBe(bindings["Ctrl+T"].editor);
    expect(bindings["Ctrl+T"].git).toBeTypeOf("function");
    expect(bindings["Ctrl+T"].editor).not.toBe(bindings["Ctrl+T"].git);
  });

  it("binds every shortcut listed in CLAUDE.md", () => {
    for (const shortcut of ["Ctrl+K", "Ctrl+Alt+K", "Ctrl+Shift+K", "Ctrl+Alt+A", "Alt+9", "Alt+`"]) {
      expect(bindings[shortcut], shortcut).toBeDefined();
    }
  });
});
