import { RangeSet, RangeSetBuilder, StateField, type Extension, type Text } from "@codemirror/state";
import { EditorView, gutter, GutterMarker } from "@codemirror/view";
import type { BlameLine } from "./api";

const UNCOMMITTED = /^0+$/;

const shortDate = (timestamp: number) =>
  new Date(timestamp * 1000).toLocaleDateString("it-IT", { day: "2-digit", month: "2-digit", year: "2-digit" });

class BlameMarker extends GutterMarker {
  /** Segue il testo alla sua destra: un inserimento a inizio riga non si porta via l'annotazione. */
  startSide = 1;
  endSide = 1;

  constructor(readonly line: BlameLine) {
    super();
  }

  get committed() {
    return !UNCOMMITTED.test(this.line.hash);
  }

  eq(other: BlameMarker) {
    return other.line.hash === this.line.hash;
  }

  toDOM() {
    const element = document.createElement("div");
    element.className = this.committed ? "cm-blame-line" : "cm-blame-line cm-blame-uncommitted";
    element.textContent = this.committed ? `${shortDate(this.line.timestamp)} ${this.line.author}` : this.line.author;
    element.title = this.committed ? `${this.line.hash.slice(0, 8)} · ${this.line.summary}` : "Modifica non ancora committata";
    return element;
  }
}

export function blameMarkers(lines: BlameLine[], doc: Text): RangeSet<GutterMarker> {
  const builder = new RangeSetBuilder<GutterMarker>();
  const count = Math.min(lines.length, doc.lines);
  for (let index = 0; index < count; index++) {
    const from = doc.line(index + 1).from;
    builder.add(from, from, new BlameMarker(lines[index]));
  }
  return builder.finish();
}

const theme = EditorView.baseTheme({
  ".cm-blame-gutter .cm-gutterElement": { padding: "0 8px 0 4px", cursor: "pointer" },
  ".cm-blame-line": { width: "20ch", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", fontSize: "12px", color: "var(--fg-muted)" },
  ".cm-blame-uncommitted": { fontStyle: "italic" },
});

export const blameField = (lines: BlameLine[]) =>
  StateField.define<RangeSet<GutterMarker>>({
    create: (state) => blameMarkers(lines, state.doc),
    update: (markers, transaction) => markers.map(transaction.changes),
  });

/** Annotazioni git blame nel gutter; il click su una riga apre il relativo commit. */
export function blameGutter(lines: BlameLine[], onSelect: (hash: string) => void): Extension {
  const field = blameField(lines);

  return [
    field,
    theme,
    gutter({
      class: "cm-blame-gutter",
      markers: (view) => view.state.field(field),
      domEventHandlers: {
        click(view, block) {
          let hash: string | null = null;
          view.state.field(field).between(block.from, block.from, (_from, _to, marker) => {
            const blame = marker as BlameMarker;
            if (blame.committed) hash = blame.line.hash;
          });
          if (hash) onSelect(hash);
          return hash !== null;
        },
      },
    }),
  ];
}
