<script lang="ts">
  import { fileName } from "../workspace/files";
  import { gitCommitChanges, gitFileAt, type ChangedFile } from "./api";
  import SideBySide from "./SideBySide.svelte";

  interface Props {
    root: string;
    hash: string;
    onopenfile: (path: string) => void;
  }

  let { root, hash, onopenfile }: Props = $props();

  const STATUS_CLASS: Record<string, string> = { A: "added", M: "modified", D: "deleted", R: "modified", C: "added", T: "modified" };

  let files = $state.raw<ChangedFile[]>([]);
  let selected = $state<ChangedFile | null>(null);
  let onlyChanges = $state(false);
  let error = $state<string | null>(null);

  const relativeDir = (path: string) => path.slice(root.length + 1, -fileName(path).length).replace(/[\\/]$/, "");

  $effect(() => {
    gitCommitChanges(root, hash)
      .then((changes) => {
        files = changes;
        selected = changes[0] ?? null;
      })
      .catch((failure) => (error = String(failure)));
  });

  async function contents(file: ChangedFile) {
    const before = file.status === "A" ? null : await gitFileAt(root, `${hash}^`, file.oldPath ?? file.path);
    const after = file.status === "D" ? null : await gitFileAt(root, hash, file.path);
    return { before: before ?? "", after: after ?? "" };
  }

  function onKeyDown(event: KeyboardEvent) {
    const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
    if (!step || !files.length) return;
    event.preventDefault();
    const index = selected ? files.indexOf(selected) : -1;
    selected = files[Math.min(Math.max(index + step, 0), files.length - 1)];
  }
</script>

<div class="changes">
  <div class="files" role="listbox" tabindex="0" onkeydown={onKeyDown}>
    <div class="count">{files.length === 1 ? "1 file cambiato" : `${files.length} file cambiati`}</div>
    {#each files as file (file.path)}
      <div
        class="file"
        class:selected={file === selected}
        role="option"
        tabindex="-1"
        aria-selected={file === selected}
        title={file.oldPath ? `${file.oldPath} → ${file.path}` : file.path}
        onclick={() => (selected = file)}
        onkeydown={() => {}}
      >
        <span class="letter git-{STATUS_CLASS[file.status]}">{file.status}</span>
        <span class="name git-{STATUS_CLASS[file.status]}">{fileName(file.path)}</span>
        <span class="dir">{relativeDir(file.path)}</span>
      </div>
    {:else}
      <p class="muted">{error ?? "Caricamento…"}</p>
    {/each}
  </div>

  <div class="view">
    {#if selected}
      {@const file = selected}
      <div class="toolbar">
        <span class="path">{relativeDir(file.path)}{relativeDir(file.path) ? "/" : ""}{fileName(file.path)}</span>
        <span class="spacer"></span>
        <label><input type="checkbox" bind:checked={onlyChanges} /> Solo le zone cambiate</label>
        {#if file.status !== "D"}<button onclick={() => onopenfile(file.path)}>Apri nell'editor</button>{/if}
      </div>
      {#await contents(file) then { before, after }}
        <SideBySide path={file.path} left={before} right={after} labels={["Prima", `Dopo (${hash.slice(0, 7)})`]} collapse={onlyChanges} />
      {:catch failure}
        <p class="muted">{failure}</p>
      {/await}
    {/if}
  </div>
</div>

<style>
  .changes {
    display: flex;
    height: 100%;
    font-family: var(--ui-font);
    font-size: 12px;
  }

  .files {
    flex: none;
    width: 280px;
    overflow: auto;
    border-right: 1px solid var(--border);
    outline: none;
  }

  .count {
    padding: 6px 10px;
    color: var(--fg-muted);
  }

  .file {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 2px 10px;
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

  .toolbar button {
    border: 1px solid var(--border);
  }

  label {
    display: flex;
    align-items: center;
    gap: 4px;
    color: var(--fg-muted);
  }

  .path {
    font-family: var(--mono);
  }

  .spacer {
    flex: 1;
  }

  .muted {
    margin: 8px 10px;
    color: var(--fg-muted);
  }
</style>
