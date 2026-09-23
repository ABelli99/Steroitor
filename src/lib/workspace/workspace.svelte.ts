import { EditorView, type ViewUpdate } from "@codemirror/view";
import { EditorState, Text, type TransactionSpec } from "@codemirror/state";
import { message, open, save } from "@tauri-apps/plugin-dialog";
import { readTextFile, writeTextFile } from "../backend";
import { detectLanguage, loadLanguage } from "../editor/languages";
import { createState, languageSlot, wrapExtension, wrapSlot } from "../editor/setup";
import { detectEol, fileName, samePath, type Eol } from "./files";
import type { SessionData, SessionTab } from "./session";

export interface Tab {
  id: string;
  path: string | null;
  name: string;
  dirty: boolean;
  encoding: string;
  bom: boolean;
  eol: Eol;
  language: string | null;
}

interface Baseline {
  text: Text;
  encoding: string;
  bom: boolean;
  eol: Eol;
}

interface NewTab {
  path: string | null;
  name: string;
  content: string;
  encoding: string;
  bom: boolean;
  eol: Eol;
  baseline: Baseline;
}

const UNTITLED = "Senza titolo";
const DEFAULT_ENCODING = "UTF-8";
const DEFAULT_EOL: Eol = navigator.userAgent.includes("Windows") ? "\r\n" : "\n";

const emptyBaseline = (): Baseline => ({ text: Text.empty, encoding: DEFAULT_ENCODING, bom: false, eol: DEFAULT_EOL });

const showError = (error: unknown) => message(String(error), { title: "Steroitor", kind: "error" });

export class Workspace {
  tabs = $state<Tab[]>([]);
  activeId = $state<string | null>(null);
  cursor = $state({ line: 1, column: 1, selected: 0 });
  wrap = $state(false);
  active = $derived(this.tabs.find((tab) => tab.id === this.activeId) ?? null);

  #states = new Map<string, EditorState>();
  #baselines = new Map<string, Baseline>();
  #view: EditorView | null = null;
  #nextId = 1;
  #untitledCount = 0;
  #listeners = new Set<() => void>();

  mount(parent: HTMLElement) {
    this.#view = new EditorView({ parent });
    const active = this.activeId && this.#states.get(this.activeId);
    if (active) this.#view.setState(active);
    return () => {
      this.#persistActiveState();
      this.#view?.destroy();
      this.#view = null;
    };
  }

