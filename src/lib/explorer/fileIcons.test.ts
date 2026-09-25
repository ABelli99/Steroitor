import { describe, expect, it } from "vitest";
import { fileBadge } from "./fileIcons";

describe("fileBadge", () => {
  it("matches by extension, case-insensitive", () => {
    expect(fileBadge("src/App.TS")?.label).toBe("TS");
  });

  it("prefers special file names", () => {
    expect(fileBadge("repo/.gitignore")?.label).toBe("G");
    expect(fileBadge("Dockerfile")?.label).toBe("D");
  });

  it("falls back to the generic icon", () => {
    expect(fileBadge("notes.xyz")).toBeNull();
    expect(fileBadge("README")).toBeNull();
    expect(fileBadge(".hidden")).toBeNull();
  });
});
