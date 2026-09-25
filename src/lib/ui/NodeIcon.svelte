<script lang="ts">
  import { glyphs } from "../explorer/fileGlyphs";
  import { fileIcon } from "../explorer/fileIcons";

  let { kind, path = "" }: { kind: "folder" | "folder-open" | "file"; path?: string } = $props();

  const icon = $derived(fileIcon(path));
</script>

<svg class="icon {kind}" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
  {#if kind === "folder"}
    <path d="M1.5 4.5a1 1 0 0 1 1-1h3.6l1.5 1.5h5.9a1 1 0 0 1 1 1v6.5a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z" />
  {:else if kind === "folder-open"}
    <path d="M1.5 12.5v-8a1 1 0 0 1 1-1h3.6l1.5 1.5h5a1 1 0 0 1 1 1v1.5" />
    <path d="M1.5 12.5l2-5h11l-2 5z" />
  {:else}
    {#each glyphs[icon.glyph] as part, index (index)}
      {@const color = part.color ?? icon.color}
      <path
        d={part.d}
        transform={part.transform}
        fill={part.fill ? color : "none"}
        stroke={part.fill ? "none" : color}
      />
    {/each}
  {/if}
</svg>

<style>
  .icon {
    flex: none;
    stroke-width: 1.2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .folder,
  .folder-open {
    fill: var(--folder-fill);
    stroke: var(--folder-stroke);
    stroke-width: 1;
  }
</style>
