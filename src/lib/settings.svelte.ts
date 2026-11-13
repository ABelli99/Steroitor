const STORAGE_KEY = "steroitor.settings";

export type UpdateMethod = "merge" | "rebase";

export interface EditorStyle {
  fontSize: number;
  lineHeight: number;
  tabSize: number;
  indentSize: number;
  useTabs: boolean;
}

interface Settings {
  updateMethod: UpdateMethod;
  warnCrlf: boolean;
  /** Ultima cartella in cui si è clonato un repository. */
  cloneParent: string;
  /** Id della shell per i nuovi terminali; vuoto = la prima rilevata. */
  terminalShell: string;
  /** Versione per cui l'utente ha chiuso l'avviso di aggiornamento. */
  skippedVersion: string;
  editor: EditorStyle;
  disabledPlugins: string[];
}

export const defaultEditorStyle: EditorStyle = { fontSize: 14, lineHeight: 1.5, tabSize: 4, indentSize: 2, useTabs: false };

const defaults: Settings = {
  updateMethod: "merge",
  warnCrlf: true,
  cloneParent: "",
  terminalShell: "",
  skippedVersion: "",
  editor: defaultEditorStyle,
  disabledPlugins: [],
};

function load(): Settings {
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
    return { ...defaults, ...saved, editor: { ...defaultEditorStyle, ...saved.editor } };
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
