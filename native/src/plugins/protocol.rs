//! I due protocolli parlati dai server: LSP (jdtls) e quello di tsserver. Qui si costruiscono le richieste e si
//! traducono le risposte negli stessi tipi; posizioni sempre come (riga, colonna UTF-16) da 0.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::process::Process;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Protocol {
    Lsp,
    TsServer,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub insert: String,
    pub sort: String,
    /// Firma e documentazione, se il server le manda già con l'elenco.
    pub doc: Option<String>,
    raw: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub start: (usize, usize),
    pub end: (usize, usize),
    pub message: String,
    pub error: bool,
}

pub enum Reply {
    /// `incomplete`: il server ha tagliato la lista, va richiesta a ogni carattere.
    Completions { ticket: u64, items: Vec<Item>, incomplete: bool },
    Doc(u64, String),
    /// Dove è dichiarato il simbolo; None se il server non lo sa.
    Definition(u64, Option<(PathBuf, (usize, usize))>),
    /// Diagnostica di un file; `slot` separa sintassi e semantica di tsserver, che arrivano in eventi distinti.
    Diagnostics { path: PathBuf, slot: usize, list: Vec<Diagnostic> },
}

#[derive(Clone)]
enum Purpose {
    Initialize,
    Completion(u64),
    Doc(u64),
    Definition(u64),
}

/// Dove era stato chiesto il completamento: tsserver lo vuole anche per i dettagli della voce.
#[derive(Clone)]
pub struct Origin {
    pub path: PathBuf,
    pub position: (usize, usize),
}

pub struct Server {
    pub protocol: Protocol,
    process: Process,
    next: u64,
    pending: HashMap<u64, Purpose>,
    /// File aperti nel server: versione, impronta del testo inviato e sua ultima posizione.
    documents: HashMap<PathBuf, (i64, u64, (usize, usize))>,
    ready: bool,
    backlog: Vec<Vec<u8>>,
    language_id: &'static str,
}

fn hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3))
}

/// Il plugin C/C++ dichiara `cpp`; i file `.c` vanno al server come C.
fn language_of<'a>(language_id: &'a str, path: &Path) -> &'a str {
    let c = path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("c"));
    if language_id == "cpp" && c { "c" } else { language_id }
}

pub fn file_uri(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let encoded: Vec<String> = text
        .split('/')
        .map(|segment| {
            if segment.len() == 2 && segment.ends_with(':') {
                return segment.to_string();
            }
            segment.bytes().map(|byte| if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) { (byte as char).to_string() } else { format!("%{byte:02X}") }).collect()
        })
        .collect();
    format!("file:///{}", encoded.join("/"))
}

pub fn uri_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file:///")?;
    let bytes = rest.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if let Some(byte) = rest.get(index + 1..index + 3).and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                decoded.push(byte);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    Some(PathBuf::from(String::from_utf8_lossy(&decoded).replace('/', "\\")))
}

/// Testo di un campo che può essere stringa, MarkupContent, MarkedString o lista di parti (`{text}` di tsserver).
fn text_of(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts.iter().map(text_of).collect::<Vec<_>>().join(if parts.iter().all(|part| part.get("text").is_some()) { "" } else { "\n" }),
        Value::Object(object) => object.get("value").or_else(|| object.get("text")).map(text_of).unwrap_or_default(),
        _ => String::new(),
    }
}

fn join_doc(signature: String, documentation: String) -> Option<String> {
    let text = [signature.trim(), documentation.trim()].iter().filter(|part| !part.is_empty()).copied().collect::<Vec<_>>().join("\n\n");
    (!text.is_empty()).then_some(text)
}

