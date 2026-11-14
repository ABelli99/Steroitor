import { snippetCompletion, type Completion, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import { ensureSyntaxTree, syntaxTree } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";
import type { SyntaxNode } from "@lezer/common";
import { completionInfo } from "../completion";
import { JDK } from "./jdk";

type Kind = "class" | "method" | "property" | "variable" | "constant";

interface Symbol {
  name: string;
  kind: Kind;
  /** Tipo del valore; per un metodo, il tipo restituito. */
  type: string;
  signature: string;
  doc: string;
  isStatic: boolean;
  /** Classe che dichiara il membro; null per classi di primo livello e variabili locali. */
  owner: string | null;
  /** Solo per le variabili locali e i parametri: dove sono visibili. */
  scope?: { from: number; to: number };
  from: number;
  superclass?: string;
}

const PARSE_TIMEOUT_MS = 200;
const CLASS_NODES = new Set(["ClassDeclaration", "InterfaceDeclaration", "EnumDeclaration", "RecordDeclaration", "AnnotationTypeDeclaration"]);
const TYPE_NODES = new Set(["TypeName", "GenericType", "PrimitiveType", "ArrayType", "ScopedTypeName", "void"]);
const SCOPE_NODES = new Set([
  "Block", "ConstructorBody", "MethodDeclaration", "ConstructorDeclaration", "ForStatement", "EnhancedForStatement",
  "CatchClause", "LambdaExpression", "TryWithResourcesStatement", "SwitchBlock",
]);
const NO_COMPLETION = new Set(["LineComment", "BlockComment", "StringLiteral", "TextBlock", "CharacterLiteral"]);

const KEYWORDS = (
  "abstract assert boolean break byte case catch char class continue default do double else enum extends final finally " +
  "float for if implements import instanceof int interface long new null package private protected public record return " +
  "short static super switch synchronized this throw throws true false try var void volatile while yield"
).split(" ");

const SNIPPETS = [
  snippetCompletion("System.out.println(${});", { label: "sout", detail: "System.out.println", type: "keyword" }),
  snippetCompletion("public static void main(String[] args) {\n\t${}\n}", { label: "main", detail: "metodo main", type: "keyword" }),
  snippetCompletion("for (int ${i} = 0; ${i} < ${n}; ${i}++) {\n\t${}\n}", { label: "fori", detail: "for con indice", type: "keyword" }),
  snippetCompletion("for (${Type} ${item} : ${items}) {\n\t${}\n}", { label: "foreach", detail: "for-each", type: "keyword" }),
  snippetCompletion("if (${condition}) {\n\t${}\n}", { label: "if", detail: "blocco if", type: "keyword" }),
  snippetCompletion("try {\n\t${}\n} catch (${Exception} e) {\n\t\n}", { label: "try", detail: "try / catch", type: "keyword" }),
  snippetCompletion("public class ${Name} {\n\t${}\n}", { label: "class", detail: "nuova classe", type: "keyword" }),
];

const typeText = (node: SyntaxNode | null, state: EditorState) => (node ? state.sliceDoc(node.from, node.to) : "");

/** `List<String>` → `List`, `int[]` → `[]`: il nome con cui cercare i membri. */
const baseType = (type: string) => (/\[\s*\]$/.test(type) ? "[]" : type.replace(/<[\s\S]*>/, "").trim().split(".").pop()!);

function typeChild(node: SyntaxNode | null) {
  for (let child = node?.firstChild; child; child = child.nextSibling) if (TYPE_NODES.has(child.name)) return child;
  return null;
}

function enclosing(node: SyntaxNode, names: Set<string>) {
  for (let parent = node.parent; parent; parent = parent.parent) if (names.has(parent.name)) return parent;
  return null;
}

const nameOf = (declaration: SyntaxNode | null, state: EditorState) => typeText(declaration?.getChild("Definition") ?? null, state) || null;

const isStaticDeclaration = (declaration: SyntaxNode, state: EditorState) => /\bstatic\b/.test(typeText(declaration.getChild("Modifiers"), state));

function symbolFor(definition: SyntaxNode, state: EditorState): Symbol | null {
  const parent = definition.parent;
  if (!parent) return null;
  const name = typeText(definition, state);
  const owner = nameOf(enclosing(parent, CLASS_NODES), state);
  const base = { name, owner, from: definition.from, doc: "" };

  if (CLASS_NODES.has(parent.name)) {
    const superclass = typeText(parent.getChild("Superclass")?.getChild("TypeName") ?? null, state) || undefined;
    return { ...base, kind: "class", type: name, signature: `class ${name}${superclass ? ` extends ${superclass}` : ""}`, isStatic: true, superclass };
  }
  if (parent.name === "MethodDeclaration") {
    const type = typeText(typeChild(parent), state) || "void";
    const params = typeText(parent.getChild("FormalParameters"), state);
    return { ...base, kind: "method", type, signature: `${type} ${name}${params}`, isStatic: isStaticDeclaration(parent, state) };
  }
  if (parent.name === "EnumConstant") {
    return { ...base, kind: "constant", type: owner ?? "", signature: `${owner}.${name}`, isStatic: true };
  }
  if (parent.name === "VariableDeclarator") {
    const declaration = parent.parent;
    if (!declaration) return null;
    // `var x = new Foo()`: il tipo viene dall'inizializzatore.
    const created = parent.getChild("ObjectCreationExpression");
    const type = typeText(typeChild(declaration), state) || typeText(typeChild(created), state);
    if (declaration.name === "FieldDeclaration") {
      return { ...base, kind: "property", type, signature: `${type} ${name}`, isStatic: isStaticDeclaration(declaration, state) };
    }
    const scope = enclosing(declaration, SCOPE_NODES);
    return { ...base, owner: null, kind: "variable", type, signature: `${type} ${name}`, isStatic: false, scope: scope ?? undefined };
  }
  // Parametri di metodi, for-each, catch, lambda: il tipo è il nodo prima del nome.
  const previous = definition.prevSibling;
  const type = previous && TYPE_NODES.has(previous.name) ? typeText(previous, state) : "";
  const scope = enclosing(definition, SCOPE_NODES);
  return { ...base, owner: null, kind: "variable", type, signature: `${type} ${name}`.trim(), isStatic: false, scope: scope ?? undefined };
}

function fileSymbols(state: EditorState) {
  const tree = ensureSyntaxTree(state, state.doc.length, PARSE_TIMEOUT_MS) ?? syntaxTree(state);
  const symbols: Symbol[] = [];
  tree.iterate({
    enter: (node) => {
      if (node.name !== "Definition") return;
      const symbol = symbolFor(node.node, state);
      if (symbol?.name) symbols.push(symbol);
    },
  });
  return { tree, symbols };
}

const JDK_SIGNATURE = /^(static )?([\s\S]+?) ([\w$]+)(\([\s\S]*\))?$/;

function jdkMembers(type: string): Symbol[] {
  const entry = JDK[type];
  if (!entry) return [];
  const own = entry.members.flatMap(([signature, doc]): Symbol[] => {
    const match = JDK_SIGNATURE.exec(signature);
    if (!match) return [];
    const [, isStatic, returnType, name, params] = match;
    const kind: Kind = params ? "method" : isStatic && /^[A-Z_]+$/.test(name) ? "constant" : "property";
    return [{ name, kind, type: returnType, signature, doc, isStatic: !!isStatic, owner: type, from: 0 }];
  });
  return [...own, ...(entry.extends ? jdkMembers(entry.extends) : [])];
}

class Scope {
  constructor(
    private readonly symbols: Symbol[],
    private readonly pos: number,
    private readonly enclosingClasses: string[],
  ) {}

  get enclosing() {
    return this.enclosingClasses[0] ?? "";
  }

  /** Membri di un tipo: classi del file (con le superclassi) o del JDK; gli array hanno `length`. */
  members(type: string, seen = new Set<string>()): Symbol[] {
    if (type === "[]") return [{ name: "length", kind: "property", type: "int", signature: "int length", doc: "Numero di elementi dell'array.", isStatic: false, owner: null, from: 0 }, ...jdkMembers("Object")];
    if (seen.has(type)) return [];
    seen.add(type);
    const declared = this.symbols.find((symbol) => symbol.kind === "class" && symbol.name === type);
    if (!declared) return jdkMembers(type);
    const own = this.symbols.filter((symbol) => symbol.owner === type && symbol.kind !== "variable");
    return [...own, ...this.members(declared.superclass ? baseType(declared.superclass) : "Object", seen)];
  }

  variable(name: string) {
    const locals = this.symbols.filter((symbol) => symbol.kind === "variable" && symbol.name === name && this.#visible(symbol));
    const local = locals.sort((a, b) => b.from - a.from)[0];
    if (local) return local;
    return this.enclosingClasses.flatMap((owner) => this.members(owner)).find((symbol) => symbol.name === name && symbol.kind === "property");
  }

  isType = (name: string) => !!JDK[name] || this.symbols.some((symbol) => symbol.kind === "class" && symbol.name === name);

  visibleNames(): Symbol[] {
    const locals = this.symbols.filter((symbol) => symbol.kind === "variable" && this.#visible(symbol));
    const members = this.enclosingClasses.flatMap((owner) => this.members(owner));
    const classes = this.symbols.filter((symbol) => symbol.kind === "class");
    return [...locals, ...members, ...classes];
  }

  #visible = (symbol: Symbol) => !!symbol.scope && symbol.scope.from <= this.pos && this.pos <= symbol.scope.to && symbol.from < this.pos;
}

/** Tipo dell'espressione prima del punto (`a`, `this.items`, `System.out`, `name.trim()`), e se è un accesso statico. */
function resolveChain(chain: string, scope: Scope): { type: string; isStatic: boolean } | null {
  const segments = [...chain.matchAll(/([\w$]+)(\([^()]*\))?\s*\.\s*/g)].map(([, name, call]) => ({ name, call: !!call }));
  const [first, ...rest] = segments;
  if (!first) return null;
  let current: { type: string; isStatic: boolean } | null = null;
  if (first.name === "this") current = { type: scope.enclosing, isStatic: false };
  else if (first.call) {
    const method = scope.visibleNames().find((symbol) => symbol.kind === "method" && symbol.name === first.name);
    current = method ? { type: baseType(method.type), isStatic: false } : null;
  } else {
    const variable = scope.variable(first.name);
    if (variable) current = { type: baseType(variable.type), isStatic: false };
    else if (scope.isType(first.name)) current = { type: first.name, isStatic: true };
  }
  for (const segment of rest) {
    if (!current) return null;
    const member = scope.members(current.type).find((symbol) => symbol.name === segment.name && (symbol.kind === "method") === segment.call);
    current = member ? { type: baseType(member.type), isStatic: false } : null;
  }
  return current;
}

const BOOST: Record<Kind, number> = { variable: 3, property: 2, method: 2, constant: 2, class: 1 };

/** Per un metodo, i parametri accanto al nome (`substring(int beginIndex, int endIndex)`); il tipo restituito va nel detail. */
const parametersOf = (symbol: Symbol) => (symbol.kind === "method" ? symbol.signature.slice(symbol.signature.indexOf(`${symbol.name}(`) + symbol.name.length) : "");

function toCompletion(symbol: Symbol): Completion {
  return {
    label: symbol.name,
    displayLabel: symbol.name + parametersOf(symbol),
    type: symbol.kind,
    detail: symbol.kind === "class" ? undefined : symbol.type,
    boost: BOOST[symbol.kind],
    info: () => completionInfo(symbol.signature, symbol.doc),
  };
}

/** Un suggerimento per nome: le variabili locali nascondono i campi omonimi, i membri ridefiniti quelli ereditati. */
function unique(symbols: Symbol[]) {
  const seen = new Set<string>();
  return symbols.filter((symbol) => {
    const key = `${symbol.kind === "method" ? "method" : "value"}:${symbol.name}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export function javaCompletions(context: CompletionContext): CompletionResult | null {
  const { state, pos } = context;
  const { tree, symbols } = fileSymbols(state);
  if (NO_COMPLETION.has(tree.resolveInner(pos, -1).name)) return null;

  const word = context.matchBefore(/[\w$]*/)!;
  const before = state.sliceDoc(state.doc.lineAt(pos).from, word.from);
  const chain = /((?:[\w$]+(?:\([^()]*\))?\s*\.\s*)+)$/.exec(before)?.[1];
  const enclosingClasses = classesAround(tree.resolveInner(pos, -1), state);
  const scope = new Scope(symbols, pos, enclosingClasses);

  if (chain) {
    const target = resolveChain(chain, scope);
    if (!target) return null;
    const members = unique(scope.members(target.type)).filter((symbol) => symbol.isStatic === target.isStatic);
    return { from: word.from, options: members.map(toCompletion), validFor: /^[\w$]*$/ };
  }

  if (word.from === word.to && !context.explicit) return null;
  const options = [
    ...unique(scope.visibleNames()).map(toCompletion),
    ...Object.entries(JDK).map(([name, type]): Completion => ({ label: name, type: "class", info: () => completionInfo(`class ${name}`, type.doc) })),
    ...KEYWORDS.map((keyword): Completion => ({ label: keyword, type: "keyword", boost: -1 })),
    ...SNIPPETS,
  ];
  return { from: word.from, options, validFor: /^[\w$]*$/ };
}

/** Classi che contengono il nodo, dalla più interna. */
function classesAround(node: SyntaxNode, state: EditorState) {
  const names: string[] = [];
  for (let parent: SyntaxNode | null = node; parent; parent = parent.parent) {
    const name = CLASS_NODES.has(parent.name) ? nameOf(parent, state) : null;
    if (name) names.push(name);
  }
  return names;
}
