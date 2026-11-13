import { EditorState, type Extension } from "@codemirror/state";
import { indentUnit } from "@codemirror/language";
import type { EditorStyle } from "../settings.svelte";

export const indentExtension = (style: EditorStyle): Extension => [
  EditorState.tabSize.of(style.tabSize),
  indentUnit.of(style.useTabs ? "\t" : " ".repeat(style.indentSize)),
];

/** Carattere e interlinea passano dalle variabili CSS, così valgono anche per le viste diff. */
export function applyTextStyle(style: EditorStyle) {
  const root = document.documentElement.style;
  root.setProperty("--editor-font-size", `${style.fontSize}px`);
  root.setProperty("--editor-line-height", String(style.lineHeight));
}