impl Server {
    pub fn new(protocol: Protocol, process: Process, root: &Path, language_id: &'static str) -> Server {
        let mut server = Server { protocol, process, next: 1, pending: HashMap::new(), documents: HashMap::new(), ready: protocol == Protocol::TsServer, backlog: Vec::new(), language_id };
        if protocol == Protocol::Lsp {
            let uri = file_uri(root);
            let name = root.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
            let params = json!({
                "processId": std::process::id(),
                "rootUri": uri,
                "workspaceFolders": [{ "uri": uri, "name": name }],
                "capabilities": { "textDocument": {
                    "synchronization": { "didSave": false },
                    "publishDiagnostics": {},
                    "hover": { "contentFormat": ["plaintext"] },
                    "completion": { "completionItem": {
                        "snippetSupport": false,
                        "documentationFormat": ["plaintext"],
                        "resolveSupport": { "properties": ["documentation", "detail"] }
                    } }
                } }
            });
            server.send(Some(Purpose::Initialize), "initialize", params, true);
        }
        server
    }

    pub fn kill(&mut self) {
        self.process.kill();
    }

    fn encode(&self, id: Option<u64>, method: &str, params: Value) -> Vec<u8> {
        match self.protocol {
            Protocol::Lsp => {
                let mut message = json!({ "jsonrpc": "2.0", "method": method, "params": params });
                if let Some(id) = id {
                    message["id"] = id.into();
                }
                let body = message.to_string();
                format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
            }
            Protocol::TsServer => format!("{}\r\n", json!({ "seq": id.unwrap_or(0), "type": "request", "command": method, "arguments": params })).into_bytes(),
        }
    }

    /// Con `purpose` è una richiesta con risposta; prima della risposta a `initialize` resta in attesa.
    fn send(&mut self, purpose: Option<Purpose>, method: &str, params: Value, now: bool) {
        let id = self.next;
        self.next += 1;
        // In LSP un messaggio con id è una richiesta; tsserver numera anche i comandi senza risposta.
        let numbered = purpose.is_some() || self.protocol == Protocol::TsServer;
        if let Some(purpose) = purpose {
            self.pending.insert(id, purpose);
        }
        let bytes = self.encode(numbered.then_some(id), method, params);
        if self.ready || now {
            self.process.write(bytes);
        } else {
            self.backlog.push(bytes);
        }
    }

    /// Apre il file nel server o ne manda il testo nuovo; nulla se il server lo ha già così.
    pub fn sync(&mut self, path: &Path, text: &str) {
        let fingerprint = hash(text);
        let previous = self.documents.get(path).copied();
        if previous.is_some_and(|(_, sent, _)| sent == fingerprint) {
            return;
        }
        let version = previous.map_or(1, |(version, _, _)| version + 1);
        self.documents.insert(path.to_path_buf(), (version, fingerprint, super::position_of(text, text.len())));
        let file = path.to_string_lossy().into_owned();
        match (self.protocol, previous) {
            (Protocol::Lsp, None) => {
                let language = language_of(self.language_id, path);
                self.send(None, "textDocument/didOpen", json!({ "textDocument": { "uri": file_uri(path), "languageId": language, "version": version, "text": text } }), false)
            }
            (Protocol::Lsp, Some(_)) => self.send(None, "textDocument/didChange", json!({ "textDocument": { "uri": file_uri(path), "version": version }, "contentChanges": [{ "text": text }] }), false),
            (Protocol::TsServer, None) => self.send(None, "updateOpen", json!({ "openFiles": [{ "file": file, "fileContent": text }] }), false),
            // Una modifica sola che sostituisce tutto il testo inviato prima, fino alla sua ultima posizione.
            (Protocol::TsServer, Some((_, _, (line, column)))) => {
                let change = json!({ "start": { "line": 1, "offset": 1 }, "end": { "line": line + 1, "offset": column + 1 }, "newText": text });
                self.send(None, "updateOpen", json!({ "changedFiles": [{ "fileName": file, "textChanges": [change] }] }), false)
            }
        }
    }

    pub fn close(&mut self, path: &Path) {
        if self.documents.remove(path).is_none() {
            return;
        }
        match self.protocol {
            Protocol::Lsp => self.send(None, "textDocument/didClose", json!({ "textDocument": { "uri": file_uri(path) } }), false),
            Protocol::TsServer => self.send(None, "close", json!({ "file": path.to_string_lossy() }), false),
        }
    }

