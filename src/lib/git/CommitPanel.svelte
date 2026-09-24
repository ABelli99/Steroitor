<script lang="ts">
  import { fileName } from "../workspace/files";
  import { OPERATION_LABELS, type StatusEntry } from "./api";
  import DiffView from "./DiffView.svelte";
  import type { GitRepo } from "./repo.svelte";
  import { diffState, gitSelection } from "./selection.svelte";
  import { commitDraft, submitCommit } from "./commitDraft.svelte";

  let { repo }: { repo: GitRepo } = $props();

  const LETTERS: Record<string, string> = { added: "A", modified: "M", deleted: "D", conflict: "C", untracked: "U" };

  const changes = $derived([...repo.changes].sort((a, b) => a.path.localeCompare(b.path)));
  const allStaged = $derived(changes.length > 0 && changes.every((entry) => entry.staged && !entry.unstaged));
  const someStaged = $derived(changes.some((entry) => entry.staged));
  const selected = $derived(changes.find((entry) => entry.path === gitSelection.file?.path) ?? null);
  const hasMessage = $derived(commitDraft.message.trim().length > 0);
  const canCommit = $derived(!repo.busy && hasMessage && (commitDraft.amend || repo.staged > 0));
  const operation = $derived(repo.operation ? OPERATION_LABELS[repo.operation] : null);

  const relativeDir = (path: string) => path.slice(repo.root.length + 1, -fileName(path).length).replace(/[\\/]$/, "");

  function toggle(entry: StatusEntry) {
    if (entry.staged && !entry.unstaged) return repo.unstage([entry.path]);
    return repo.stage([entry.path]);
  }

  function toggleAll() {
    const paths = changes.map((entry) => entry.path);
    if (allStaged) return repo.unstage(paths);
    return repo.stage(paths);
  }

  function select(entry: StatusEntry) {
    gitSelection.file = { path: entry.path, status: entry.status };
  }

  async function commit(andPush: boolean) {
    if (canCommit) await submitCommit(repo, andPush);
  }

  async function toggleAmend() {
    commitDraft.amend = !commitDraft.amend;
    if (commitDraft.amend && !hasMessage) commitDraft.message = await repo.history.lastCommitMessage();
  }

  function onMessageKeyDown(event: KeyboardEvent) {
    if (event.key === "Enter" && event.ctrlKey) {
      event.preventDefault();
      commit(event.altKey);
    }
  }

  function onListKeyDown(event: KeyboardEvent) {
    const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
    if (event.key === " " && selected) {
      event.preventDefault();
      toggle(selected);
      return;
    }
    if (!step || !changes.length) return;
    event.preventDefault();
    const index = selected ? changes.indexOf(selected) : -1;
    select(changes[Math.min(Math.max(index + step, 0), changes.length - 1)]);
  }
</script>

