import { acceptCompletion } from "@codemirror/autocomplete";
import { Prec } from "@codemirror/state";
import { keymap } from "@codemirror/view";

/** Tab accetta il suggerimento; con il popup chiuso acceptCompletion non fa nulla e Tab torna a indentare. */
export const acceptWithTab = Prec.highest(keymap.of([{ key: "Tab", run: acceptCompletion }]));

/** Riquadro accanto al suggerimento: firma e documentazione. */
export function completionInfo(signature: string, documentation = "") {
  const dom = document.createElement("div");
  dom.className = "cm-completionDocs";
  dom.appendChild(document.createElement("code")).textContent = signature;
  if (documentation) dom.appendChild(document.createElement("p")).textContent = documentation;
  return dom;
}
