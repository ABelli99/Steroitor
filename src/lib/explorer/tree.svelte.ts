import { SvelteMap, SvelteSet } from "svelte/reactivity";
import { listDir, listFiles, watchFolder, type Entry } from "../backend";
import { fileName, isInsideDir as isInside } from "../workspace/files";

export interface Row {
  entry: Entry;
  depth: number;
  expanded: boolean;
}

export interface TreeSnapshot {
  folder: string | null;
  expanded: string[];
}

export class FileTree {
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
    await watchFolder(root).catch(console.error);
    this.#emitChange();
  }

  close() {
    this.#reset();
    watchFolder(null).catch(console.error);
    this.#emitChange();
  }

  async toggle(dir: string) {
    if (this.#expanded.has(dir)) return this.collapse(dir);
    return this.expand(dir);
  }

  async expand(dir: string) {
    if (dir === this.root) return;
    if (!this.#children.has(dir) && !(await this.#load(dir))) return;
    this.#expanded.add(dir);
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

  #flatten(): Row[] {
    const rows: Row[] = [];
    const walk = (dir: string, depth: number) => {
      for (const entry of this.#children.get(dir) ?? []) {
        const expanded = entry.isDir && this.#expanded.has(entry.path);
        rows.push({ entry, depth, expanded });
        if (expanded) walk(entry.path, depth + 1);
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
