export type Eol = "\n" | "\r\n";

export function detectEol(content: string, fallback: Eol): Eol {
  const lf = content.indexOf("\n");
  if (lf === -1) return fallback;
  return content[lf - 1] === "\r" ? "\r\n" : "\n";
}

export const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

const normalize = (path: string) => path.replace(/\//g, "\\").toLowerCase();

export const parentDir = (path: string) => path.replace(/[\\/][^\\/]*$/, "");

export const samePath = (a: string, b: string) => normalize(a) === normalize(b);

export function isInsideDir(path: string, dir: string) {
  const target = normalize(path);
  const base = normalize(dir);
  return target === base || target.startsWith(base.endsWith("\\") ? base : `${base}\\`);
}

export const eolLabel = (eol: Eol) => (eol === "\r\n" ? "CRLF" : "LF");
