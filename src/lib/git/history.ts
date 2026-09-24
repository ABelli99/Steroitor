import { ask } from "@tauri-apps/plugin-dialog";
import { askMultiline, askText } from "../ui/prompt.svelte";
import {
  gitAbortOperation, gitAmend, gitCheckoutCommit, gitCherryPick, gitContinueOperation, gitCreateBranch, gitCreateTag,
  gitHeadIsPushed, gitLastCommitMessage, gitMerge, gitRebase, gitReset, gitRevert, gitStashDrop, gitStashPop, gitStashPush,
  gitCommitMessage, gitReword, OPERATION_LABELS, type ResetMode, type Stash,
} from "./api";
import type { GitRepo } from "./repo.svelte";

const short = (hash: string) => hash.slice(0, 7);

const confirm = (question: string, title: string, okLabel: string) =>
  ask(question, { title, kind: "warning", okLabel, cancelLabel: "Annulla" });

/** Azioni di F5: log, merge/rebase, amend, stash. Tutte passano da `repo.run`. */
export class GitHistory {
  constructor(private repo: GitRepo) {}

  get #root() {
    return this.repo.root;
  }

  async checkoutCommit(hash: string) {
    const question = `Checkout di ${short(hash)} in detached HEAD? I commit fatti lì non apparterranno a nessun branch.`;
    if (!(await confirm(question, "Detached HEAD", "Checkout"))) return false;
    return this.repo.run(`Checkout ${short(hash)}`, () => gitCheckoutCommit(this.#root, hash));
  }

  async branchFrom(hash: string) {
    const name = await askText(`Nuovo branch da ${short(hash)}`);
    if (!name) return false;
    return this.repo.run(`Nuovo branch ${name}`, () => gitCreateBranch(this.#root, name, true, hash));
  }

  async tag(hash: string) {
    const name = await askText(`Nuovo tag su ${short(hash)}`);
    if (!name) return false;
    return this.repo.run(`Tag ${name}`, () => gitCreateTag(this.#root, name, hash));
  }

  cherryPick(hash: string) {
    return this.repo.run(`Cherry-pick ${short(hash)}`, () => gitCherryPick(this.#root, hash));
  }

  revert(hash: string) {
    return this.repo.run(`Revert ${short(hash)}`, () => gitRevert(this.#root, hash));
  }

  async reset(hash: string, mode: ResetMode) {
    const branch = this.repo.branch?.head ?? "HEAD";
    if (mode === "hard") {
      const question = `Reset --hard di "${branch}" a ${short(hash)}: le modifiche non committate e i commit successivi andranno persi. Continuare?`;
      if (!(await confirm(question, "Reset hard", "Reset hard"))) return false;
    }
    return this.repo.run(`Reset ${mode}`, () => gitReset(this.#root, hash, mode));
  }

  merge(branch: string) {
    return this.repo.run(`Merge ${branch}`, () => gitMerge(this.#root, branch));
  }

  rebase(onto: string) {
    return this.repo.run(`Rebase su ${onto}`, () => gitRebase(this.#root, onto));
  }

  continueOperation() {
    const operation = this.repo.operation;
    if (!operation) return Promise.resolve(false);
    return this.repo.run(`Continua ${OPERATION_LABELS[operation].toLowerCase()}`, () => gitContinueOperation(this.#root));
  }

  async abortOperation() {
    const operation = this.repo.operation;
    if (!operation) return false;
    const label = OPERATION_LABELS[operation];
    if (!(await confirm(`Annullare il ${label.toLowerCase()} e tornare allo stato precedente?`, label, "Annulla operazione"))) return false;
    return this.repo.run(`Annulla ${label.toLowerCase()}`, () => gitAbortOperation(this.#root));
  }

  async reword(hash: string) {
    const current = await gitCommitMessage(this.#root, hash).catch(() => "");
    const message = await askMultiline(`Messaggio del commit ${short(hash)}`, current);
    if (!message || message === current) return false;
    return this.repo.run("Modifica messaggio", () => gitReword(this.#root, hash, message));
  }

  lastCommitMessage() {
    return gitLastCommitMessage(this.#root).catch(() => "");
  }

  async amend(message: string) {
    const pushed = await gitHeadIsPushed(this.#root).catch(() => false);
    if (pushed) {
      const question =
        "L'ultimo commit è già su un branch remoto: l'amend riscrive la storia e servirà un push forzato (da terminale). Continuare?";
      if (!(await confirm(question, "Amend di un commit pubblicato", "Amend"))) return false;
    }
    return this.repo.run("Amend", () => gitAmend(this.#root, message));
  }

  async stash() {
    const message = await askText("Stash: descrizione (facoltativa)");
    if (message === null) return false;
    return this.repo.run("Stash", () => gitStashPush(this.#root, message));
  }

  unstash(stash: Stash) {
    return this.repo.run(`Pop ${stash.name}`, () => gitStashPop(this.#root, stash.name));
  }

  async dropStash(stash: Stash) {
    if (!(await confirm(`Eliminare ${stash.name} (${stash.message})?`, "Elimina stash", "Elimina"))) return false;
    return this.repo.run(`Elimina ${stash.name}`, () => gitStashDrop(this.#root, stash.name));
  }
}
