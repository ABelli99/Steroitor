<script lang="ts">
  import { prompts, settlePrompt } from "./prompt.svelte";

  let input = $state<HTMLInputElement | HTMLTextAreaElement>();
  let value = $state("");

  $effect(() => {
    const request = prompts.current;
    if (!request || !input) return;
    value = request.value;
    input.focus();
    queueMicrotask(() => input?.setSelectionRange(0, request.selectUntil));
  });

  function onKeyDown(event: KeyboardEvent) {
    const submit = prompts.current?.multiline ? event.key === "Enter" && event.ctrlKey : event.key === "Enter";
    if (submit) {
      event.preventDefault();
      settlePrompt(value);
    }
    if (event.key === "Escape") settlePrompt(null);
  }
</script>

{#if prompts.current}
  <div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && settlePrompt(null)}>
    <div class="dialog" role="dialog" aria-label={prompts.current.title}>
      <label>
        <span>{prompts.current.title}</span>
        {#if prompts.current.multiline}
          <textarea bind:this={input} bind:value onkeydown={onKeyDown} spellcheck="false" rows="6"></textarea>
          <small>Ctrl+Invio per confermare, Esc per annullare</small>
        {:else}
          <input bind:this={input} bind:value onkeydown={onKeyDown} spellcheck="false" />
        {/if}
      </label>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 15vh;
    background: rgba(0, 0, 0, 0.15);
    z-index: 20;
  }

  .dialog {
    width: min(560px, calc(100vw - 32px));
    padding: 12px;
    background: var(--panel-bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  input,
  textarea {
    font: inherit;
    color: var(--fg);
    background: var(--editor-bg);
    border: 1px solid var(--accent);
    border-radius: 4px;
    padding: 5px 8px;
    outline: none;
  }

  textarea {
    resize: vertical;
    font-family: var(--mono);
  }

  small {
    color: var(--fg-muted);
  }
</style>
