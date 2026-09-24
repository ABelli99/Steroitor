import type { FileStatus } from "./api";

export interface FileSelection {
  path: string;
  status: FileStatus;
}

/** Elemento selezionato nei pannelli Git, usato da Ctrl+D (Show Diff). */
export const gitSelection = $state<{ commit: string | null; file: FileSelection | null }>({ commit: null, file: null });

export type DiffRequest = { kind: "file"; file: FileSelection } | { kind: "commit"; hash: string } | { kind: "merge"; path: string };

/** Un file in conflitto si apre nel merge tool, gli altri nel diff. */
export const fileRequest = (file: FileSelection): DiffRequest =>
  file.status === "conflict" ? { kind: "merge", path: file.path } : { kind: "file", file };

/** Diff aperto nel dialog (Ctrl+D, doppio click). */
export const diffState = $state<{ request: DiffRequest | null }>({ request: null });

export function showSelectedDiff(panel: "git" | "commit") {
  if (panel === "git" && gitSelection.commit) diffState.request = { kind: "commit", hash: gitSelection.commit };
  if (panel === "commit" && gitSelection.file) diffState.request = fileRequest(gitSelection.file);
}
