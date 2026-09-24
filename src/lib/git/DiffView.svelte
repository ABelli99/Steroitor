<script lang="ts">
  import { readTextFile } from "../backend";
  import { gitHeadContent } from "./api";
  import type { FileSelection } from "./selection.svelte";
  import SideBySide from "./SideBySide.svelte";

  let { root, file }: { root: string; file: FileSelection } = $props();

  async function contents(selection: FileSelection) {
    const isNew = selection.status === "added" || selection.status === "untracked";
    const head = isNew ? "" : ((await gitHeadContent(root, selection.path))?.content ?? "");
    const current = selection.status === "deleted" ? "" : (await readTextFile(selection.path)).content;
    return { head, current };
  }
</script>

{#await contents(file) then { head, current }}
  <SideBySide path={file.path} left={head} right={current} labels={["HEAD", "Working tree"]} />
{:catch failure}
  <p class="error">{failure}</p>
{/await}

<style>
  .error {
    margin: 8px;
    color: var(--danger);
  }
</style>
