import { describe, expect, it } from "vitest";
import { repoNameFromUrl } from "./clone";

describe("repoNameFromUrl", () => {
  it("handles SSH, HTTPS and local paths", () => {
    expect(repoNameFromUrl("git@github.com:ABelli99/Steroitor.git")).toBe("Steroitor");
    expect(repoNameFromUrl("https://github.com/org/progetto.git/")).toBe("progetto");
    expect(repoNameFromUrl("https://gitlab.com/org/sub/app")).toBe("app");
    expect(repoNameFromUrl("C:\\repos\\locale.git")).toBe("locale");
  });

  it("falls back to a generic name", () => {
    expect(repoNameFromUrl("   ")).toBe("repository");
  });
});
