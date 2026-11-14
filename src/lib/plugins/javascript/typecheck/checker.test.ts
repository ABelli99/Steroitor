import { describe, expect, it } from "vitest";
import type { Probe } from "../../../backend";
import { Checker, type Answer } from "./checker";

const disk: Record<string, string> = {
  "C:/app/tsconfig.json": JSON.stringify({ compilerOptions: { strict: true, target: "ES2022", module: "ESNext", moduleResolution: "Bundler" } }),
  "C:/app/src/user.ts": "export interface User { name: string }\nexport const makeUser = (name: string): User => ({ name });",
};

function probe(path: string): Probe {
  const normalized = path.replaceAll("\\", "/").replace(/\/+$/, "");
  const file = Object.keys(disk).find((candidate) => candidate.toLowerCase() === normalized.toLowerCase());
  if (file) return { kind: "file", content: disk[file] };
  const prefix = `${normalized.toLowerCase()}/`;
  const children = Object.keys(disk).filter((candidate) => candidate.toLowerCase().startsWith(prefix));
  if (!children.length) return { kind: "missing" };
  const dirs = children.map((child) => child.slice(prefix.length).split("/")).filter((parts) => parts.length > 1).map((parts) => parts[0]);
  return { kind: "dir", dirs: [...new Set(dirs)] };
}

const libs = import.meta.glob<string>("/node_modules/typescript/lib/lib.*.d.ts", { query: "?raw", import: "default" });
const checker = new Checker(async (fileName) => libs[`/node_modules/typescript/lib/${fileName}`]?.());

async function resolve<T>(ask: () => Promise<Answer<T>>): Promise<T> {
  for (let round = 0; round < 20; round++) {
    const result = await ask();
    if ("value" in result) return result.value;
    checker.provide(result.missing.map((missing) => [missing, probe(missing)]));
  }
  throw new Error("too many rounds");
}

const check = (path: string, content: string) => resolve(() => checker.diagnose(path, content));

describe("Checker", () => {
  it("resolves imports from disk and reports type errors", async () => {
    const content = `import { makeUser } from "./user";\nconst age: number = makeUser("Ada").name;\n`;
    const diagnostics = await check("C:\\app\\src\\main.ts", content);
    expect(diagnostics).toHaveLength(1);
    expect(diagnostics[0].severity).toBe("error");
    expect(content.slice(diagnostics[0].from, diagnostics[0].to)).toBe("age");
  });

  it("accepts correct code using the standard library", async () => {
    const diagnostics = await check("C:\\app\\src\\main.ts", `import { makeUser } from "./user";\nconst names: string[] = [makeUser("Ada")].map((user) => user.name.toUpperCase());\n`);
    expect(diagnostics).toEqual([]);
  });

  it("sees a dependency change after invalidation", async () => {
    const content = `import { makeUser } from "./user";\nconst name: string = makeUser("Ada").name;\n`;
    expect(await check("C:\\app\\src\\main.ts", content)).toEqual([]);
    disk["C:/app/src/user.ts"] = "export const makeUser = (name: string) => ({ name: name.length });";
    checker.invalidate(["C:\\app\\src"]);
    expect(await check("C:\\app\\src\\main.ts", content)).toHaveLength(1);
  });

  it("completes array methods with their documentation", async () => {
    const path = "C:\\app\\src\\main.ts";
    const content = "const a = [1, 2, 3];\na.";
    const items = await resolve(() => checker.complete(path, content, content.length));
    const map = items.find((item) => item.name === "map");
    expect(map?.kind).toBe("method");
    const details = await resolve(() => checker.details(path, content, content.length, map!));
    expect(details?.signature).toContain("map<U>");
    expect(details?.documentation).toContain("callback");
  });

  it("replaces the dot when a member of a possibly undefined value needs optional chaining", async () => {
    const content = "declare const win: { event: string } | undefined;\nwin.";
    const items = await resolve(() => checker.complete("C:\\app\\src\\main.ts", content, content.length));
    const event = items.find((item) => item.name === "event")!;
    const span = event.replacementSpan!;
    const applied = content.slice(0, span.start) + event.insertText + content.slice(span.start + span.length);
    expect(applied.endsWith("win?.event")).toBe(true);
  });
});
