import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { settings } from "../settings.svelte";

export interface Shell {
  id: string;
  name: string;
}

export interface TerminalSession {
  id: number;
  title: string;
  exited: boolean;
}

type Sink = (data: string) => void;

/**
 * Sessioni PTY aperte di un progetto. L'output può arrivare prima che il frontend conosca l'id (o prima che la
 * vista xterm sia montata): resta in coda finché qualcuno si collega.
 */
export class Terminals {
  sessions = $state<TerminalSession[]>([]);
  activeId = $state<number | null>(null);
  shells = $state<Shell[]>([]);

  #sinks = new Map<number, Sink>();
  #pending = new Map<number, string[]>();
  #listening: Promise<Array<() => void>> | null = null;

  constructor(readonly project: string) {}

  async loadShells() {
    this.shells = await invoke<Shell[]>("terminal_shells");
  }

  /** `shell` è l'id di una shell rilevata; senza, quella predefinita nelle impostazioni (o la prima trovata). */
  async open(cwd: string | null, shell = settings.terminalShell || null, cols = 80, rows = 24) {
    await this.#listen();
    const opened = await invoke<{ id: number; shell: string }>("terminal_open", { project: this.project, cwd, shell, cols, rows });
    this.sessions.push({ id: opened.id, title: this.#titleFor(opened.shell), exited: false });
    this.activeId = opened.id;
    return opened.id;
  }

  #titleFor(shell: string) {
    const taken = new Set(this.sessions.map((session) => session.title));
    let title = shell;
    for (let n = 2; taken.has(title); n++) title = `${shell} (${n})`;
    return title;
  }

  /** Cambio di cartella: chiude tutti i terminali e, se ce n'erano, ne riapre uno solo in `cwd`. */
  async relocate(cwd: string | null) {
    if (!this.sessions.length) return;
    const showingTerminal = this.activeId !== null;
    for (const session of [...this.sessions]) this.close(session.id);
    await this.open(cwd);
    if (!showingTerminal) this.activeId = null;
  }

  attach(id: number, sink: Sink) {
    this.#sinks.set(id, sink);
    for (const chunk of this.#pending.get(id) ?? []) sink(chunk);
    this.#pending.delete(id);
    return () => {
      if (this.#sinks.get(id) === sink) this.#sinks.delete(id);
    };
  }

  write(id: number, data: string) {
    invoke("terminal_write", { id, data }).catch(() => {});
  }

  resize(id: number, cols: number, rows: number) {
    invoke("terminal_resize", { id, cols, rows }).catch(() => {});
  }

  close(id: number) {
    invoke("terminal_close", { id }).catch(() => {});
    const index = this.sessions.findIndex((session) => session.id === id);
    if (index !== -1) this.sessions.splice(index, 1);
    this.#sinks.delete(id);
    this.#pending.delete(id);
    if (this.activeId === id) this.activeId = this.sessions.at(Math.max(0, index - 1))?.id ?? null;
  }

  /** Progetto chiuso: shell terminate e niente più eventi. */
  async dispose() {
    for (const session of [...this.sessions]) this.close(session.id);
    const unlisteners = await this.#listening;
    unlisteners?.forEach((unlisten) => unlisten());
    this.#listening = null;
  }

  #deliver(id: number, data: string) {
    const sink = this.#sinks.get(id);
    if (sink) return sink(data);
    const queue = this.#pending.get(id) ?? [];
    queue.push(data);
    this.#pending.set(id, queue);
  }

  #listen() {
    const appWindow = getCurrentWebviewWindow();
    this.#listening ??= Promise.all([
      appWindow.listen<{ project: string; id: number; data: string }>("terminal-output", (event) => {
        if (event.payload.project === this.project) this.#deliver(event.payload.id, event.payload.data);
      }),
      appWindow.listen<{ project: string; id: number; code: number | null }>("terminal-exit", (event) => {
        if (event.payload.project !== this.project) return;
        const session = this.sessions.find((candidate) => candidate.id === event.payload.id);
        if (session) session.exited = true;
        this.#deliver(event.payload.id, `\r\n\x1b[2m[processo terminato${event.payload.code === null ? "" : `, codice ${event.payload.code}`}]\x1b[0m\r\n`);
      }),
    ]);
    return this.#listening;
  }
}
