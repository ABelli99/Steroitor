<script lang="ts">
  import { languageName } from "../editor/languages";
  import { eolLabel } from "../workspace/files";
  import type { GitRepo } from "../git/repo.svelte";
  import { useProject } from "../project/context";
  import type { Workspace } from "../workspace/workspace.svelte";

  interface Props {
    workspace: Workspace;
    repos: GitRepo[];
    selected: GitRepo | null;
    onvcsmenu: (event: MouseEvent, repo: GitRepo) => void;
  }

  let { workspace, repos, selected, onvcsmenu }: Props = $props();
  const { activity } = useProject();
  const tab = $derived(workspace.active);
  const several = $derived(repos.length > 1);

  const vcsTitle = (repo: GitRepo) => (several ? `Branch e operazioni Git di ${repo.root} (Alt+\` per il repository selezionato)` : "Branch e operazioni Git (Alt+`)");

  const branchLabel = (branch: NonNullable<GitRepo["branch"]>) => branch.head ?? `HEAD staccato (${branch.oid?.slice(0, 7) ?? "?"})`;
</script>

<footer class="status">
  {#each repos as repo (repo.root)}
    {#if repo.branch}
      {@const branch = repo.branch}
      <button
        class="vcs"
        class:selected={several && repo === selected}
        title={vcsTitle(repo)}
        onclick={(event) => onvcsmenu(event, repo)}
      >
        {#if several}<span class="repo">{repo.name}</span>{/if}
        ⎇ {branchLabel(branch)}{#if branch.ahead} ↑{branch.ahead}{/if}{#if branch.behind} ↓{branch.behind}{/if}
      </button>
      {#if repo.operation}
        <span class="operation">{repo.operation.toUpperCase()}{#if repo.conflicts} · {repo.conflicts} conflitti{/if}</span>
      {/if}
      {#if repo.busy}<span class="busy">{repo.busy}…</span>{/if}
    {/if}
  {/each}
  {#if activity.label}<span class="busy">{activity.label}…</span>{/if}
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
  <button title="Minimap" class:on={workspace.minimap} onclick={() => workspace.setMinimap(!workspace.minimap)}>Minimap</button>
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

  .operation {
    color: var(--danger);
    font-weight: 600;
  }

  .vcs {
    color: var(--fg);
  }

  .vcs.selected {
    box-shadow: inset 0 -2px 0 var(--accent);
  }

  .repo {
    color: var(--fg-muted);
  }

  .on {
    color: var(--accent);
  }
</style>
