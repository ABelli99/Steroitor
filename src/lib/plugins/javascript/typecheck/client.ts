import { probePaths, type Probe } from "../../../backend";
import type { Answer, CompletionDetails, CompletionItem, TypeDiagnostic } from "./checker";

export type Request =
  | { type: "diagnose"; path: string; content: string }
  | { type: "complete"; path: string; content: string; pos: number }
  | { type: "details"; path: string; content: string; pos: number; item: CompletionItem }
  | { type: "provide"; entries: [string, Probe][] }
  | { type: "invalidate"; dirs: string[] };

/** Ogni giro risolve un livello di import: le catene di `node_modules` (es. Angular, rxjs) arrivano a qualche decina. */
const MAX_ROUNDS = 60;

let worker: Worker | null = null;
let nextId = 0;
const pending = new Map<number, { resolve: (value: unknown) => void; reject: (error: Error) => void }>();
let queue: Promise<unknown> = Promise.resolve();

export function startTypeCheck() {
  if (worker) return;
  worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
  worker.onmessage = ({ data }: MessageEvent<{ id: number; result?: unknown; error?: string }>) => {
    const call = pending.get(data.id);
    pending.delete(data.id);
    if (data.error !== undefined) call?.reject(new Error(data.error));
    else call?.resolve(data.result);
  };
}

function call<T>(request: Request): Promise<T> {
  startTypeCheck();
  const id = nextId++;
  return new Promise<T>((resolve, reject) => {
    pending.set(id, { resolve: resolve as (value: unknown) => void, reject });
    worker!.postMessage({ id, request });
  });
}

/** In fila: i giri di lettura di una richiesta non si mescolano con quelli di un'altra. */
function serial<T>(task: () => Promise<T>): Promise<T> {
  const run = queue.then(task, task);
  queue = run.catch(() => {});
  return run;
}

/** Ripete la richiesta leggendo da disco i file che mancano, finché il worker può rispondere. */
const answer = <T>(request: Request) =>
  serial(async (): Promise<T> => {
    for (let round = 0; round < MAX_ROUNDS; round++) {
      const result = await call<Answer<T>>(request);
      if ("value" in result) return result.value;
      const probes = await probePaths(result.missing);
      await call({ type: "provide", entries: result.missing.map((missing, index): [string, Probe] => [missing, probes[index]]) });
    }
    throw new Error("Troppi giri di lettura dei file del progetto");
  });

export const typeCheck = (path: string, content: string) => answer<TypeDiagnostic[]>({ type: "diagnose", path, content });

export const complete = (path: string, content: string, pos: number) => answer<CompletionItem[]>({ type: "complete", path, content, pos });

export const completionDetails = (path: string, content: string, pos: number, item: CompletionItem) =>
  answer<CompletionDetails | null>({ type: "details", path, content, pos, item });

export const invalidateTypeCheck = (dirs: string[]) => serial(() => call<void>({ type: "invalidate", dirs }));
