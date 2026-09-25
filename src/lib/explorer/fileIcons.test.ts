import { describe, expect, it } from "vitest";
import { fileIcon } from "./fileIcons";

describe("fileIcon", () => {
  it("matches by extension, case-insensitive", () => {
    expect(fileIcon("src/App.TS")).toEqual({ glyph: "code", color: "#3178c6" });
  });

  it("prefers special file names", () => {
    expect(fileIcon("repo/.gitignore").glyph).toBe("git");
    expect(fileIcon("Dockerfile").glyph).toBe("docker");
  });

  it("falls back to the generic icon", () => {
    expect(fileIcon("notes.xyz").glyph).toBe("file");
    expect(fileIcon("README").glyph).toBe("file");
    expect(fileIcon(".hidden").glyph).toBe("file");
  });
});
