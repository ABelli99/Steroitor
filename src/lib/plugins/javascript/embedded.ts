import { LanguageSupport } from "@codemirror/language";
import { javascript, javascriptLanguage, jsxLanguage, tsxLanguage, typescriptLanguage } from "@codemirror/lang-javascript";
import { htmlLanguage } from "@codemirror/lang-html";
import { cssLanguage } from "@codemirror/lang-css";
import { angularLanguage } from "@codemirror/lang-angular";
import { parseMixed, type Input, type Parser, type SyntaxNodeRef } from "@lezer/common";

type TemplateKind = "angular" | "html" | "css";

const parsers: Record<TemplateKind, Parser> = {
  angular: angularLanguage.parser,
  html: htmlLanguage.parser,
  css: cssLanguage.parser,
};

const TAGS: Record<string, TemplateKind> = { html: "html", svg: "html", css: "css" };
const PROPERTIES: Record<string, TemplateKind> = { template: "angular", styles: "css" };

const read = (input: Input, node: { from: number; to: number } | null) => (node ? input.read(node.from, node.to) : "");

/**
 * Che cosa contiene una template string: il `template:` / `styles:` di un `@Component` Angular,
 * oppure una stringa taggata `html` / `svg` / `css` (Lit e simili).
 */
export function templateKind(node: SyntaxNodeRef, input: Input): TemplateKind | null {
  const parent = node.node.parent;
  if (!parent) return null;
  if (parent.name === "TaggedTemplateExpression") return TAGS[read(input, parent.firstChild).split(".").pop()!] ?? null;
  const property = parent.name === "ArrayExpression" ? parent.parent : parent;
  if (property?.name !== "Property") return null;
  return PROPERTIES[read(input, property.getChild("PropertyDefinition"))] ?? null;
}

/** Il contenuto della template string, esclusi i backtick e le interpolazioni `${…}`. */
function overlay(node: SyntaxNodeRef, input: Input) {
  const ranges: { from: number; to: number }[] = [];
  let from = node.from + 1;
  for (let child = node.node.firstChild; child; child = child.nextSibling) {
    if (child.name !== "Interpolation") continue;
    ranges.push({ from, to: child.from });
    from = child.to;
  }
  ranges.push({ from, to: input.read(node.to - 1, node.to) === "`" ? node.to - 1 : node.to });
  return ranges.filter((range) => range.to > range.from);
}

const embedTemplates = parseMixed((node, input) => {
  if (node.name !== "TemplateString") return null;
  const kind = templateKind(node, input);
  const ranges = kind ? overlay(node, input) : [];
  return kind && ranges.length ? { parser: parsers[kind], overlay: ranges } : null;
});

/** JavaScript/TypeScript con l'HTML e il CSS dei template inline evidenziati come tali. */
export function scriptWithTemplates(options: { typescript?: boolean; jsx?: boolean }) {
  const base = options.jsx ? (options.typescript ? tsxLanguage : jsxLanguage) : options.typescript ? typescriptLanguage : javascriptLanguage;
  return new LanguageSupport(base.configure({ wrap: embedTemplates }), javascript(options).support);
}
