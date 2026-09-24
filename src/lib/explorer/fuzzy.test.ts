import { describe, expect, it } from "vitest";
import { fuzzyFilter, fuzzyScore } from "./fuzzy";

describe("fuzzyScore", () => {
  it("rejects targets that do not contain the query in order", () => {
    expect(fuzzyScore("xyz", "src/app.ts")).toBeNull();
    expect(fuzzyScore("ppa", "src/app.ts")).toBeNull();
  });

  it("ignores case and spaces", () => {
    expect(fuzzyScore("A PP", "src/app.ts")).not.toBeNull();
  });
});

describe("fuzzyFilter", () => {
  const paths = [
    "C:\\repo\\src\\lib\\workspace\\workspace.svelte.ts",
    "C:\\repo\\src\\App.svelte",
    "C:\\repo\\src\\lib\\ui\\TabBar.svelte",
    "C:\\repo\\apps\\legacy\\presentation.txt",
  ];

  it("prefers matches in the file name over matches in directories", () => {
    expect(fuzzyFilter("app", paths, 10)[0]).toBe("C:\\repo\\src\\App.svelte");
  });

  it("prefers word-boundary matches", () => {
    expect(fuzzyFilter("tb", paths, 10)[0]).toBe("C:\\repo\\src\\lib\\ui\\TabBar.svelte");
  });

  it("respects the limit", () => {
    expect(fuzzyFilter("s", paths, 2)).toHaveLength(2);
  });
});
