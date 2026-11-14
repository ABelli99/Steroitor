import type { EditorState, Extension } from "@codemirror/state";
import type { LineMatch } from "../backend";

export interface Target {
  name: string;
  from: number;
  to: number;
}

/** Ciò che l'app mette a disposizione di un plugin per navigare tra i file del progetto. */
export interface DefinitionHost {
  root: string | null;
  currentPath: string | null;
  goToPosition(pos: number): void;
  openAt(path: string, line: number): Promise<void>;
  fileIndex(): Promise<string[]>;
  searchFiles(root: string, needle: string, extensions: string[]): Promise<LineMatch[]>;
  readFile(path: string): Promise<string | null>;
}

export interface DefinitionProvider {
  targetAt(state: EditorState, pos: number): Target | null;
  goTo(host: DefinitionHost, state: EditorState, pos: number): Promise<void>;
}

export interface PluginModule {
  /** Estensioni CodeMirror per linguaggio: i propri e quelli che il plugin arricchisce (es. `html`). */
  extensions: Record<string, Extension>;
  /** File cambiati su disco nelle cartelle indicate (watcher del progetto). */
  onFilesChanged?(dirs: string[]): void;
}

export interface PluginLanguage {
  id: string;
  name: string;
  fileExtensions: string[];
}

/** Interruttore del plugin, mostrato nella pagina Plugin; vale dal prossimo avvio. */
export interface PluginOption {
  id: string;
  label: string;
  description: string;
  /** Valore finché l'utente non lo cambia; senza, spenta. */
  default?: boolean;
}

export interface Plugin {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  languages: PluginLanguage[];
  options?: PluginOption[];
  load(options: Record<string, boolean>): Promise<PluginModule>;
}
