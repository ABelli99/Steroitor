<script lang="ts">
  import { focusProjectWindow, type ProjectWindow } from "../backend";
  import { folderName, projects } from "./projects.svelte";
  import icon from "../../../src-tauri/icons/32x32.png";

  let hint = $state<{ text: string; top: number } | null>(null);

  function showHint(event: MouseEvent, window: ProjectWindow) {
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    hint = { text: folderName(window.folder), top: box.top + box.height / 2 };
  }
</script>

<nav class="bar" aria-label="Finestre aperte">
  {#each projects.windows as window (window.label)}
    <button
      class:current={window.label === projects.current}
      aria-label={folderName(window.folder)}
      onclick={() => focusProjectWindow(window.label)}
      onmouseenter={(event) => showHint(event, window)}
      onmouseleave={() => (hint = null)}
    >
      <img src={icon} alt="" />
    </button>
  {/each}
</nav>

{#if hint}
  <div class="hint" style:top="{hint.top}px">{hint.text}</div>
{/if}

<style>
  .bar {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    flex: none;
    width: 36px;
    padding: 6px 0;
    overflow-y: auto;
    background: var(--panel-bg);
    border-right: 1px solid var(--border);
  }

  button {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border-left: 2px solid transparent;
    border-radius: 4px;
  }

  button.current {
    border-left-color: var(--accent);
    background: var(--hover);
  }

  img {
    width: 20px;
    height: 20px;
  }

  .hint {
    position: fixed;
    left: 42px;
    transform: translateY(-50%);
    z-index: 100;
    padding: 3px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--editor-bg);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.15);
    white-space: nowrap;
    pointer-events: none;
  }
</style>
