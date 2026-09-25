<script lang="ts">
  import { openTerminal } from "../terminal/openTerminal";
  import { terminals } from "../terminal/terminals.svelte";
  import GitCommandLog from "./GitCommandLog.svelte";

  let { cwd }: { cwd: string | null } = $props();

  /** null = log dei comandi Git, altrimenti l'id del terminale mostrato. */
  const showing = $derived(terminals.activeId);

  function closeOnMiddleClick(event: MouseEvent, id: number) {
    if (event.button === 1) terminals.close(id);
  }
</script>

<div class="console">
  <div class="subtabs" role="tablist">
    <button role="tab" aria-selected={showing === null} class:active={showing === null} onclick={() => (terminals.activeId = null)}>
      Comandi Git
    </button>
    {#each terminals.sessions as session (session.id)}
      <span
        class="subtab"
        class:active={showing === session.id}
        class:exited={session.exited}
        role="presentation"
        onauxclick={(e) => closeOnMiddleClick(e, session.id)}
      >
        <button role="tab" aria-selected={showing === session.id} onclick={() => (terminals.activeId = session.id)}>{session.title}</button>
        <button class="close" aria-label="Chiudi {session.title}" onclick={() => terminals.close(session.id)}>×</button>
      </span>
    {/each}
    <button class="add" title="Nuovo terminale" onclick={() => openTerminal(cwd)}>+ Terminale</button>
  </div>

  <div class="body">
    {#if showing === null}
      <GitCommandLog />
    {/if}
    {#if terminals.sessions.length}
      {#await import("../terminal/TerminalView.svelte") then { default: TerminalView }}
        {#each terminals.sessions as session (session.id)}
          <TerminalView id={session.id} visible={showing === session.id} />
        {/each}
      {/await}
    {/if}
  </div>
</div>

<style>
  .console {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .subtabs {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: none;
    padding: 2px 6px;
    font-family: var(--ui-font);
    font-size: 12px;
    border-bottom: 1px solid var(--border);
  }

  .subtabs .active,
  .subtab.active button {
    color: var(--accent);
  }

  .subtab {
    display: flex;
    align-items: center;
  }

  .subtab.exited button {
    color: var(--fg-muted);
    font-style: italic;
  }

  .close {
    padding: 0 4px;
  }

  .add {
    color: var(--fg-muted);
  }

  .body {
    flex: 1;
    min-height: 0;
  }
</style>
