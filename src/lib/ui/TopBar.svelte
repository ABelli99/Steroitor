<script lang="ts">
  import type { Workspace } from "../workspace/workspace.svelte";
  import { layout, saveLayout } from "./layout.svelte";

  interface Props {
    workspace: Workspace;
    onopenfolder: () => void;
    onnewproject: () => void;
    onclone: () => void;
  }

  let { workspace, onopenfolder, onnewproject, onclone }: Props = $props();

  function toggle(key: "explorerVisible" | "panelVisible") {
    layout[key] = !layout[key];
    saveLayout();
  }
</script>

<header class="bar">
  <button title="Nuovo (Ctrl+N)" onclick={() => workspace.newUntitled()}>Nuovo</button>
  <button title="Apri (Ctrl+O)" onclick={() => workspace.openDialog()}>Apri</button>
  <button title="Apri cartella" onclick={onopenfolder}>Apri cartella</button>
  <button title="Clona un repository Git" onclick={onclone}>Clona</button>
  <button title="Apri una cartella in una nuova tab progetto" onclick={onnewproject}>Nuovo progetto</button>
  <span class="spacer"></span>
  <button title="Explorer (Alt+1)" class:on={layout.explorerVisible} onclick={() => toggle("explorerVisible")}>
    Explorer
  </button>
  <button title="Pannello inferiore" class:on={layout.panelVisible} onclick={() => toggle("panelVisible")}>
    Pannello
  </button>
</header>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: none;
    height: 32px;
    padding: 0 6px;
    background: var(--panel-bg);
    border-bottom: 1px solid var(--border);
  }

  .spacer {
    flex: 1;
  }

  .on {
    color: var(--accent);
  }
</style>
