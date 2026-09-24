<script lang="ts">
  import { untrack } from "svelte";
  import { gitLog, type Commit } from "./api";
  import { absoluteTime, refLabel, relativeTime } from "./format";
  import type { GitRepo } from "./repo.svelte";

  let { repo }: { repo: GitRepo } = $props();

  const PAGE_SIZE = 200;

  let reference = $state<string | null>(null);
  let commits = $state.raw<Commit[]>([]);
  let selected = $state<Commit | null>(null);
  let loading = $state(false);
  let complete = $state(false);
  let error = $state<string | null>(null);

  const local = $derived(repo.branches.filter((branch) => !branch.remote));
  const remote = $derived(repo.branches.filter((branch) => branch.remote));

  let generation = 0;

  $effect(() => {
    repo.revision;
    reference;
    untrack(reload);
  });

  function reload() {
    generation += 1;
    commits = [];
    complete = false;
    loading = false;
    selected = null;
    loadMore();
  }

  async function loadMore() {
    if (loading || complete) return;
    const current = generation;
    loading = true;
    try {
      const page = await gitLog(repo.root, reference, commits.length, PAGE_SIZE);
      if (current !== generation) return;
      commits = [...commits, ...page];
      complete = page.length < PAGE_SIZE;
      error = null;
    } catch (failure) {
      if (current === generation) error = String(failure);
    } finally {
      if (current === generation) loading = false;
    }
  }

  function onScroll(event: Event) {
    const list = event.currentTarget as HTMLElement;
    if (list.scrollHeight - list.scrollTop - list.clientHeight < 200) loadMore();
  }

  function onKeyDown(event: KeyboardEvent) {
    const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
    if (!step || !commits.length) return;
    event.preventDefault();
    const index = selected ? commits.indexOf(selected) : -1;
    selected = commits[Math.min(Math.max(index + step, 0), commits.length - 1)];
    document.getElementById(`commit-${selected.hash}`)?.scrollIntoView({ block: "nearest" });
  }
</script>

<div class="git">
  <nav class="branches">
    <button class:active={reference === null} onclick={() => (reference = null)}>Tutti i branch</button>
    {#if local.length}
      <h4>Locali</h4>
      {#each local as branch (branch.name)}
        <button class:active={reference === branch.name} class:current={branch.current} title={branch.upstream ? `→ ${branch.upstream}` : branch.name} onclick={() => (reference = branch.name)}>
          {branch.name}
        </button>
      {/each}
    {/if}
    {#if remote.length}
      <h4>Remoti</h4>
      {#each remote as branch (branch.name)}
        <button class:active={reference === branch.name} onclick={() => (reference = branch.name)}>{branch.name}</button>
      {/each}
    {/if}
  </nav>

  <div class="log" role="grid" tabindex="0" onscroll={onScroll} onkeydown={onKeyDown}>
    {#each commits as commit (commit.hash)}
      <div
        id="commit-{commit.hash}"
        class="commit"
        class:selected={selected === commit}
        role="row"
        tabindex="-1"
        onclick={() => (selected = commit)}
        onkeydown={() => {}}
      >
        <span class="hash">{commit.shortHash}</span>
        <span class="subject">
          {#each commit.refs as ref (ref)}
            {@const decoration = refLabel(ref)}
            <span class="ref {decoration.kind}">{decoration.label}</span>
          {/each}
          {commit.subject}
        </span>
        <span class="author" title={commit.email}>{commit.author}</span>
        <span class="date" title={absoluteTime(commit.timestamp)}>{relativeTime(commit.timestamp)}</span>
      </div>
    {:else}
      <p class="empty">{error ?? (loading ? "Caricamento…" : "Nessun commit.")}</p>
    {/each}
    {#if loading && commits.length}<p class="empty">Caricamento…</p>{/if}
  </div>

  {#if selected}
    <aside class="details">
      <p class="subject-full">{selected.subject}</p>
      {#if selected.body}<pre>{selected.body}</pre>{/if}
      <dl>
        <dt>Commit</dt>
        <dd class="mono">{selected.hash}</dd>
        <dt>Autore</dt>
        <dd>{selected.author} &lt;{selected.email}&gt;</dd>
        <dt>Data</dt>
        <dd>{absoluteTime(selected.timestamp)}</dd>
        {#if selected.parents.length}
          <dt>{selected.parents.length > 1 ? "Parent (merge)" : "Parent"}</dt>
          <dd class="mono">{selected.parents.map((parent) => parent.slice(0, 7)).join(", ")}</dd>
        {/if}
      </dl>
    </aside>
  {/if}
</div>

<style>
  .git {
    display: flex;
    height: 100%;
    font-family: var(--ui-font);
    font-size: 12px;
  }

  .branches {
    flex: none;
    width: 180px;
    overflow: auto;
    padding: 4px;
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--border);
  }

  .branches button {
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .branches .active {
    background: var(--selection);
  }

  .branches .current::before {
    content: "● ";
    color: var(--accent);
  }

  h4 {
    margin: 8px 8px 2px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    color: var(--fg-muted);
  }

  .log {
    flex: 1;
    min-width: 0;
    overflow: auto;
    outline: none;
  }

  .commit {
    display: grid;
    grid-template-columns: 64px 1fr 140px 110px;
    gap: 10px;
    padding: 2px 8px;
    white-space: nowrap;
    cursor: pointer;
  }

  .commit:hover {
    background: var(--hover);
  }

  .commit.selected {
    background: var(--selection);
  }

  .hash,
  .mono {
    font-family: var(--mono);
  }

  .hash,
  .author,
  .date {
    color: var(--fg-muted);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .subject {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ref {
    display: inline-block;
    margin-right: 4px;
    padding: 0 5px;
    border-radius: 3px;
    font-size: 11px;
    border: 1px solid var(--border);
  }

  .ref.head {
    border-color: var(--accent);
    color: var(--accent);
  }

  .ref.tag {
    border-color: var(--git-added);
    color: var(--git-added);
  }

  .details {
    flex: none;
    width: 300px;
    overflow: auto;
    padding: 8px 12px;
    border-left: 1px solid var(--border);
    user-select: text;
  }

  .subject-full {
    margin: 0 0 6px;
    font-weight: 600;
  }

  pre {
    margin: 0 0 8px;
    white-space: pre-wrap;
    font: inherit;
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 2px 8px;
    margin: 0;
  }

  dt {
    color: var(--fg-muted);
  }

  dd {
    margin: 0;
    word-break: break-all;
  }

  .empty {
    margin: 8px;
    color: var(--fg-muted);
  }
</style>
