import { Compartment, EditorState, type Extension } from "@codemirror/state";
import {
  crosshairCursor, drawSelection, dropCursor, EditorView, highlightActiveLine, highlightActiveLineGutter,
  highlightSpecialChars, keymap, lineNumbers, rectangularSelection, type ViewUpdate,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { bracketMatching, foldGutter, foldKeymap, indentOnInput } from "@codemirror/language";
import { highlightSelectionMatches, openSearchPanel, searchKeymap } from "@codemirror/search";
import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import { editorTheme } from "./theme";

export const languageSlot = new Compartment();
export const wrapSlot = new Compartment();
export const gitSlot = new Compartment();

export const wrapExtension = (enabled: boolean): Extension => (enabled ? EditorView.lineWrapping : []);

export function createState(content: string, wrap: boolean, onUpdate: (update: ViewUpdate) => void): EditorState {
  return EditorState.create({
    doc: content,
    extensions: [
      lineNumbers(),
      gitSlot.of([]),
      highlightActiveLineGutter(),
      highlightSpecialChars(),
      history(),
      foldGutter(),
      drawSelection(),
      dropCursor(),
      EditorState.allowMultipleSelections.of(true),
      indentOnInput(),
      bracketMatching(),
      closeBrackets(),
      rectangularSelection(),
      crosshairCursor(),
      highlightActiveLine(),
      highlightSelectionMatches(),
      keymap.of([
        { key: "Mod-h", run: openSearchPanel },
        ...closeBracketsKeymap,
        ...defaultKeymap,
        ...searchKeymap,
        ...historyKeymap,
        ...foldKeymap,
        indentWithTab,
      ]),
      editorTheme,
      languageSlot.of([]),
      wrapSlot.of(wrapExtension(wrap)),
      EditorView.updateListener.of(onUpdate),
    ],
  });
}
