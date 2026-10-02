<script lang="ts">
  import { onMount } from "svelte";
  import { fileName } from "../workspace/files";
  import { gitCommitMessage } from "./api";
  import CommitChanges from "./CommitChanges.svelte";
  import DiffView from "./DiffView.svelte";
  import { canJumpTo } from "./jumpToSource";
  import type { DiffRequest, FileSelection } from "./selection.svelte";

  interface Props {
    root: string;
    request: DiffRequest;
    onclose: () => void;
    onopenfile: (path: string) => void;
    /** Apre il file del diff nell'editor sulla prima riga modificata. */
    onjump: (file: FileSelection) => void;
  }

  let { root, request, onclose, onopenfile, onjump }: Props = $props();

  let dialog = $state<HTMLElement>();
  let subject = $state("");

  const title = $derived(
    request.kind === "commit" ? `${request.hash.slice(0, 7)}${subject ? ` · ${subject}` : ""}` : fileName(request.kind === "file" ? request.file.path : request.path),
  );

  onMount(() => {
    dialog?.focus();
    if (request.kind !== "commit") return;
    gitCommitMessage(root, request.hash)
      .then((message) => (subject = message.split("\n")[0]))
      .catch(() => {});
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
      {#if request.kind === "file" && canJumpTo(request.file)}
        {@const file = request.file}
        <button title="Apri nell'editor alla prima modifica" onclick={() => onjump(file)}>Apri nell'editor</button>
      {/if}
      <button onclick={onclose}>Chiudi</button>
    </header>
    <div class="body">
      {#if request.kind === "file"}
        <DiffView {root} file={request.file} />
      {:else if request.kind === "commit"}
        <CommitChanges {root} hash={request.hash} {onopenfile} />
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
    width: calc(100vw - 48px);
    height: calc(100vh - 48px);
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
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .path {
    color: var(--fg-muted);
    font-size: 12px;
  }

  .spacer {
    flex: 1;
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
</style>
