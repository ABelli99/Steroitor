import { readPreference, writePreference } from "./preferences";

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
  /** Opzioni dei plugin: id plugin → id opzione → attiva. */
  pluginOptions: Record<string, Record<string, boolean>>;
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
  pluginOptions: {},
};

function load(): Settings {
  const saved = readPreference("settings") as Partial<Settings>;
  return { ...defaults, ...saved, editor: { ...defaultEditorStyle, ...saved.editor } };
}

export const settings = $state<Settings>(load());

export const saveSettings = () => writePreference("settings", settings);
