import type { EditorState } from "@codemirror/state";
import { readTextFile, searchFiles } from "../backend";
import type { FileTree } from "../explorer/tree.svelte";
import { samePath } from "../workspace/files";
import type { Workspace } from "../workspace/workspace.svelte";
import { declarationLine, declaresSelector, findDefinition, resolveImport, targetAt } from "./definition";

const COMPONENT_EXTENSIONS = ["ts", "js"];

async function openAtLine(workspace: Workspace, path: string, line: number) {
  await workspace.openPath(path);
  if (workspace.active?.path && samePath(workspace.active.path, path)) workspace.goToLine(line);
}

/** Ctrl+click: porta alla dichiarazione del metodo, della variabile o del componente sotto il cursore. */
export async function goToDefinition(workspace: Workspace, tree: FileTree, state: EditorState, pos: number) {
  const target = targetAt(state, pos);
  const definition = target && findDefinition(state, target);
  if (!definition) return;

  if (definition.kind === "local") return workspace.goToPosition(definition.from);

  const root = tree.root;
  if (!root) return;

  if (definition.kind === "selector") {
    const matches = await searchFiles(root, definition.selector, COMPONENT_EXTENSIONS);
    const match = matches.find((candidate) => declaresSelector(candidate.text, definition.selector));
    if (match) await openAtLine(workspace, match.path, match.line);
    return;
  }

  const current = workspace.active?.path;
  if (!current) return;
  const path = resolveImport(current, definition.source, await tree.fileIndex());
  if (!path) return;
  const file = await readTextFile(path).catch(() => null);
  await openAtLine(workspace, path, file ? declarationLine(file.content, definition.name) : 1);
}
