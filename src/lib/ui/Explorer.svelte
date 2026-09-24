<script lang="ts">
  import type { Entry } from "../backend";
  import type { ExplorerActions } from "../explorer/actions";
  import type { FileTree, Row } from "../explorer/tree.svelte";
  import { samePath } from "../workspace/files";
  import type { Workspace } from "../workspace/workspace.svelte";
  import type { FileStatus } from "../git/api";
  import ContextMenu, { type MenuItem } from "./ContextMenu.svelte";

  interface Props {
    tree: FileTree;
    workspace: Workspace;
    actions: ExplorerActions;
    onopenfolder: () => void;
    onclone: () => void;
    statusOf?: (path: string) => FileStatus | null;
    /** Voci aggiuntive del context menu (es. Annotate quando c'è un repository). */
    extraItems?: (entry: Entry) => MenuItem[];
  }

  let { tree, workspace, actions, onopenfolder, onclone, statusOf, extraItems }: Props = $props();

  const ROW_HEIGHT = 22;
  const OVERSCAN = 10;

  let list = $state<HTMLElement>();
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  let menu = $state<{ x: number; y: number; entry: Entry | null } | null>(null);

  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN));
  const visible = $derived(tree.rows.slice(first, first + Math.ceil(viewportHeight / ROW_HEIGHT) + OVERSCAN * 2));
  const selectedIndex = $derived(tree.rows.findIndex((row) => row.entry.path === tree.selected));
  const activePath = $derived(workspace.active?.path ?? null);

  function activate(row: Row) {
    tree.selected = row.entry.path;
    if (row.entry.isDir) tree.toggle(row.entry.path);
    else workspace.openPath(row.entry.path);
  }

  function select(index: number) {
    const row = tree.rows[index];
    if (!row || !list) return;
    tree.selected = row.entry.path;
    const top = index * ROW_HEIGHT;
    if (top < list.scrollTop) list.scrollTop = top;
    else if (top + ROW_HEIGHT > list.scrollTop + list.clientHeight) list.scrollTop = top + ROW_HEIGHT - list.clientHeight;
  }

  function parentIndex(index: number) {
    const depth = tree.rows[index].depth;
    for (let i = index - 1; i >= 0; i--) if (tree.rows[i].depth < depth) return i;
    return -1;
  }

  function onKeyDown(event: KeyboardEvent) {
    const row = tree.rows[selectedIndex];
    const handlers: Record<string, () => void> = {
      ArrowDown: () => select(Math.min(selectedIndex + 1, tree.rows.length - 1)),
      ArrowUp: () => select(Math.max(selectedIndex - 1, 0)),
      Home: () => select(0),
      End: () => select(tree.rows.length - 1),
      ArrowRight: () => {
        if (!row?.entry.isDir) return;
        if (row.expanded) select(selectedIndex + 1);
        else tree.expand(row.entry.path);
      },
      ArrowLeft: () => {
        if (!row) return;
        if (row.expanded) tree.collapse(row.entry.path);
        else select(parentIndex(selectedIndex));
      },
      Enter: () => row && activate(row),
      F2: () => row && actions.rename(row.entry),
      Delete: () => row && actions.remove(row.entry),
    };
    const handler = handlers[event.key];
    if (!handler || event.ctrlKey || event.altKey) return;
    event.preventDefault();
    if (selectedIndex === -1 && tree.rows.length) return select(0);
    handler();
  }

  function openMenu(event: MouseEvent, entry: Entry | null) {
    event.preventDefault();
    event.stopPropagation();
    if (entry) tree.selected = entry.path;
    menu = { x: event.clientX, y: event.clientY, entry };
  }

  function menuItems(entry: Entry | null): MenuItem[] {
    const dir = actions.targetDir(entry)!;
    const path = entry?.path ?? tree.root!;
    const items: MenuItem[] = [
      { label: "Nuovo file…", run: () => actions.newFile(dir) },
      { label: "Nuova cartella…", run: () => actions.newFolder(dir) },
    ];
    if (entry) {
      items.push(
        { label: "Rinomina…", run: () => actions.rename(entry), separatorBefore: true },
        { label: "Elimina", run: () => actions.remove(entry), danger: true },
      );
    }
    items.push(
      { label: "Mostra in Esplora risorse", run: () => actions.reveal(path), separatorBefore: true },
      { label: "Copia percorso", run: () => actions.copyPath(path) },
    );
    if (!entry) items.push({ label: "Aggiorna", run: () => tree.refresh(), separatorBefore: true });
    if (entry && extraItems) items.push(...extraItems(entry));
    return items;
  }
