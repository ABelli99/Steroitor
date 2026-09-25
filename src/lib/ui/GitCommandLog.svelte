<script lang="ts">
  import { tick } from "svelte";
  import type { ConsoleEntry } from "../console/console.svelte";
  import { useProject } from "../project/context";

  const consoleLog = useProject().console;

  let list = $state<HTMLElement>();
  let stickToBottom = true;

  const time = (entry: ConsoleEntry) => new Date(entry.startedAt).toLocaleTimeString("it-IT");
  const failed = (entry: ConsoleEntry) => entry.code !== 0;

  $effect(() => {
    consoleLog.visible;
    if (stickToBottom) tick().then(() => list && (list.scrollTop = list.scrollHeight));
  });

  function onScroll() {
    if (!list) return;
    stickToBottom = list.scrollHeight - list.scrollTop - list.clientHeight < 24;
  }
</script>

<div class="console">
  <div class="toolbar">
    <label title="Mostra anche status, show, rev-parse e gli altri comandi eseguiti in automatico">
      <input type="checkbox" bind:checked={consoleLog.showBackground} />
      Comandi in background
    </label>
    {#if consoleLog.hidden}<span class="hidden">({consoleLog.hidden} nascosti)</span>{/if}
    <span class="spacer"></span>
    <button onclick={() => navigator.clipboard.writeText(consoleLog.asText())}>Copia</button>
    <button onclick={() => consoleLog.clear()}>Pulisci</button>
  </div>

  <div class="entries" bind:this={list} onscroll={onScroll}>
    {#each consoleLog.visible as entry (entry.id)}
      <details class="entry" class:failed={failed(entry)} open={failed(entry) || !entry.background}>
        <summary>
          <span class="time">{time(entry)}</span>
          <span class="command">$ {entry.command}</span>
          <span class="meta">{entry.code ?? "—"} · {entry.durationMs} ms</span>
        </summary>
        {#if entry.stdout}<pre>{entry.stdout}</pre>{/if}
        {#if entry.stderr}<pre class="stderr">{entry.stderr}</pre>{/if}
      </details>
    {:else}
      <p class="empty">Nessun comando eseguito.</p>
    {/each}
  </div>
</div>

<style>
  .console {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: none;
    padding: 2px 8px;
    font-family: var(--ui-font);
    font-size: 12px;
    border-bottom: 1px solid var(--border);
  }

  label {
    display: flex;
    align-items: center;
    gap: 4px;
    color: var(--fg-muted);
  }

  .hidden {
    color: var(--fg-muted);
  }

  .spacer {
    flex: 1;
  }

  .entries {
    flex: 1;
    overflow: auto;
    padding: 4px 8px;
  }

  summary {
    display: flex;
    gap: 10px;
    cursor: pointer;
    white-space: nowrap;
  }

  .time,
  .meta {
    color: var(--fg-muted);
  }

  .command {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .failed .meta,
  .stderr {
    color: var(--danger);
  }

  pre {
    margin: 2px 0 6px 18px;
    white-space: pre-wrap;
    word-break: break-all;
    font: inherit;
  }

  .empty {
    margin: 4px 0;
    color: var(--fg-muted);
  }
</style>
