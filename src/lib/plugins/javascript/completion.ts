import { insertCompletionText, pickedCompletion, type Completion, type CompletionContext, type CompletionResult, type CompletionSource } from "@codemirror/autocomplete";
import { localCompletionSource } from "@codemirror/lang-javascript";
import { documentPath } from "../../editor/setup";
import { completionInfo } from "../completion";
import type { CompletionDetails, CompletionItem } from "./typecheck/checker";

type Client = typeof import("./typecheck/client");

/** Tipi di TypeScript (`ScriptElementKind`) → icone di CodeMirror. */
const ICONS: Record<string, string> = {
  method: "method",
  function: "function",
  "local function": "function",
  property: "property",
  getter: "property",
  setter: "property",
  var: "variable",
  let: "variable",
  "local var": "variable",
  parameter: "variable",
  alias: "variable",
  const: "constant",
  class: "class",
  "local class": "class",
  interface: "interface",
  type: "type",
  "type parameter": "type",
  enum: "enum",
  "enum member": "enum",
  keyword: "keyword",
  module: "namespace",
  "external module name": "namespace",
};

const renderDetails = (details: CompletionDetails | null) => details && completionInfo(details.signature, details.documentation);

/** Ordine di TypeScript (`sortText`, più basso = più rilevante) tradotto nel `boost` di CodeMirror. */
const boostOf = (sortText: string) => -Math.min(99, parseInt(sortText, 10) || 0);

/** Con `replacementSpan` TypeScript sostituisce anche testo prima della parola (es. `win.` + `?.event` → `win?.event`). */
function applyOf(item: CompletionItem): Completion["apply"] {
  const span = item.replacementSpan;
  if (!span) return item.insertText;
  const text = item.insertText ?? item.name;
  return (view, completion, _from, to) =>
    view.dispatch({ ...insertCompletionText(view.state, text, span.start, to), annotations: pickedCompletion.of(completion) });
}

/** Completamento dal language service: membri del tipo (es. i metodi di un array) con firma e JSDoc. */
export const typeCompletions = (client: Client): CompletionSource => async (context: CompletionContext): Promise<CompletionResult | null> => {
  const path = context.state.facet(documentPath);
  if (!path) return localCompletionSource(context);
  const word = context.matchBefore(/[\w$]*/)!;
  const afterDot = context.state.sliceDoc(word.from - 1, word.from) === ".";
  if (word.from === word.to && !afterDot && !context.explicit) return null;

  const content = context.state.doc.toString();
  const items = await client.complete(path, content, context.pos).catch((error) => {
    console.error("Completamento", error);
    return [] as CompletionItem[];
  });
  if (context.aborted || !items.length) return null;

  return {
    from: word.from,
    validFor: /^[\w$]*$/,
    options: items.map(
      (item): Completion => ({
        label: item.name,
        apply: applyOf(item),
        type: ICONS[item.kind],
        boost: boostOf(item.sortText),
        info: () => client.completionDetails(path, content, context.pos, item).then(renderDetails, () => null),
      }),
    ),
  };
};
