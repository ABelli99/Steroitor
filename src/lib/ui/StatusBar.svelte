<script lang="ts">
  import { languageName } from "../editor/languages";
  import { eolLabel } from "../workspace/files";
  import type { GitRepo } from "../git/repo.svelte";
  import type { Workspace } from "../workspace/workspace.svelte";

  interface Props {
    workspace: Workspace;
    git: GitRepo | null;
    onvcsmenu: (event: MouseEvent) => void;
  }

  let { workspace, git, onvcsmenu }: Props = $props();
  const tab = $derived(workspace.active);
  const branch = $derived(git?.branch ?? null);
  const branchLabel = $derived(branch ? (branch.head ?? `HEAD staccato (${branch.oid?.slice(0, 7) ?? "?"})`) : null);
</script>

<footer class="status">
  {#if branch}
    <button class="vcs" title="Branch e operazioni Git (Alt+`)" onclick={onvcsmenu}>
      ⎇ {branchLabel}{#if branch.ahead} ↑{branch.ahead}{/if}{#if branch.behind} ↓{branch.behind}{/if}
    </button>
    {#if git?.busy}<span class="busy">{git.busy}…</span>{/if}
  {/if}
  <span class="spacer"></span>
  {#if tab}
    <span>
      Ln {workspace.cursor.line}, Col {workspace.cursor.column}
      {#if workspace.cursor.selected}({workspace.cursor.selected} selezionati){/if}
    </span>
    <button title="Cambia fine riga" onclick={() => workspace.setEol(tab.eol === "\r\n" ? "\n" : "\r\n")}>
      {eolLabel(tab.eol)}
    </button>
    <span>{tab.encoding}{tab.bom ? " BOM" : ""}</span>
    <span>{languageName(tab.language)}</span>
  {/if}
  <button title="A capo automatico (Alt+Z)" class:on={workspace.wrap} onclick={() => workspace.toggleWrap()}>
    A capo
  </button>
</footer>

<style>
  .status {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: none;
    height: 24px;
    padding: 0 8px;
    font-size: 12px;
    color: var(--fg-muted);
    background: var(--panel-bg);
    border-top: 1px solid var(--border);
  }

  .status > span {
    padding: 0 8px;
  }

  .spacer {
    flex: 1;
  }

  .busy {
    color: var(--accent);
  }

  .vcs {
    color: var(--fg);
  }

  .on {
    color: var(--accent);
  }
</style>
