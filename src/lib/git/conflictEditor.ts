import { EditorSelection, RangeSetBuilder, StateField, type EditorState, type Extension } from "@codemirror/state";
import { Decoration, EditorView, WidgetType, type DecorationSet } from "@codemirror/view";
import { parseConflicts, resolution, type Choice, type Conflict, type Span } from "./conflicts";

const CHOICES: Array<[Choice, string]> = [
  ["ours", "Accetta nostra"],
  ["theirs", "Accetta loro"],
  ["ours-theirs", "Nostra + loro"],
  ["theirs-ours", "Loro + nostra"],
];

export function applyChoice(view: EditorView, index: number, choice: Choice) {
  const text = view.state.doc.toString();
  const conflict = parseConflicts(text)[index];
  if (!conflict) return;
  view.dispatch({ changes: { from: conflict.from, to: conflict.to, insert: resolution(text, conflict, choice) } });
}

class ConflictActions extends WidgetType {
  constructor(readonly index: number) {
    super();
  }

  eq(other: ConflictActions) {
    return other.index === this.index;
  }

  toDOM(view: EditorView) {
    const bar = document.createElement("div");
    bar.className = "cm-conflict-actions";
    for (const [choice, label] of CHOICES) {
      const button = document.createElement("button");
      button.textContent = label;
      button.onmousedown = (event) => event.preventDefault();
      button.onclick = () => applyChoice(view, this.index, choice);
      bar.append(button);
    }
    return bar;
  }

  ignoreEvent() {
    return true;
  }
}

const lineClass = (className: string) => Decoration.line({ class: className });
const LINES = {
  marker: lineClass("cm-conflict-marker"),
  ours: lineClass("cm-conflict-ours"),
  base: lineClass("cm-conflict-base"),
  theirs: lineClass("cm-conflict-theirs"),
};

function decorate(state: EditorState): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  const doc = state.doc;
  const addLines = (span: Span, decoration: Decoration) => {
    if (span.to <= span.from) return;
    const last = doc.lineAt(span.to - 1).number;
    for (let number = doc.lineAt(span.from).number; number <= last; number++) {
      const from = doc.line(number).from;
      builder.add(from, from, decoration);
    }
  };
  const addMarker = (position: number) => {
    const line = doc.lineAt(Math.min(position, doc.length));
    builder.add(line.from, line.from, LINES.marker);
  };

  parseConflicts(doc.toString()).forEach((conflict: Conflict, index) => {
    builder.add(conflict.from, conflict.from, Decoration.widget({ widget: new ConflictActions(index), block: true, side: -1 }));
    addMarker(conflict.from);
    addLines(conflict.ours, LINES.ours);
    if (conflict.base) {
      addMarker(conflict.ours.to);
      addLines(conflict.base, LINES.base);
    }
    addMarker(conflict.base ? conflict.base.to : conflict.ours.to);
    addLines(conflict.theirs, LINES.theirs);
    addMarker(conflict.theirs.to);
  });
  return builder.finish();
}

const conflictField = StateField.define<DecorationSet>({
  create: decorate,
  update: (decorations, transaction) => (transaction.docChanged ? decorate(transaction.state) : decorations),
  provide: (field) => EditorView.decorations.from(field),
});

const theme = EditorView.baseTheme({
  ".cm-conflict-actions": {
    display: "flex",
    gap: "4px",
    padding: "2px 0",
    fontFamily: "var(--ui-font)",
    fontSize: "12px",
  },
  ".cm-conflict-actions button": {
    font: "inherit",
    color: "var(--accent)",
    background: "none",
    border: "1px solid var(--border)",
    borderRadius: "3px",
    padding: "0 6px",
    cursor: "pointer",
  },
  ".cm-conflict-marker": { color: "var(--fg-muted)", backgroundColor: "var(--active-line)" },
  ".cm-conflict-ours": { backgroundColor: "rgba(79, 140, 247, 0.14)" },
  ".cm-conflict-base": { backgroundColor: "rgba(128, 128, 128, 0.12)" },
  ".cm-conflict-theirs": { backgroundColor: "rgba(46, 160, 67, 0.14)" },
});

export const conflictEditor: Extension = [conflictField, theme];

/** Sposta il cursore al conflitto successivo (o precedente) rispetto alla posizione corrente. */
export function jumpToConflict(view: EditorView, step: 1 | -1): boolean {
  const conflicts = parseConflicts(view.state.doc.toString());
  if (!conflicts.length) return false;
  const head = view.state.selection.main.head;
  const target =
    step === 1
      ? (conflicts.find((conflict) => conflict.from > head) ?? conflicts[0])
      : ([...conflicts].reverse().find((conflict) => conflict.from < head) ?? conflicts[conflicts.length - 1]);
  view.dispatch({ selection: EditorSelection.cursor(target.from), effects: EditorView.scrollIntoView(target.from, { y: "center" }) });
  return true;
}
