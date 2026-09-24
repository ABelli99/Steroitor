<script lang="ts" module>
  export interface MenuItem {
    label: string;
    run: () => void;
    danger?: boolean;
    disabled?: boolean;
    hint?: string;
    separatorBefore?: boolean;
  }
</script>

<script lang="ts">
  interface Props {
    x: number;
    y: number;
    items: MenuItem[];
    onclose: () => void;
  }

  let { x, y, items, onclose }: Props = $props();
  let menu = $state<HTMLElement>();
  let position = $state({ left: 0, top: 0 });

  $effect(() => {
    if (!menu) return;
    const { width, height } = menu.getBoundingClientRect();
    position = {
      left: Math.min(x, window.innerWidth - width - 4),
      top: Math.min(y, window.innerHeight - height - 4),
    };
    menu.querySelector("button")?.focus();
  });

  function choose(item: MenuItem) {
    onclose();
    item.run();
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === "Escape") return onclose();
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    const buttons = [...menu!.querySelectorAll("button")];
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const step = event.key === "ArrowDown" ? 1 : -1;
    buttons[(index + step + buttons.length) % buttons.length]?.focus();
  }
</script>

<svelte:window onpointerdown={(e) => !menu?.contains(e.target as Node) && onclose()} onblur={onclose} />

<div
  bind:this={menu}
  class="menu"
  role="menu"
  tabindex="-1"
  style:left="{position.left}px"
  style:top="{position.top}px"
  onkeydown={onKeyDown}
>
  {#each items as item, index (index)}
    {#if item.separatorBefore}<hr />{/if}
    {#if item.disabled}
      <div class="section">{item.label}</div>
    {:else}
      <button role="menuitem" class:danger={item.danger} onclick={() => choose(item)}>
        <span>{item.label}</span>
        {#if item.hint}<span class="hint">{item.hint}</span>{/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 30;
    min-width: 180px;
    max-height: calc(100vh - 16px);
    overflow-y: auto;
    padding: 4px;
    display: flex;
    flex-direction: column;
    background: var(--panel-bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.25);
  }

  button {
    display: flex;
    justify-content: space-between;
    gap: 24px;
    text-align: left;
    padding: 4px 10px;
  }

  .hint,
  .section {
    color: var(--fg-muted);
  }

  .section {
    padding: 4px 10px 2px;
    font-size: 11px;
    text-transform: uppercase;
  }

  button:focus {
    background: var(--hover);
    outline: none;
  }

  .danger {
    color: var(--danger);
  }

  hr {
    margin: 4px 0;
    border: none;
    border-top: 1px solid var(--border);
  }
</style>
