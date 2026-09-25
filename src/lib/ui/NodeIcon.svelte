<script lang="ts">
  import { fileBadge } from "../explorer/fileIcons";

  let { kind, path = "" }: { kind: "folder" | "folder-open" | "file"; path?: string } = $props();

  const badge = $derived(kind === "file" ? fileBadge(path) : null);
</script>

<svg class="icon {kind}" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
  {#if kind === "folder"}
    <path d="M1.5 4.5a1 1 0 0 1 1-1h3.6l1.5 1.5h5.9a1 1 0 0 1 1 1v6.5a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z" />
  {:else if kind === "folder-open"}
    <path d="M1.5 12.5v-8a1 1 0 0 1 1-1h3.6l1.5 1.5h5a1 1 0 0 1 1 1v1.5" />
    <path d="M1.5 12.5l2-5h11l-2 5z" />
  {:else if badge}
    <rect class="badge" x="1.5" y="1.5" width="13" height="13" rx="3" fill={badge.color} />
    <text
      class="label"
      class:dark={badge.dark}
      x="8"
      y="8.5"
      font-size={badge.label.length > 1 ? 6.5 : 9}
    >{badge.label}</text>
  {:else}
    <path d="M3.5 2.5h6l3 3v8h-9z" />
    <path d="M9.5 2.5v3h3" />
  {/if}
</svg>

<style>
  .icon {
    flex: none;
    stroke-width: 1;
    stroke-linejoin: round;
  }

  .folder,
  .folder-open {
    fill: var(--folder-fill);
    stroke: var(--folder-stroke);
  }

  .file {
    fill: none;
    stroke: var(--fg-muted);
  }

  .badge {
    stroke: none;
  }

  .label {
    fill: #fff;
    stroke: none;
    font-family: var(--ui-font);
    font-weight: 700;
    text-anchor: middle;
    dominant-baseline: central;
    letter-spacing: -0.2px;
  }

  .label.dark {
    fill: #1f1f1f;
  }
</style>
