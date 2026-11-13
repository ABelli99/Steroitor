import type { EditorState, Extension } from "@codemirror/state";
import { autocompletion } from "@codemirror/autocomplete";
import { ensureSyntaxTree } from "@codemirror/language";
import { linter, type Diagnostic } from "@codemirror/lint";
import { IterMode } from "@lezer/common";
import { definitionProvider } from "../../editor/definitionLink";
import { scriptWithTemplates } from "./embedded";
import { navigation } from "./navigation";

const PARSE_TIMEOUT_MS = 200;

/**
 * Solo sull'albero completo: uno parziale chiude i costrutti aperti con nodi di errore.
 * Si saltano i template inline (alberi montati): le loro grammatiche non conoscono la sintassi Angular.
 */
export function syntaxErrors(state: EditorState): Diagnostic[] {
  const tree = ensureSyntaxTree(state, state.doc.length, PARSE_TIMEOUT_MS);
  if (!tree) return [];
  const diagnostics: Diagnostic[] = [];
  tree.iterate({
    mode: IterMode.IgnoreMounts,
    enter: (node) => {
      if (node.type.isError) diagnostics.push({ from: node.from, to: node.to, severity: "error", message: "Errore di sintassi" });
    },
  });
  return diagnostics;
}

// La grammatica Lezer di TypeScript non copre tutto (es. `(x): x is T =>`): sul TS gli errori sarebbero falsi positivi.
const script = (options: { typescript?: boolean; jsx?: boolean }): Extension => [
  scriptWithTemplates(options),
  autocompletion(),
  options.typescript ? [] : linter((view) => syntaxErrors(view.state)),
  definitionProvider.of(navigation),
];

export const extensions: Record<string, Extension> = {
  javascript: script({}),
  jsx: script({ jsx: true }),
  typescript: script({ typescript: true }),
  tsx: script({ typescript: true, jsx: true }),
  // Ctrl+click sui tag dei componenti nei template Angular in file .html.
  html: definitionProvider.of(navigation),
};