</script>

<aside class="explorer">
  <header class="header">
    <span class="title" title={tree.root}>{tree.rootName ?? "Explorer"}</span>
    {#if tree.root}
      <button title="Nuovo file" onclick={() => actions.newFile(actions.targetDir(tree.rows[selectedIndex]?.entry ?? null)!)}>+</button>
      <button title="Aggiorna" onclick={() => tree.refresh()}>⟳</button>
      <button title="Comprimi tutto" onclick={() => tree.collapseAll()}>⊟</button>
    {/if}
    <button title="Apri cartella" onclick={onopenfolder}>…</button>
  </header>

  {#if tree.root}
    <div
      class="list"
      role="tree"
      tabindex="0"
      bind:this={list}
      bind:clientHeight={viewportHeight}
      onscroll={() => (scrollTop = list!.scrollTop)}
      onkeydown={onKeyDown}
      oncontextmenu={(e) => openMenu(e, null)}
    >
      <div class="spacer" style:height="{tree.rows.length * ROW_HEIGHT}px">
        {#each visible as row, i (row.entry.path)}
          <div
            class="row"
            class:selected={row.entry.path === tree.selected}
            class:active={!row.entry.isDir && activePath !== null && samePath(row.entry.path, activePath)}
            role="treeitem"
            tabindex="-1"
            aria-selected={row.entry.path === tree.selected}
            aria-expanded={row.entry.isDir ? row.expanded : undefined}
            title={row.entry.path}
            style:transform="translateY({(first + i) * ROW_HEIGHT}px)"
            style:padding-left="{8 + row.depth * 14}px"
            onclick={() => activate(row)}
            onkeydown={() => {}}
            oncontextmenu={(e) => openMenu(e, row.entry)}
          >
            <span class="chevron">{row.entry.isDir ? (row.expanded ? "▾" : "▸") : ""}</span>
            <span class="name git-{statusOf?.(row.entry.path) ?? "clean"}" class:dir={row.entry.isDir}>{row.entry.name}</span>
          </div>
        {/each}
      </div>
    </div>
  {:else}
    <div class="empty">
      <p>Nessuna cartella aperta.</p>
      <button class="open" onclick={onopenfolder}>Apri cartella</button>
      <button class="open" onclick={onclone}>Clona repository…</button>
    </div>
  {/if}
</aside>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menuItems(menu.entry)} onclose={() => (menu = null)} />
{/if}

<style>
  .explorer {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--panel-bg);
  }

  .header {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: none;
    height: 30px;
    padding: 0 4px 0 12px;
  }

  .title {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--fg-muted);
  }

  .header button {
    padding: 0 6px;
    color: var(--fg-muted);
  }

  .list {
    flex: 1;
    overflow: auto;
    outline: none;
  }

  .spacer {
    position: relative;
  }

  .row {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 22px;
    display: flex;
    align-items: center;
    gap: 4px;
    padding-right: 8px;
    white-space: nowrap;
    cursor: pointer;
  }

  .row:hover {
    background: var(--hover);
  }

  .row.active .name {
    color: var(--accent);
  }

  .row.selected {
    background: var(--selection-match);
  }

  .list:focus .row.selected {
    background: var(--selection);
  }

  .chevron {
    flex: none;
    width: 12px;
    font-size: 10px;
    color: var(--fg-muted);
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .git-added {
    color: var(--git-added);
  }

  .git-modified {
    color: var(--git-modified);
  }

  .git-untracked,
  .git-conflict {
    color: var(--git-untracked);
  }

  .git-ignored {
    color: var(--git-ignored);
  }

  .empty {
    padding: 0 12px;
    color: var(--fg-muted);
  }

  .open {
    display: block;
    margin-bottom: 6px;
    border: 1px solid var(--border);
    padding: 4px 12px;
  }
</style>
