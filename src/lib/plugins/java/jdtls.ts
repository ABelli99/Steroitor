import {
  insertCompletionText, pickedCompletion, type Completion, type CompletionContext, type CompletionResult, type CompletionSource,
} from "@codemirror/autocomplete";
import type { Text } from "@codemirror/state";
import { ViewPlugin, type EditorView, type ViewUpdate } from "@codemirror/view";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { lspSend, lspStart } from "../../backend";
import { documentPath } from "../../editor/setup";
import { completionInfo } from "../completion";
import { reportPluginProblem } from "../problems.svelte";
import { fileUri, LspConnection } from "./lsp";

/** Prima che jdtls abbia importato il progetto le risposte tardano: oltre questo limite si usa il completamento locale. */
const COMPLETION_TIMEOUT_MS = 3000;
const RESOLVE_TIMEOUT_MS = 2000;
const SYNC_DELAY_MS = 300;

interface Position {
  line: number;
  character: number;
}

interface TextEdit {
  range: { start: Position; end: Position };
  newText: string;
}

interface LspCompletionItem {
  label: string;
  kind?: number;
  detail?: string;
  labelDetails?: { detail?: string; description?: string };
  documentation?: string | { value: string };
  sortText?: string;
  insertText?: string;
  textEdit?: TextEdit;
  additionalTextEdits?: TextEdit[];
}

/** `CompletionItemKind` del protocollo → icone di CodeMirror. */
const ICONS: Record<number, string> = {
  2: "method", 3: "function", 4: "function", 5: "property", 6: "variable", 7: "class", 8: "interface", 9: "namespace",
  10: "property", 13: "enum", 14: "keyword", 15: "keyword", 20: "enum", 21: "constant", 22: "class", 25: "type",
};

const byId = new Map<number, LspConnection>();
const byFile = new Map<string, Promise<LspConnection>>();
let listening: Promise<unknown> | null = null;
/** shortcut: dopo un avvio fallito o una chiusura non si riprova fino al riavvio dell'app, per non rilanciare Java a ogni tasto. */
let failure: Error | null = null;

function listen() {
  const appWindow = getCurrentWebviewWindow();
  listening ??= Promise.all([
    appWindow.listen<{ id: number; message: string }>("lsp-message", ({ payload }) => byId.get(payload.id)?.receive(payload.message)),
    appWindow.listen<{ id: number; error: string }>("lsp-exit", ({ payload }) => {
      const connection = byId.get(payload.id);
      byId.delete(payload.id);
      connection?.close("jdtls si è chiuso");
      byFile.clear();
      fail("jdtls si è chiuso: uso il completamento locale fino al riavvio", payload.error || "Nessun messaggio di errore da jdtls.");
    }),
  ]);
  return listening;
}

function fail(message: string, detail: string) {
  failure = new Error(`${message}\n${detail}`);
  console.error(failure);
  reportPluginProblem({ plugin: "Java", message, detail });
}

/** Un server per progetto: il backend riusa quello già avviato per la stessa cartella. */
function connectionFor(path: string) {
  if (failure) return Promise.reject(failure);
  let connection = byFile.get(path);
  if (connection) return connection;
  connection = listen()
    .then(() => lspStart("jdtls", path))
    .then(({ id, root }) => {
      const existing = byId.get(id);
      if (existing) return existing;
      const created = new LspConnection((message) => lspSend(id, message), root);
      byId.set(id, created);
      return created;
    });
  connection.catch((error) => fail("jdtls non è partito: uso il completamento locale", error instanceof Error ? error.message : String(error)));
  byFile.set(path, connection);
  return connection;
}

const positionOf = (doc: Text, pos: number): Position => {
  const line = doc.lineAt(pos);
  return { line: line.number - 1, character: pos - line.from };
};

const offsetOf = (doc: Text, position: Position) => {
  const line = doc.line(Math.min(doc.lines, position.line + 1));
  return Math.min(line.to, line.from + position.character);
};

const documentationOf = (item: LspCompletionItem) => (typeof item.documentation === "string" ? item.documentation : (item.documentation?.value ?? ""));

/**
 * Inserisce il suggerimento e poi le modifiche aggiuntive del server (es. l'`import` della classe), che arrivano con resolve.
 * shortcut: le modifiche aggiuntive si applicano alle posizioni del documento attuale, giusto finché stanno sopra il cursore (gli import).
 */
