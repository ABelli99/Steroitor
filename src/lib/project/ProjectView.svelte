<script lang="ts">
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import type { Entry } from "../backend";
  import { jumpToSource } from "../git/jumpToSource";
  import { gitShortcuts } from "../git/shortcuts";
  import { vcsMenuItems } from "../git/vcsMenu";
  import { installShortcuts } from "../shortcuts";
  import { openTerminal } from "../terminal/openTerminal";
  import BottomPanel from "../ui/BottomPanel.svelte";
  import type { MenuItem } from "../ui/ContextMenu.svelte";
  import Editor from "../ui/Editor.svelte";
  import Explorer from "../ui/Explorer.svelte";
  import { clamp, layout, saveLayout } from "../ui/layout.svelte";
  import { openMenu } from "../ui/menu.svelte";
  import { showPanel } from "../ui/panels";
  import QuickOpen from "../ui/QuickOpen.svelte";
  import Splitter from "../ui/Splitter.svelte";
  import StatusBar from "../ui/StatusBar.svelte";
  import TabBar from "../ui/TabBar.svelte";
  import TopBar from "../ui/TopBar.svelte";
  import { samePath } from "../workspace/files";
  import { provideProject } from "./context";
  import type { Project } from "./project.svelte";

  interface Props {
    project: Project;
    active: boolean;
    onopenfolder: (folder?: string) => void;
    onnewproject: () => void;
  }

  let { project, active, onopenfolder, onnewproject }: Props = $props();
  // La vista è keyed per id: resta legata allo stesso progetto per tutta la sua vita.
  // svelte-ignore state_referenced_locally
  provideProject(project);

  const { workspace, tree, actions } = $derived(project);
  const git = $derived(project.git);
  const mergeRepo = $derived(project.repos.find((repo) => project.diff.request && samePath(repo.root, project.diff.request.root)) ?? null);
  let quickOpen = $state(false);
  let cloneOpen = $state(false);

  $effect(() => {
    if (!active) return;
    const tab = workspace.active;
    const title = tab ? `${tab.dirty ? "● " : ""}${tab.name} — Steroitor` : "Steroitor";
    getCurrentWebviewWindow().setTitle(title);
  });

  $effect(() => {
    if (!active) {
      quickOpen = false;
      return;
    }
    requestAnimationFrame(() => workspace.focusEditor());
    return installShortcuts({
      "Ctrl+N": { editor: () => workspace.newUntitled() },
      "Ctrl+O": { editor: () => workspace.openDialog() },
      "Ctrl+S": { editor: () => workspace.save() },
      "Ctrl+Shift+S": { editor: () => workspace.saveAs() },
      "Ctrl+W": { editor: () => workspace.close() },
      "Ctrl+Tab": { editor: () => workspace.cycle(1) },
      "Ctrl+Shift+Tab": { editor: () => workspace.cycle(-1) },
      "Alt+Z": { editor: () => workspace.toggleWrap() },
      "Alt+1": { editor: toggleExplorer, git: toggleExplorer, terminal: toggleExplorer },
      "Ctrl+P": { editor: () => (quickOpen = !quickOpen), git: () => (quickOpen = !quickOpen) },
      ...gitShortcuts({
        repo: () => project.git,
        repoFor: (path) => project.repoFor(path),
        workspace,
        commitDraft: project.commitDraft,
        selection: project.selection,
        diff: project.diff,
        openVcsMenu: () => openVcsMenu(),
        openTerminal: () => openTerminal(project.terminals, project.root),
      }),
    });
  });

  function toggleExplorer() {
    layout.explorerVisible = !layout.explorerVisible;
    saveLayout();
  }

  async function onCloned(folder: string) {
    cloneOpen = false;
    onopenfolder(folder);
  }

  function openVcsMenu(x = 8, y = window.innerHeight - 28, repo = git) {
    if (!repo) return;
    project.select(repo);
    const open = (items: MenuItem[]) => openMenu(x, y, items);
    open(vcsMenuItems(repo, { openCommit: () => showPanel("commit", "#commit-message"), open }));
  }

  function blameItem(tabId: string, path: string): MenuItem[] {
    const repo = project.repoFor(path);
    if (!repo) return [];
    const annotated = repo.annotated.has(tabId);
    return [{ label: annotated ? "Chiudi annotazioni" : "Annotate con Git Blame", run: () => repo.toggleBlame(tabId, path) }];
  }

  function openGutterMenu(event: MouseEvent) {
    const tab = workspace.active;
    const items = tab?.path ? blameItem(tab.id, tab.path) : [];
    openMenu(event.clientX, event.clientY, items);
  }

  function explorerGitItems(entry: Entry): MenuItem[] {
    const repo = entry.isDir ? null : project.repoFor(entry.path);
    if (!repo) return [];
    const annotate = async () => {
      await workspace.openPath(entry.path);
      const tab = workspace.tabs.find((candidate) => candidate.path && samePath(candidate.path, entry.path));
      if (tab && !repo.annotated.has(tab.id)) repo.toggleBlame(tab.id, entry.path);
    };
    return [{ label: "Annotate con Git Blame", run: annotate, separatorBefore: true }];
  }

  function closeQuickOpen() {
    quickOpen = false;
    workspace.focusEditor();
  }