    fn location(&self, origin: &Origin) -> Value {
        let (line, column) = origin.position;
        match self.protocol {
            Protocol::Lsp => json!({ "textDocument": { "uri": file_uri(&origin.path) }, "position": { "line": line, "character": column } }),
            Protocol::TsServer => json!({ "file": origin.path.to_string_lossy(), "line": line + 1, "offset": column + 1 }),
        }
    }

    pub fn completion(&mut self, ticket: u64, origin: &Origin, trigger: Option<char>) {
        let mut params = self.location(origin);
        match self.protocol {
            Protocol::Lsp => self.send(Some(Purpose::Completion(ticket)), "textDocument/completion", params, false),
            Protocol::TsServer => {
                if let Some(trigger) = trigger {
                    params["triggerCharacter"] = trigger.to_string().into();
                }
                params["includeExternalModuleExports"] = false.into();
                self.send(Some(Purpose::Completion(ticket)), "completionInfo", params, false)
            }
        }
    }

    pub fn item_doc(&mut self, ticket: u64, origin: &Origin, item: &Item) {
        match self.protocol {
            Protocol::Lsp => self.send(Some(Purpose::Doc(ticket)), "completionItem/resolve", item.raw.clone(), false),
            Protocol::TsServer => {
                let mut params = self.location(origin);
                let mut entry = json!({ "name": item.raw["name"] });
                for key in ["source", "data"] {
                    if let Some(value) = item.raw.get(key) {
                        entry[key] = value.clone();
                    }
                }
                params["entryNames"] = json!([entry]);
                self.send(Some(Purpose::Doc(ticket)), "completionEntryDetails", params, false)
            }
        }
    }

    pub fn definition(&mut self, ticket: u64, origin: &Origin) {
        let params = self.location(origin);
        let method = if self.protocol == Protocol::Lsp { "textDocument/definition" } else { "definition" };
        self.send(Some(Purpose::Definition(ticket)), method, params, false);
    }

    pub fn hover(&mut self, ticket: u64, origin: &Origin) {
        let params = self.location(origin);
        let method = if self.protocol == Protocol::Lsp { "textDocument/hover" } else { "quickinfo" };
        self.send(Some(Purpose::Doc(ticket)), method, params, false);
    }

    /// tsserver controlla solo su richiesta; jdtls pubblica da solo la diagnostica dopo ogni modifica.
    pub fn check(&mut self, path: &Path) {
        if self.protocol == Protocol::TsServer {
            self.send(None, "geterr", json!({ "files": [path.to_string_lossy()], "delay": 0 }), false);
        }
    }

    pub fn receive(&mut self, message: &Value) -> Option<Reply> {
        match self.protocol {
            Protocol::Lsp => self.receive_lsp(message),
            Protocol::TsServer => self.receive_ts(message),
        }
    }

    fn receive_lsp(&mut self, message: &Value) -> Option<Reply> {
        let id = message.get("id").and_then(Value::as_u64);
        if let Some(method) = message.get("method").and_then(Value::as_str) {
            // Richieste del server (configurazione, registrazioni, progressi): basta una risposta vuota.
            if let Some(id) = message.get("id") {
                let body = json!({ "jsonrpc": "2.0", "id": id, "result": Value::Null }).to_string();
                self.process.write(format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes());
                return None;
            }
            if method != "textDocument/publishDiagnostics" {
                return None;
            }
            let params = &message["params"];
            let path = uri_path(params["uri"].as_str()?)?;
            let list = params["diagnostics"].as_array()?.iter().filter_map(lsp_diagnostic).collect();
            return Some(Reply::Diagnostics { path, slot: 0, list });
        }
        let purpose = self.pending.remove(&id?)?;
        let result = message.get("result").unwrap_or(&Value::Null);
        match purpose {
            Purpose::Initialize => {
                self.ready = true;
                self.process.write(self.encode(None, "initialized", json!({})));
                for bytes in std::mem::take(&mut self.backlog) {
                    self.process.write(bytes);
                }
                None
            }
            Purpose::Completion(ticket) => {
                let items = result.get("items").unwrap_or(result).as_array().map(|items| items.iter().filter_map(lsp_item).collect()).unwrap_or_default();
                Some(Reply::Completions { ticket, items, incomplete: result["isIncomplete"] == true })
            }
            Purpose::Doc(ticket) => {
                let text = match result.get("contents") {
                    Some(contents) => text_of(contents),
                    None => join_doc(result.get("detail").map(text_of).unwrap_or_default(), result.get("documentation").map(text_of).unwrap_or_default()).unwrap_or_default(),
                };
                Some(Reply::Doc(ticket, text))
            }
            Purpose::Definition(ticket) => {
                // Location, Location[] o LocationLink[]: vale la prima.
                let first = result.get(0).unwrap_or(result);
                let uri = first.get("targetUri").or_else(|| first.get("uri")).and_then(Value::as_str);
                let start = &first.get("targetSelectionRange").or_else(|| first.get("range")).unwrap_or(&Value::Null)["start"];
                let point = start["line"].as_u64().zip(start["character"].as_u64()).map(|(line, column)| (line as usize, column as usize));
                Some(Reply::Definition(ticket, uri.and_then(uri_path).zip(point)))
            }
        }
    }

