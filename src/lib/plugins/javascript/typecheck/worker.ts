import { Checker } from "./checker";
import type { Request } from "./client";

// Un chunk per file: si caricano solo le lib richieste dal `target`/`lib` del progetto.
const libs = import.meta.glob<string>("/node_modules/typescript/lib/lib.*.d.ts", { query: "?raw", import: "default" });

const checker = new Checker(async (fileName) => libs[`/node_modules/typescript/lib/${fileName}`]?.());

function handle(request: Request) {
  if (request.type === "diagnose") return checker.diagnose(request.path, request.content);
  if (request.type === "complete") return checker.complete(request.path, request.content, request.pos);
  if (request.type === "details") return checker.details(request.path, request.content, request.pos, request.item);
  if (request.type === "provide") return checker.provide(request.entries);
  return checker.invalidate(request.dirs);
}

self.onmessage = async ({ data }: MessageEvent<{ id: number; request: Request }>) => {
  try {
    self.postMessage({ id: data.id, result: await handle(data.request) });
  } catch (error) {
    self.postMessage({ id: data.id, error: String(error) });
  }
};
