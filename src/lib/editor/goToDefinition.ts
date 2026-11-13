import type { EditorState } from "@codemirror/state";
import { readTextFile, searchFiles } from "../backend";
import type { FileTree } from "../explorer/tree.svelte";
import { samePath } from "../workspace/files";
import type { Workspace } from "../workspace/workspace.svelte";
import { definitionProvider } from "./definitionLink";

/** Ctrl+click: la navigazione la decide il plugin del linguaggio, l'app gli presta workspace e file del progetto. */
export async function goToDefinition(workspace: Workspace, tree: FileTree, state: EditorState, pos: number) {
  const provider = state.facet(definitionProvider);
  if (!provider) return;
  await provider.goTo(
    {
      root: tree.root,
      currentPath: workspace.active?.path ?? null,
      goToPosition: (target) => workspace.goToPosition(target),
      async openAt(path, line) {
        await workspace.openPath(path);
        if (workspace.active?.path && samePath(workspace.active.path, path)) workspace.goToLine(line);
      },
      fileIndex: () => tree.fileIndex(),
      searchFiles,
      readFile: (path) => readTextFile(path).then((file) => file.content, () => null),
    },
    state,
    pos,
  );
}
