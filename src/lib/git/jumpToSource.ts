import { samePath } from "../workspace/files";
import type { Workspace } from "../workspace/workspace.svelte";
import { gitHeadContent } from "./api";
import { firstChangedLine, toText } from "./hunks";
import type { FileSelection } from "./selection.svelte";

export const canJumpTo = (file: FileSelection) => file.status !== "deleted";

/** Apre il file modificato nell'editor con il cursore sulla prima riga cambiata rispetto a HEAD. */
export async function jumpToSource(workspace: Workspace, root: string, file: FileSelection) {
  if (!canJumpTo(file)) return;
  await workspace.openPath(file.path);
  const active = workspace.active;
  if (!active?.path || !samePath(active.path, file.path)) return;
  const isNew = file.status === "added" || file.status === "untracked";
  const head = isNew ? null : await gitHeadContent(root, file.path).catch(() => null);
  const current = workspace.activeText();
  workspace.goToLine(head && current ? firstChangedLine(toText(head.content), current) : 1);
}
