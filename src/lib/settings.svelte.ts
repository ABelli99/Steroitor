const STORAGE_KEY = "steroitor.settings";

export type UpdateMethod = "merge" | "rebase";

interface Settings {
  updateMethod: UpdateMethod;
  warnCrlf: boolean;
  /** Ultima cartella in cui si è clonato un repository. */
  cloneParent: string;
  /** Id della shell per i nuovi terminali; vuoto = la prima rilevata. */
  terminalShell: string;
  /** Versione per cui l'utente ha chiuso l'avviso di aggiornamento. */
  skippedVersion: string;
}

const defaults: Settings = { updateMethod: "merge", warnCrlf: true, cloneParent: "", terminalShell: "", skippedVersion: "" };

function load(): Settings {
  try {
    return { ...defaults, ...JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") };
  } catch {
    return defaults;
  }
}

export const settings = $state<Settings>(load());

export function saveSettings() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    /* preferenza non essenziale */
  }
}
