import { EditorView } from "@codemirror/view";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";

const chrome = EditorView.theme({
  "&": { height: "100%", color: "var(--fg)", backgroundColor: "var(--editor-bg)", fontSize: "var(--editor-font-size)" },
  ".cm-scroller": { fontFamily: "var(--mono)", lineHeight: "1.5", overflowY: "scroll" },
  ".cm-scroller::-webkit-scrollbar": { width: "12px", height: "12px" },
  ".cm-scroller::-webkit-scrollbar-thumb": {
    backgroundColor: "var(--scrollbar-thumb)",
    backgroundClip: "padding-box",
    border: "3px solid transparent",
    borderRadius: "6px",
  },
  ".cm-scroller::-webkit-scrollbar-corner": { backgroundColor: "transparent" },
  ".cm-content": { caretColor: "var(--accent)" },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--accent)" },
  "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "var(--selection)",
  },
  ".cm-gutters": { backgroundColor: "var(--editor-bg)", color: "var(--fg-muted)", border: "none" },
  ".cm-activeLine": { backgroundColor: "var(--active-line)" },
  ".cm-activeLineGutter": { backgroundColor: "var(--active-line)", color: "var(--fg)" },
  ".cm-selectionMatch": { backgroundColor: "var(--selection-match)" },
  ".cm-searchMatch": { backgroundColor: "var(--search-match)", outline: "1px solid var(--accent)" },
  ".cm-panels": { backgroundColor: "var(--panel-bg)", color: "var(--fg)" },
  ".cm-panels.cm-panels-top": { borderBottom: "1px solid var(--border)" },
  ".cm-panel input, .cm-panel button": { font: "inherit" },
});

const highlight = HighlightStyle.define([
  { tag: [t.keyword, t.modifier, t.operatorKeyword, t.controlKeyword], color: "var(--hl-keyword)" },
  { tag: [t.string, t.special(t.string), t.regexp], color: "var(--hl-string)" },
  { tag: [t.number, t.bool, t.null, t.atom], color: "var(--hl-number)" },
  { tag: [t.comment, t.lineComment, t.blockComment], color: "var(--hl-comment)", fontStyle: "italic" },
  { tag: [t.function(t.variableName), t.function(t.propertyName)], color: "var(--hl-function)" },
  { tag: [t.typeName, t.className, t.namespace], color: "var(--hl-type)" },
  { tag: [t.propertyName, t.attributeName], color: "var(--hl-property)" },
  { tag: [t.tagName, t.heading], color: "var(--hl-keyword)", fontWeight: "bold" },
  { tag: t.link, color: "var(--hl-string)", textDecoration: "underline" },
  { tag: t.emphasis, fontStyle: "italic" },
  { tag: t.strong, fontWeight: "bold" },
  { tag: t.invalid, color: "var(--danger)" },
]);

export const editorTheme = [chrome, syntaxHighlighting(highlight)];
