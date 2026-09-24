import { ask, message } from "@tauri-apps/plugin-dialog";
import { createDir, createFile, deleteToTrash, renamePath, revealInOs, type Entry } from "../backend";
import { askText } from "../ui/prompt.svelte";
import type { Workspace } from "../workspace/workspace.svelte";
import type { FileTree } from "./tree.svelte";

const parentOf = (path: string) => path.replace(/[\\/][^\\/]+$/, "");

const showError = (error: unknown) => message(String(error), { title: "Steroitor", kind: "error" });

export class ExplorerActions {
  constructor(
    private tree: FileTree,
    private workspace: Workspace,
  ) {}

  /** Directory in cui creare nuovi elementi partendo dall'elemento selezionato. */
  targetDir(entry: Entry | null): string | null {
    if (!entry) return this.tree.root;
    return entry.isDir ? entry.path : parentOf(entry.path);
  }

  async newFile(dir: string) {
    const name = await askText("Nuovo file");
    if (!name) return;
    const path = await this.#run(() => createFile(dir, name));
    if (!path) return;
    await this.#showIn(dir, path);
    await this.workspace.openPath(path);
  }

  async newFolder(dir: string) {
    const name = await askText("Nuova cartella");
    if (!name) return;
    const path = await this.#run(() => createDir(dir, name));
    if (path) await this.#showIn(dir, path);
  }

  async rename(entry: Entry) {
    const extension = entry.isDir ? -1 : entry.name.lastIndexOf(".");
    const name = await askText("Rinomina", entry.name, extension > 0 ? extension : entry.name.length);
    if (!name || name === entry.name) return;
    const path = await this.#run(() => renamePath(entry.path, name));
    if (!path) return;
    this.workspace.pathRenamed(entry.path, path);
    await this.#showIn(parentOf(entry.path), path);
  }

  async remove(entry: Entry) {
    const kind = entry.isDir ? "la cartella" : "il file";
    const confirmed = await ask(`Spostare nel Cestino ${kind} "${entry.name}"?`, {
      title: "Steroitor",
      kind: "warning",
      okLabel: "Elimina",
      cancelLabel: "Annulla",
    });
    if (!confirmed) return;
    try {
      await deleteToTrash(entry.path);
    } catch (error) {
      return showError(error);
    }
    await this.tree.refresh([parentOf(entry.path)]);
  }

  reveal(path: string) {
    revealInOs(path).catch(showError);
  }

  copyPath(path: string) {
    navigator.clipboard.writeText(path).catch(showError);
  }

  async #showIn(dir: string, path: string) {
    await this.tree.refresh([dir]);
    await this.tree.expand(dir);
    this.tree.selected = path;
  }

  async #run<T>(operation: () => Promise<T>): Promise<T | null> {
    try {
      return await operation();
    } catch (error) {
      await showError(error);
      return null;
    }
  }
}
