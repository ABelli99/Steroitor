<script lang="ts">
  import { onMount } from "svelte";
  import type { Workspace } from "../workspace/workspace.svelte";

  let { workspace, ongutterMenu }: { workspace: Workspace; ongutterMenu?: (event: MouseEvent) => void } = $props();
  let host: HTMLDivElement;

  onMount(() => workspace.mount(host));

  function onContextMenu(event: MouseEvent) {
    if (!ongutterMenu || !(event.target as Element).closest(".cm-gutters")) return;
    event.preventDefault();
    ongutterMenu(event);
  }
</script>

<div class="area">
  <div class="editor" class:hidden={!workspace.tabs.length} role="presentation" bind:this={host} data-shortcut-context="editor" oncontextmenu={onContextMenu}></div>
  {#if !workspace.tabs.length}
    <div class="empty" data-shortcut-context="editor">
      <p>Nessun file aperto</p>
      <button onclick={() => workspace.newUntitled()}>Nuovo file <kbd>Ctrl+N</kbd></button>
      <button onclick={() => workspace.openDialog()}>Apri file <kbd>Ctrl+O</kbd></button>
    </div>
  {/if}
</div>

<style>
  .area {
    display: flex;
    flex: 1;
    min-height: 0;
  }

  .editor {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    user-select: text;
  }

  .hidden {
    display: none;
  }

  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    color: var(--fg-muted);
    background: var(--editor-bg);
  }

  .empty p {
    margin: 0 0 4px;
    font-size: 15px;
  }

  .empty button {
    min-width: 200px;
    display: flex;
    justify-content: space-between;
    gap: 16px;
  }

  kbd {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--fg-muted);
  }

  .editor :global(.cm-editor) {
    height: 100%;
  }

  .editor :global(.cm-editor.cm-focused) {
    outline: none;
  }
</style>
