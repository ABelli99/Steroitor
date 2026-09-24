<script lang="ts">
  import type { GitRepo } from "../git/repo.svelte";
  import ConsolePanel from "./ConsolePanel.svelte";
  import { layout, saveLayout } from "./layout.svelte";

  let { git }: { git: GitRepo | null } = $props();

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
  <div class="content" tabindex="-1" data-panel-content data-shortcut-context={layout.panelTab === "git" ? "git" : "editor"}>
    {#if layout.panelTab === "console"}
      <ConsolePanel />
    {:else if git}
      {#await import("../git/GitPanel.svelte") then { default: GitPanel }}
        <GitPanel repo={git} />
      {/await}
    {:else}
      <p class="empty">Nessun repository Git nella cartella aperta.</p>
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
    min-height: 0;
    overflow: hidden;
    font-family: var(--mono);
    font-size: 12px;
    user-select: text;
    outline: none;
  }

  .empty {
    margin: 8px 12px;
    color: var(--fg-muted);
  }
</style>
