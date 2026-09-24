import { describe, expect, it } from "vitest";
import { refLabel, relativeTime } from "./format";

describe("relativeTime", () => {
  const now = 1_700_000_000;

  it("uses the largest fitting unit", () => {
    expect(relativeTime(now - 30, now)).toBe("adesso");
    expect(relativeTime(now - 3 * 3600, now)).toBe("3 ore fa");
    expect(relativeTime(now - 86_400, now)).toBe("ieri");
  });
});

describe("refLabel", () => {
  it("classifies decorations from git log %D", () => {
    expect(refLabel("HEAD -> main")).toEqual({ label: "main", kind: "head" });
    expect(refLabel("tag: v1.0")).toEqual({ label: "v1.0", kind: "tag" });
    expect(refLabel("origin/main")).toEqual({ label: "origin/main", kind: "branch" });
  });
});
