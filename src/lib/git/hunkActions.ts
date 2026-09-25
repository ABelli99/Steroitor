import type { Text } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";
import { message } from "@tauri-apps/plugin-dialog";
import { openMenu } from "../ui/menu.svelte";
import type { MenuItem } from "../ui/ContextMenu.svelte";
import { gitIndexContent, gitStageContent } from "./api";
import type { HunkClick } from "./gutter";
import { chunksInLines, firstLine, lastLine, rollbackChange, stagedContent, toText } from "./hunks";
import type { GitRepo } from "./repo.svelte";

/** Mette in stage le modifiche del buffer nelle righe [from, to], senza toccare il file su disco. */
async function stageLines(repo: GitRepo, tabId: string, path: string, buffer: Text, from: number, to: number) {
  const index = await gitIndexContent(repo.root, path).catch(() => null);
  const { content, chunks } = stagedContent(toText(index ?? ""), buffer, from, to);
  if (!chunks) return message("Nessuna modifica da mettere in stage in queste righe.", { title: "Stage", kind: "info" });
  const eol = repo.workspace.tabs.find((tab) => tab.id === tabId)?.eol ?? "\n";
  return repo.run("Stage parziale", () => gitStageContent(repo.root, path, content.split("\n").join(eol)));
}

/** Popup del click su un marker del gutter: rollback, stage del hunk o delle righe selezionate, diff. */
export function hunkMenu(repo: GitRepo, tabId: string, path: string): HunkClick {
  return (view: EditorView, position, chunks, baseline, event) => {
    const doc = view.state.doc;
    const line = doc.lineAt(position).number;
    const [chunk] = chunksInLines(chunks, doc, line, line);
    if (!chunk) return;

    const selection = view.state.selection.main;
    const items: MenuItem[] = [
      { label: "Rollback della modifica", run: () => view.dispatch({ changes: rollbackChange(baseline, doc, chunk) }) },
      { label: "Stage della modifica", run: () => stageLines(repo, tabId, path, doc, firstLine(chunk, doc), lastLine(chunk, doc)) },
    ];
    if (!selection.empty) {
      const from = doc.lineAt(selection.from).number;
      const to = doc.lineAt(selection.to).number;
      items.push({ label: `Stage delle righe selezionate (${from}–${to})`, run: () => stageLines(repo, tabId, path, doc, from, to) });
    }
    items.push({
      label: "Mostra diff del file",
      separatorBefore: true,
      run: () => (repo.diff.request = { kind: "file", file: { path, status: repo.statusOf(path) ?? "modified" } }),
    });
    openMenu(event.clientX, event.clientY, items);
  };
}
