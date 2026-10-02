import type { FileStatus } from "./api";

export interface FileSelection {
  path: string;
  status: FileStatus;
}

/** Elemento selezionato nei pannelli Git, usato da Ctrl+D (Show Diff). */
export interface GitSelection {
  commit: string | null;
  file: FileSelection | null;
}

/** `root` è il repository a cui appartiene: con più repository non è per forza quello selezionato. */
export type DiffRequest = { root: string } & ({ kind: "file"; file: FileSelection } | { kind: "commit"; hash: string } | { kind: "merge"; path: string });

/** Diff aperto nel dialog (Ctrl+D, doppio click). */
export interface DiffState {
  request: DiffRequest | null;
}

export function createGitSelection(): GitSelection {
  const selection = $state<GitSelection>({ commit: null, file: null });
  return selection;
}

export function createDiffState(): DiffState {
  const diff = $state<DiffState>({ request: null });
  return diff;
}

/** Un file in conflitto si apre nel merge tool, gli altri nel diff. */
export const fileRequest = (root: string, file: FileSelection): DiffRequest =>
  file.status === "conflict" ? { root, kind: "merge", path: file.path } : { root, kind: "file", file };

export function showSelectedDiff(root: string, selection: GitSelection, diff: DiffState, panel: "git" | "commit") {
  if (panel === "git" && selection.commit) diff.request = { root, kind: "commit", hash: selection.commit };
  if (panel === "commit" && selection.file) diff.request = fileRequest(root, selection.file);
}
