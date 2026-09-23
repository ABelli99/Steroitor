<script lang="ts">
  import type { Workspace } from "../workspace/workspace.svelte";

  let { workspace }: { workspace: Workspace } = $props();
  let draggedId: string | null = null;

  function onAuxClick(event: MouseEvent, id: string) {
    if (event.button === 1) workspace.close(id);
  }

  function onDrop(index: number) {
    if (draggedId) workspace.move(draggedId, index);
    draggedId = null;
  }
</script>

<div class="tabs" role="tablist" tabindex="-1" ondblclick={(e) => e.target === e.currentTarget && workspace.newUntitled()}>
  {#each workspace.tabs as tab, index (tab.id)}
    <div
      class="tab"
      class:active={tab.id === workspace.activeId}
      role="tab"
      tabindex="-1"
      aria-selected={tab.id === workspace.activeId}
      title={tab.path ?? tab.name}
      draggable="true"
      onclick={() => workspace.activate(tab.id)}
      onkeydown={(e) => e.key === "Enter" && workspace.activate(tab.id)}
      onauxclick={(e) => onAuxClick(e, tab.id)}
      ondragstart={() => (draggedId = tab.id)}
      ondragover={(e) => e.preventDefault()}
      ondrop={() => onDrop(index)}
    >
      <span class="name">{tab.name}</span>
      <button
        class="close"
        class:dirty={tab.dirty}
        aria-label="Chiudi {tab.name}"
        onclick={(e) => {
          e.stopPropagation();
          workspace.close(tab.id);
        }}
      ></button>
    </div>
  {/each}
</div>

<style>
  .tabs {
    display: flex;
    flex: none;
    height: 34px;
    overflow-x: auto;
    background: var(--panel-bg);
    border-bottom: 1px solid var(--border);
    scrollbar-width: thin;
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 6px 0 12px;
    border-right: 1px solid var(--border);
    white-space: nowrap;
    cursor: pointer;
    color: var(--fg-muted);
  }

  .tab.active {
    color: var(--fg);
    background: var(--editor-bg);
    box-shadow: inset 0 -2px 0 var(--accent);
  }

  .close {
    width: 18px;
    height: 18px;
    padding: 0;
    position: relative;
    visibility: hidden;
  }

  .close::before {
    content: "×";
    font-size: 15px;
    line-height: 18px;
  }

  .tab:hover .close,
  .tab.active .close,
  .close.dirty {
    visibility: visible;
  }

  .close.dirty::before {
    content: "●";
    font-size: 10px;
  }

  .close.dirty:hover::before {
    content: "×";
    font-size: 15px;
  }
</style>
