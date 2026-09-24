<script lang="ts">
  import { onMount } from "svelte";
  import { fileName } from "../workspace/files";
  import { gitShowCommit } from "./api";
  import DiffView from "./DiffView.svelte";
  import type { DiffRequest } from "./selection.svelte";

  let { root, request, onclose }: { root: string; request: DiffRequest; onclose: () => void } = $props();

  const MAX_PATCH_LINES = 5000;

  let dialog = $state<HTMLElement>();
  let patch = $state<string[] | null>(null);
  let error = $state<string | null>(null);

  const title = $derived(
    request.kind === "file" ? fileName(request.file.path) : request.kind === "commit" ? `Commit ${request.hash.slice(0, 7)}` : fileName(request.path),
  );
  const lineClass = (line: string) =>
    line.startsWith("+++") || line.startsWith("---") ? "meta" : line.startsWith("+") ? "add" : line.startsWith("-") ? "del" : line.startsWith("@@") ? "hunk" : line.startsWith("diff ") ? "file" : "";

  onMount(() => {
    dialog?.focus();
    if (request.kind !== "commit") return;
    gitShowCommit(root, request.hash)
      .then((text) => (patch = text.split("\n")))
      .catch((failure) => (error = String(failure)));
  });
</script>

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && onclose()}>
  <div
    class="dialog"
    role="dialog"
    aria-label={title}
    tabindex="-1"
    bind:this={dialog}
    onkeydown={(e) => e.key === "Escape" && onclose()}
  >
    <header>
      <span class="title">{title}</span>
      {#if request.kind === "file"}<span class="path">{request.file.path}</span>{/if}
      <span class="spacer"></span>
      <button onclick={onclose}>Chiudi</button>
    </header>
    <div class="body">
      {#if request.kind === "file"}
        <DiffView {root} file={request.file} />
      {:else if error}
        <p class="error">{error}</p>
      {:else if patch}
        <pre>{#each patch.slice(0, MAX_PATCH_LINES) as line, index (index)}<span class={lineClass(line)}>{line}</span>
{/each}{#if patch.length > MAX_PATCH_LINES}<span class="meta">… altre {patch.length - MAX_PATCH_LINES} righe</span>{/if}</pre>
      {:else}
        <p class="muted">Caricamento…</p>
      {/if}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 25;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.3);
  }

  .dialog {
    width: calc(100vw - 64px);
    height: calc(100vh - 64px);
    display: flex;
    flex-direction: column;
    background: var(--editor-bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
    overflow: hidden;
    outline: none;
  }

  header {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: none;
    padding: 6px 10px;
    background: var(--panel-bg);
    border-bottom: 1px solid var(--border);
  }

  .title {
    font-weight: 600;
  }

  .path,
  .muted {
    color: var(--fg-muted);
    font-size: 12px;
  }

  .spacer {
    flex: 1;
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  pre {
    margin: 0;
    padding: 8px 12px;
    font-family: var(--mono);
    font-size: 12px;
    user-select: text;
  }

  .add {
    color: var(--git-added);
  }

  .del {
    color: var(--git-untracked);
  }

  .hunk {
    color: var(--accent);
  }

  .file {
    font-weight: 600;
  }

  .meta {
    color: var(--fg-muted);
  }

  .error {
    margin: 12px;
    color: var(--danger);
  }
</style>
