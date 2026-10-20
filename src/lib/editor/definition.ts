import { ensureSyntaxTree, syntaxTree } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";
import type { SyntaxNode } from "@lezer/common";

export type Target = { name: string; from: number; to: number; tag: boolean };

export type Definition = { kind: "local"; from: number } | { kind: "import"; source: string; name: string } | { kind: "selector"; selector: string };

const PARSE_TIMEOUT_MS = 200;

function wordAround(text: string, offset: number, chars: RegExp): [number, number] {
  let from = offset;
  let to = offset;
  while (from > 0 && chars.test(text[from - 1])) from--;
  while (to < text.length && chars.test(text[to])) to++;
  return [from, to];
}

/** Identificatore (o nome di tag, trattini compresi) sotto `pos`, se c'è. */
export function targetAt(state: EditorState, pos: number): Target | null {
  const line = state.doc.lineAt(pos);
  const offset = pos - line.from;
  const [tagFrom, tagTo] = wordAround(line.text, offset, /[\w$-]/);
  const tag = /<\/?$/.test(line.text.slice(0, tagFrom)) || syntaxTree(state).resolveInner(pos, 1).name === "TagName";
  const [from, to] = tag ? [tagFrom, tagTo] : wordAround(line.text, offset, /[\w$]/);
  const name = line.text.slice(from, to);
  if (!/^[A-Za-z_$][\w$-]*$/.test(name)) return null;
  return { name, from: line.from + from, to: line.from + to, tag };
}

const RANK: Record<string, number> = { class: 0, member: 1, declaration: 2, import: 3, parameter: 4 };

function classify(node: SyntaxNode): keyof typeof RANK | null {
  const parent = node.parent;
  if (!parent) return null;
  if (node.name === "PropertyDefinition") return parent.name === "MethodDeclaration" || parent.name === "PropertyDeclaration" ? "member" : null;
  if (parent.name === "ClassDeclaration") return "class";
  if (parent.name === "ImportDeclaration" || parent.name === "ImportGroup") return "import";
  if (parent.name === "ParamList" || parent.parent?.name === "ParamList") return "parameter";
  return "declaration";
}

/** Nome con cui il modulo esporta un import: `B` in `import { B as C }`. */
function importedName(state: EditorState, node: SyntaxNode) {
  const keyword = node.prevSibling;
  if (keyword?.name !== "as" || !keyword.prevSibling) return state.sliceDoc(node.from, node.to);
  const original = state.sliceDoc(keyword.prevSibling.from, keyword.prevSibling.to);
  return original === "*" ? state.sliceDoc(node.from, node.to) : original.replace(/^["']|["']$/g, "");
}

function importSource(state: EditorState, node: SyntaxNode) {
  let declaration: SyntaxNode | null = node;
  while (declaration && declaration.name !== "ImportDeclaration") declaration = declaration.parent;
  const source = declaration?.getChild("String");
  return source ? state.sliceDoc(source.from + 1, source.to - 1) : null;
}

/** Dove è dichiarato il simbolo: nel file, in un modulo importato o, per un tag, nel `selector` di un componente. */
export function findDefinition(state: EditorState, target: Target): Definition | null {
  if (target.tag && target.name.includes("-")) return { kind: "selector", selector: target.name };
  const tree = ensureSyntaxTree(state, state.doc.length, PARSE_TIMEOUT_MS) ?? syntaxTree(state);
  let best: { node: SyntaxNode; rank: number } | null = null;
  tree.iterate({
    enter: (ref) => {
      if (ref.name !== "VariableDefinition" && ref.name !== "TypeDefinition" && ref.name !== "PropertyDefinition") return;
      if (ref.to - ref.from !== target.name.length || state.sliceDoc(ref.from, ref.to) !== target.name) return;
      const kind = classify(ref.node);
      if (kind === null) return;
      if (!best || RANK[kind] < best.rank) best = { node: ref.node, rank: RANK[kind] };
    },
  });
  const found = best as { node: SyntaxNode; rank: number } | null;
  if (!found || (found.node.from <= target.from && found.node.to >= target.to && found.rank !== RANK.import)) return null;
  if (found.rank === RANK.import) {
    const source = importSource(state, found.node);
    return source ? { kind: "import", source, name: importedName(state, found.node) } : null;
  }
  return { kind: "local", from: found.node.from };
}

const SCRIPT_EXTENSIONS = [".ts", ".tsx", ".js", ".jsx", ".mts", ".mjs"];

/**
 * File candidati per lo specificatore di un import: relativo alla cartella del file
 * oppure, per gli alias (`@app/…`, `src/…`), cercato per suffisso tra i file del progetto.
 */
export function resolveImport(fromFile: string, source: string, files: string[]): string | null {
  const normalize = (path: string) => path.replaceAll("\\", "/").toLowerCase();
  const index = new Map(files.map((file) => [normalize(file), file]));
  const candidates = (base: string) => {
    const stem = base.replace(/\.(js|jsx|mjs)$/, "");
    return [base, ...SCRIPT_EXTENSIONS.map((ext) => stem + ext), ...SCRIPT_EXTENSIONS.map((ext) => `${base}/index${ext}`)];
  };

  if (source.startsWith(".")) {
    const parts = normalize(fromFile).split("/").slice(0, -1);
    for (const segment of source.split("/")) {
      if (segment === "..") parts.pop();
      else if (segment !== ".") parts.push(segment.toLowerCase());
    }
    for (const candidate of candidates(parts.join("/"))) if (index.has(candidate)) return index.get(candidate)!;
    return null;
  }

  const segments = source.toLowerCase().split("/");
  const suffixes = [segments.join("/"), segments.slice(1).join("/")].filter(Boolean);
  for (const suffix of suffixes) {
    for (const candidate of candidates(suffix)) {
      const match = [...index.keys()].find((file) => file.endsWith(`/${candidate}`) && !file.includes("/node_modules/"));
      if (match) return index.get(match)!;
    }
  }
  return null;
}

const escape = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** Riga (1-based) in cui un modulo dichiara `name`, o 1 se non si trova. */
export function declarationLine(content: string, name: string): number {
  const id = escape(name);
  const patterns = [
    new RegExp(`\\b(class|interface|type|enum|function\\*?|const|let|var|namespace)\\s+${id}\\b`),
    new RegExp(`^\\s*export\\s*\\{[^}]*\\b${id}\\b`),
    name === "default" ? /^\s*export\s+default\b/ : null,
  ].filter((pattern): pattern is RegExp => pattern !== null);
  const lines = content.split(/\r\n|\r|\n/);
  for (const pattern of patterns) {
    const index = lines.findIndex((line) => pattern.test(line));
    if (index !== -1) return index + 1;
  }
  return 1;
}

/** Vero se la riga dichiara il `selector` di un componente che corrisponde al tag. */
export function declaresSelector(line: string, selector: string) {
  const match = /\bselector\s*:\s*(['"`])(.*?)\1/.exec(line);
  return !!match && match[2].split(",").some((part) => part.trim() === selector);
}
