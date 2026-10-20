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
import { definitionLink } from "./definitionLink";

// Ctrl+click porta alla dichiarazione: i cursori multipli passano ad Alt+Shift+click come in IntelliJ.
const addsCursor = (event: MouseEvent) => event.altKey && event.shiftKey;

export const languageSlot = new Compartment();
export const wrapSlot = new Compartment();
export const gitSlot = new Compartment();
export const blameSlot = new Compartment();
export const minimapSlot = new Compartment();

export const wrapExtension = (enabled: boolean): Extension => (enabled ? EditorView.lineWrapping : []);

export function createState(content: string, wrap: boolean, minimap: Extension, onUpdate: (update: ViewUpdate) => void): EditorState {
  return EditorState.create({
    doc: content,
    extensions: [
      blameSlot.of([]),
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
      rectangularSelection({ eventFilter: (event) => event.altKey && !event.shiftKey && event.button === 0 }),
      EditorView.clickAddsSelectionRange.of(addsCursor),
      definitionLink,
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
      minimapSlot.of(minimap),
      EditorView.updateListener.of(onUpdate),
    ],
  });
}
