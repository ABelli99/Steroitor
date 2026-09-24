<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open } from "@tauri-apps/plugin-dialog";
  import { appReady, startupFiles } from "./lib/backend";
  import { installShortcuts } from "./lib/shortcuts";
  import { Workspace } from "./lib/workspace/workspace.svelte";
  import { FileTree } from "./lib/explorer/tree.svelte";
  import { ExplorerActions } from "./lib/explorer/actions";
  import { persistSession, readSession } from "./lib/workspace/session";
  import { clamp, layout, saveLayout } from "./lib/ui/layout.svelte";
  import TopBar from "./lib/ui/TopBar.svelte";
  import Explorer from "./lib/ui/Explorer.svelte";
  import TabBar from "./lib/ui/TabBar.svelte";
  import Editor from "./lib/ui/Editor.svelte";
  import BottomPanel from "./lib/ui/BottomPanel.svelte";
  import StatusBar from "./lib/ui/StatusBar.svelte";
  import Splitter from "./lib/ui/Splitter.svelte";
  import QuickOpen from "./lib/ui/QuickOpen.svelte";
  import PromptDialog from "./lib/ui/PromptDialog.svelte";

  const workspace = new Workspace();
  const tree = new FileTree();
  const actions = new ExplorerActions(tree, workspace);
  let quickOpen = $state(false);
  const appWindow = getCurrentWindow();

  $effect(() => {
    const tab = workspace.active;
    const title = tab ? `${tab.dirty ? "● " : ""}${tab.name} — Steroitor` : "Steroitor";
    appWindow.setTitle(title);
  });

  const toggleExplorer = () => {
    layout.explorerVisible = !layout.explorerVisible;
    saveLayout();
  };

  async function openFolder() {
    const folder = await open({ directory: true });
    if (!folder) return;
    await tree.open(folder);
    layout.explorerVisible = true;
    saveLayout();
  }

  function closeQuickOpen() {
    quickOpen = false;
    workspace.focusEditor();
  }

  onMount(() => {
    const session = persistSession(workspace, tree);
    const cleanups: Array<() => void> = [session.stop];

    cleanups.push(
      installShortcuts({
        "Ctrl+N": { editor: () => workspace.newUntitled() },
        "Ctrl+T": { editor: () => workspace.newUntitled() },
        "Ctrl+O": { editor: () => workspace.openDialog() },
        "Ctrl+S": { editor: () => workspace.save() },
        "Ctrl+Shift+S": { editor: () => workspace.saveAs() },
        "Ctrl+W": { editor: () => workspace.close() },
        "Ctrl+Tab": { editor: () => workspace.cycle(1) },
        "Ctrl+Shift+Tab": { editor: () => workspace.cycle(-1) },
        "Alt+Z": { editor: () => workspace.toggleWrap() },
        "Alt+1": { editor: toggleExplorer, git: toggleExplorer },
        "Ctrl+P": { editor: () => (quickOpen = !quickOpen), git: () => (quickOpen = !quickOpen) },
      }),
    );

    (async () => {
      const saved = await readSession();
      if (saved) await workspace.restore(saved);
      if (saved?.folder) await tree.open(saved.folder, saved.expanded);
      for (const path of await startupFiles()) await workspace.openPath(path);
      if (workspace.tabs.length === 0) workspace.newUntitled();
      workspace.focusEditor();
      appReady();
    })();

    listen<string[]>("fs-changed", (event) => tree.refresh(event.payload)).then((unlisten) => cleanups.push(unlisten));

    listen<string[]>("open-files", async (event) => {
      for (const path of event.payload) await workspace.openPath(path);
    }).then((unlisten) => cleanups.push(unlisten));

    appWindow.onCloseRequested(() => session.flush()).then((unlisten) => cleanups.push(unlisten));

    return () => cleanups.forEach((cleanup) => cleanup());
  });
</script>

<div class="app">
  <TopBar {workspace} onopenfolder={openFolder} />
  <div class="main">
    {#if layout.explorerVisible}
      <div class="explorer" style:width="{layout.explorerWidth}px">
        <Explorer {tree} {workspace} {actions} onopenfolder={openFolder} />
      </div>
      <Splitter
        direction="horizontal"
        ondrag={(delta) => (layout.explorerWidth = clamp(layout.explorerWidth + delta, 140, 600))}
        onend={saveLayout}
      />
    {/if}
    <div class="center">
      <TabBar {workspace} />
      <Editor {workspace} />
    </div>
  </div>
  {#if layout.panelVisible}
    <Splitter
      direction="vertical"
      ondrag={(delta) => (layout.panelHeight = clamp(layout.panelHeight - delta, 80, window.innerHeight - 200))}
      onend={saveLayout}
    />
    <div class="bottom" style:height="{layout.panelHeight}px">
      <BottomPanel />
    </div>
  {/if}
  <StatusBar {workspace} />
</div>

{#if quickOpen}
  <QuickOpen {tree} {workspace} onclose={closeQuickOpen} />
{/if}
<PromptDialog />

<style>
  .app {
    display: flex;
    flex-direction: column;
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
</style>
