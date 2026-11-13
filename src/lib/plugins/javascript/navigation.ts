import type { DefinitionProvider } from "../types";
import { declarationLine, declaresSelector, findDefinition, resolveImport, targetAt } from "./definition";

const COMPONENT_EXTENSIONS = ["ts", "js"];

/** Ctrl+click: porta alla dichiarazione del metodo, della variabile o del componente sotto il cursore. */
export const navigation: DefinitionProvider = {
  targetAt,
  async goTo(host, state, pos) {
    const target = targetAt(state, pos);
    const definition = target && findDefinition(state, target);
    if (!definition) return;

    if (definition.kind === "local") return host.goToPosition(definition.from);
    if (!host.root) return;

    if (definition.kind === "selector") {
      const matches = await host.searchFiles(host.root, definition.selector, COMPONENT_EXTENSIONS);
      const match = matches.find((candidate) => declaresSelector(candidate.text, definition.selector));
      if (match) await host.openAt(match.path, match.line);
      return;
    }

    if (!host.currentPath) return;
    const path = resolveImport(host.currentPath, definition.source, await host.fileIndex());
    if (!path) return;
    const content = await host.readFile(path);
    await host.openAt(path, content ? declarationLine(content, definition.name) : 1);
  },
};
