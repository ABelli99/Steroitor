import { Text } from "@codemirror/state";
import { isInsideDir } from "../workspace/files";
import type { Workspace } from "../workspace/workspace.svelte";
import { gitBranches, gitHeadContent, gitStatus, type Branch, type BranchInfo, type FileStatus } from "./api";
import { gitGutter } from "./gutter";

const STATUS_DELAY_MS = 300;

const key = (path: string) => path.replace(/\//g, "\\").toLowerCase();

export class GitRepo {
  branch = $state<BranchInfo | null>(null);
  branches = $state.raw<Branch[]>([]);
  /** Incrementato quando cambiano HEAD o i refs: il log si ricarica. */
  revision = $state(0);

  #files = $state.raw(new Map<string, FileStatus>());
  #dirs = $state.raw<Array<[string, FileStatus]>>([]);
  #gutters = new Map<string, string>();
  #timer: ReturnType<typeof setTimeout> | undefined;
  #unsubscribe: () => void;

  constructor(
    readonly root: string,
    private workspace: Workspace,
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
    await Promise.all([this.refreshStatus(), this.#refreshBranches()]);
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
    this.#files = files;
    this.#dirs = dirs;
  }

  dispose() {
    clearTimeout(this.#timer);
    this.#unsubscribe();
    for (const id of this.#gutters.keys()) this.workspace.configureGit(id, []);
    this.#gutters.clear();
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