    fn receive_ts(&mut self, message: &Value) -> Option<Reply> {
        if message["type"] == "event" {
            let slot = match message["event"].as_str()? {
                "syntaxDiag" => 0,
                "semanticDiag" => 1,
                _ => return None,
            };
            let body = &message["body"];
            let path = PathBuf::from(body["file"].as_str()?.replace('/', "\\"));
            let list = body["diagnostics"].as_array()?.iter().filter_map(ts_diagnostic).collect();
            return Some(Reply::Diagnostics { path, slot, list });
        }
        let purpose = self.pending.remove(&message.get("request_seq").and_then(Value::as_u64)?)?;
        let body = &message["body"];
        match purpose {
            Purpose::Initialize => None,
            Purpose::Completion(ticket) => {
                let items = body["entries"].as_array().map(|entries| entries.iter().filter_map(ts_item).collect()).unwrap_or_default();
                Some(Reply::Completions { ticket, items, incomplete: body["isIncomplete"] == true })
            }
            Purpose::Doc(ticket) => {
                let text = if let Some(details) = body.get(0) {
                    let tags: Vec<String> = details["tags"].as_array().into_iter().flatten().map(|tag| format!("@{} {}", text_of(&tag["name"]), text_of(&tag["text"])).trim_end().to_string()).collect();
                    let documentation = [text_of(&details["documentation"]), tags.join("\n")].iter().filter(|part| !part.is_empty()).cloned().collect::<Vec<_>>().join("\n");
                    join_doc(text_of(&details["displayParts"]), documentation)
                } else {
                    join_doc(text_of(&body["displayString"]), text_of(&body["documentation"]))
                };
                Some(Reply::Doc(ticket, text.unwrap_or_default()))
            }
            Purpose::Definition(ticket) => {
                let first = &body[0];
                let file = first["file"].as_str().map(|file| PathBuf::from(file.replace('/', "\\")));
                let start = &first["start"];
                let point = start["line"].as_u64().zip(start["offset"].as_u64()).map(|(line, offset)| ((line as usize).saturating_sub(1), (offset as usize).saturating_sub(1)));
                Some(Reply::Definition(ticket, file.zip(point)))
            }
        }
    }
}

fn lsp_item(item: &Value) -> Option<Item> {
    let label = item["label"].as_str()?;
    let insert = item["textEdit"]["newText"].as_str().or_else(|| item["insertText"].as_str()).unwrap_or(label);
    let mut detail = item.get("detail").map(text_of).unwrap_or_default();
    // clangd mette nel dettaglio solo il tipo (`size_type`) e la firma nell'etichetta (` size() const`).
    let signature = label.trim().trim_start_matches('•');
    if signature.contains('(') && !detail.contains('(') {
        detail = format!("{detail} {signature}").trim().to_string();
    }
    let doc = join_doc(detail, item.get("documentation").map(text_of).unwrap_or_default());
    Some(Item { insert: insert.to_string(), sort: item["sortText"].as_str().unwrap_or(label).to_string(), doc, raw: item.clone() })
}

