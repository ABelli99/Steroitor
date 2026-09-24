<script lang="ts">
  import { MergeView } from "@codemirror/merge";
  import { EditorState, type Extension } from "@codemirror/state";
  import { EditorView, lineNumbers } from "@codemirror/view";
  import { detectLanguage, loadLanguage } from "../editor/languages";
  import { editorTheme } from "../editor/theme";

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
    });

    return () => {
      cancelled = true;
      view?.destroy();
    };
  });
</script>

<div class="diff">
  <div class="labels"><span>{labels[0]}</span><span>{labels[1]}</span></div>
  <div class="host" bind:this={host}></div>
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

  .host {
    flex: 1;
    min-height: 0;
    overflow: auto;
    user-select: text;
  }

  .host :global(.cm-mergeView) {
    min-height: 100%;
  }

  .host :global(.cm-editor) {
    height: auto;
  }
</style>
