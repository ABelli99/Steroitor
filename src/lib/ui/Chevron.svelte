<script lang="ts">
  let { expanded }: { expanded: boolean } = $props();
</script>

<svg class="chevron" class:expanded viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
  <path d="M6 4l4 4-4 4" />
</svg>

<style>
  .chevron {
    flex: none;
    fill: none;
    stroke: var(--fg-muted);
    stroke-width: 1.3;
    stroke-linecap: round;
    stroke-linejoin: round;
    transition: transform 0.12s ease;
  }

  .expanded {
    transform: rotate(90deg);
  }
</style>