fn ts_item(entry: &Value) -> Option<Item> {
    let name = entry["name"].as_str()?;
    let insert = entry["insertText"].as_str().unwrap_or(name);
    Some(Item { insert: insert.to_string(), sort: entry["sortText"].as_str().unwrap_or("").to_string(), doc: None, raw: entry.clone() })
}

fn lsp_diagnostic(diagnostic: &Value) -> Option<Diagnostic> {
    let point = |value: &Value| Some((value["line"].as_u64()? as usize, value["character"].as_u64()? as usize));
    let range = &diagnostic["range"];
    // Severità 1 = errore; 2 avviso; 3 e 4 (informazioni, suggerimenti) non si mostrano.
    let severity = diagnostic["severity"].as_u64().unwrap_or(1);
    if severity > 2 {
        return None;
    }
    Some(Diagnostic { start: point(&range["start"])?, end: point(&range["end"])?, message: text_of(&diagnostic["message"]), error: severity == 1 })
}

fn ts_diagnostic(diagnostic: &Value) -> Option<Diagnostic> {
    let point = |value: &Value| Some(((value["line"].as_u64()? as usize).saturating_sub(1), (value["offset"].as_u64()? as usize).saturating_sub(1)));
    let category = diagnostic["category"].as_str().unwrap_or("error");
    if category != "error" && category != "warning" {
        return None;
    }
    Some(Diagnostic { start: point(&diagnostic["start"])?, end: point(&diagnostic["end"])?, message: text_of(&diagnostic["text"]), error: category == "error" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_clangd_type_and_signature() {
        let item = lsp_item(&json!({"label": " size() const", "textEdit": {"newText": "size()"}, "detail": "size_type"})).unwrap();
        assert_eq!(item.doc.as_deref(), Some("size_type size() const"));
        let java = lsp_item(&json!({"label": "println(Object x) : void", "detail": "void println(Object x)"})).unwrap();
        assert_eq!(java.doc.as_deref(), Some("void println(Object x)"));
    }

    #[test]
    fn sends_c_files_as_c() {
        assert_eq!(language_of("cpp", Path::new("a/main.C")), "c");
        assert_eq!(language_of("cpp", Path::new("a/main.h")), "cpp");
        assert_eq!(language_of("java", Path::new("Main.java")), "java");
    }

    #[test]
    fn converts_paths_and_uris() {
        let path = Path::new(r"D:\progetti\con spazio\Main.java");
        let uri = file_uri(path);
        assert_eq!(uri, "file:///D:/progetti/con%20spazio/Main.java");
        assert_eq!(uri_path(&uri).unwrap(), path);
        assert_eq!(uri_path("file:///d%3A/x/A.java").unwrap(), PathBuf::from(r"d:\x\A.java"));
    }

    #[test]
    fn reads_lsp_items_and_diagnostics() {
        let item = lsp_item(&json!({"label": "println(Object x) : void", "textEdit": {"newText": "println"}, "sortText": "0001", "detail": "void println(Object x)"})).unwrap();
        assert_eq!(item.insert, "println");
        assert_eq!(item.doc.as_deref(), Some("void println(Object x)"));
        let diagnostic = lsp_diagnostic(&json!({"range": {"start": {"line": 2, "character": 4}, "end": {"line": 2, "character": 9}}, "severity": 1, "message": "x cannot be resolved"})).unwrap();
        assert_eq!((diagnostic.start, diagnostic.end, diagnostic.error), ((2, 4), (2, 9), true));
        assert!(lsp_diagnostic(&json!({"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}}, "severity": 4, "message": "hint"})).is_none());
    }

    #[test]
    fn reads_tsserver_diagnostics_and_docs() {
        let diagnostic = ts_diagnostic(&json!({"start": {"line": 1, "offset": 7}, "end": {"line": 1, "offset": 8}, "text": "Type 'string' is not assignable", "category": "error"})).unwrap();
        assert_eq!((diagnostic.start, diagnostic.end), ((0, 6), (0, 7)));
        let parts = json!([{"text": "(method) "}, {"text": "trim"}, {"text": "(): string"}]);
        assert_eq!(text_of(&parts), "(method) trim(): string");
        assert_eq!(join_doc("sig".into(), " ".into()).as_deref(), Some("sig"));
    }
}
