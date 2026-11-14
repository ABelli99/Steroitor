import type { EditorState, Extension } from "@codemirror/state";
import { autocompletion } from "@codemirror/autocomplete";
import { ensureSyntaxTree } from "@codemirror/language";
import { linter, type Diagnostic } from "@codemirror/lint";
import { IterMode } from "@lezer/common";
import { definitionProvider } from "../../editor/definitionLink";
import { documentPath } from "../../editor/setup";
import { acceptWithTab } from "../completion";
import type { PluginModule } from "../types";
import { typeCompletions } from "./completion";
import { scriptWithTemplates } from "./embedded";
import { navigation } from "./navigation";
import type { TypeDiagnostic } from "./typecheck/checker";

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

const syntaxLinter = linter((view) => syntaxErrors(view.state));

const typeLinter = (check: (path: string, content: string) => Promise<TypeDiagnostic[]>) =>
  linter(async (view) => {
    const { state } = view;
    const path = state.facet(documentPath);
    if (!path) return [];
    const found = await check(path, state.doc.toString()).catch((error) => {
      console.error("Type-check", error);
      return [];
    });
    const length = state.doc.length;
    return found.map((diagnostic) => ({ ...diagnostic, from: Math.min(diagnostic.from, length), to: Math.min(diagnostic.to, length) }));
  });

export async function create(options: Record<string, boolean>): Promise<PluginModule> {
  const checker = options.typeCheck ? await import("./typecheck/client") : null;
  // Il worker parte subito: il compilatore si carica mentre l'utente apre i file.
  checker?.startTypeCheck();
  const diagnostics = checker && typeLinter(checker.typeCheck);
  const completion = checker ? typeCompletions(checker) : undefined;

  // Senza type-check, errori di sintassi solo su JS: la grammatica Lezer di TypeScript non copre tutto (es. `(x): x is T =>`).
  const script = (language: { typescript?: boolean; jsx?: boolean }): Extension => [
    scriptWithTemplates(language, completion),
    autocompletion(),
    acceptWithTab,
    diagnostics ?? (language.typescript ? [] : syntaxLinter),
    definitionProvider.of(navigation),
  ];

  return {
    extensions: {
      javascript: script({}),
      jsx: script({ jsx: true }),
      typescript: script({ typescript: true }),
      tsx: script({ typescript: true, jsx: true }),
      // Ctrl+click sui tag dei componenti nei template Angular in file .html.
      html: definitionProvider.of(navigation),
    },
    onFilesChanged: checker ? (dirs) => void checker.invalidateTypeCheck(dirs).catch(console.error) : undefined,
  };
}