</script>

<div class="project" class:hidden={!active} data-active-project={active ? "" : undefined}>
  <TopBar {workspace} onopenfolder={() => onopenfolder()} {onnewproject} onclone={() => (cloneOpen = true)} />
  <div class="main">
    {#if layout.explorerVisible}
      <div class="explorer" style:width="{layout.explorerWidth}px">
        <Explorer
          {tree}
          {workspace}
          {actions}
          onopenfolder={() => onopenfolder()}
          onclone={() => (cloneOpen = true)}
          statusOf={project.repos.length ? (path) => project.statusOf(path) : undefined}
          extraItems={explorerGitItems}
        />
      </div>
      <Splitter
        direction="horizontal"
        ondrag={(delta) => (layout.explorerWidth = clamp(layout.explorerWidth + delta, 140, 600))}
        onend={saveLayout}
      />
    {/if}
    <div class="center">
      <TabBar {workspace} />
      <Editor {workspace} ongutterMenu={openGutterMenu} />
    </div>
  </div>
  <div class="panel-area" class:hidden={!layout.panelVisible}>
    <Splitter
      direction="vertical"
      ondrag={(delta) => (layout.panelHeight = clamp(layout.panelHeight - delta, 80, window.innerHeight - 200))}
      onend={saveLayout}
    />
    <div class="bottom" style:height="{layout.panelHeight}px">
      <BottomPanel {git} folder={tree.root} oninit={() => project.initRepository()} onclone={() => (cloneOpen = true)} />
    </div>
  </div>
  <StatusBar {workspace} repos={project.repos} selected={git} onvcsmenu={(event, repo) => openVcsMenu(event.clientX, event.clientY, repo)} />

  {#if cloneOpen}
    {#await import("../git/CloneDialog.svelte") then { default: CloneDialog }}
      <CloneDialog onclose={() => (cloneOpen = false)} oncloned={onCloned} />
    {/await}
  {/if}

  {#if quickOpen}
    <QuickOpen {tree} {workspace} onclose={closeQuickOpen} />
  {/if}

  {#if project.diff.request?.kind === "merge" && mergeRepo}
    {#await import("../git/MergeTool.svelte") then { default: MergeTool }}
      <MergeTool repo={mergeRepo} path={project.diff.request.path} onclose={() => (project.diff.request = null)} />
    {/await}
  {:else if project.diff.request}
    {#await import("../git/DiffDialog.svelte") then { default: DiffDialog }}
      <DiffDialog
        root={project.diff.request.root}
        request={project.diff.request}
        onclose={() => (project.diff.request = null)}
        onopenfile={(path) => {
          project.diff.request = null;
          workspace.openPath(path);
        }}
        onjump={(file) => {
          const root = project.diff.request?.root;
          project.diff.request = null;
          if (root) jumpToSource(workspace, root, file);
        }}
      />
    {/await}
  {/if}
</div>

<style>
  .project {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    height: 100%;
  }

  .main {
    display: flex;
    flex: 1;
    min-height: 0;
  }

  .explorer {
    flex: none;
  }

  .center {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  .bottom {
    flex: none;
  }

  .panel-area {
    display: flex;
    flex-direction: column;
    flex: none;
  }

  .hidden {
    display: none;
  }
</style>
