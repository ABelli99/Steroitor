import { message } from "@tauri-apps/plugin-dialog";
import { gitRepoRoot } from "../backend";
import { ConsoleLog } from "../console/console.svelte";
import { ExplorerActions } from "../explorer/actions";
import { FileTree } from "../explorer/tree.svelte";
import { gitInit } from "../git/clone";
import { createCommitDraft } from "../git/commitDraft.svelte";
import type { GitRepo } from "../git/repo.svelte";
import { createDiffState, createGitSelection } from "../git/selection.svelte";
import { Terminals } from "../terminal/terminals.svelte";
import { Activity } from "../ui/activity.svelte";
import { persistSession, readSession } from "../workspace/session";
import { Workspace } from "../workspace/workspace.svelte";

const newId = () => crypto.randomUUID().slice(0, 8);

/** Tutto ciò che vive dentro una tab progetto: editor, explorer, Git, console e terminali. */
export class Project {
  readonly workspace = new Workspace();
  readonly tree: FileTree;
  readonly actions: ExplorerActions;
  readonly console = new ConsoleLog();
  readonly terminals: Terminals;
  readonly activity = new Activity();
  readonly commitDraft = createCommitDraft();
  readonly selection = createGitSelection();
  readonly diff = createDiffState();
  git = $state<GitRepo | null>(null);
  folder = $state<string | null>(null);
  /** La vista si monta alla prima attivazione e poi resta viva (nascosta) finché il progetto è aperto. */
  mounted = $state(false);

  #session: ReturnType<typeof persistSession> | null = null;
  #started: Promise<void> | null = null;

  constructor(
    readonly id = newId(),
    folder: string | null = null,
  ) {
    this.folder = folder;
    this.tree = new FileTree(id);
    this.actions = new ExplorerActions(this.tree, this.workspace);
    this.terminals = new Terminals(id);
  }

  /** Cartella che fa da radice per i comandi Git e i terminali. */
  get root() {
    return this.git?.root ?? this.folder;
  }

  start() {
    this.#started ??= this.#restore();
    return this.#started;
  }

  async #restore() {
    const saved = await readSession(this.folder);
    if (saved) await this.workspace.restore(saved);
    if (this.folder) {
      await this.tree.open(this.folder, saved?.expanded);
      this.connectRepo(this.folder);
    }
    this.#session = persistSession(this.workspace, this.tree);
  }

  async openFolder(folder: string) {
    this.folder = folder;
    await this.tree.open(folder);
    await this.connectRepo(folder);
    this.terminals.relocate(this.root).catch(console.error);
  }

  /** Il modulo Git si carica solo se la cartella è dentro un repository. */
  async connectRepo(folder: string) {
    this.git?.dispose();
    this.git = null;
    const root = await gitRepoRoot(folder).catch(() => null);
    if (!root) return;
    const { GitRepo } = await import("../git/repo.svelte");
    const repo = new GitRepo(root, this.workspace, this.diff);
    this.git = repo;
    await repo.refresh();
  }

  async initRepository() {
    if (!this.folder) return;
    try {
      await gitInit(this.folder);
      await this.connectRepo(this.folder);
    } catch (error) {
      await message(String(error), { title: "git init non riuscito", kind: "error" });
    }
  }

  onFilesChanged(dirs: string[]) {
    this.tree.refresh(dirs);
    this.workspace.reloadCleanIn(dirs);
    this.git?.scheduleStatus();
  }

  flush() {
    return this.#session?.flush() ?? Promise.resolve();
  }

  /** Salva la sessione e rilascia shell, watcher e Git: dopo, il progetto può riaprirsi altrove. */
  async dispose() {
    if (!this.#started) return;
    await this.#started;
    await this.flush();
    this.#session?.stop();
    this.git?.dispose();
    this.git = null;
    await this.terminals.dispose();
    this.tree.close();
  }
}
