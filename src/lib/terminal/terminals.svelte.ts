import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface TerminalSession {
  id: number;
  title: string;
  exited: boolean;
}

type Sink = (data: string) => void;

/**
 * Sessioni PTY aperte. L'output può arrivare prima che il frontend conosca l'id (o prima che la
 * vista xterm sia montata): resta in coda finché qualcuno si collega.
 */
class Terminals {
  sessions = $state<TerminalSession[]>([]);
  activeId = $state<number | null>(null);

  #sinks = new Map<number, Sink>();
  #pending = new Map<number, string[]>();
  #listening: Promise<unknown> | null = null;
  #count = 0;

  async open(cwd: string | null, cols = 80, rows = 24) {
    await this.#listen();
    const id = await invoke<number>("terminal_open", { cwd, cols, rows });
    this.#count += 1;
    this.sessions.push({ id, title: `Terminale ${this.#count}`, exited: false });
    this.activeId = id;
    return id;
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

  #deliver(id: number, data: string) {
    const sink = this.#sinks.get(id);
    if (sink) return sink(data);
    const queue = this.#pending.get(id) ?? [];
    queue.push(data);
    this.#pending.set(id, queue);
  }

  #listen() {
    this.#listening ??= Promise.all([
      listen<{ id: number; data: string }>("terminal-output", (event) => this.#deliver(event.payload.id, event.payload.data)),
      listen<{ id: number; code: number | null }>("terminal-exit", (event) => {
        const session = this.sessions.find((candidate) => candidate.id === event.payload.id);
        if (session) session.exited = true;
        this.#deliver(event.payload.id, `\r\n\x1b[2m[processo terminato${event.payload.code === null ? "" : `, codice ${event.payload.code}`}]\x1b[0m\r\n`);
      }),
    ]);
    return this.#listening;
  }
}

export const terminals = new Terminals();
