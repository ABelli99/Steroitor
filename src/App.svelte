<script lang="ts">
  import { onMount, tick } from "svelte";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { message, open } from "@tauri-apps/plugin-dialog";
  import {
    activateProject, appReady, detachProject, locateProject, setWindowState, startupFiles, windowState, type WindowState,
  } from "./lib/backend";
  import type { ConsoleEntry } from "./lib/console/console.svelte";
  import { Project } from "./lib/project/project.svelte";
  import ProjectView from "./lib/project/ProjectView.svelte";
  import { commandOwner, moveTo } from "./lib/project/routing";
  import ContextMenu from "./lib/ui/ContextMenu.svelte";
  import { layout, saveLayout } from "./lib/ui/layout.svelte";
  import { closeMenu, menuState } from "./lib/ui/menu.svelte";
  import ProjectBar from "./lib/ui/ProjectBar.svelte";
  import PromptDialog from "./lib/ui/PromptDialog.svelte";
  import { readSession } from "./lib/workspace/session";

  const MAIN_WINDOW = "main";
  /** Il rilascio indica dove finisce la barra del titolo della nuova finestra, non il suo angolo. */
  const DETACH_OFFSET = { x: 80, y: 12 };

  const appWindow = getCurrentWebviewWindow();
  let projects = $state<Project[]>([]);
  let activeId = $state<string | null>(null);
  let ready = $state(false);

  const byId = (id: string | null) => projects.find((project) => project.id === id);

  $effect(() => {
    if (!ready) return;
    const state: WindowState = { projects: projects.map((project) => ({ id: project.id, folder: project.folder })), active: activeId };
    setWindowState(state).catch(console.error);
  });

  function activate(id: string) {
    const project = byId(id);
    if (!project) return;
    activeId = id;
    project.mounted = true;
    project.start().catch(console.error);
  }

  /** Prima della 1.1 la finestra aveva una sola sessione: diventa il primo progetto. */
  async function initialState(): Promise<{ projects: Project[]; active: string | null }> {
    const saved = await windowState().catch(() => null);
    if (saved) return { projects: saved.projects.map((ref) => new Project(ref.id, ref.folder)), active: saved.active };
    const legacy = await readSession(null);
    const project = new Project(undefined, legacy?.folder ?? null);
    return { projects: [project], active: project.id };
  }

  /** Se la cartella è già aperta in un progetto lo mostra, anche se sta in un'altra finestra. */
  async function reveal(folder: string) {
    const found = await locateProject(folder).catch(() => null);
    if (!found) return false;
    if (found.window === appWindow.label) activate(found.project);
    else await activateProject(found.window, found.project).catch(console.error);
    return true;
  }

  const pickFolder = async () => (await open({ directory: true })) ?? null;

  async function newProject() {
    const folder = await pickFolder();
    if (!folder || (await reveal(folder))) return;
    const project = new Project(undefined, folder);
    projects.push(project);
    activate(project.id);
  }

  async function openFolderIn(project: Project, folder?: string) {
    const target = folder ?? (await pickFolder());
    if (!target || (await reveal(target))) return;
    await project.openFolder(target);
    layout.explorerVisible = true;
    saveLayout();
  }

  async function closeProject(id: string) {
    const project = byId(id);
    if (!project || projects.length < 2) return;
    const index = projects.indexOf(project);
    if (activeId === id) activate((projects[index + 1] ?? projects[index - 1]).id);
    projects.splice(index, 1);
    await tick();
    await project.dispose();
  }

  async function detach(id: string, screenX: number, screenY: number) {
    const project = byId(id);
    if (!project || projects.length < 2) return;
    const ref = { id: project.id, folder: project.folder };
    await closeProject(id);
    await detachProject(ref, screenX - DETACH_OFFSET.x, screenY - DETACH_OFFSET.y).catch((error) =>
      message(String(error), { title: "Nuova finestra non riuscita", kind: "error" }),
    );
  }

  onMount(() => {
    const cleanups: Array<() => void> = [];
    const track = (listening: Promise<() => void>) => listening.then((unlisten) => cleanups.push(unlisten));

    track(appWindow.listen<{ project: string; dirs: string[] }>("fs-changed", (event) => byId(event.payload.project)?.onFilesChanged(event.payload.dirs)));
    track(appWindow.listen<{ project: string; repo: string }>("git-changed", (event) => byId(event.payload.project)?.onGitChanged(event.payload.repo)));
    track(appWindow.listen<ConsoleEntry>("git-command", (event) => commandOwner(projects, event.payload.cwd, activeId)?.console.push(event.payload)));
    track(appWindow.listen<string>("activate-project", (event) => activate(event.payload)));
    track(
      appWindow.listen<string[]>("open-files", async (event) => {
        const project = byId(activeId);
        for (const path of event.payload) await project?.workspace.openPath(path);
      }),
    );
    track(appWindow.onCloseRequested(() => Promise.all(projects.map((project) => project.flush())).then(() => {})));

    (async () => {
      const initial = await initialState();
      projects = initial.projects;
      activate(byId(initial.active) ? initial.active! : projects[0].id);
      ready = true;
      const project = byId(activeId)!;
      await project.start();
      if (appWindow.label === MAIN_WINDOW) for (const path of await startupFiles()) await project.workspace.openPath(path);
      project.workspace.focusEditor();
      appReady();
    })();

    return () => cleanups.forEach((cleanup) => cleanup());
  });
</script>

<div class="app">
  {#if projects.length > 1}
    <ProjectBar
      {projects}
      {activeId}
      onactivate={activate}
      onclose={closeProject}
      onmove={(id, toIndex) => (projects = moveTo(projects, id, toIndex))}
      ondetach={detach}
    />
  {/if}
  {#each projects as project (project.id)}
    {#if project.mounted}
      <ProjectView
        {project}
        active={project.id === activeId}
        onopenfolder={(folder) => openFolderIn(project, folder)}
        onnewproject={newProject}
      />
    {/if}
  {/each}
</div>

<PromptDialog />

{#if menuState.current}
  <ContextMenu x={menuState.current.x} y={menuState.current.y} items={menuState.current.items} onclose={closeMenu} />
{/if}

<style>
  .app {
    display: flex;
    height: 100%;
  }
</style>
