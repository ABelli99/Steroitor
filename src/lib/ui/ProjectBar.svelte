<script lang="ts">
  import type { Project } from "../project/project.svelte";
  import { folderInitial, folderName } from "./projects";
  import icon from "../../../src-tauri/icons/32x32.png";

  interface Props {
    projects: Project[];
    activeId: string | null;
    onactivate: (id: string) => void;
    onclose: (id: string) => void;
    /** `toIndex` è la posizione d'inserimento nell'elenco attuale (0…length). */
    onmove: (id: string, toIndex: number) => void;
    ondetach: (id: string, screenX: number, screenY: number) => void;
  }

  let { projects, activeId, onactivate, onclose, onmove, ondetach }: Props = $props();

  const DRAG_THRESHOLD = 4;
  /** Oltre questa distanza dal bordo della barra il rilascio stacca il progetto. */
  const DETACH_MARGIN = 24;

  interface Drag {
    id: string;
    startX: number;
    startY: number;
    x: number;
    y: number;
    moved: boolean;
    outside: boolean;
    target: number;
  }

  let bar = $state<HTMLElement>();
  let hint = $state<{ text: string; top: number } | null>(null);
  let drag = $state<Drag | null>(null);
  let suppressClick = false;

  function showHint(event: MouseEvent, project: Project) {
    if (drag?.moved) return;
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    hint = { text: folderName(project.folder), top: box.top + box.height / 2 };
  }

  function onPointerDown(event: PointerEvent, project: Project) {
    if (event.button !== 0) return;
    suppressClick = false;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    const { clientX: x, clientY: y } = event;
    drag = { id: project.id, startX: x, startY: y, x, y, moved: false, outside: false, target: 0 };
  }

  function isOutside(x: number, y: number) {
    const box = bar!.getBoundingClientRect();
    return x > box.right + DETACH_MARGIN || x < box.left - DETACH_MARGIN || y < box.top || y > box.bottom;
  }

  function insertionIndex(y: number) {
    const buttons = [...bar!.querySelectorAll<HTMLElement>("button")];
    return buttons.filter((button) => {
      const box = button.getBoundingClientRect();
      return box.top + box.height / 2 < y;
    }).length;
  }

  function onPointerMove(event: PointerEvent) {
    if (!drag) return;
    const { clientX: x, clientY: y } = event;
    if (!drag.moved && Math.hypot(x - drag.startX, y - drag.startY) < DRAG_THRESHOLD) return;
    hint = null;
    drag = { ...drag, x, y, moved: true, outside: isOutside(x, y), target: insertionIndex(y) };
  }

  function onPointerUp(event: PointerEvent) {
    const done = drag;
    drag = null;
    if (!done?.moved) return;
    suppressClick = true;
    if (done.outside) return ondetach(done.id, event.screenX, event.screenY);
    onmove(done.id, done.target);
  }

  function onClick(project: Project) {
    if (suppressClick) {
      suppressClick = false;
      return;
    }
    onactivate(project.id);
  }

  function closeOnMiddleClick(event: MouseEvent, project: Project) {
    if (event.button !== 1) return;
    hint = null;
    onclose(project.id);
  }

  const showsDropAt = (index: number) => drag?.moved && !drag.outside && drag.target === index;
</script>

<nav class="bar" aria-label="Progetti aperti" bind:this={bar}>
  {#each projects as project, index (project.id)}
    {#if showsDropAt(index)}<div class="drop"></div>{/if}
    <button
      class:current={project.id === activeId}
      class:dragging={drag?.moved && drag.id === project.id}
      aria-label={folderName(project.folder)}
      onclick={() => onClick(project)}
      onauxclick={(event) => closeOnMiddleClick(event, project)}
      onpointerdown={(event) => onPointerDown(event, project)}
      onpointermove={onPointerMove}
      onpointerup={onPointerUp}
      onlostpointercapture={() => (drag = null)}
      onmouseenter={(event) => showHint(event, project)}
      onmouseleave={() => (hint = null)}
    >
      <img src={icon} alt="" draggable="false" />
      <span class="initial">{folderInitial(project.folder)}</span>
    </button>
  {/each}
  {#if showsDropAt(projects.length)}<div class="drop"></div>{/if}
</nav>

{#if hint}
  <div class="hint" style:top="{hint.top}px">{hint.text}</div>
{/if}

{#if drag?.moved && drag.outside}
  <div class="ghost" style:left="{drag.x + 12}px" style:top="{drag.y + 12}px">Rilascia per aprire in una nuova finestra</div>
{/if}

<style>
  .bar {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    flex: none;
    width: 36px;
    padding: 6px 0;
    overflow-y: auto;
    background: var(--panel-bg);
    border-right: 1px solid var(--border);
  }

  button {
    position: relative;
    display: grid;
    place-items: center;
    flex: none;
    width: 28px;
    height: 28px;
    padding: 0;
    border-left: 2px solid transparent;
    border-radius: 4px;
    touch-action: none;
  }

  button.current {
    border-left-color: var(--accent);
    background: var(--hover);
  }

  button.dragging {
    opacity: 0.4;
  }

  img {
    width: 20px;
    height: 20px;
    pointer-events: none;
  }

  .initial {
    position: absolute;
    right: 0;
    bottom: 0;
    min-width: 13px;
    padding: 0 2px;
    border-radius: 3px;
    background: var(--accent);
    color: #fff;
    font-size: 10px;
    font-weight: 600;
    line-height: 13px;
  }

  .drop {
    flex: none;
    width: 24px;
    height: 2px;
    margin: -3px 0;
    background: var(--accent);
  }

  .hint,
  .ghost {
    position: fixed;
    z-index: 100;
    padding: 3px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--editor-bg);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.15);
    white-space: nowrap;
    pointer-events: none;
  }

  .hint {
    left: 42px;
    transform: translateY(-50%);
  }
</style>
