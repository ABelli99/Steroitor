import { loadSession, saveSession } from "../backend";
import type { FileTree, TreeSnapshot } from "../explorer/tree.svelte";
import { samePath, type Eol } from "./files";
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

function parse(raw: string | null): SessionData | null {
  if (!raw) return null;
  try {
    const data = JSON.parse(raw.replace(/^﻿/, "")) as SessionData;
    return data.version === 1 ? data : null;
  } catch {
    return null;
  }
}

/** Sessione salvata per `folder`; prima della 1.2 ce n'era una sola, che vale ancora per la sua cartella. */
export async function readSession(folder: string | null): Promise<SessionData | null> {
  const saved = parse(await loadSession(folder).catch(() => null));
  if (saved || !folder) return saved;
  const legacy = parse(await loadSession(null).catch(() => null));
  return legacy?.folder && samePath(legacy.folder, folder) ? legacy : null;
}

export function persistSession(workspace: Workspace, tree: FileTree) {
  let timer: ReturnType<typeof setTimeout> | undefined;

  const flush = async () => {
    clearTimeout(timer);
    timer = undefined;
    const data: SessionData = { ...workspace.snapshot(), ...tree.snapshot() };
    await saveSession(tree.root, JSON.stringify(data)).catch(console.error);
  };

  const schedule = () => {
    clearTimeout(timer);
    timer = setTimeout(flush, SAVE_DELAY_MS);
  };

  const unsubscribers = [workspace.onChange(schedule), tree.onChange(schedule)];
  return { flush, stop: () => unsubscribers.forEach((unsubscribe) => unsubscribe()) };
}
