import { loadSession, saveSession } from "../backend";
import type { Eol } from "./files";
import type { Workspace } from "./workspace.svelte";

export interface SessionTab {
  path: string | null;
  name: string;
  content?: string;
  encoding: string;
  bom: boolean;
  eol: Eol;
  active: boolean;
}

export interface SessionData {
  version: 1;
  wrap: boolean;
  tabs: SessionTab[];
}

const SAVE_DELAY_MS = 1000;

export async function readSession(): Promise<SessionData | null> {
  const raw = await loadSession().catch(() => null);
  if (!raw) return null;
  try {
    const data = JSON.parse(raw) as SessionData;
    return data.version === 1 ? data : null;
  } catch {
    return null;
  }
}

export function persistSession(workspace: Workspace) {
  let timer: ReturnType<typeof setTimeout> | undefined;

  const flush = async () => {
    clearTimeout(timer);
    timer = undefined;
    await saveSession(JSON.stringify(workspace.snapshot())).catch(console.error);
  };

  const unsubscribe = workspace.onChange(() => {
    clearTimeout(timer);
    timer = setTimeout(flush, SAVE_DELAY_MS);
  });

  return { flush, stop: unsubscribe };
}
