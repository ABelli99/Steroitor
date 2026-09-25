import { SvelteMap, SvelteSet } from "svelte/reactivity";
import { listDir, listFiles, watchFolder, type Entry } from "../backend";
import { fileName, isInsideDir as isInside } from "../workspace/files";

export interface Row {
  /** Elemento su cui agiscono le azioni: in una catena compattata è l'ultima cartella. */
  entry: Entry;
  /** Prima cartella della catena: è quella che si espande o comprime. */
  head: string;
  /** Nome mostrato, es. "dir1/dir2" per una catena compattata. */
  label: string;
  depth: number;
  expanded: boolean;
}

const MAX_CHAIN = 32;

export interface TreeSnapshot {
  folder: string | null;
  expanded: string[];
}

export class FileTree {
  constructor(readonly project: string) {}

  root = $state<string | null>(null);
  selected = $state<string | null>(null);
  rootName = $derived(this.root ? fileName(this.root) : null);
  rows = $derived(this.#flatten());

  #children = new SvelteMap<string, Entry[]>();
  #expanded = new SvelteSet<string>();
  #index: Promise<string[]> | null = null;
  #listeners = new Set<() => void>();

  onChange(listener: () => void) {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  async open(root: string, expanded: string[] = []) {
    this.#reset();
    this.root = root;
    await this.#load(root);
    const byDepth = [...expanded].sort((a, b) => a.length - b.length);
    for (const dir of byDepth) {
      if (isInside(dir, root) && (await this.#load(dir))) this.#expanded.add(dir);
    }
    await watchFolder(this.project, root).catch(console.error);
    this.#emitChange();
  }

  close() {
    this.#reset();
    watchFolder(this.project, null).catch(console.error);
    this.#emitChange();
  }

  async toggle(dir: string) {
    if (this.#expanded.has(dir)) return this.collapse(dir);
    return this.expand(dir);
  }

  /** Espande `dir` e, se contiene solo una cartella, anche quella: la catena si compatta in una riga. */
  async expand(dir: string) {
    if (dir === this.root) return;
    let current: string | undefined = dir;
    for (let step = 0; current && step < MAX_CHAIN; step++) {
      if (!this.#children.has(current) && !(await this.#load(current))) break;
      this.#expanded.add(current);
      current = this.#onlyDir(current)?.path;
    }
    this.#emitChange();
  }

  collapse(dir: string) {
    this.#expanded.delete(dir);
    this.#emitChange();
  }

  collapseAll() {
    this.#expanded.clear();
    this.#emitChange();
  }

  isExpanded(dir: string) {
    return this.#expanded.has(dir);
  }

  /** Ricarica le directory indicate che sono già state caricate (tutte se omesse). */
  async refresh(dirs: string[] = [...this.#children.keys()]) {
    this.#index = null;
    const loaded = dirs.filter((dir) => this.#children.has(dir));
    await Promise.all(loaded.map((dir) => this.#load(dir)));
  }

  /** Elenco dei file per Quick Open, ricalcolato solo dopo modifiche su disco. */
  fileIndex(): Promise<string[]> {
    if (!this.root) return Promise.resolve([]);
    this.#index ??= listFiles(this.root);
    return this.#index;
  }

  snapshot(): TreeSnapshot {
    return { folder: this.root, expanded: [...this.#expanded] };
  }

  async #load(dir: string): Promise<boolean> {
    try {
      this.#children.set(dir, await listDir(dir));
      return true;
    } catch {
      this.#forget(dir);
      return false;
    }
  }

  #forget(dir: string) {
    for (const key of [...this.#children.keys()]) if (isInside(key, dir)) this.#children.delete(key);
    for (const key of [...this.#expanded]) if (isInside(key, dir)) this.#expanded.delete(key);
  }

  #onlyDir(dir: string): Entry | null {
    const children = this.#children.get(dir);
    return children?.length === 1 && children[0].isDir ? children[0] : null;
  }

  #flatten(): Row[] {
    const rows: Row[] = [];
    const walk = (dir: string, depth: number) => {
      for (const entry of this.#children.get(dir) ?? []) {
        if (!entry.isDir || !this.#expanded.has(entry.path)) {
          rows.push({ entry, head: entry.path, label: entry.name, depth, expanded: false });
          continue;
        }
        let tail = entry;
        const names = [entry.name];
        for (let next = this.#onlyDir(tail.path); next && this.#expanded.has(next.path) && names.length < MAX_CHAIN; next = this.#onlyDir(tail.path)) {
          names.push(next.name);
          tail = next;
        }
        rows.push({ entry: tail, head: entry.path, label: names.join("/"), depth, expanded: true });
        walk(tail.path, depth + 1);
      }
    };
    if (this.root) walk(this.root, 0);
    return rows;
  }

  #reset() {
    this.root = null;
    this.selected = null;
    this.#children.clear();
    this.#expanded.clear();
    this.#index = null;
  }

  #emitChange() {
    this.#listeners.forEach((listener) => listener());
  }
}
