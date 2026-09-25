import { describe, expect, it } from "vitest";
import { commandOwner, moveTo } from "./routing";

const ids = (items: Array<{ id: string }>) => items.map((item) => item.id).join("");

describe("moveTo", () => {
  const items = [{ id: "a" }, { id: "b" }, { id: "c" }, { id: "d" }];

  it("moves an item down, before the insertion point", () => {
    expect(ids(moveTo(items, "a", 3))).toBe("bcad");
    expect(ids(moveTo(items, "a", 4))).toBe("bcda");
  });

  it("moves an item up", () => {
    expect(ids(moveTo(items, "d", 0))).toBe("dabc");
    expect(ids(moveTo(items, "c", 1))).toBe("acbd");
  });

  it("leaves the order alone when dropped in place or unknown", () => {
    expect(ids(moveTo(items, "b", 1))).toBe("abcd");
    expect(ids(moveTo(items, "b", 2))).toBe("abcd");
    expect(moveTo(items, "x", 0)).toBe(items);
  });
});

describe("commandOwner", () => {
  const app = { id: "app", root: "C:\\repos\\app" };
  const lib = { id: "lib", root: "C:\\repos\\app\\lib" };
  const other = { id: "other", root: "C:\\repos\\other" };
  const empty = { id: "empty", root: null };

  it("picks the most specific root containing the command folder", () => {
    expect(commandOwner([app, lib, other], "C:\\repos\\app\\lib", "other")?.id).toBe("lib");
    expect(commandOwner([app, lib, other], "C:\\repos\\app", "lib")?.id).toBe("app");
  });

  it("prefers the active project between two with the same root", () => {
    const twin = { id: "twin", root: "C:\\repos\\app" };
    expect(commandOwner([app, twin], "C:\\repos\\app", "twin")?.id).toBe("twin");
    expect(commandOwner([twin, app], "C:\\repos\\app", "app")?.id).toBe("app");
  });

  it("falls back to the active project for commands outside every root", () => {
    expect(commandOwner([app, empty], "D:\\cloni", "empty")?.id).toBe("empty");
  });
});
