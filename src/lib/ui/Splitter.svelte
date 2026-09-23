<script lang="ts">
  interface Props {
    direction: "horizontal" | "vertical";
    ondrag: (delta: number) => void;
    onend?: () => void;
  }

  let { direction, ondrag, onend }: Props = $props();
  let last = 0;

  const position = (event: PointerEvent) => (direction === "horizontal" ? event.clientX : event.clientY);

  function start(event: PointerEvent) {
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    last = position(event);
  }

  function move(event: PointerEvent) {
    if (!(event.currentTarget as HTMLElement).hasPointerCapture(event.pointerId)) return;
    const current = position(event);
    ondrag(current - last);
    last = current;
  }
</script>

<div
  class="splitter {direction}"
  role="separator"
  aria-orientation={direction === "horizontal" ? "vertical" : "horizontal"}
  onpointerdown={start}
  onpointermove={move}
  onpointerup={() => onend?.()}
></div>

<style>
  .splitter {
    flex: none;
    background: var(--border);
    touch-action: none;
  }

  .horizontal {
    width: 1px;
    cursor: col-resize;
  }

  .vertical {
    height: 1px;
    cursor: row-resize;
  }

  .splitter::after {
    content: "";
    display: block;
    position: relative;
  }

  .horizontal::after {
    width: 7px;
    height: 100%;
    left: -3px;
  }

  .vertical::after {
    height: 7px;
    width: 100%;
    top: -3px;
  }

  .splitter:hover {
    background: var(--accent);
  }
</style>
