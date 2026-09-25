import { isInsideDir } from "../workspace/files";

interface Identified {
  id: string;
}

interface Rooted extends Identified {
  root: string | null;
}

/** Sposta `id` alla posizione d'inserimento `toIndex`, calcolata sull'elenco prima dello spostamento. */
export function moveTo<T extends Identified>(items: T[], id: string, toIndex: number): T[] {
  const from = items.findIndex((item) => item.id === id);
  if (from === -1) return items;
  const next = items.filter((item) => item.id !== id);
  next.splice(toIndex > from ? toIndex - 1 : toIndex, 0, items[from]);
  return next;
}

/**
 * Progetto a cui appartiene un comando Git eseguito in `cwd`: la radice più specifica che lo contiene.
 * A pari radice (due cartelle dello stesso repository) vince il progetto attivo, che è quello su cui si sta lavorando.
 */
export function commandOwner<T extends Rooted>(projects: T[], cwd: string, activeId: string | null): T | null {
  const active = projects.find((project) => project.id === activeId) ?? null;
  let owner: T | null = null;
  for (const project of projects) {
    if (!project.root || !isInsideDir(cwd, project.root)) continue;
    const longer = !owner?.root || project.root.length > owner.root.length;
    const tieWithActive = owner?.root?.length === project.root.length && project === active;
    if (longer || tieWithActive) owner = project;
  }
  return owner ?? active;
}
