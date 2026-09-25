export interface ConsoleEntry {
  id: number;
  command: string;
  cwd: string;
  code: number | null;
  durationMs: number;
  startedAt: number;
  stdout: string;
  stderr: string;
  background: boolean;
}

const MAX_ENTRIES = 500;

export class ConsoleLog {
  entries = $state.raw<ConsoleEntry[]>([]);
  showBackground = $state(false);
  hidden = $derived(this.showBackground ? 0 : this.entries.filter((entry) => entry.background && entry.code === 0).length);
  visible = $derived(this.showBackground ? this.entries : this.entries.filter((entry) => !entry.background || entry.code !== 0));

  push(entry: ConsoleEntry) {
    const next = [...this.entries, entry];
    this.entries = next.length > MAX_ENTRIES ? next.slice(-MAX_ENTRIES) : next;
  }

  clear() {
    this.entries = [];
  }

  asText() {
    return this.visible
      .map((entry) => [`$ ${entry.command}`, entry.stdout, entry.stderr].filter(Boolean).join("\n"))
      .join("\n\n");
  }
}
