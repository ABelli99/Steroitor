<script lang="ts">
  import type { GitRepo } from "../git/repo.svelte";
  import ConsolePanel from "./ConsolePanel.svelte";
  import { layout, saveLayout } from "./layout.svelte";

  let { git, folder }: { git: GitRepo | null; folder: string | null } = $props();

  const tabs = [
    { id: "console", label: "Console" },
    { id: "git", label: "Git" },
    { id: "commit", label: "Commit" },
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
  <div class="content" tabindex="-1" data-panel-content data-shortcut-context={layout.panelTab === "console" ? "editor" : "git"}>
    <div class="keep-alive" class:hidden={layout.panelTab !== "console"}>
      <ConsolePanel cwd={git?.root ?? folder} />
    </div>
    {#if layout.panelTab === "console"}
      <!-- sempre montata: i terminali devono sopravvivere al cambio di tab -->
    {:else if git && layout.panelTab === "git"}
      {#await import("../git/GitPanel.svelte") then { default: GitPanel }}
        <GitPanel repo={git} />
      {/await}
    {:else if git}
      {#await import("../git/CommitPanel.svelte") then { default: CommitPanel }}
        <CommitPanel repo={git} />
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

  .keep-alive {
    height: 100%;
  }

  .hidden {
    display: none;
  }

  .empty {
    margin: 8px 12px;
    color: var(--fg-muted);
  }
</style>
