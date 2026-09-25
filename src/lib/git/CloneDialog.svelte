<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { homeDir } from "@tauri-apps/api/path";
  import { open } from "@tauri-apps/plugin-dialog";
  import { saveSettings, settings } from "../settings.svelte";
  import { useProject } from "../project/context";
  import { parentDir } from "../workspace/files";
  import { gitClone, repoNameFromUrl } from "./clone";

  interface Props {
    onclose: () => void;
    oncloned: (folder: string) => void;
  }

  let { onclose, oncloned }: Props = $props();
  const { activity } = useProject();

  type PathState = "missing" | "emptyDir" | "dir" | "file";

  const CHECK_DELAY_MS = 250;
  const DESTINATION_ERRORS: Partial<Record<PathState, string>> = {
    dir: "La cartella esiste già e non è vuota: scegline un'altra.",
    file: "Esiste già un file con questo nome.",
  };

  let url = $state("");
  let directory = $state("");
  let parent = $state("");
  let editedByHand = $state(false);
  let destination = $state<PathState | null>(null);
  let cloning = $state(false);
  let error = $state<string | null>(null);
  let urlInput = $state<HTMLInputElement>();

  const join = (base: string, name: string) => `${base.replace(/[\\/]+$/, "")}\\${name}`;
  const destinationError = $derived(destination ? DESTINATION_ERRORS[destination] : undefined);
  const canClone = $derived(!cloning && url.trim().length > 0 && directory.trim().length > 0 && destination !== null && !destinationError);

  onMount(async () => {
    urlInput?.focus();
    parent = settings.cloneParent || (await homeDir());
  });

  $effect(() => {
    const name = url.trim() ? repoNameFromUrl(url) : "";
    if (!editedByHand && parent) directory = name ? join(parent, name) : parent;
  });

  $effect(() => {
    const target = directory.trim();
    destination = null;
    if (!target) return;
    const timer = setTimeout(async () => {
      destination = await invoke<PathState>("path_state", { path: target }).catch(() => null);
    }, CHECK_DELAY_MS);
    return () => clearTimeout(timer);
  });

  async function browse() {
    const chosen = await open({ directory: true, defaultPath: parent || undefined, title: "Cartella in cui clonare" });
    if (!chosen) return;
    parent = chosen;
    editedByHand = false;
    directory = url.trim() ? join(chosen, repoNameFromUrl(url)) : chosen;
  }

  async function clone() {
    if (!canClone) return;
    cloning = true;
    error = null;
    const target = directory.trim();
    try {
      await activity.track(`Clone di ${repoNameFromUrl(url)}`, () => gitClone(url.trim(), target));
      settings.cloneParent = parentDir(target);
      saveSettings();
      oncloned(target);
    } catch (failure) {
      error = String(failure);
    } finally {
      cloning = false;
    }
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === "Escape" && !cloning) onclose();
    if (event.key === "Enter") clone();
  }
</script>

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && !cloning && onclose()}>
  <div class="dialog" role="dialog" aria-label="Clona repository" tabindex="-1" onkeydown={onKeyDown}>
    <h2>Clona repository</h2>

    <label for="clone-url">URL</label>
    <input id="clone-url" bind:this={urlInput} bind:value={url} placeholder="git@github.com:utente/progetto.git oppure https://…" spellcheck="false" disabled={cloning} />

    <label for="clone-directory">Directory</label>
    <div class="row">
      <input id="clone-directory" bind:value={directory} oninput={() => (editedByHand = true)} spellcheck="false" disabled={cloning} />
      <button onclick={browse} disabled={cloning}>Sfoglia…</button>
    </div>
    <p class="hint" class:bad={!!destinationError}>
      {#if destinationError}
        {destinationError}
      {:else if destination === "missing"}
        La cartella verrà creata.
      {:else if destination === "emptyDir"}
        La cartella esiste ed è vuota.
      {:else}
        &nbsp;
      {/if}
    </p>

    {#if error}<pre class="error">{error}</pre>{/if}

    <div class="actions">
      {#if cloning}<span class="busy">Clone in corso… (l'output è nella Console)</span>{/if}
      <span class="spacer"></span>
      <button onclick={onclose} disabled={cloning}>Annulla</button>
      <button class="primary" onclick={clone} disabled={!canClone}>Clona</button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 25;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
    background: rgba(0, 0, 0, 0.25);
  }

  .dialog {
    width: min(620px, calc(100vw - 32px));
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 14px 16px;
    background: var(--panel-bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
    outline: none;
  }

  h2 {
    margin: 0 0 8px;
    font-size: 14px;
  }

  label {
    margin-top: 6px;
    color: var(--fg-muted);
    font-size: 12px;
  }

  .row {
    display: flex;
    gap: 6px;
  }

  input {
    flex: 1;
    font: inherit;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--fg);
    background: var(--editor-bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 5px 8px;
    outline: none;
  }

  input:focus {
    border-color: var(--accent);
  }

  button {
    border: 1px solid var(--border);
    padding: 4px 12px;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .primary:not(:disabled) {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  .hint {
    margin: 2px 0 0;
    font-size: 12px;
    color: var(--fg-muted);
  }

  .hint.bad {
    color: var(--danger);
  }

  .error {
    margin: 8px 0 0;
    padding: 6px 8px;
    max-height: 120px;
    overflow: auto;
    white-space: pre-wrap;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--danger);
    background: var(--editor-bg);
    border-radius: 4px;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 12px;
  }

  .spacer {
    flex: 1;
  }

  .busy {
    color: var(--fg-muted);
    font-size: 12px;
  }
</style>
