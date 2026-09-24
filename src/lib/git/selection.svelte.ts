import type { FileStatus } from "./api";

export interface FileSelection {
  path: string;
  status: FileStatus;
}

/** Elemento selezionato nei pannelli Git, usato da Ctrl+D (Show Diff). */
export const gitSelection = $state<{ commit: string | null; file: FileSelection | null }>({ commit: null, file: null });

export type DiffRequest = { kind: "file"; file: FileSelection } | { kind: "commit"; hash: string };

/** Diff aperto nel dialog (Ctrl+D, doppio click). */
export const diffState = $state<{ request: DiffRequest | null }>({ request: null });

export function showSelectedDiff(panel: "git" | "commit") {
  if (panel === "git" && gitSelection.commit) diffState.request = { kind: "commit", hash: gitSelection.commit };
  if (panel === "commit" && gitSelection.file) diffState.request = { kind: "file", file: gitSelection.file };
}
