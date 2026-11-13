import type { Extension } from "@codemirror/state";
import { pluginExtensions, pluginLanguageFor, pluginLanguageName } from "../plugins/registry.svelte";
import { fileName } from "../workspace/files";

interface LanguageDef {
  name: string;
  load: () => Promise<Extension>;
}

const languages: Record<string, LanguageDef> = {
  python: { name: "Python", load: () => import("@codemirror/lang-python").then((m) => m.python()) },
  java: { name: "Java", load: () => import("@codemirror/lang-java").then((m) => m.java()) },
  markdown: { name: "Markdown", load: () => import("@codemirror/lang-markdown").then((m) => m.markdown()) },
  json: { name: "JSON", load: () => import("@codemirror/lang-json").then((m) => m.json()) },
  yaml: { name: "YAML", load: () => import("@codemirror/lang-yaml").then((m) => m.yaml()) },
  html: { name: "HTML", load: () => import("@codemirror/lang-html").then((m) => m.html()) },
  css: { name: "CSS", load: () => import("@codemirror/lang-css").then((m) => m.css()) },
};

const byExtension: Record<string, string> = {
  py: "python", pyw: "python",
  java: "java",
  md: "markdown", markdown: "markdown",
  json: "json", jsonc: "json",
  yml: "yaml", yaml: "yaml",
  html: "html", htm: "html",
  css: "css",
};

/** I plugin attivi hanno la precedenza sui linguaggi integrati. */
export function detectLanguage(path: string | null): string | null {
  const extension = fileName(path ?? "").split(".").pop()?.toLowerCase();
  if (!extension) return null;
  return pluginLanguageFor(extension)?.id ?? byExtension[extension] ?? null;
}

export const languageName = (id: string | null) => (id ? (languages[id]?.name ?? pluginLanguageName(id)) : null) ?? "Testo semplice";

export async function loadLanguage(id: string | null): Promise<Extension> {
  if (!id) return [];
  const [core, extra] = await Promise.all([languages[id]?.load() ?? [], pluginExtensions(id)]);
  return [core, ...extra];
}
