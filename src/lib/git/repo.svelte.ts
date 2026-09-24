import { Text } from "@codemirror/state";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { saveSettings, settings } from "../settings.svelte";
import { askText } from "../ui/prompt.svelte";
import { isInsideDir } from "../workspace/files";
import type { Workspace } from "../workspace/workspace.svelte";
import {
  gitBlame, gitBranches, gitCommit, gitCreateBranch, gitFetch, gitHeadContent, gitPull, gitPush, gitStage, gitStagedCrlfFiles,
  gitStashes, gitStatus, gitSwitch, gitUnstage, OPERATION_LABELS, type Branch, type BranchInfo, type FileStatus, type Operation, type Stash,
  type StatusEntry,
} from "./api";
import { blameGutter } from "./blame";
import { gitGutter } from "./gutter";
import { GitHistory } from "./history";
import { diffState } from "./selection.svelte";

const STATUS_DELAY_MS = 300;
const TITLE = "Git";

const key = (path: string) => path.replace(/\//g, "\\").toLowerCase();

export class GitRepo {
  branch = $state<BranchInfo | null>(null);
  branches = $state.raw<Branch[]>([]);
  /** File modificati, nuovi, cancellati o in conflitto (esclusi gli ignorati). */
  changes = $state.raw<StatusEntry[]>([]);
  /** Operazione in corso ("Push", "Update"…), mostrata nella status bar. */
  busy = $state<string | null>(null);
  /** Incrementato quando cambiano HEAD o i refs: il log si ricarica. */
  revision = $state(0);
  staged = $derived(this.changes.filter((entry) => entry.staged).length);
  /** Merge/rebase/cherry-pick/revert fermo su conflitti. */
  operation = $state<Operation | null>(null);
  conflicts = $derived(this.changes.filter((entry) => entry.status === "conflict").length);
  stashes = $state.raw<Stash[]>([]);
  /** Tab con le annotazioni blame attive. */
  annotated = $state.raw(new Set<string>());
  readonly history = new GitHistory(this);

  #files = $state.raw(new Map<string, FileStatus>());
  #dirs = $state.raw<Array<[string, FileStatus]>>([]);
  #gutters = new Map<string, string>();
  #timer: ReturnType<typeof setTimeout> | undefined;
  #unsubscribe: () => void;

  constructor(
    readonly root: string,
    readonly workspace: Workspace,
  ) {
    this.#unsubscribe = workspace.onChange(() => this.#syncGutters());
  }

  statusOf(path: string): FileStatus | null {
    const normalized = key(path);
    const exact = this.#files.get(normalized);
    if (exact) return exact;
    for (const [dir, status] of this.#dirs) {
      if (normalized.startsWith(dir) || `${normalized}\\` === dir) return status;
    }
    return null;
  }

  /** Dopo cambi a HEAD, index o refs: status, branch, log e baseline dei gutter. */
  async refresh() {
    await Promise.all([this.refreshStatus(), this.#refreshBranches(), this.#refreshStashes()]);
    this.revision += 1;
    this.#gutters.clear();
    this.#syncGutters();
  }

  /** Dopo modifiche nel working tree: basta lo status, con debounce. */
  scheduleStatus() {
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => this.refreshStatus(), STATUS_DELAY_MS);
  }

  async refreshStatus() {
    const status = await gitStatus(this.root).catch(() => null);
    if (!status) return;
    const files = new Map<string, FileStatus>();
    const dirs: Array<[string, FileStatus]> = [];
    for (const entry of status.files) {
      const path = key(entry.path);
      if (path.endsWith("\\")) dirs.push([path, entry.status]);
      else files.set(path, entry.status);
    }
    this.branch = status.branch;
    this.operation = status.operation;
    this.changes = status.files.filter((entry) => entry.status !== "ignored");
    this.#files = files;
    this.#dirs = dirs;
  }

  // ---------- operazioni ----------

  stage(paths: string[]) {
    return this.run("Add", () => gitStage(this.root, paths));
  }

  unstage(paths: string[]) {
    return this.run("Unstage", () => gitUnstage(this.root, paths));
  }

  async commit(text: string, andPush = false): Promise<boolean> {
    if (!text.trim()) return false;
    if (!this.staged) {
      await message("Nessun file selezionato per il commit.", { title: TITLE, kind: "info" });
      return false;
    }
    if (!(await this.#confirmDetachedCommit()) || !(await this.#confirmCrlf())) return false;
    const committed = await this.run("Commit", () => gitCommit(this.root, text));
    if (committed && andPush) await this.push(false);
    return committed;
  }

  async push(confirm = true): Promise<boolean> {
    const branch = this.branch;
    if (!branch?.head) {
      await message("HEAD è staccato: fai checkout di un branch prima del push.", { title: TITLE, kind: "warning" });
      return false;
    }
    if (confirm && !(await ask(this.#pushQuestion(branch), { title: "Push", okLabel: "Push", cancelLabel: "Annulla" }))) return false;
    return this.run("Push", () => gitPush(this.root));
  }

  update() {
    const rebase = settings.updateMethod === "rebase";
    return this.run(rebase ? "Update (rebase)" : "Update (merge)", () => gitPull(this.root, rebase));
  }

  fetch() {
    return this.run("Fetch", () => gitFetch(this.root));
  }

  checkout(branch: Branch) {
    if (branch.current) return Promise.resolve(true);
    return this.run(`Checkout ${branch.name}`, () => gitSwitch(this.root, branch.name, branch.remote));
  }

  async newBranch(): Promise<boolean> {
    const name = await askText("Nuovo branch (checkout immediato)");
    if (!name) return false;
    return this.run(`Nuovo branch ${name}`, () => gitCreateBranch(this.root, name, true));
  }

  dispose() {
    clearTimeout(this.#timer);
    this.#unsubscribe();
    for (const id of this.#gutters.keys()) this.workspace.configureGit(id, []);
    for (const id of this.annotated) this.workspace.configureBlame(id, []);
    this.#gutters.clear();
  }

  /** Esegue un'operazione Git mostrando lo stato "busy"; gli errori diventano dialoghi. */
  async run(label: string, task: () => Promise<unknown>): Promise<boolean> {
    if (this.busy) return false;
    this.busy = label;
    const operationBefore = this.operation;
    try {
      await task();
      return true;
    } catch (error) {
      await this.refreshStatus();
      if (this.operation && !operationBefore) await this.#explainConflict();
      else await message(String(error), { title: `${label} non riuscito`, kind: "error" });
      return false;
    } finally {
      this.busy = null;
      await this.refresh();
    }
  }

  async toggleBlame(tabId: string, path: string) {
    if (this.annotated.has(tabId)) return this.#setBlame(tabId, null);
    const lines = await gitBlame(this.root, path).catch(async (error) => {
      await message(String(error), { title: "Annotate non disponibile", kind: "error" });
      return null;
    });
    if (lines) this.#setBlame(tabId, blameGutter(lines, (hash) => (diffState.request = { kind: "commit", hash })));
  }

  #setBlame(tabId: string, extension: ReturnType<typeof blameGutter> | null) {
    this.workspace.configureBlame(tabId, extension ?? []);
    const next = new Set(this.annotated);
    if (extension) next.add(tabId);
    else next.delete(tabId);
    this.annotated = next;
  }

  #explainConflict() {
    const label = OPERATION_LABELS[this.operation!];
    const count = this.conflicts === 1 ? "1 file in conflitto" : `${this.conflicts} file in conflitto`;
    return message(
      `${label} fermo: ${count}.

Apri i file (sono in rosso nel pannello Commit), risolvi i marker <<<<<<< / >>>>>>>, mettili in stage e usa "Continua ${label.toLowerCase()}". Oppure annulla dal popup VCS.`,
      { title: `${label} con conflitti`, kind: "warning" },
    );
  }

  #pushQuestion(branch: BranchInfo) {
    if (!branch.upstream) return `Il branch "${branch.head}" non ha un upstream. Pubblicarlo sul remote e impostarlo come upstream?`;
    const commits = branch.ahead === 1 ? "1 commit" : `${branch.ahead} commit`;
    return branch.ahead ? `Push di ${commits} su ${branch.upstream}?` : `Nessun commit da inviare a ${branch.upstream}. Eseguire comunque il push?`;
  }

  async #confirmDetachedCommit(): Promise<boolean> {
    if (this.branch?.head || !this.branch?.oid) return true;
    return ask("HEAD è staccato: il commit non apparterrà a nessun branch e potresti perderlo facendo checkout. Continuare?", {
      title: "Detached HEAD",
      kind: "warning",
      okLabel: "Committa",
      cancelLabel: "Annulla",
    });
  }

  async #confirmCrlf(): Promise<boolean> {
    if (!settings.warnCrlf) return true;
    const files = await gitStagedCrlfFiles(this.root).catch(() => []);
    if (!files.length) return true;
    const list = files.slice(0, 5).join("\n") + (files.length > 5 ? `\n… e altri ${files.length - 5}` : "");
    const answer = await message(
      `Questi file hanno terminazioni di riga CRLF e core.autocrlf non è impostato, quindi finiranno nel repository così:\n\n${list}\n\nPer convertirle in automatico: git config --global core.autocrlf true`,
      { title: "Terminazioni CRLF", kind: "warning", buttons: { yes: "Committa", no: "Committa e non avvisare più", cancel: "Annulla" } },
    );
    if (answer === "No" || answer === "Committa e non avvisare più") {
      settings.warnCrlf = false;
      saveSettings();
      return true;
    }
    return answer === "Yes" || answer === "Committa";
  }

  async #refreshStashes() {
    this.stashes = await gitStashes(this.root).catch(() => []);
  }

  async #refreshBranches() {
    this.branches = await gitBranches(this.root).catch(() => []);
  }

  #syncGutters() {
    const open = new Set<string>();
    for (const tab of this.workspace.tabs) {
      if (!tab.path || !isInsideDir(tab.path, this.root)) continue;
      open.add(tab.id);
      if (this.#gutters.get(tab.id) === tab.path) continue;
      this.#gutters.set(tab.id, tab.path);
      this.#loadGutter(tab.id, tab.path);
    }
    for (const id of this.#gutters.keys()) if (!open.has(id)) this.#gutters.delete(id);
  }

  async #loadGutter(id: string, path: string) {
    const head = await gitHeadContent(this.root, path).catch(() => null);
    if (this.#gutters.get(id) !== path) return;
    if (head) return this.workspace.configureGit(id, gitGutter(Text.of(head.content.split(/\r\n|\r|\n/))));
    const tracked = this.statusOf(path) === "added";
    this.workspace.configureGit(id, tracked ? gitGutter(Text.empty) : []);
  }
}
