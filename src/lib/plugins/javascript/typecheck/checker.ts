import ts from "typescript";
import type { Probe } from "../../../backend";

export const LIB_DIR = "/ts-lib";

export interface TypeDiagnostic {
  from: number;
  to: number;
  severity: "error" | "warning" | "info";
  message: string;
}

/** O i file da leggere da disco prima di poter rispondere, o le diagnostiche del file. */
export type DiagnoseResult = { missing: string[] } | { diagnostics: TypeDiagnostic[] };

type Entry = Probe & { version: number };

const key = (path: string) => path.replaceAll("\\", "/").replace(/\/+$/, "").toLowerCase();
const parentOf = (pathKey: string) => pathKey.slice(0, pathKey.lastIndexOf("/"));
const dirname = (path: string) => path.replaceAll("\\", "/").replace(/\/[^/]*$/, "");
const isDiskPath = (path: string) => /^[a-z]:[\\/]/i.test(path) || /^[\\/]{2}/.test(path);

const DEFAULT_OPTIONS: ts.CompilerOptions = {
  target: ts.ScriptTarget.ESNext,
  module: ts.ModuleKind.ESNext,
  moduleResolution: ts.ModuleResolutionKind.Bundler,
  jsx: ts.JsxEmit.Preserve,
};

const SEVERITY: Record<ts.DiagnosticCategory, TypeDiagnostic["severity"]> = {
  [ts.DiagnosticCategory.Error]: "error",
  [ts.DiagnosticCategory.Warning]: "warning",
  [ts.DiagnosticCategory.Suggestion]: "info",
  [ts.DiagnosticCategory.Message]: "info",
};

function toDiagnostic(diagnostic: ts.Diagnostic): TypeDiagnostic {
  const from = diagnostic.start ?? 0;
  return {
    from,
    to: from + (diagnostic.length ?? 0),
    severity: SEVERITY[diagnostic.category],
    message: ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n"),
  };
}

/**
 * Language service di TypeScript su un file system virtuale riempito a richiesta: l'host è sincrono,
 * quindi ogni file che manca si annota, il chiamante lo legge da disco e si riprova.
 */
export class Checker {
  #entries = new Map<string, Entry>([[key(LIB_DIR), { kind: "dir", dirs: [], version: 0 }]]);
  #missing = new Map<string, string>();
  #services = new Map<string, ts.LanguageService>();
  #registry = ts.createDocumentRegistry(false);
  #current: { key: string; content: string; version: number } | null = null;
  #roots: string[] = [];
  #version = 0;

  constructor(private readonly loadLib: (fileName: string) => Promise<string | undefined>) {}

  provide(entries: [string, Probe][]) {
    for (const [path, probe] of entries) this.#entries.set(key(path), { ...probe, version: ++this.#version });
    // I servizi ricordano i moduli non risolti: con file nuovi si ricreano, i sorgenti già letti restano nel registry.
    this.#services.clear();
  }

  invalidate(dirs: string[]) {
    const changed = new Set(dirs.map(key));
    for (const pathKey of [...this.#entries.keys()]) {
      if (changed.has(pathKey) || changed.has(parentOf(pathKey))) this.#entries.delete(pathKey);
    }
    this.#services.clear();
  }

  async diagnose(path: string, content: string): Promise<DiagnoseResult> {
    const pathKey = key(path);
    if (this.#current?.key !== pathKey || this.#current.content !== content) this.#current = { key: pathKey, content, version: ++this.#version };

    for (;;) {
      this.#missing.clear();
      const service = this.#serviceFor(path);
      service.getProgram();
      const missing = [...this.#missing.values()];
      const libs = missing.filter((candidate) => key(candidate).startsWith(`${LIB_DIR}/`));
      if (libs.length) {
        await this.#loadLibs(libs);
        continue;
      }
      const disk = missing.filter(isDiskPath);
      if (disk.length) return { missing: disk };
      for (const other of missing) this.#entries.set(key(other), { kind: "missing", version: 0 });
      return { diagnostics: [...service.getSyntacticDiagnostics(path), ...service.getSemanticDiagnostics(path)].map(toDiagnostic) };
    }
  }

  async #loadLibs(paths: string[]) {
    const contents = await Promise.all(paths.map((path) => this.loadLib(key(path).slice(LIB_DIR.length + 1))));
    this.provide(paths.map((path, index): [string, Probe] => [path, contents[index] === undefined ? { kind: "missing" } : { kind: "file", content: contents[index]! }]));
  }

  #serviceFor(path: string) {
    const configPath = ts.findConfigFile(dirname(path), (candidate) => this.#fileExists(candidate));
    this.#roots = [path];
    const id = configPath ? key(configPath) : "";
    let service = this.#services.get(id);
    if (!service) {
      service = this.#createService(this.#optionsFrom(configPath), configPath ? dirname(configPath) : dirname(path));
      this.#services.set(id, service);
    }
    return service;
  }

  #optionsFrom(configPath: string | undefined): ts.CompilerOptions {
    const read = configPath ? ts.readConfigFile(configPath, (candidate) => this.#readFile(candidate)) : null;
    if (!configPath || !read || read.error) return { ...DEFAULT_OPTIONS, allowJs: true, noEmit: true };
    const parseHost: ts.ParseConfigHost = {
      useCaseSensitiveFileNames: false,
      // Si controlla solo il file aperto: l'elenco dei file del progetto non serve.
      readDirectory: () => [],
      fileExists: (candidate) => this.#fileExists(candidate),
      readFile: (candidate) => this.#readFile(candidate),
    };
    const parsed = ts.parseJsonConfigFileContent(read.config, parseHost, dirname(configPath), undefined, configPath);
    return { ...parsed.options, allowJs: true, noEmit: true };
  }

  #createService(options: ts.CompilerOptions, cwd: string) {
    const host: ts.LanguageServiceHost = {
      getCompilationSettings: () => options,
      getScriptFileNames: () => this.#roots,
      getScriptVersion: (path) => {
        const pathKey = key(path);
        return pathKey === this.#current?.key ? `open-${this.#current.version}` : String(this.#entries.get(pathKey)?.version ?? 0);
      },
      getScriptSnapshot: (path) => {
        const text = this.#readFile(path);
        return text === undefined ? undefined : ts.ScriptSnapshot.fromString(text);
      },
      getCurrentDirectory: () => cwd,
      getDefaultLibFileName: (compilerOptions) => `${LIB_DIR}/${ts.getDefaultLibFileName(compilerOptions)}`,
      fileExists: (path) => this.#fileExists(path),
      readFile: (path) => this.#readFile(path),
      directoryExists: (path) => this.#lookup(path)?.kind === "dir",
      getDirectories: (path) => {
        const entry = this.#lookup(path);
        return entry?.kind === "dir" ? entry.dirs : [];
      },
      useCaseSensitiveFileNames: () => false,
    };
    return ts.createLanguageService(host, this.#registry);
  }

  #lookup(path: string) {
    const pathKey = key(path);
    const entry = this.#entries.get(pathKey);
    if (!entry) this.#missing.set(pathKey, path);
    return entry;
  }

  #fileExists(path: string) {
    return key(path) === this.#current?.key || this.#lookup(path)?.kind === "file";
  }

  #readFile(path: string) {
    if (key(path) === this.#current?.key) return this.#current.content;
    const entry = this.#lookup(path);
    return entry?.kind === "file" ? entry.content : undefined;
  }
}
