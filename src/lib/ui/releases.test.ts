import { describe, expect, it } from "vitest";
import { newerRelease, parseVersion } from "./releases";

const release = (tag: string) => ({ tag_name: tag, html_url: `https://github.com/ABelli99/Steroitor/releases/tag/${tag}` });

describe("parseVersion", () => {
  it("reads tags with or without the v prefix", () => {
    expect(parseVersion("v1.10.2")).toEqual([1, 10, 2]);
    expect(parseVersion("1.1.1")).toEqual([1, 1, 1]);
  });

  it("rejects anything that is not major.minor.patch", () => {
    expect(parseVersion("v1.2")).toBeNull();
    expect(parseVersion("v1.2.3-beta")).toBeNull();
    expect(parseVersion("nightly")).toBeNull();
  });
});

describe("newerRelease", () => {
  it("reports a newer release with its page", () => {
    expect(newerRelease("1.1.1", release("v1.2.0"))).toEqual({
      version: "1.2.0",
      url: "https://github.com/ABelli99/Steroitor/releases/tag/v1.2.0",
    });
  });

  it("compares numerically, not as text", () => {
    expect(newerRelease("1.9.0", release("v1.10.0"))?.version).toBe("1.10.0");
    expect(newerRelease("1.10.0", release("v1.9.9"))).toBeNull();
  });

  it("ignores the same version and malformed answers", () => {
    expect(newerRelease("1.1.1", release("v1.1.1"))).toBeNull();
    expect(newerRelease("1.1.1", { message: "Not Found" } as never)).toBeNull();
  });
});
