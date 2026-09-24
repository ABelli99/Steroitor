import { loadSession, saveSession } from "../backend";
import type { FileTree, TreeSnapshot } from "../explorer/tree.svelte";
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

export interface SessionData extends Partial<TreeSnapshot> {
  version: 1;
  wrap: boolean;
  minimap?: boolean;
  tabs: SessionTab[];
}

const SAVE_DELAY_MS = 1000;

export async function readSession(): Promise<SessionData | null> {
  const raw = await loadSession().catch(() => null);
  if (!raw) return null;
  try {
    const data = JSON.parse(raw.replace(/^\uFEFF/, "")) as SessionData;
    return data.version === 1 ? data : null;
  } catch {
    return null;
  }
}

export function persistSession(workspace: Workspace, tree: FileTree) {
  let timer: ReturnType<typeof setTimeout> | undefined;

  const flush = async () => {
    clearTimeout(timer);
    timer = undefined;
    const data: SessionData = { ...workspace.snapshot(), ...tree.snapshot() };
    await saveSession(JSON.stringify(data)).catch(console.error);
  };

  const schedule = () => {
    clearTimeout(timer);
    timer = setTimeout(flush, SAVE_DELAY_MS);
  };

  const unsubscribers = [workspace.onChange(schedule), tree.onChange(schedule)];
  return { flush, stop: () => unsubscribers.forEach((unsubscribe) => unsubscribe()) };
}
