import { autocompletion } from "@codemirror/autocomplete";
import { javaLanguage } from "@codemirror/lang-java";
import { acceptWithTab } from "../completion";
import type { PluginModule } from "../types";
import { javaCompletions } from "./completion";

/** Solo il completamento: l'evidenziazione di Java è già nei linguaggi integrati. */
export async function create(options: Record<string, boolean>): Promise<PluginModule> {
  if (!options.completion) return { extensions: {} };
  const jdtls = options.jdtls ? await import("./jdtls") : null;
  const source = jdtls ? jdtls.jdtlsCompletions(javaCompletions) : javaCompletions;
  return {
    extensions: {
      java: [javaLanguage.data.of({ autocomplete: source }), autocompletion(), acceptWithTab, jdtls?.documentSync ?? []],
    },
  };
}
