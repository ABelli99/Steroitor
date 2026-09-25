import type { EditorState, RangeSet, Text } from "@codemirror/state";
import { EditorView, ViewPlugin, type GutterMarker, type ViewUpdate } from "@codemirror/view";
import type { ChangeKind } from "./gutter";

interface Span {
  kind: ChangeKind;
  from: number;
  to: number;
}

interface Stripe {
  kind: ChangeKind;
  top: number;
  height: number;
}

interface Layout {
  top: number;
  width: number;
  height: number;
  stripes: Stripe[];
}

const MIN_STRIPE = 2;

/** Righe consecutive dello stesso tipo fuse in un unico intervallo (posizioni di inizio riga). */
export function mergeSpans(markers: RangeSet<GutterMarker>, doc: Text): Span[] {
  const spans: Span[] = [];
  let lastLine = -1;
  for (const cursor = markers.iter(); cursor.value; cursor.next()) {
    const kind = (cursor.value as unknown as { kind: ChangeKind }).kind;
    const line = doc.lineAt(cursor.from).number;
    const previous = spans[spans.length - 1];
    if (previous?.kind === kind && line === lastLine + 1) previous.to = cursor.from;
    else spans.push({ kind, from: cursor.from, to: cursor.from });
    lastLine = line;
  }
  return spans;
}

const theme = EditorView.baseTheme({
  ".cm-git-overview": { position: "absolute", right: "0", pointerEvents: "none", zIndex: "300" },
  ".cm-git-overview > div": { position: "absolute", left: "2px", right: "2px" },
  ".cm-git-overview-added": { backgroundColor: "var(--git-added)" },
  ".cm-git-overview-modified": { backgroundColor: "var(--git-modified)" },
  ".cm-git-overview-deleted": { backgroundColor: "var(--git-untracked)" },
});

/** Stripe delle modifiche sopra la scrollbar verticale, come la error stripe di IntelliJ. */
export function overviewRuler(source: (state: EditorState) => RangeSet<GutterMarker>) {
  const plugin = ViewPlugin.fromClass(
    class {
      readonly dom = document.createElement("div");

      constructor(readonly view: EditorView) {
        this.dom.className = "cm-git-overview";
        view.dom.appendChild(this.dom);
        this.schedule();
      }

      update(update: ViewUpdate) {
        if (update.docChanged || update.geometryChanged || update.heightChanged || source(update.startState) !== source(update.state)) {
          this.schedule();
        }
      }

      schedule() {
        this.view.requestMeasure({ key: this, read: () => this.measure(), write: (layout) => this.draw(layout) });
      }

      measure(): Layout {
        const { view } = this;
        const scroller = view.scrollDOM;
        const height = scroller.clientHeight;
        const scale = height / Math.max(scroller.scrollHeight, 1);
        const offset = view.documentPadding.top;
        const stripes = mergeSpans(source(view.state), view.state.doc).map(({ kind, from, to }) => {
          const top = (view.lineBlockAt(from).top + offset) * scale;
          const bottom = (view.lineBlockAt(to).bottom + offset) * scale;
          return { kind, top, height: Math.max(bottom - top, MIN_STRIPE) };
        });
        return { top: scroller.offsetTop, width: scroller.offsetWidth - scroller.clientWidth, height, stripes };
      }

      draw({ top, width, height, stripes }: Layout) {
        Object.assign(this.dom.style, { top: `${top}px`, width: `${width}px`, height: `${height}px` });
        this.dom.replaceChildren(
          ...stripes.map(({ kind, top, height }) => {
            const stripe = document.createElement("div");
            stripe.className = `cm-git-overview-${kind}`;
            Object.assign(stripe.style, { top: `${top}px`, height: `${height}px` });
            return stripe;
          }),
        );
      }

      destroy() {
        this.dom.remove();
      }
    },
  );
  return [plugin, theme];
}
