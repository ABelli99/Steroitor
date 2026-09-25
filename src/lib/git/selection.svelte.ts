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

export type DiffRequest = { kind: "file"; file: FileSelection } | { kind: "commit"; hash: string } | { kind: "merge"; path: string };

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
export const fileRequest = (file: FileSelection): DiffRequest =>
  file.status === "conflict" ? { kind: "merge", path: file.path } : { kind: "file", file };

export function showSelectedDiff(selection: GitSelection, diff: DiffState, panel: "git" | "commit") {
  if (panel === "git" && selection.commit) diff.request = { kind: "commit", hash: selection.commit };
  if (panel === "commit" && selection.file) diff.request = fileRequest(selection.file);
}