  onChange(listener: () => void) {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  focusEditor() {
    this.#view?.focus();
  }

  newUntitled(content = "") {
    this.#untitledCount += 1;
    this.#addTab({
      path: null,
      name: `${UNTITLED} ${this.#untitledCount}`,
      content,
      encoding: DEFAULT_ENCODING,
      bom: false,
      eol: DEFAULT_EOL,
      baseline: emptyBaseline(),
    });
  }

  async openPath(path: string) {
    const existing = this.tabs.find((tab) => tab.path && samePath(tab.path, path));
    if (existing) return this.activate(existing.id);

    try {
      const file = await readTextFile(path);
      const eol = detectEol(file.content, DEFAULT_EOL);
      const disposable = this.#disposableUntitled();
      this.#addTab({
        path,
        name: fileName(path),
        content: file.content,
        encoding: file.encoding,
        bom: file.bom,
        eol,
        baseline: { text: Text.of(file.content.split(/\r\n|\r|\n/)), encoding: file.encoding, bom: file.bom, eol },
      });
      if (disposable) this.#remove(disposable.id);
    } catch (error) {
      await showError(error);
    }
  }

  async openDialog() {
    const selected = await open({ multiple: true, directory: false });
    for (const path of selected ?? []) await this.openPath(path);
  }

  async save(id = this.activeId): Promise<boolean> {
    const tab = this.#tab(id);
    if (!tab) return false;
    if (!tab.path) return this.saveAs(tab.id);
    return this.#writeTo(tab, tab.path);
  }

  async saveAs(id = this.activeId): Promise<boolean> {
    const tab = this.#tab(id);
    if (!tab) return false;
    const path = await save({ defaultPath: tab.path ?? tab.name });
    if (!path) return false;
    if (!(await this.#writeTo(tab, path))) return false;

    tab.path = path;
    tab.name = fileName(path);
    this.#applyLanguage(tab, detectLanguage(path));
    this.#emitChange();
    return true;
  }

  async close(id = this.activeId) {
    const tab = this.#tab(id);
    if (!tab) return;
    if (tab.dirty && !(await this.#confirmDiscard(tab))) return;
    this.#remove(tab.id);
    if (this.tabs.length === 0) this.newUntitled();
  }

  activate(id: string) {
    const target = this.#states.get(id);
    if (!target || id === this.activeId) return;
    this.#persistActiveState();
    this.activeId = id;
    this.#view?.setState(target);
    this.#updateCursor(target);
    this.#view?.focus();
  }

  cycle(step: 1 | -1) {
    if (this.tabs.length < 2) return;
    const index = this.tabs.findIndex((tab) => tab.id === this.activeId);
    const next = (index + step + this.tabs.length) % this.tabs.length;
    this.activate(this.tabs[next].id);
  }

  move(id: string, toIndex: number) {
    const from = this.tabs.findIndex((tab) => tab.id === id);
    if (from === -1 || from === toIndex) return;
    const [tab] = this.tabs.splice(from, 1);
    this.tabs.splice(toIndex, 0, tab);
    this.#emitChange();
  }

  setEol(eol: Eol) {
    const tab = this.active;
    if (!tab || tab.eol === eol) return;
    tab.eol = eol;
    this.#refreshDirty(tab);
    this.#emitChange();
  }

  toggleWrap() {
    this.wrap = !this.wrap;
    const effect = wrapSlot.reconfigure(wrapExtension(this.wrap));
    for (const tab of this.tabs) this.#dispatch(tab.id, { effects: effect });
    this.#emitChange();
  }

  snapshot(): SessionData {
    this.#persistActiveState();
    const tabs = this.tabs
      .map((tab): SessionTab | null => {
        const content = this.#states.get(tab.id)!.doc.toString();
        if (!tab.path && !content) return null;
        const keepContent = tab.dirty || !tab.path;
        return {
          path: tab.path,
          name: tab.name,
          content: keepContent ? content : undefined,
          encoding: tab.encoding,
          bom: tab.bom,
          eol: tab.eol,
          active: tab.id === this.activeId,
        };
      })
      .filter((tab): tab is SessionTab => tab !== null);
    return { version: 1, wrap: this.wrap, tabs };
  }

  async restore(session: SessionData) {
    this.wrap = session.wrap;
    let activeId: string | null = null;
    for (const saved of session.tabs) {
      const id = await this.#restoreTab(saved);
      if (id && saved.active) activeId = id;
    }
    if (activeId) this.activate(activeId);
  }

  async #restoreTab(saved: SessionTab): Promise<string | null> {
    if (!saved.path) {
      this.#untitledCount = Math.max(this.#untitledCount, untitledNumber(saved.name));
      return this.#addTab({ ...saved, content: saved.content ?? "", baseline: emptyBaseline() });
    }

    const disk = await readTextFile(saved.path).catch(() => null);
    if (!disk && saved.content === undefined) return null;

    const baseline = disk
      ? { text: Text.of(disk.content.split(/\r\n|\r|\n/)), encoding: disk.encoding, bom: disk.bom, eol: detectEol(disk.content, DEFAULT_EOL) }
      : emptyBaseline();
    const meta = saved.content === undefined ? baseline : saved;
    return this.#addTab({
      path: saved.path,
      name: fileName(saved.path),
      content: saved.content ?? disk!.content,
      encoding: meta.encoding,
      bom: meta.bom,
      eol: meta.eol,
      baseline,
    });
  }

  #addTab(spec: NewTab): string {
    const id = `tab-${this.#nextId++}`;
    const language = detectLanguage(spec.path);
    const state = createState(spec.content, this.wrap, (update) => this.#handleUpdate(id, update));
    this.#states.set(id, state);
    this.#baselines.set(id, spec.baseline);
    this.tabs.push({
      id,
      path: spec.path,
      name: spec.name,
      dirty: false,
      encoding: spec.encoding,
      bom: spec.bom,
      eol: spec.eol,
      language: null,
    });
    const tab = this.#tab(id)!;
    this.#refreshDirty(tab);
    this.activate(id);
    this.#applyLanguage(tab, language);
    this.#emitChange();
    return id;
  }

  #remove(id: string) {
    const index = this.tabs.findIndex((tab) => tab.id === id);
    if (index === -1) return;
    const wasActive = id === this.activeId;
    this.tabs.splice(index, 1);
    this.#states.delete(id);
    this.#baselines.delete(id);
    if (wasActive) {
      this.activeId = null;
      const neighbour = this.tabs[Math.min(index, this.tabs.length - 1)];
      if (neighbour) this.activate(neighbour.id);
    }
    this.#emitChange();
  }

  async #writeTo(tab: Tab, path: string): Promise<boolean> {
    const doc = this.#stateOf(tab.id).doc;
    try {
      await writeTextFile(path, doc.toJSON().join(tab.eol), tab.encoding, tab.bom);
    } catch (error) {
      await showError(error);
      return false;
    }
    this.#baselines.set(tab.id, { text: doc, encoding: tab.encoding, bom: tab.bom, eol: tab.eol });
    this.#refreshDirty(tab);
    this.#emitChange();
    return true;
  }

  async #confirmDiscard(tab: Tab): Promise<boolean> {
    const answer = await message(`Salvare le modifiche a "${tab.name}"?`, {
      title: "Steroitor",
      kind: "warning",
      buttons: { yes: "Salva", no: "Non salvare", cancel: "Annulla" },
    });
    if (answer === "Yes" || answer === "Salva") return this.save(tab.id);
    return answer === "No" || answer === "Non salvare";
  }

  #applyLanguage(tab: Tab, language: string | null) {
    tab.language = language;
    loadLanguage(language).then((extension) => {
      const current = this.#tab(tab.id);
      if (current?.language !== language) return;
      this.#dispatch(tab.id, { effects: languageSlot.reconfigure(extension) });
    });
  }

  #handleUpdate(id: string, update: ViewUpdate) {
    if (update.docChanged) {
      const tab = this.#tab(id);
      if (tab) this.#refreshDirty(tab, update.state.doc);
      this.#emitChange();
    }
    if ((update.docChanged || update.selectionSet) && id === this.activeId) this.#updateCursor(update.state);
  }

  #refreshDirty(tab: Tab, doc = this.#stateOf(tab.id).doc) {
    const baseline = this.#baselines.get(tab.id)!;
    tab.dirty =
      !doc.eq(baseline.text) || tab.eol !== baseline.eol || tab.encoding !== baseline.encoding || tab.bom !== baseline.bom;
  }

  #updateCursor(state: EditorState) {
    const head = state.selection.main.head;
    const line = state.doc.lineAt(head);
    this.cursor = {
      line: line.number,
      column: head - line.from + 1,
      selected: state.selection.ranges.reduce((sum, range) => sum + range.to - range.from, 0),
    };
  }

  #dispatch(id: string, spec: TransactionSpec) {
    if (id === this.activeId && this.#view) return this.#view.dispatch(spec);
    const state = this.#states.get(id);
    if (state) this.#states.set(id, state.update(spec).state);
  }

  #stateOf(id: string): EditorState {
    return id === this.activeId && this.#view ? this.#view.state : this.#states.get(id)!;
  }

  #persistActiveState() {
    if (this.activeId && this.#view && this.#states.has(this.activeId)) this.#states.set(this.activeId, this.#view.state);
  }

  #disposableUntitled(): Tab | null {
    const [only] = this.tabs;
    if (this.tabs.length !== 1 || only.path || only.dirty) return null;
    return this.#stateOf(only.id).doc.length === 0 ? only : null;
  }

  #tab(id: string | null) {
    return id ? (this.tabs.find((tab) => tab.id === id) ?? null) : null;
  }

  #emitChange() {
    this.#listeners.forEach((listener) => listener());
  }
}

function untitledNumber(name: string) {
  const match = name.match(new RegExp(`^${UNTITLED} (\\d+)$`));
  return match ? Number(match[1]) : 0;
}
