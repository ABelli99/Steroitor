import type { GitRepo } from "./repo.svelte";

/** Messaggio di commit in corso: sopravvive al cambio di tab del pannello. */
export const commitDraft = $state({ message: "", amend: false });

/** Commit (o amend) del messaggio corrente; svuota la bozza se va a buon fine. */
export async function submitCommit(repo: GitRepo, andPush: boolean) {
  const { message, amend } = commitDraft;
  if (!message.trim() || repo.operation) return false;
  const done = amend ? await repo.history.amend(message) : await repo.commit(message, andPush);
  if (!done) return false;
  commitDraft.message = "";
  commitDraft.amend = false;
  return true;
}
