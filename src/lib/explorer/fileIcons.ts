import { fileName } from "../workspace/files";

export interface FileBadge {
  label: string;
  color: string;
  dark?: boolean;
}

const badge = (label: string, color: string, dark = false): FileBadge => ({ label, color, dark });

const javascript = badge("JS", "#e8c547", true);
const typescript = badge("TS", "#3178c6");
const shell = badge(">_", "#4e9a3f");
const image = badge("IM", "#9b59b6");
const config = badge("CF", "#6c707e");
const markup = badge("<>", "#e0632e");
const c = badge("C", "#5d6c8c");
const cpp = badge("C+", "#5d6c8c");

const byExtension: Record<string, FileBadge> = {
  js: javascript, mjs: javascript, cjs: javascript,
  ts: typescript, mts: typescript, cts: typescript,
  jsx: badge("JX", "#1ba2c7"), tsx: badge("TX", "#1ba2c7"),
  py: badge("Py", "#3572a5"), pyw: badge("Py", "#3572a5"),
  java: badge("J", "#e76f00"),
  kt: badge("K", "#7f52ff"), kts: badge("K", "#7f52ff"),
  rs: badge("RS", "#b7410e"),
  go: badge("GO", "#00add8"),
  c, h: c, cpp, cc: cpp, hpp: cpp,
  cs: badge("C#", "#68217a"),
  rb: badge("RB", "#cc342d"),
  php: badge("P", "#777bb3"),
  swift: badge("SW", "#f05138"),
  svelte: badge("S", "#ff3e00"),
  vue: badge("V", "#41b883"),
  html: markup, htm: markup, xml: markup, svg: badge("<>", "#d4a017", true),
  css: badge("#", "#2965f1"), scss: badge("#", "#cd6799"), less: badge("#", "#1d365d"),
  json: badge("{}", "#c99a2e"), jsonc: badge("{}", "#c99a2e"),
  yml: badge("Y", "#cb4b3e"), yaml: badge("Y", "#cb4b3e"),
  toml: badge("T", "#9c4221"), ini: config, env: config, conf: config,
  md: badge("M", "#4a7fc1"), markdown: badge("M", "#4a7fc1"),
  sql: badge("DB", "#336791"),
  sh: shell, bash: shell, zsh: shell,
  ps1: badge(">_", "#2671be"), bat: badge(">_", "#6c707e"), cmd: badge(">_", "#6c707e"),
  png: image, jpg: image, jpeg: image, gif: image, webp: image, ico: image, bmp: image,
  lock: badge("LK", "#6c707e"),
};

const byName: Record<string, FileBadge> = {
  ".gitignore": badge("G", "#f05033"),
  ".gitattributes": badge("G", "#f05033"),
  ".gitmodules": badge("G", "#f05033"),
  dockerfile: badge("D", "#1d63ed"),
  ".env": config,
};

/** Badge per nome file o estensione; `null` → icona file generica. */
export function fileBadge(path: string): FileBadge | null {
  const name = fileName(path).toLowerCase();
  if (byName[name]) return byName[name];
  const dot = name.lastIndexOf(".");
  if (dot <= 0) return null;
  return byExtension[name.slice(dot + 1)] ?? null;
}
