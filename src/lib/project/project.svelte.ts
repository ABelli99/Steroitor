import { message } from "@tauri-apps/plugin-dialog";
import { gitFindRepos, gitRepoRoot } from "../backend";
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
import { isInsideDir, samePath } from "../workspace/files";
import { Workspace } from "../workspace/workspace.svelte";
import { innermost } from "./routing";

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
  /** Repository della cartella: quello che la contiene e quelli annidati (es. `gipso/gipso-fe`, `gipso/gipso-be`). */
  repos = $state.raw<GitRepo[]>([]);
  selectedRoot = $state<string | null>(null);
  /** Repository mostrato nei pannelli Git e Commit e usato dalle scorciatoie. */
  git = $derived(this.repos.find((repo) => this.selectedRoot && samePath(repo.root, this.selectedRoot)) ?? this.repos[0] ?? null);
  folder = $state<string | null>(null);
  /** La vista si monta alla prima attivazione e poi resta viva (nascosta) finché il progetto è aperto. */
  mounted = $state(false);

  #session: ReturnType<typeof persistSession> | null = null;
  #started: Promise<void> | null = null;
  /** Repository che contiene la cartella aperta, anche se la sua radice sta più in alto. */
  #enclosing = $state<string | null>(null);
  #connecting = 0;

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
    return this.#enclosing ?? this.folder;
  }

  /** Il repository più interno che contiene `path`: è lui a gestirne stato, gutter e blame. */
  repoFor(path: string) {
    return innermost(this.repos, path);
  }

  statusOf(path: string) {
    return this.repoFor(path)?.statusOf(path) ?? null;
  }

  select(repo: GitRepo) {
    this.selectedRoot = repo.root;
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
      this.connectRepos(this.folder);
    }
    this.#session = persistSession(this.workspace, this.tree);
  }

  async openFolder(folder: string) {
    this.folder = folder;
    await this.tree.open(folder);
    await this.connectRepos(folder);
    this.terminals.relocate(this.root).catch(console.error);
  }

  /** Il modulo Git si carica solo se la cartella è dentro un repository o ne contiene. */
  async connectRepos(folder: string) {
    const run = ++this.#connecting;
    const [enclosing, nested] = await Promise.all([gitRepoRoot(folder).catch(() => null), gitFindRepos(folder).catch(() => [])]);
    const roots = [enclosing, ...nested].filter((root): root is string => !!root);
    const unique = roots.filter((root, index) => roots.findIndex((other) => samePath(other, root)) === index);
    const { GitRepo } = unique.length ? await import("../git/repo.svelte") : { GitRepo: null };
    if (run !== this.#connecting) return;

    this.#disposeRepos();
    this.#enclosing = enclosing;
    if (!GitRepo) return;
    const repos = unique.map((root) => {
      const repo: GitRepo = new GitRepo(root, this.workspace, this.diff, (path) => this.repoFor(path) === repo);
      return repo;
    });
    this.repos = repos;
    if (!repos.some((repo) => this.selectedRoot && samePath(repo.root, this.selectedRoot))) this.selectedRoot = repos[0].root;
    await Promise.all(repos.map((repo) => repo.refresh()));
  }

  /** Un repository sconosciuto (appena creato con git init o clonato) fa ricalcolare l'elenco. */
  onGitChanged(repoDir: string) {
    const repo = this.repos.find((candidate) => samePath(candidate.root, repoDir));
    if (repo) return repo.refresh();
    if (this.folder) return this.connectRepos(this.folder);
  }

  #disposeRepos() {
    for (const repo of this.repos) repo.dispose();
    this.repos = [];
  }

  async initRepository() {
    if (!this.folder) return;
    try {
      await gitInit(this.folder);
      await this.connectRepos(this.folder);
    } catch (error) {
      await message(String(error), { title: "git init non riuscito", kind: "error" });
    }
  }

  onFilesChanged(dirs: string[]) {
    this.tree.refresh(dirs);
    this.workspace.reloadCleanIn(dirs);
    for (const repo of this.repos) {
      if (dirs.some((dir) => isInsideDir(dir, repo.root) || isInsideDir(repo.root, dir))) repo.scheduleStatus();
    }
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
    this.#connecting++;
    this.#disposeRepos();
    await this.terminals.dispose();
    this.tree.close();
  }
}
