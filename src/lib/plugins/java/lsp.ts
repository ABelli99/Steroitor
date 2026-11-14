/** Client JSON-RPC minimo per un language server: richieste, notifiche e sincronizzazione dei documenti (testo intero). */

type Pending = { resolve: (value: unknown) => void; reject: (error: Error) => void };

export const fileUri = (path: string) =>
  "file:///" +
  path
    .replaceAll("\\", "/")
    .split("/")
    .map((segment) => (/^[a-z]:$/i.test(segment) ? segment : encodeURIComponent(segment)))
    .join("/");

export class LspConnection {
  #nextId = 1;
  #pending = new Map<number, Pending>();
  #documents = new Map<string, { version: number; text: string }>();
  #closed: Error | null = null;
  readonly ready: Promise<unknown>;

  constructor(
    private readonly transport: (message: string) => Promise<void>,
    root: string,
  ) {
    this.ready = this.#initialize(root);
  }

  async #initialize(root: string) {
    await this.#call("initialize", {
      processId: null,
      rootUri: fileUri(root),
      workspaceFolders: [{ uri: fileUri(root), name: root.split(/[\\/]/).pop() }],
      capabilities: {
        textDocument: {
          synchronization: { didSave: false },
          completion: {
            completionItem: {
              snippetSupport: false,
              labelDetailsSupport: true,
              documentationFormat: ["plaintext"],
              resolveSupport: { properties: ["documentation", "detail", "additionalTextEdits"] },
            },
          },
        },
      },
    });
    this.notify("initialized", {});
  }

  receive(raw: string) {
    const message = JSON.parse(raw);
    // Richieste del server (configurazione, registrazione di capacità, progressi): basta una risposta vuota.
    if (message.id !== undefined && message.method) return void this.#write({ jsonrpc: "2.0", id: message.id, result: null });
    if (message.id === undefined) return;
    const call = this.#pending.get(message.id);
    this.#pending.delete(message.id);
    if (message.error) call?.reject(new Error(message.error.message));
    else call?.resolve(message.result);
  }

  close(reason: string) {
    this.#closed = new Error(reason);
    for (const call of this.#pending.values()) call.reject(this.#closed);
    this.#pending.clear();
  }

  get closed() {
    return this.#closed !== null;
  }

  async request<T>(method: string, params: unknown, timeoutMs?: number): Promise<T> {
    await this.ready;
    return this.#call<T>(method, params, timeoutMs);
  }

  notify(method: string, params: unknown) {
    void this.#write({ jsonrpc: "2.0", method, params }).catch(() => {});
  }

  /** Apre il documento o ne invia il testo nuovo; nulla se il server lo ha già così. */
  async sync(path: string, text: string) {
    await this.ready;
    const uri = fileUri(path);
    const document = this.#documents.get(uri);
    if (document?.text === text) return;
    if (!document) {
      this.#documents.set(uri, { version: 1, text });
      return this.notify("textDocument/didOpen", { textDocument: { uri, languageId: "java", version: 1, text } });
    }
    const version = document.version + 1;
    this.#documents.set(uri, { version, text });
    this.notify("textDocument/didChange", { textDocument: { uri, version }, contentChanges: [{ text }] });
  }

  forget(path: string) {
    const uri = fileUri(path);
    if (!this.#documents.delete(uri)) return;
    this.notify("textDocument/didClose", { textDocument: { uri } });
  }

  #call<T>(method: string, params: unknown, timeoutMs?: number): Promise<T> {
    if (this.#closed) return Promise.reject(this.#closed);
    const id = this.#nextId++;
    return new Promise<T>((resolve, reject) => {
      const timer = timeoutMs === undefined ? undefined : setTimeout(() => {
        this.#pending.delete(id);
        this.notify("$/cancelRequest", { id });
        reject(new Error(`${method}: nessuna risposta entro ${timeoutMs} ms`));
      }, timeoutMs);
      this.#pending.set(id, {
        resolve: (value) => {
          clearTimeout(timer);
          resolve(value as T);
        },
        reject: (error) => {
          clearTimeout(timer);
          reject(error);
        },
      });
      this.#write({ jsonrpc: "2.0", id, method, params }).catch((error) => {
        this.#pending.delete(id);
        reject(error instanceof Error ? error : new Error(String(error)));
      });
    });
  }

  #write(message: object) {
    return this.transport(JSON.stringify(message));
  }
}
