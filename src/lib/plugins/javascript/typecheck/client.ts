import { probePaths, type Probe } from "../../../backend";
import type { DiagnoseResult, TypeDiagnostic } from "./checker";

export type Request =
  | { type: "diagnose"; path: string; content: string }
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

/** In fila: i giri di lettura di un controllo non si mescolano con quelli di un altro. */
function serial<T>(task: () => Promise<T>): Promise<T> {
  const run = queue.then(task, task);
  queue = run.catch(() => {});
  return run;
}

export const typeCheck = (path: string, content: string) =>
  serial(async (): Promise<TypeDiagnostic[]> => {
    for (let round = 0; round < MAX_ROUNDS; round++) {
      const result = await call<DiagnoseResult>({ type: "diagnose", path, content });
      if ("diagnostics" in result) return result.diagnostics;
      const probes = await probePaths(result.missing);
      await call({ type: "provide", entries: result.missing.map((missing, index): [string, Probe] => [missing, probes[index]]) });
    }
    return [];
  });

export const invalidateTypeCheck = (dirs: string[]) => serial(() => call<void>({ type: "invalidate", dirs }));
