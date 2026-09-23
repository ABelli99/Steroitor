import type { Extension } from "@codemirror/state";
import { fileName } from "../workspace/files";

interface LanguageDef {
  name: string;
  load: () => Promise<Extension>;
}

const javascript = (options: { typescript?: boolean; jsx?: boolean } = {}) => () =>
  import("@codemirror/lang-javascript").then((m) => m.javascript(options));

const languages: Record<string, LanguageDef> = {
  javascript: { name: "JavaScript", load: javascript() },
  jsx: { name: "JavaScript JSX", load: javascript({ jsx: true }) },
  typescript: { name: "TypeScript", load: javascript({ typescript: true }) },
  tsx: { name: "TypeScript JSX", load: javascript({ typescript: true, jsx: true }) },
  python: { name: "Python", load: () => import("@codemirror/lang-python").then((m) => m.python()) },
  java: { name: "Java", load: () => import("@codemirror/lang-java").then((m) => m.java()) },
  markdown: { name: "Markdown", load: () => import("@codemirror/lang-markdown").then((m) => m.markdown()) },
  json: { name: "JSON", load: () => import("@codemirror/lang-json").then((m) => m.json()) },
  yaml: { name: "YAML", load: () => import("@codemirror/lang-yaml").then((m) => m.yaml()) },
  html: { name: "HTML", load: () => import("@codemirror/lang-html").then((m) => m.html()) },
  css: { name: "CSS", load: () => import("@codemirror/lang-css").then((m) => m.css()) },
};

const byExtension: Record<string, string> = {
  js: "javascript", mjs: "javascript", cjs: "javascript",
  jsx: "jsx",
  ts: "typescript", mts: "typescript", cts: "typescript",
  tsx: "tsx",
  py: "python", pyw: "python",
  java: "java",
  md: "markdown", markdown: "markdown",
  json: "json", jsonc: "json",
  yml: "yaml", yaml: "yaml",
  html: "html", htm: "html",
  css: "css",
};

export function detectLanguage(path: string | null): string | null {
  const extension = fileName(path ?? "").split(".").pop()?.toLowerCase();
  return extension ? (byExtension[extension] ?? null) : null;
}

export const languageName = (id: string | null) => (id ? languages[id]?.name : null) ?? "Testo semplice";

export const loadLanguage = (id: string | null): Promise<Extension> =>
  id && languages[id] ? languages[id].load() : Promise.resolve([]);
