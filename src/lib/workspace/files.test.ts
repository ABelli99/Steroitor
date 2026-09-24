import { describe, expect, it } from "vitest";
import { detectEol, fileName, isInsideDir, samePath } from "./files";
import { detectLanguage } from "../editor/languages";

describe("detectEol", () => {
  it("uses the first line break found", () => {
    expect(detectEol("a\r\nb\nc", "\n")).toBe("\r\n");
    expect(detectEol("a\nb\r\nc", "\r\n")).toBe("\n");
  });

  it("falls back when there are no line breaks", () => {
    expect(detectEol("single line", "\r\n")).toBe("\r\n");
  });
});

describe("paths", () => {
  it("extracts the file name from Windows and POSIX paths", () => {
    expect(fileName("C:\\Users\\me\\notes.txt")).toBe("notes.txt");
    expect(fileName("/home/me/notes.txt")).toBe("notes.txt");
  });

  it("compares Windows paths ignoring case and separators", () => {
    expect(samePath("C:\\Repo\\File.ts", "c:/repo/file.ts")).toBe(true);
    expect(samePath("C:\\Repo\\a.ts", "C:\\Repo\\b.ts")).toBe(false);
  });

  it("detects containment without matching sibling prefixes", () => {
    expect(isInsideDir("C:\\Repo\\src\\a.ts", "c:/repo")).toBe(true);
    expect(isInsideDir("C:\\Repo", "C:\\Repo")).toBe(true);
    expect(isInsideDir("C:\\Repository\\a.ts", "C:\\Repo")).toBe(false);
    expect(isInsideDir("C:\\a.ts", "C:\\")).toBe(true);
  });
});

describe("detectLanguage", () => {
  it("maps extensions to languages", () => {
    expect(detectLanguage("C:\\src\\App.TSX")).toBe("tsx");
    expect(detectLanguage("script.py")).toBe("python");
    expect(detectLanguage("README")).toBeNull();
    expect(detectLanguage(null)).toBeNull();
  });
});
