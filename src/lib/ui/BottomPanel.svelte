<script lang="ts">
  import { layout, saveLayout } from "./layout.svelte";

  const tabs = [
    { id: "console", label: "Console" },
    { id: "git", label: "Git" },
  ] as const;

  function select(id: (typeof tabs)[number]["id"]) {
    layout.panelTab = id;
    saveLayout();
  }
</script>

<section class="panel">
  <div class="tabs" role="tablist">
    {#each tabs as tab (tab.id)}
      <button role="tab" aria-selected={layout.panelTab === tab.id} class:active={layout.panelTab === tab.id} onclick={() => select(tab.id)}>
        {tab.label}
      </button>
    {/each}
  </div>
  <div class="content" tabindex="-1" data-shortcut-context={layout.panelTab === "git" ? "git" : "editor"}>
    {#if layout.panelTab === "console"}
      <p class="empty">Nessun comando eseguito.</p>
    {:else}
      <p class="empty">Nessun repository Git rilevato.</p>
    {/if}
  </div>
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--panel-bg);
  }

  .tabs {
    display: flex;
    flex: none;
    gap: 2px;
    padding: 4px 6px;
    border-bottom: 1px solid var(--border);
  }

  .tabs .active {
    color: var(--accent);
    box-shadow: inset 0 -2px 0 var(--accent);
    border-radius: 0;
  }

  .content {
    flex: 1;
    overflow: auto;
    padding: 8px 12px;
    font-family: var(--mono);
    font-size: 12px;
    user-select: text;
    outline: none;
  }

  .empty {
    margin: 0;
    color: var(--fg-muted);
  }
</style>
