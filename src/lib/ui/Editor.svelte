<script lang="ts">
  import { onMount } from "svelte";
  import type { Workspace } from "../workspace/workspace.svelte";

  let { workspace }: { workspace: Workspace } = $props();
  let host: HTMLDivElement;

  onMount(() => workspace.mount(host));
</script>

<div class="editor" bind:this={host} data-shortcut-context="editor"></div>

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
