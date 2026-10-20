<script lang="ts">
  import { gitFileAt, gitFileHistory, type FileRevision } from "./api";
  import { absoluteTime, relativeTime } from "./format";
  import SideBySide from "./SideBySide.svelte";

  interface Props {
    root: string;
    path: string;
    lines: [number, number] | null;
  }

  let { root, path, lines }: Props = $props();

  let revisions = $state.raw<FileRevision[]>([]);
  let selected = $state<FileRevision | null>(null);
  let onlyChanges = $state(false);
  let loading = $state(true);
  let error = $state<string | null>(null);

  $effect(() => {
    gitFileHistory(root, path, lines)
      .then((history) => {
        revisions = history;
        selected = history[0] ?? null;
        onlyChanges = lines !== null;
      })
      .catch((failure) => (error = String(failure)))
      .finally(() => (loading = false));
  });

  async function contents(revision: FileRevision) {
    const { commit } = revision;
    const before = commit.parents.length ? await gitFileAt(root, `${commit.hash}^`, revision.oldPath ?? revision.path) : null;
    const after = await gitFileAt(root, commit.hash, revision.path);
    return { before: before ?? "", after: after ?? "" };
  }

  function onKeyDown(event: KeyboardEvent) {
    const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
    if (!step || !revisions.length) return;
    event.preventDefault();
    const index = selected ? revisions.indexOf(selected) : -1;
    selected = revisions[Math.min(Math.max(index + step, 0), revisions.length - 1)];
  }
</script>

<div class="history">
  <div class="revisions" role="listbox" tabindex="0" onkeydown={onKeyDown}>
    <div class="count">
      {revisions.length === 1 ? "1 commit" : `${revisions.length} commit`}{lines ? ` · righe ${lines[0]}–${lines[1]}` : ""}
    </div>
    {#each revisions as revision (revision.commit.hash)}
      {@const commit = revision.commit}
      <div
        class="revision"
        class:selected={revision === selected}
        role="option"
        tabindex="-1"
        aria-selected={revision === selected}
        title={`${commit.subject}\n${commit.author} · ${absoluteTime(commit.timestamp)}`}
        onclick={() => (selected = revision)}
        onkeydown={() => {}}
      >
        <span class="subject">{commit.subject}</span>
        <span class="meta"><span class="hash">{commit.shortHash}</span> · {commit.author} · {relativeTime(commit.timestamp)}</span>
      </div>
    {:else}
      <p class="muted">{error ?? (loading ? "Caricamento…" : "Nessun commit tocca questo file.")}</p>
    {/each}
  </div>

  <div class="view">
    {#if selected}
      {@const revision = selected}
      <div class="toolbar">
        <span class="hash">{revision.commit.shortHash}</span>
        <span class="subject">{revision.commit.subject}</span>
        {#if revision.oldPath}<span class="muted-inline">rinominato</span>{/if}
        <span class="spacer"></span>
        <label><input type="checkbox" bind:checked={onlyChanges} /> Solo le zone cambiate</label>
      </div>
      {#await contents(revision) then { before, after }}
        <SideBySide
          path={revision.path}
          left={before}
          right={after}
          labels={[revision.commit.parents.length ? "Prima" : "(file nuovo)", `Dopo (${revision.commit.shortHash})`]}
          collapse={onlyChanges}
        />
      {:catch failure}
        <p class="muted">{failure}</p>
      {/await}
    {/if}
  </div>
</div>

<style>
  .history {
    display: flex;
    height: 100%;
    font-family: var(--ui-font);
    font-size: 12px;
  }

  .revisions {
    flex: none;
    width: 320px;
    overflow: auto;
    border-right: 1px solid var(--border);
    outline: none;
  }

  .count {
    padding: 6px 10px;
    color: var(--fg-muted);
  }

  .revision {
    display: flex;
    flex-direction: column;
    padding: 3px 10px;
    cursor: pointer;
  }

  .revision:hover {
    background: var(--hover);
  }

  .revision.selected {
    background: var(--selection);
  }

  .subject,
  .meta {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta,
  .muted-inline {
    color: var(--fg-muted);
  }

  .hash {
    font-family: var(--mono);
    color: var(--fg-muted);
  }

  .view {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
    padding: 4px 10px;
    border-bottom: 1px solid var(--border);
  }

  .toolbar .subject {
    min-width: 0;
  }

  label {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: none;
    color: var(--fg-muted);
  }

  .spacer {
    flex: 1;
  }

  .muted {
    margin: 8px 10px;
    color: var(--fg-muted);
  }
</style>
