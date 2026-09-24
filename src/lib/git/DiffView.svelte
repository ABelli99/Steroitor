<script lang="ts">
  import { MergeView } from "@codemirror/merge";
  import { EditorState, type Extension } from "@codemirror/state";
  import { EditorView, lineNumbers } from "@codemirror/view";
  import { readTextFile } from "../backend";
  import { detectLanguage, loadLanguage } from "../editor/languages";
  import { editorTheme } from "../editor/theme";
  import { gitHeadContent } from "./api";
  import type { FileSelection } from "./selection.svelte";

  let { root, file }: { root: string; file: FileSelection } = $props();

  let host = $state<HTMLElement>();
  let error = $state<string | null>(null);

  async function contents(selection: FileSelection) {
    const isNew = selection.status === "added" || selection.status === "untracked";
    const head = isNew ? "" : ((await gitHeadContent(root, selection.path))?.content ?? "");
    const current = selection.status === "deleted" ? "" : (await readTextFile(selection.path)).content;
    return { head, current };
  }

  function side(doc: string, language: Extension) {
    return {
      doc,
      extensions: [lineNumbers(), EditorState.readOnly.of(true), EditorView.editable.of(false), editorTheme, language],
    };
  }

  $effect(() => {
    const selection = file;
    const parent = host;
    if (!parent) return;
    let view: MergeView | null = null;
    let cancelled = false;

    Promise.all([contents(selection), loadLanguage(detectLanguage(selection.path))])
      .then(([{ head, current }, language]) => {
        if (cancelled) return;
        error = null;
        view = new MergeView({
          a: side(head, language),
          b: side(current, language),
          parent,
          collapseUnchanged: { margin: 3, minSize: 6 },
        });
      })
      .catch((failure) => {
        if (!cancelled) error = String(failure);
      });

    return () => {
      cancelled = true;
      view?.destroy();
    };
  });
</script>

<div class="diff">
  <div class="labels"><span>HEAD</span><span>Working tree</span></div>
  {#if error}<p class="error">{error}</p>{/if}
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

  .error {
    margin: 8px;
    color: var(--danger);
  }
</style>
