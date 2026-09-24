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

<div class="editor" role="presentation" bind:this={host} data-shortcut-context="editor" oncontextmenu={onContextMenu}></div>

<style>
  .editor {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    user-select: text;
  }

  .editor :global(.cm-editor) {
    height: 100%;
  }

  .editor :global(.cm-editor.cm-focused) {
    outline: none;
  }
</style>
