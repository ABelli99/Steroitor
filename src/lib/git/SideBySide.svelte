<script lang="ts">
  import { MergeView } from "@codemirror/merge";
  import { EditorState, type Extension } from "@codemirror/state";
  import { EditorView, lineNumbers } from "@codemirror/view";
  import { detectLanguage, loadLanguage } from "../editor/languages";
  import { editorTheme } from "../editor/theme";
  import type { ChangeKind } from "./gutter";

  interface Props {
    /** Usato solo per l'evidenziazione della sintassi. */
    path: string;
    left: string;
    right: string;
    labels: [string, string];
    /** true: mostra solo le zone cambiate; false: il file intero. */
    collapse?: boolean;
  }

  let { path, left, right, labels, collapse = true }: Props = $props();

  let host = $state<HTMLElement>();
  let stripes = $state.raw<{ kind: ChangeKind; top: number; height: number }[]>([]);
  let ruler = $state({ width: 0, height: 0 });

  const MIN_STRIPE = 2;

  /** Stripe delle modifiche sopra la scrollbar del diff, come nell'editor: i due lati sono allineati, basta uno. */
  function measureOverview(view: MergeView, scroller: HTMLElement) {
    const scale = scroller.clientHeight / Math.max(scroller.scrollHeight, 1);
    const scrollerTop = scroller.getBoundingClientRect().top - scroller.scrollTop;
    stripes = view.chunks.map((chunk) => {
      const kind: ChangeKind = chunk.fromA === chunk.toA ? "added" : chunk.fromB === chunk.toB ? "deleted" : "modified";
      const [editor, from, to] = kind === "deleted" ? [view.a, chunk.fromA, chunk.toA] : [view.b, chunk.fromB, chunk.toB];
      const origin = editor.documentTop - scrollerTop;
      const top = editor.lineBlockAt(from).top;
      const bottom = editor.lineBlockAt(Math.max(from, to - 1)).bottom;
      return { kind, top: (origin + top) * scale, height: Math.max((bottom - top) * scale, MIN_STRIPE) };
    });
    ruler = { width: scroller.offsetWidth - scroller.clientWidth, height: scroller.clientHeight };
  }

  const side = (doc: string, language: Extension) => ({
    doc,
    extensions: [lineNumbers(), EditorState.readOnly.of(true), EditorView.editable.of(false), editorTheme, language],
  });

  $effect(() => {
    const parent = host;
    const config = { path, left, right, collapse };
    if (!parent) return;
    let view: MergeView | null = null;
    let cancelled = false;
    let frame = 0;
    const schedule = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => view && measureOverview(view, parent));
    };
    const resize = new ResizeObserver(schedule);
    resize.observe(parent);

    loadLanguage(detectLanguage(config.path)).then((language) => {
      if (cancelled) return;
      view = new MergeView({
        a: side(config.left, language),
        b: side(config.right, language),
        parent,
        collapseUnchanged: config.collapse ? { margin: 3, minSize: 6 } : undefined,
      });
      const firstChange = view.chunks[0];
      if (firstChange) view.b.dispatch({ effects: EditorView.scrollIntoView(firstChange.fromB, { y: "center" }) });
      // Espandere una zona compressa cambia l'altezza del contenuto.
      resize.observe(view.dom);
      schedule();
    });

    return () => {
      cancelled = true;
      cancelAnimationFrame(frame);
      resize.disconnect();
      stripes = [];
      view?.destroy();
    };
  });
</script>

<div class="diff">
  <div class="labels"><span>{labels[0]}</span><span>{labels[1]}</span></div>
  <div class="body">
    <div class="host" bind:this={host}></div>
    <div class="overview" style:width="{ruler.width}px" style:height="{ruler.height}px" aria-hidden="true">
      {#each stripes as stripe, index (index)}
        <div class="stripe {stripe.kind}" style:top="{stripe.top}px" style:height="{stripe.height}px"></div>
      {/each}
    </div>
  </div>
</div>

<style>
  .diff {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .labels {
    display: grid;
    grid-template-columns: 1fr 1fr;
    flex: none;
    padding: 2px 8px;
    font-family: var(--ui-font);
    font-size: 11px;
    color: var(--fg-muted);
    border-bottom: 1px solid var(--border);
  }

  .body {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .host {
    flex: 1;
    min-width: 0;
    overflow: auto;
    user-select: text;
  }

  .overview {
    position: absolute;
    top: 0;
    right: 0;
    pointer-events: none;
    z-index: 5;
  }

  .stripe {
    position: absolute;
    left: 2px;
    right: 2px;
  }

  .stripe.added {
    background: var(--git-added);
  }

  .stripe.modified {
    background: var(--git-modified);
  }

  .stripe.deleted {
    background: var(--git-untracked);
  }

  .host :global(.cm-mergeView) {
    min-height: 100%;
  }

  .host :global(.cm-editor) {
    height: auto;
  }
</style>