<div class="commit-panel">
  <div class="left">
    <div class="list-header">
      <label>
        <input
          type="checkbox"
          checked={allStaged}
          indeterminate={someStaged && !allStaged}
          disabled={!changes.length || !!repo.busy}
          onchange={toggleAll}
        />
        Modifiche ({changes.length}) · {repo.staged} in stage
      </label>
    </div>
    <div class="list" role="listbox" tabindex="0" onkeydown={onListKeyDown}>
      {#each changes as entry (entry.path)}
        <div
          class="file"
          class:selected={entry === selected}
          role="option"
          tabindex="-1"
          aria-selected={entry === selected}
          title={entry.path}
          onclick={() => select(entry)}
          ondblclick={() => (diffState.request = { kind: "file", file: { path: entry.path, status: entry.status } })}
          onkeydown={() => {}}
        >
          <input
            type="checkbox"
            checked={entry.staged && !entry.unstaged}
            indeterminate={entry.staged && entry.unstaged}
            disabled={!!repo.busy}
            onclick={(e) => e.stopPropagation()}
            onchange={() => toggle(entry)}
          />
          <span class="letter git-{entry.status}">{LETTERS[entry.status] ?? "?"}</span>
          <span class="name git-{entry.status}">{fileName(entry.path)}</span>
          <span class="dir">{relativeDir(entry.path)}</span>
        </div>
      {:else}
        <p class="empty">Nessuna modifica.</p>
      {/each}
    </div>
    {#if operation}
      <div class="operation">
        <strong>{operation} in corso</strong>
        {#if repo.conflicts}· {repo.conflicts} file in conflitto: risolvili e mettili in stage{:else}· conflitti risolti{/if}
      </div>
      <div class="actions">
        <button class="primary" disabled={!!repo.busy || repo.conflicts > 0} onclick={() => repo.history.continueOperation()}>
          Continua {operation.toLowerCase()}
        </button>
        <button disabled={!!repo.busy} onclick={() => repo.history.abortOperation()}>Annulla {operation.toLowerCase()}</button>
        <span class="spacer"></span>
        {#if repo.busy}<span class="busy">{repo.busy}…</span>{/if}
      </div>
    {:else}
    <textarea
      id="commit-message"
      placeholder="Messaggio di commit (Ctrl+Invio per committare)"
      bind:value={commitDraft.message}
      onkeydown={onMessageKeyDown}
      spellcheck="false"
    ></textarea>
    <div class="actions">
      <button class="primary" disabled={!canCommit} title="Ctrl+K" onclick={() => commit(false)}>
        {commitDraft.amend ? "Amend" : "Commit"}
      </button>
      {#if !commitDraft.amend}
        <button disabled={!canCommit} title="Ctrl+Alt+K" onclick={() => commit(true)}>Commit and Push</button>
      {/if}
      <label class="amend" title="Modifica l'ultimo commit invece di crearne uno nuovo">
        <input type="checkbox" checked={commitDraft.amend} onchange={toggleAmend} /> Amend
      </label>
      <span class="spacer"></span>
      {#if repo.busy}<span class="busy">{repo.busy}…</span>{/if}
    </div>
    {/if}
  </div>

  <div class="preview">
    {#if selected}
      <DiffView root={repo.root} file={{ path: selected.path, status: selected.status }} />
    {:else}
      <p class="empty">Seleziona un file per vedere le modifiche.</p>
    {/if}
  </div>
</div>

<style>
  .commit-panel {
    display: flex;
    height: 100%;
    font-family: var(--ui-font);
    font-size: 12px;
  }

  .left {
    display: flex;
    flex-direction: column;
    flex: none;
    width: min(420px, 45%);
    border-right: 1px solid var(--border);
  }

  .list-header {
    flex: none;
    padding: 4px 8px;
    color: var(--fg-muted);
    border-bottom: 1px solid var(--border);
  }

  label {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .list {
    flex: 1;
    min-height: 40px;
    overflow: auto;
    outline: none;
  }

  .file {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 1px 8px;
    white-space: nowrap;
    cursor: pointer;
  }

  .file:hover {
    background: var(--hover);
  }

  .file.selected {
    background: var(--selection);
  }

  .letter {
    width: 10px;
    font-family: var(--mono);
    font-weight: 600;
  }

  .dir {
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--fg-muted);
  }

  .git-added {
    color: var(--git-added);
  }

  .git-modified {
    color: var(--git-modified);
  }

  .git-deleted {
    color: var(--fg-muted);
    text-decoration: line-through;
  }

  .git-untracked,
  .git-conflict {
    color: var(--git-untracked);
  }

  textarea {
    flex: none;
    height: 64px;
    margin: 6px 8px 0;
    padding: 6px 8px;
    resize: vertical;
    font: inherit;
    font-family: var(--mono);
    color: var(--fg);
    background: var(--editor-bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    outline: none;
  }

  textarea:focus {
    border-color: var(--accent);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: none;
    padding: 6px 8px;
  }

  .actions button {
    border: 1px solid var(--border);
    padding: 3px 10px;
  }

  .actions .primary:not(:disabled) {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .spacer {
    flex: 1;
  }

  .busy {
    color: var(--fg-muted);
  }

  .amend {
    margin-left: 6px;
    color: var(--fg-muted);
  }

  .operation {
    margin: 6px 8px 0;
    padding: 6px 8px;
    border-radius: 4px;
    color: var(--fg);
    background: var(--search-match);
  }

  .preview {
    flex: 1;
    min-width: 0;
  }

  .empty {
    margin: 8px;
    color: var(--fg-muted);
  }
</style>
