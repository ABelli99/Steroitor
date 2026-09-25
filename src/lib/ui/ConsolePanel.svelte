<script lang="ts">
  import { saveSettings, settings } from "../settings.svelte";
  import { openTerminal } from "../terminal/openTerminal";
  import { useProject } from "../project/context";
  import ContextMenu, { type MenuItem } from "./ContextMenu.svelte";
  import GitCommandLog from "./GitCommandLog.svelte";

  let { cwd }: { cwd: string | null } = $props();
  const { terminals } = useProject();

  /** null = log dei comandi Git, altrimenti l'id del terminale mostrato. */
  const showing = $derived(terminals.activeId);

  let shellMenu = $state<{ x: number; y: number } | null>(null);

  async function openShellMenu(event: MouseEvent) {
    const { left, bottom } = (event.currentTarget as HTMLElement).getBoundingClientRect();
    if (!terminals.shells.length) await terminals.loadShells();
    shellMenu = { x: left, y: bottom };
  }

  function setDefaultShell(id: string) {
    settings.terminalShell = id;
    saveSettings();
  }

  function shellItems(): MenuItem[] {
    const mark = (id: string) => (settings.terminalShell === id ? "● " : "   ");
    const choices = [{ id: "", name: "Automatica" }, ...terminals.shells];
    return [
      ...terminals.shells.map((shell) => ({ label: `Nuovo ${shell.name}`, run: () => openTerminal(terminals, cwd, shell.id) })),
      ...choices.map((shell, index) => ({
        label: `${mark(shell.id)}Predefinita: ${shell.name}`,
        separatorBefore: index === 0,
        run: () => setDefaultShell(shell.id),
      })),
    ];
  }

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
    <button class="add" title="Nuovo terminale (Ctrl+T)" onclick={() => openTerminal(terminals, cwd)}>+ Terminale</button>
    <button class="add shells" title="Scegli la shell" aria-label="Scegli la shell" onclick={openShellMenu}>
      <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="M4 6l4 4 4-4" /></svg>
    </button>
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

{#if shellMenu}
  <ContextMenu x={shellMenu.x} y={shellMenu.y} items={shellItems()} onclose={() => (shellMenu = null)} />
{/if}

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

  .shells {
    display: flex;
    padding: 2px;
  }

  .shells path {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .body {
    flex: 1;
    min-height: 0;
  }
</style>
