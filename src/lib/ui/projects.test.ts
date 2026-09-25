import { describe, expect, it } from "vitest";
import { folderInitial, folderName } from "./projects";

describe("folderInitial", () => {
  it("uses the first letter or digit of the folder name", () => {
    expect(folderInitial("C:\\Repos\\steroitor\\")).toBe("S");
    expect(folderInitial("/home/me/.config")).toBe("C");
    expect(folderInitial("C:\\Repos\\2026-notes")).toBe("2");
  });

  it("falls back to a question mark without a folder", () => {
    expect(folderInitial(null)).toBe("?");
    expect(folderName(null)).toBe("Nessuna cartella");
  });
});
