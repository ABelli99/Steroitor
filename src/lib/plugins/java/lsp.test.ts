import { describe, expect, it } from "vitest";
import { fileUri, LspConnection } from "./lsp";

type Sent = { id?: number; method?: string; params?: { textDocument?: { version?: number } } };

/** Server finto: registra i messaggi e risponde subito a initialize. */
function connect() {
  const sent: Sent[] = [];
  const connection: LspConnection = new LspConnection(async (raw) => {
    const message = JSON.parse(raw) as Sent;
    sent.push(message);
    if (message.method === "initialize") queueMicrotask(() => connection.receive(JSON.stringify({ jsonrpc: "2.0", id: message.id, result: {} })));
  }, "C:\\work\\app");
  return { connection, sent, methods: () => sent.map((message) => message.method) };
}

describe("LspConnection", () => {
  it("initializes before the first request and matches responses by id", async () => {
    const { connection, sent, methods } = connect();
    const result = connection.request("textDocument/completion", {});
    await connection.ready;
    await Promise.resolve();
    expect(methods().slice(0, 3)).toEqual(["initialize", "initialized", "textDocument/completion"]);
    connection.receive(JSON.stringify({ jsonrpc: "2.0", id: sent[2].id, result: { items: [] } }));
    await expect(result).resolves.toEqual({ items: [] });
  });

  it("answers server requests with an empty result", async () => {
    const { connection, sent } = connect();
    await connection.ready;
    connection.receive(JSON.stringify({ jsonrpc: "2.0", id: 99, method: "workspace/configuration", params: {} }));
    expect(sent.at(-1)).toMatchObject({ id: 99 });
  });

  it("opens a document once and sends changes only when the text differs", async () => {
    const { connection, sent, methods } = connect();
    await connection.sync("C:\\work\\app\\Main.java", "class A {}");
    await connection.sync("C:\\work\\app\\Main.java", "class A {}");
    await connection.sync("C:\\work\\app\\Main.java", "class B {}");
    expect(methods().filter((method) => method?.startsWith("textDocument/"))).toEqual(["textDocument/didOpen", "textDocument/didChange"]);
    expect(sent.at(-1)?.params?.textDocument?.version).toBe(2);
  });

  it("rejects pending requests on timeout and when the server exits", async () => {
    const { connection } = connect();
    await expect(connection.request("slow", {}, 10)).rejects.toThrow("nessuna risposta");
    const pending = connection.request("never", {});
    await Promise.resolve();
    connection.close("jdtls si è chiuso");
    await expect(pending).rejects.toThrow("jdtls si è chiuso");
    await expect(connection.request("after", {})).rejects.toThrow("jdtls si è chiuso");
  });

  it("builds file URIs that keep the drive letter", () => {
    expect(fileUri("C:\\My Projects\\App.java")).toBe("file:///C:/My%20Projects/App.java");
  });
});
