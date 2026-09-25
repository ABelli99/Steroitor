import { fileName } from "../workspace/files";
import type { GlyphName } from "./fileGlyphs";

export interface FileIcon {
  glyph: GlyphName;
  color: string;
}

const icon = (glyph: GlyphName, color: string): FileIcon => ({ glyph, color });

const muted = "var(--fg-muted)";
const javascript = icon("code", "#d4a72c");
const typescript = icon("code", "#3178c6");
const react = icon("atom", "#1ba2c7");
const python = icon("python", "");
const shell = icon("terminal", "#4e9a3f");
const script = icon("terminal", muted);
const image = icon("image", "#9b59b6");
const config = icon("sliders", muted);
const git = icon("git", "#f05033");
const c = icon("hexagon", "#6b7fa8");
const cpp = icon("hexagon", "#00599c");
const html = icon("shield", "#e0632e");
const json = icon("braces", "#c99a2e");
const yaml = icon("list", "#cb4b3e");
const markdown = icon("markdown", "#4a7fc1");
const text = icon("text", muted);

const byExtension: Record<string, FileIcon> = {
  js: javascript, mjs: javascript, cjs: javascript,
  ts: typescript, mts: typescript, cts: typescript,
  jsx: react, tsx: react,
  py: python, pyw: python,
  java: icon("cup", "#e76f00"),
  kt: icon("kotlin", "#7f52ff"), kts: icon("kotlin", "#7f52ff"),
  rs: icon("gear", "#b7410e"),
  go: icon("code", "#00add8"),
  c, h: c, cpp, cc: cpp, hpp: cpp,
  cs: icon("hexagon", "#9b4f96"),
  rb: icon("gem", "#cc342d"),
  php: icon("code", "#777bb3"),
  swift: icon("code", "#f05138"),
  svelte: icon("svelte", "#ff3e00"),
  vue: icon("vue", "#41b883"),
  html, htm: html,
  xml: icon("brackets", "#e0632e"),
  svg: icon("bezier", "#d4a017"),
  css: icon("hash", "#2965f1"), scss: icon("hash", "#cd6799"), less: icon("hash", "#3d6fb0"),
  json, jsonc: json,
  yml: yaml, yaml,
  toml: icon("list", "#9c4221"),
  ini: config, env: config, conf: config,
  md: markdown, markdown,
  sql: icon("database", "#336791"),
  sh: shell, bash: shell, zsh: shell,
  ps1: icon("terminal", "#2671be"), bat: script, cmd: script,
  png: image, jpg: image, jpeg: image, gif: image, webp: image, ico: image, bmp: image,
  lock: icon("lock", muted),
  txt: text, log: text,
};

const byName: Record<string, FileIcon> = {
  ".gitignore": git,
  ".gitattributes": git,
  ".gitmodules": git,
  ".env": config,
  dockerfile: icon("docker", "#1d63ed"),
};

const generic = icon("file", muted);

/** Icona per nome file speciale o estensione, altrimenti quella generica. */
export function fileIcon(path: string): FileIcon {
  const name = fileName(path).toLowerCase();
  if (byName[name]) return byName[name];
  const dot = name.lastIndexOf(".");
  if (dot <= 0) return generic;
  return byExtension[name.slice(dot + 1)] ?? generic;
}
