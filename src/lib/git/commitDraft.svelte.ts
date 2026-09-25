import type { GitRepo } from "./repo.svelte";

/** Messaggio di commit in corso: sopravvive al cambio di tab del pannello. */
export interface CommitDraft {
  message: string;
  amend: boolean;
}

export function createCommitDraft(): CommitDraft {
  const draft = $state({ message: "", amend: false });
  return draft;
}

/** Commit (o amend) del messaggio corrente; svuota la bozza se va a buon fine. */
export async function submitCommit(repo: GitRepo, draft: CommitDraft, andPush: boolean) {
  const { message, amend } = draft;
  if (!message.trim() || repo.operation) return false;
  const done = amend ? await repo.history.amend(message) : await repo.commit(message, andPush);
  if (!done) return false;
  draft.message = "";
  draft.amend = false;
  return true;
}