function applyItem(connection: LspConnection, item: LspCompletionItem): Completion["apply"] {
  return (view, completion, from, to) => {
    const text = item.textEdit?.newText ?? item.insertText ?? item.label;
    view.dispatch({ ...insertCompletionText(view.state, text, from, to), annotations: pickedCompletion.of(completion) });
    void connection
      .request<LspCompletionItem>("completionItem/resolve", item, RESOLVE_TIMEOUT_MS)
      .then((resolved) => {
        const edits = resolved?.additionalTextEdits ?? [];
        if (!edits.length) return;
        const { doc } = view.state;
        view.dispatch({ changes: edits.map((edit) => ({ from: offsetOf(doc, edit.range.start), to: offsetOf(doc, edit.range.end), insert: edit.newText })) });
      })
      .catch(() => {});
  };
}

function toCompletion(connection: LspConnection, item: LspCompletionItem, boost: number): Completion {
  return {
    label: item.textEdit?.newText ?? item.insertText ?? item.label,
    displayLabel: item.label + (item.labelDetails?.detail ?? ""),
    detail: item.labelDetails?.description,
    type: item.kind ? ICONS[item.kind] : undefined,
    boost,
    apply: applyItem(connection, item),
    info: () =>
      connection
        .request<LspCompletionItem>("completionItem/resolve", item, RESOLVE_TIMEOUT_MS)
        .then((resolved) => completionInfo(resolved?.detail ?? item.detail ?? item.label, documentationOf(resolved ?? item)))
        .catch(() => completionInfo(item.detail ?? item.label)),
  };
}

/** L'ordine di jdtls (`sortText`) diventa il `boost`: rango 0 → 99, poi a scendere. */
function boosts(items: LspCompletionItem[]) {
  const ranks = [...new Set(items.map((item) => item.sortText ?? item.label))].sort();
  return (item: LspCompletionItem) => 99 - Math.min(198, ranks.indexOf(item.sortText ?? item.label));
}

/** Completamento da jdtls: classi, metodi e campi del progetto e delle dipendenze; senza risposta, `fallback`. */
export const jdtlsCompletions = (fallback: CompletionSource): CompletionSource => async (context: CompletionContext) => {
  const path = context.state.facet(documentPath);
  const word = context.matchBefore(/[\w$]*/)!;
  const afterDot = context.state.sliceDoc(word.from - 1, word.from) === ".";
  if (!path || (word.from === word.to && !afterDot && !context.explicit)) return fallback(context);

  try {
    const connection = await connectionFor(path);
    await connection.sync(path, context.state.doc.toString());
    const result = await connection.request<{ items: LspCompletionItem[]; isIncomplete?: boolean } | LspCompletionItem[] | null>(
      "textDocument/completion",
      { textDocument: { uri: fileUri(path) }, position: positionOf(context.state.doc, context.pos) },
      COMPLETION_TIMEOUT_MS,
    );
    const items = Array.isArray(result) ? result : (result?.items ?? []);
    if (context.aborted) return null;
    if (!items.length) return fallback(context);
    const boostOf = boosts(items);
    const incomplete = !Array.isArray(result) && !!result?.isIncomplete;
    return {
      from: word.from,
      options: items.map((item) => toCompletion(connection, item, boostOf(item))),
      validFor: incomplete ? undefined : /^[\w$]*$/,
    } satisfies CompletionResult;
  } catch {
    return fallback(context);
  }
};

/** Tiene jdtls allineato al testo delle tab Java aperte, anche quelle in cui non si chiede il completamento. */
export const documentSync = ViewPlugin.fromClass(
  class {
    #timer: ReturnType<typeof setTimeout> | undefined;
    #path: string | null;

    constructor(private readonly view: EditorView) {
      this.#path = view.state.facet(documentPath);
      this.#sync();
    }

    update(update: ViewUpdate) {
      if (!update.docChanged) return;
      clearTimeout(this.#timer);
      this.#timer = setTimeout(() => this.#sync(), SYNC_DELAY_MS);
    }

    destroy() {
      clearTimeout(this.#timer);
      const path = this.#path;
      if (path) void byFile.get(path)?.then((connection) => connection.forget(path)).catch(() => {});
    }

    #sync() {
      const path = this.#path;
      if (!path) return;
      void connectionFor(path)
        .then((connection) => connection.sync(path, this.view.state.doc.toString()))
        .catch(() => {});
    }
  },
);
