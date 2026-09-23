export type Eol = "\n" | "\r\n";

export function detectEol(content: string, fallback: Eol): Eol {
  const lf = content.indexOf("\n");
  if (lf === -1) return fallback;
  return content[lf - 1] === "\r" ? "\r\n" : "\n";
}

export const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

export const samePath = (a: string, b: string) => a.replace(/\//g, "\\").toLowerCase() === b.replace(/\//g, "\\").toLowerCase();

export const eolLabel = (eol: Eol) => (eol === "\r\n" ? "CRLF" : "LF");
