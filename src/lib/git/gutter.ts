import { Chunk } from "@codemirror/merge";
import { RangeSet, RangeSetBuilder, StateField, type Extension, type Text } from "@codemirror/state";
import { EditorView, gutter, GutterMarker } from "@codemirror/view";

/** Click su un marker: riga cliccata, chunk HEAD↔buffer correnti e baseline (HEAD). */
export type HunkClick = (view: EditorView, position: number, chunks: readonly Chunk[], baseline: Text, event: MouseEvent) => void;

type ChangeKind = "added" | "modified" | "deleted";

class ChangeMarker extends GutterMarker {
  constructor(readonly kind: ChangeKind) {
    super();
  }

  eq(other: ChangeMarker) {
    return other.kind === this.kind;
  }

  toDOM() {
    const element = document.createElement("div");
    element.className = `cm-git-${this.kind}`;
    return element;
  }
}

const markers: Record<ChangeKind, ChangeMarker> = {
  added: new ChangeMarker("added"),
  modified: new ChangeMarker("modified"),
  deleted: new ChangeMarker("deleted"),
};

const DIFF_CONFIG = { timeout: 200 };

interface GutterState {
  chunks: readonly Chunk[];
  markers: RangeSet<GutterMarker>;
}

function allAdded(doc: Text): RangeSet<GutterMarker> {
  const builder = new RangeSetBuilder<GutterMarker>();
  for (let number = 1; number <= doc.lines; number++) {
    const from = doc.line(number).from;
    builder.add(from, from, markers.added);
  }
  return builder.finish();
}

export function buildMarkers(baseline: Text, chunks: readonly Chunk[], doc: Text): RangeSet<GutterMarker> {
  if (baseline.length === 0) return allAdded(doc);
  const builder = new RangeSetBuilder<GutterMarker>();
  for (const chunk of chunks) {
    if (chunk.fromB === chunk.toB) {
      const line = doc.lineAt(Math.min(chunk.fromB, doc.length));
      builder.add(line.from, line.from, markers.deleted);
      continue;
    }
    const marker = chunk.fromA === chunk.toA ? markers.added : markers.modified;
    const last = doc.lineAt(chunk.endB).number;
    for (let number = doc.lineAt(chunk.fromB).number; number <= last; number++) {
      const from = doc.line(number).from;
      builder.add(from, from, marker);
    }
  }
  return builder.finish();
}

const theme = EditorView.baseTheme({
  ".cm-git-gutter": { width: "4px", marginRight: "2px" },
  ".cm-git-gutter .cm-gutterElement": { padding: "0", cursor: "pointer" },
  ".cm-git-added, .cm-git-modified": { width: "3px", height: "100%" },
  ".cm-git-added": { backgroundColor: "var(--git-added)" },
  ".cm-git-modified": { backgroundColor: "var(--git-modified)" },
  ".cm-git-deleted": {
    width: "0",
    height: "0",
    borderTop: "4px solid transparent",
    borderBottom: "4px solid transparent",
    borderLeft: "5px solid var(--git-untracked)",
    transform: "translateY(-4px)",
  },
});

/** Marker di modifica rispetto a `baseline` (il contenuto del file a HEAD). */
export function gitGutter(baseline: Text, onClick?: HunkClick): Extension {
  const field = StateField.define<GutterState>({
    create(state) {
      const chunks = Chunk.build(baseline, state.doc, DIFF_CONFIG);
      return { chunks, markers: buildMarkers(baseline, chunks, state.doc) };
    },
    update(value, transaction) {
      if (!transaction.docChanged) return value;
      const chunks = Chunk.updateB(value.chunks, baseline, transaction.newDoc, transaction.changes, DIFF_CONFIG);
      return { chunks, markers: buildMarkers(baseline, chunks, transaction.newDoc) };
    },
  });

  return [
    field,
    theme,
    gutter({
      class: "cm-git-gutter",
      markers: (view) => view.state.field(field).markers,
      domEventHandlers: {
        click(view, block, event) {
          if (!onClick) return false;
          onClick(view, block.from, view.state.field(field).chunks, baseline, event as MouseEvent);
          return true;
        },
      },
    }),
  ];
}
