<script lang="ts">
  import { onMount } from "svelte";
  import { fuzzyFilter } from "../explorer/fuzzy";
  import type { FileTree } from "../explorer/tree.svelte";
  import { fileName } from "../workspace/files";
  import type { Workspace } from "../workspace/workspace.svelte";

  interface Props {
    tree: FileTree;
    workspace: Workspace;
    onclose: () => void;
  }

  let { tree, workspace, onclose }: Props = $props();

  const LIMIT = 50;

  let query = $state("");
  let files = $state<string[]>([]);
  let loading = $state(true);
  let highlighted = $state(0);
  let input = $state<HTMLInputElement>();

  const results = $derived(query.trim() ? fuzzyFilter(query, files, LIMIT) : files.slice(0, LIMIT));
  const relative = (path: string) => (tree.root ? path.slice(tree.root.length).replace(/^[\\/]/, "") : path);
  const directory = (path: string) => relative(path).slice(0, -fileName(path).length).replace(/[\\/]$/, "");

  onMount(() => {
    input?.focus();
    tree.fileIndex().then((paths) => {
      files = paths;
      loading = false;
    });
  });

  $effect(() => {
    query;
    highlighted = 0;
  });

  function choose(path: string | undefined) {
    if (!path) return;
    onclose();
    workspace.openPath(path);
  }

  function onKeyDown(event: KeyboardEvent) {
    const moves: Record<string, number> = { ArrowDown: 1, ArrowUp: -1, PageDown: 10, PageUp: -10 };
    if (event.key in moves) {
      event.preventDefault();
      highlighted = Math.min(Math.max(highlighted + moves[event.key], 0), results.length - 1);
      document.getElementById(`quick-open-${highlighted}`)?.scrollIntoView({ block: "nearest" });
      return;
    }
    if (event.key === "Enter") choose(results[highlighted]);
    if (event.key === "Escape") onclose();
  }
</script>

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="palette" role="dialog" aria-label="Quick Open">
    <input
      bind:this={input}
      bind:value={query}
      onkeydown={onKeyDown}
      placeholder={tree.root ? "Cerca file per nome…" : "Apri prima una cartella"}
      spellcheck="false"
    />
    <ul role="listbox">
      {#each results as path, index (path)}
        <li
          id="quick-open-{index}"
          role="option"
          aria-selected={index === highlighted}
          class:highlighted={index === highlighted}
          onpointerdown={(e) => {
            e.preventDefault();
            choose(path);
          }}
          onpointermove={() => (highlighted = index)}
        >
          <span class="name">{fileName(path)}</span>
          <span class="dir">{directory(path)}</span>
        </li>
      {:else}
        <li class="none">{loading ? "Indicizzazione…" : "Nessun risultato"}</li>
      {/each}
    </ul>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 10vh;
    z-index: 20;
  }

  .palette {
    width: min(600px, calc(100vw - 32px));
    display: flex;
    flex-direction: column;
    background: var(--panel-bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
    overflow: hidden;
  }

  input {
    font: inherit;
    color: var(--fg);
    background: var(--editor-bg);
    border: none;
    border-bottom: 1px solid var(--border);
    padding: 8px 12px;
    outline: none;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 4px 0;
    max-height: 50vh;
    overflow-y: auto;
  }

  li {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 4px 12px;
    cursor: pointer;
    white-space: nowrap;
  }

  .highlighted {
    background: var(--selection);
  }

  .dir {
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 12px;
    color: var(--fg-muted);
  }

  .none {
    color: var(--fg-muted);
    cursor: default;
  }
</style>
