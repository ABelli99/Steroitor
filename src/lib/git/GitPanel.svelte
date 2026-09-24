<script lang="ts">
  import { untrack } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { buildBranchTree, flattenBranchTree, foldersToCollapse, type BranchNode } from "./branchTree";
  import ContextMenu, { type MenuItem } from "../ui/ContextMenu.svelte";
  import { gitLog, type Branch, type Commit } from "./api";
  import { absoluteTime, refLabel, relativeTime } from "./format";
  import { layoutGraph } from "./graph";
  import type { GitRepo } from "./repo.svelte";
  import { diffState, gitSelection } from "./selection.svelte";

  let { repo }: { repo: GitRepo } = $props();

  const PAGE_SIZE = 200;
  const FILTER_DELAY_MS = 300;
  const ROW_HEIGHT = 22;
  const LANE_WIDTH = 12;
  const MAX_LANES = 12;
  const LANE_COLORS = ["#4f8cf7", "#e5a50a", "#2ea043", "#d2527f", "#8b6cf6", "#1fb5b5", "#e0663a"];

  let reference = $state<string | null>(null);
  let text = $state("");
  let author = $state("");
  let path = $state("");
  let filters = $state({ text: "", author: "", path: "" });
  let commits = $state.raw<Commit[]>([]);
  let selected = $state<Commit | null>(null);
  let loading = $state(false);
  let complete = $state(false);
  let error = $state<string | null>(null);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  const local = $derived(repo.branches.filter((branch) => !branch.remote));
  const remote = $derived(repo.branches.filter((branch) => branch.remote));
  const filtered = $derived(Boolean(filters.text || filters.author || filters.path));
  const graph = $derived(filtered ? [] : layoutGraph(commits));
  const lanes = $derived(Math.min(MAX_LANES, Math.max(1, ...graph.map((row) => row.width))));
  const current = $derived(repo.branch?.head ?? null);
  const localTree = $derived(buildBranchTree(local, "local"));
  const remoteTree = $derived(buildBranchTree(remote, "remote"));
  const collapsed = new SvelteSet<string>();
  let collapsedOnce = false;

  $effect(() => {
    if (collapsedOnce || !repo.branches.length) return;
    collapsedOnce = true;
    const remoteNested = foldersToCollapse(remoteTree).filter((key) => key.split("/").length > 2);
    untrack(() => [...foldersToCollapse(localTree), ...remoteNested].forEach((key) => collapsed.add(key)));
  });

  function toggleFolder(key: string) {
    if (collapsed.has(key)) collapsed.delete(key);
    else collapsed.add(key);
  }

  const rowKey = (node: BranchNode) => (node.kind === "folder" ? node.key : `${node.branch.remote ? "r" : "l"}:${node.branch.name}`);

  const x = (lane: number) => lane * LANE_WIDTH + LANE_WIDTH / 2;
  const color = (index: number) => LANE_COLORS[index % LANE_COLORS.length];

  let generation = 0;

  $effect(() => {
    gitSelection.commit = selected?.hash ?? null;
  });

  $effect(() => {
    const next = { text: text.trim(), author: author.trim(), path: path.trim() };
    const unchanged = untrack(() => next.text === filters.text && next.author === filters.author && next.path === filters.path);
    if (unchanged) return;
    const timer = setTimeout(() => (filters = next), FILTER_DELAY_MS);
    return () => clearTimeout(timer);
  });

  $effect(() => {
    repo.revision;
    reference;
    filters;
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
    const requested = generation;
    loading = true;
    try {
      const page = await gitLog(repo.root, { reference, ...filters }, commits.length, PAGE_SIZE);
      if (requested !== generation) return;
      commits = [...commits, ...page];
      complete = page.length < PAGE_SIZE;
      error = null;
    } catch (failure) {
      if (requested === generation) error = String(failure);
    } finally {
      if (requested === generation) loading = false;
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

  function commitMenu(event: MouseEvent, commit: Commit) {
    event.preventDefault();
    selected = commit;
    const history = repo.history;
    const target = current ? `"${current}"` : "HEAD";
    menu = {
      x: event.clientX,
      y: event.clientY,
      items: [
        { label: "Copia commit ID", run: () => navigator.clipboard.writeText(commit.hash) },
        { label: "Mostra modifiche", hint: "Ctrl+D", run: () => (diffState.request = { kind: "commit", hash: commit.hash }) },
        { label: "Checkout (detached)", run: () => history.checkoutCommit(commit.hash), separatorBefore: true },
        { label: "Nuovo branch da qui…", run: () => history.branchFrom(commit.hash) },
        { label: "Crea tag…", run: () => history.tag(commit.hash) },
        { label: "Modifica messaggio…", run: () => history.reword(commit.hash) },
        { label: "Cherry-pick", run: () => history.cherryPick(commit.hash), separatorBefore: true },
        { label: "Revert", run: () => history.revert(commit.hash) },
        { label: `Reset ${target} qui`, disabled: true, separatorBefore: true, run: () => {} },
        { label: "Soft (tiene le modifiche in stage)", run: () => history.reset(commit.hash, "soft") },
        { label: "Mixed (tiene le modifiche, fuori stage)", run: () => history.reset(commit.hash, "mixed") },
        { label: "Hard (scarta tutto)…", danger: true, run: () => history.reset(commit.hash, "hard") },
      ],
    };
  }

  function branchMenu(event: MouseEvent, branch: Branch) {
    event.preventDefault();
    const history = repo.history;
    const items: MenuItem[] = [{ label: "Mostra log", run: () => (reference = branch.name) }];
    if (!branch.current) {
      items.push(
        { label: "Checkout", run: () => repo.checkout(branch), separatorBefore: true },
        { label: `Merge in ${current ?? "HEAD"}`, run: () => history.merge(branch.name) },
        { label: `Rebase ${current ?? "HEAD"} su ${branch.name}`, run: () => history.rebase(branch.name) },
      );
    }
    items.push({ label: "Nuovo branch da qui…", run: () => history.branchFrom(branch.name), separatorBefore: true });
    menu = { x: event.clientX, y: event.clientY, items };
  }
</script>

<div class="git">
  <nav class="branches">
    <button class="all" class:active={reference === null} onclick={() => (reference = null)}>Tutti i branch</button>
    {#if local.length}
      <h4>Locali</h4>
      {@render branchRows(localTree)}
    {/if}
    {#if remote.length}
      <h4>Remoti</h4>
      {@render branchRows(remoteTree)}
    {/if}
  </nav>

  {#snippet branchRows(tree: BranchNode[])}
    {#each flattenBranchTree(tree, collapsed) as row (rowKey(row.node))}
      {@const node = row.node}
      {#if node.kind === "folder"}
        <button class="folder" style:padding-left="{8 + row.depth * 12}px" onclick={() => toggleFolder(node.key)}>
          <span class="chevron">{collapsed.has(node.key) ? "▸" : "▾"}</span>{node.name}
        </button>
      {:else}
        <button
          class:active={reference === node.branch.name}
          class:current={node.branch.current}
          style:padding-left="{8 + row.depth * 12}px"
          title={node.branch.upstream ? `${node.branch.name} → ${node.branch.upstream}` : node.branch.name}
          onclick={() => (reference = node.branch.name)}
          oncontextmenu={(e) => branchMenu(e, node.branch)}
        >
          {node.name}
        </button>
      {/if}
    {/each}
  {/snippet}

  <div class="main">
    <div class="filters">
      <input placeholder="Testo nel messaggio" bind:value={text} spellcheck="false" />
      <input placeholder="Autore" bind:value={author} spellcheck="false" />
      <input placeholder="Path (es. src/)" bind:value={path} spellcheck="false" />
      {#if filtered}<span class="note">grafo nascosto con i filtri</span>{/if}
    </div>

    <div class="log" role="grid" tabindex="0" onscroll={onScroll} onkeydown={onKeyDown}>
      {#each commits as commit, index (commit.hash)}
        {@const row = graph[index]}
        <div
          id="commit-{commit.hash}"
          class="commit"
          class:selected={selected === commit}
          role="row"
          tabindex="-1"
          style:grid-template-columns="{filtered ? '' : `${lanes * LANE_WIDTH}px `}64px 1fr 140px 110px"
          onclick={() => (selected = commit)}
          ondblclick={() => (diffState.request = { kind: "commit", hash: commit.hash })}
          oncontextmenu={(e) => commitMenu(e, commit)}
          onkeydown={() => {}}
        >
          {#if row}
            <svg class="graph" width={lanes * LANE_WIDTH} height={ROW_HEIGHT} aria-hidden="true">
              {#each row.segments as segment, s (s)}
                <line
                  x1={x(segment.from)}
                  y1={segment.half === "top" ? 0 : ROW_HEIGHT / 2}
                  x2={x(segment.to)}
                  y2={segment.half === "top" ? ROW_HEIGHT / 2 : ROW_HEIGHT}
                  stroke={color(segment.color)}
                />
              {/each}
              <circle cx={x(row.lane)} cy={ROW_HEIGHT / 2} r={commit.parents.length > 1 ? 4 : 3.5} fill={color(row.color)} />
            </svg>
          {/if}
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

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

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

  .branches > * {
    flex-shrink: 0;
  }

  .branches button {
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .branches .folder {
    color: var(--fg-muted);
  }

  .chevron {
    display: inline-block;
    width: 12px;
    font-size: 10px;
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

  .main {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  .filters {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: none;
    padding: 4px 8px;
    border-bottom: 1px solid var(--border);
  }

  .filters input {
    width: 150px;
    font: inherit;
    color: var(--fg);
    background: var(--editor-bg);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 2px 6px;
    outline: none;
  }

  .filters input:focus {
    border-color: var(--accent);
  }

  .note {
    color: var(--fg-muted);
  }

  .log {
    flex: 1;
    min-height: 0;
    overflow: auto;
    outline: none;
  }

  .commit {
    display: grid;
    align-items: center;
    gap: 10px;
    height: 22px;
    padding: 0 8px;
    white-space: nowrap;
    cursor: pointer;
  }

  .commit:hover {
    background: var(--hover);
  }

  .commit.selected {
    background: var(--selection);
  }

  .graph {
    display: block;
    overflow: hidden;
  }

  .graph line {
    stroke-width: 1.5;
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
    line-height: 16px;
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
