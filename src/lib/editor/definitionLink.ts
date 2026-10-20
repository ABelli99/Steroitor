import { StateEffect, StateField, type Extension } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, type DecorationSet } from "@codemirror/view";
import { targetAt } from "./definition";

const setLink = StateEffect.define<{ from: number; to: number } | null>();
const linkMark = Decoration.mark({ class: "cm-definition-link" });

const linkField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(links, tr) {
    for (const effect of tr.effects) if (effect.is(setLink)) return effect.value ? Decoration.set(linkMark.range(effect.value.from, effect.value.to)) : Decoration.none;
    return tr.docChanged ? Decoration.none : links;
  },
  provide: (field) => EditorView.decorations.from(field),
});

/** Con Ctrl premuto, l'identificatore sotto il mouse diventa un link alla dichiarazione (come in IntelliJ). */
const hover = ViewPlugin.fromClass(
  class {
    #current: { from: number; to: number } | null = null;
    #destroyed = false;
    #onKeyUp = (event: KeyboardEvent) => (event.key === "Control" || event.key === "Meta" ? this.show(null) : undefined);
    #onBlur = () => this.show(null);

    constructor(readonly view: EditorView) {
      window.addEventListener("keyup", this.#onKeyUp);
      window.addEventListener("blur", this.#onBlur);
    }

    show(range: { from: number; to: number } | null) {
      if (range?.from === this.#current?.from && range?.to === this.#current?.to) return;
      this.#current = range;
      // Il dispatch non può avvenire dentro un update dell'editor: si rimanda al prossimo tick.
      queueMicrotask(() => !this.#destroyed && this.view.dispatch({ effects: setLink.of(range) }));
    }

    destroy() {
      this.#destroyed = true;
      window.removeEventListener("keyup", this.#onKeyUp);
      window.removeEventListener("blur", this.#onBlur);
    }
  },
  {
    eventHandlers: {
      mousemove(event) {
        const pos = event.ctrlKey || event.metaKey ? this.view.posAtCoords({ x: event.clientX, y: event.clientY }, false) : null;
        const target = pos === null ? null : targetAt(this.view.state, pos);
        this.show(target && { from: target.from, to: target.to });
      },
      mouseleave() {
        this.show(null);
      },
    },
  },
);

export const definitionLink: Extension = [
  linkField,
  hover,
  EditorView.baseTheme({ ".cm-definition-link": { textDecoration: "underline", cursor: "pointer", color: "var(--accent)" } }),
];
